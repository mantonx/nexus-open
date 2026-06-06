package tray

import (
	"bytes"
	"context"
	_ "embed"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/getlantern/systray"
)

//go:embed icon.png
var iconData []byte

// Manager handles system tray integration and Flutter UI lifecycle
type Manager struct {
	logger      *slog.Logger
	apiAddr     string // e.g. "localhost:1985"
	flutterCmd  *exec.Cmd
	showCh      chan struct{}
	hideCh      chan struct{}
	quitCh      chan struct{}
	ctx         context.Context
	cancel      context.CancelFunc
}

// New creates a new tray manager. apiAddr is the host:port of the API server
// (e.g. "localhost:1985") so tray commands reach the correct port.
func New(logger *slog.Logger, apiAddr string) *Manager {
	ctx, cancel := context.WithCancel(context.Background())
	return &Manager{
		logger:  logger,
		apiAddr: apiAddr,
		showCh:  make(chan struct{}, 1),
		hideCh:  make(chan struct{}, 1),
		quitCh:  make(chan struct{}, 1),
		ctx:     ctx,
		cancel:  cancel,
	}
}

// Run starts the system tray
func (m *Manager) Run() {
	systray.Run(m.onReady, m.onExit)
}

// onReady is called when the systray is ready
func (m *Manager) onReady() {
	systray.SetIcon(iconData)
	systray.SetTitle("Nexus Open")
	systray.SetTooltip("Nexus Open - Device Monitor")

	// Add menu items
	mShow := systray.AddMenuItem("Show", "Show the application window")
	mHide := systray.AddMenuItem("Hide", "Hide the application window")
	systray.AddSeparator()
	mQuit := systray.AddMenuItem("Quit", "Quit Nexus Open")

	m.logger.Info("system tray initialized")

	// Start Flutter UI and wait until it is ready before accepting tray events.
	flutterReady := make(chan struct{})
	if err := m.startFlutter(); err != nil {
		m.logger.Error("failed to start Flutter UI", "error", err)
		mShow.Disable()
		mHide.Disable()
		systray.SetTooltip("Nexus Open — UI failed to start")
		close(flutterReady)
	} else {
		go func() {
			if err := m.waitForFlutter(5 * time.Second); err != nil {
				m.logger.Warn("Flutter UI did not become ready in time", "error", err)
				mShow.Disable()
				mHide.Disable()
				systray.SetTooltip("Nexus Open — UI not responding")
			}
			close(flutterReady)
		}()
	}

	// Handle menu clicks — Show/Hide only fire after Flutter is ready.
	go func() {
		<-flutterReady
		for {
			select {
			case <-m.ctx.Done():
				return
			case <-mShow.ClickedCh:
				m.logger.Debug("show clicked")
				m.showWindow()
			case <-mHide.ClickedCh:
				m.logger.Debug("hide clicked")
				m.hideWindow()
			case <-mQuit.ClickedCh:
				m.logger.Info("quit clicked")
				m.quit()
				return
			}
		}
	}()
}

// onExit is called when the systray exits
func (m *Manager) onExit() {
	m.logger.Info("system tray exiting")
	m.stopFlutter()
	m.cancel()
}

// startFlutter launches the Flutter application
func (m *Manager) startFlutter() error {
	// Find the Flutter executable
	flutterPath, err := m.findFlutterExecutable()
	if err != nil {
		return err
	}

	m.logger.Info("starting Flutter UI minimized", "path", flutterPath)

	m.flutterCmd = exec.CommandContext(m.ctx, flutterPath)
	m.flutterCmd.Stdout = os.Stdout
	m.flutterCmd.Stderr = os.Stderr
	// Inherit the current environment and add the minimized flag so the
	// Flutter window starts hidden — the user brings it up via the tray.
	m.flutterCmd.Env = append(os.Environ(),
		"NEXUS_START_MINIMIZED=1",
		"WAYLAND_DISPLAY=", // Flutter GTK3 embedder crashes on native Wayland; force XWayland
	)

	if err := m.flutterCmd.Start(); err != nil {
		return err
	}

	m.logger.Info("Flutter UI started", "pid", m.flutterCmd.Process.Pid)
	return nil
}

