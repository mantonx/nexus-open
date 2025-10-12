package tray

import (
	"bytes"
	"context"
	_ "embed"
	"log/slog"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"

	"github.com/getlantern/systray"
)

//go:embed icon.png
var iconData []byte

// Manager handles system tray integration and Flutter UI lifecycle
type Manager struct {
	logger      *slog.Logger
	flutterCmd  *exec.Cmd
	showCh      chan struct{}
	hideCh      chan struct{}
	quitCh      chan struct{}
	ctx         context.Context
	cancel      context.CancelFunc
}

// New creates a new tray manager
func New(logger *slog.Logger) *Manager {
	ctx, cancel := context.WithCancel(context.Background())
	return &Manager{
		logger:  logger,
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

	// Start Flutter UI
	if err := m.startFlutter(); err != nil {
		m.logger.Error("failed to start Flutter UI", "error", err)
	}

	// Handle menu clicks
	go func() {
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

	m.logger.Info("starting Flutter UI", "path", flutterPath)

	// Start Flutter process
	m.flutterCmd = exec.CommandContext(m.ctx, flutterPath)
	m.flutterCmd.Stdout = os.Stdout
	m.flutterCmd.Stderr = os.Stderr

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

	// Look for Flutter executable in the same directory
	var flutterName string
	switch runtime.GOOS {
	case "linux":
		flutterName = "nexus-open-ui"
	case "darwin":
		flutterName = "nexus-open-ui.app/Contents/MacOS/nexus-open-ui"
	case "windows":
		flutterName = "nexus-open-ui.exe"
	default:
		flutterName = "nexus-open-ui"
	}

	flutterPath := filepath.Join(exeDir, flutterName)
	if _, err := os.Stat(flutterPath); err != nil {
		// Development mode: look in ui/build directory
		devPath := filepath.Join(exeDir, "..", "ui", "build", "linux", "x64", "release", "bundle", "nexus_open")
		if _, err := os.Stat(devPath); err == nil {
			return devPath, nil
		}
		return "", err
	}

	return flutterPath, nil
}

// showWindow shows the Flutter window
func (m *Manager) showWindow() {
	// Call API to tell Flutter to show window
	resp, err := http.Post("http://localhost:1985/api/window/show", "application/json", bytes.NewReader([]byte("{}")))
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
	// Call API to tell Flutter to hide window
	resp, err := http.Post("http://localhost:1985/api/window/hide", "application/json", bytes.NewReader([]byte("{}")))
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