// stopFlutter terminates the Flutter application
func (m *Manager) stopFlutter() {
	if m.flutterCmd != nil && m.flutterCmd.Process != nil {
		m.logger.Info("stopping Flutter UI", "pid", m.flutterCmd.Process.Pid)
		if err := m.flutterCmd.Process.Kill(); err != nil {
			m.logger.Error("failed to kill Flutter process", "error", err)
		}
	}
}

// findFlutterExecutable locates the Flutter executable
func (m *Manager) findFlutterExecutable() (string, error) {
	// Get the directory of the current executable
	exePath, err := os.Executable()
	if err != nil {
		return "", err
	}
	exeDir := filepath.Dir(exePath)

	// Priority 1: XDG install path (~/.local/share/nexus-open/ui)
	xdgData := os.Getenv("XDG_DATA_HOME")
	if xdgData == "" {
		xdgData = filepath.Join(os.Getenv("HOME"), ".local", "share")
	}
	installedPath := filepath.Join(xdgData, "nexus-open", "ui")
	if _, err := os.Stat(installedPath); err == nil {
		return installedPath, nil
	}

	// Priority 2: sibling to the daemon binary (single-dir deployment)
	siblingPath := filepath.Join(exeDir, "ui")
	if _, err := os.Stat(siblingPath); err == nil {
		return siblingPath, nil
	}

	// Priority 3: development build path (running from repo root)
	devPath := filepath.Join(exeDir, "..", "ui", "build", "linux", "x64", "release", "bundle", "ui")
	if _, err := os.Stat(devPath); err == nil {
		return devPath, nil
	}

	return "", fmt.Errorf("Flutter UI binary not found; run 'cd ui && flutter build linux --release'")
}

// showWindow shows the Flutter window, forwarding an XDG activation token if
// one is present in the environment so the compositor grants focus permission.
func (m *Manager) showWindow() {
	url := "http://" + m.apiAddr + "/api/window/show"
	req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader([]byte("{}")))
	if err != nil {
		m.logger.Error("failed to build show window request", "error", err)
		return
	}
	req.Header.Set("Content-Type", "application/json")
	if token := os.Getenv("XDG_ACTIVATION_TOKEN"); token != "" {
		req.Header.Set("X-XDG-Activation-Token", token)
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		m.logger.Error("failed to send show window command", "error", err)
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		m.logger.Error("show window command failed", "status", resp.StatusCode)
		return
	}

	m.logger.Debug("show window command sent")
}

// hideWindow hides the Flutter window
func (m *Manager) hideWindow() {
	url := "http://" + m.apiAddr + "/api/window/hide"
	resp, err := http.Post(url, "application/json", bytes.NewReader([]byte("{}")))
	if err != nil {
		m.logger.Error("failed to send hide window command", "error", err)
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		m.logger.Error("hide window command failed", "status", resp.StatusCode)
		return
	}

	m.logger.Debug("hide window command sent")
}

// quit stops everything and exits
func (m *Manager) quit() {
	select {
	case m.quitCh <- struct{}{}:
	default:
	}
	systray.Quit()
}

// QuitChannel returns the quit channel
func (m *Manager) QuitChannel() <-chan struct{} {
	return m.quitCh
}

// waitForFlutter polls GET /api/window/state until the Flutter UI responds or
// the timeout elapses. This prevents Show/Hide tray clicks from silently
// failing in the first few seconds after launch.
func (m *Manager) waitForFlutter(timeout time.Duration) error {
	url := "http://" + m.apiAddr + "/api/window/state"
	deadline := time.Now().Add(timeout)
	client := &http.Client{Timeout: 500 * time.Millisecond}

	for time.Now().Before(deadline) {
		resp, err := client.Get(url)
		if err == nil {
			resp.Body.Close()
			if resp.StatusCode == http.StatusOK {
				m.logger.Info("Flutter UI is ready")
				return nil
			}
		}
		time.Sleep(250 * time.Millisecond)
	}
	return fmt.Errorf("Flutter UI did not respond within %s", timeout)
}
