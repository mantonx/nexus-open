// Package app provides the main application orchestration and dependency injection.
package app

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/mantonx/nexus-open/internal/api"
	"github.com/mantonx/nexus-open/internal/device"
	settings "github.com/mantonx/nexus-open/internal/settings"
	"github.com/mantonx/nexus-open/internal/store"
	"github.com/mantonx/nexus-open/internal/touch"
	"github.com/mantonx/nexus-open/internal/zone"
)

// App is the main application container that holds all dependencies.
// It follows the dependency injection pattern to manage component lifecycle.
type App struct {
	ctx    context.Context
	cancel context.CancelFunc
	logger *slog.Logger

	// Configuration
	configPath string
	apiPort    int
	layoutPath string
	pluginsDir string

	// Components
	store        *store.DB
	cfg          *settings.Manager
	device       device.Device
	apiServer    *api.Server
	zoneCfg      *zone.ConfigManager
	zoneManager  *zone.Manager
	zoneSampler  *zone.Sampler
	touchHandler *touch.Handler

	// Lifecycle
	shutdownOnce sync.Once
	shutdownCh   chan struct{}
	wg           sync.WaitGroup
}

// New creates a new application instance with the given options.
func New(opts ...Option) (*App, error) {
	ctx, cancel := context.WithCancel(context.Background())

	app := &App{
		ctx:        ctx,
		cancel:     cancel,
		logger:     slog.Default(),
		configPath: "",
		apiPort:    1985,
		shutdownCh: make(chan struct{}),
	}

	// Apply options
	for _, opt := range opts {
		opt(app)
	}

	// Initialize components
	if err := app.initialize(); err != nil {
		cancel()
		return nil, fmt.Errorf("initialization failed: %w", err)
	}

	return app, nil
}

// Run starts all application components and blocks until shutdown.
func (a *App) Run(ctx context.Context) error {
	a.logger.Info("application starting")

	// Start components
	if err := a.start(); err != nil {
		return fmt.Errorf("failed to start: %w", err)
	}

	// Block until context is canceled or shutdown is called
	var runErr error
	select {
	case <-ctx.Done():
		a.logger.Info("context canceled")
		runErr = ctx.Err()
	case <-a.shutdownCh:
		a.logger.Info("shutdown requested")
	}
	a.Shutdown() //nolint:errcheck
	return runErr
}

// APIServer returns the underlying API server, used by callers that need to
// wire channels (e.g. the tray manager listening for window-closed signals).
func (a *App) APIServer() *api.Server {
	return a.apiServer
}

// Shutdown gracefully stops all application components.
func (a *App) Shutdown() error {
	var shutdownErr error

	a.shutdownOnce.Do(func() {
		a.logger.Info("initiating shutdown")
		close(a.shutdownCh)
		a.cancel()

		// Stop sampler (kills plugin subprocesses) and API server.
		// Both can block if a plugin hangs or the OS is slow, so enforce a
		// deadline. Hardware shutdown proceeds regardless after the deadline.
		stopDone := make(chan struct{})
		go func() {
			a.zoneSampler.Stop()
			close(stopDone)
		}()

		stopCtx, stopCancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer stopCancel()
		select {
		case <-stopDone:
		case <-stopCtx.Done():
			a.logger.Warn("sampler stop timed out after 10s; forcing shutdown")
		}

		if a.apiServer != nil {
			apiCtx, apiCancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer apiCancel()
			a.apiServer.Shutdown(apiCtx) //nolint:errcheck
		}

		// Wait for render loop and device watcher goroutines to exit before
		// closing the HID device — they may be mid-write and still hold a reference.
		wgDone := make(chan struct{})
		go func() { a.wg.Wait(); close(wgDone) }()
		wgCtx, wgCancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer wgCancel()
		select {
		case <-wgDone:
		case <-wgCtx.Done():
			a.logger.Warn("render loop did not exit cleanly within 5s")
		}

		// Send a blank frame so the device shows black rather than frozen content.
		// The Corsair firmware has no "release to native" command and will reset
		// on touch after shutdown — this is a firmware limitation with no
		// software workaround on Linux.
		if a.device != nil && a.device.IsConnected() {
			blank := make([]byte, 640*48*4)
			if err := a.device.SendFrame(context.Background(), blank); err != nil {
				a.logger.Debug("failed to send blank frame on shutdown", "error", err)
			}
		}

		// Close HID device last, after all goroutines have stopped.
		if a.device != nil {
			if err := a.device.Disconnect(); err != nil {
				a.logger.Error("error disconnecting device", "error", err)
			}
		}

		if a.store != nil {
			if err := a.store.Close(); err != nil {
				a.logger.Error("error closing store", "error", err)
			}
		}

		a.logger.Info("shutdown complete")
	})

	return shutdownErr
}

// initialize sets up all application components.
func (a *App) initialize() error {
	a.logger.Debug("initializing application components")

	// 1. Open the SQLite store — single source of truth for all config.
	// configPath is the DB path (empty = default ~/.config/nexus-open/nexus.db).
	var err error
	a.store, err = store.Open(a.configPath, a.logger)
	if err != nil {
		return fmt.Errorf("failed to open store: %w", err)
	}

	// 2. Build settings manager from store.
	a.cfg, err = settings.NewManager(a.store, a.logger)
	if err != nil {
		return fmt.Errorf("failed to create config manager: %w", err)
	}

	// On first run import any existing config.yaml so upgrades are seamless.
	if a.store.IsFirstRun() {
		legacyYAML := a.configPath
		if legacyYAML == "" {
			if dir, e := os.UserConfigDir(); e == nil {
				legacyYAML = dir + "/nexus-open/config.yaml"
			}
		}
		if err := a.cfg.ImportFromYAML(legacyYAML, a.logger); err != nil {
			a.logger.Warn("settings: yaml import failed (continuing with defaults)", "error", err)
		}
	}
	a.logger.Info("configuration loaded", "first_run", a.store.IsFirstRun())

	// 3. Build zone config manager from the same store.
	a.zoneCfg = zone.NewConfigManager(a.store, a.logger)

	a.logger.Info("zone config manager initialized")

	// 3. Create device connection
	// Check if mock device mode is enabled (useful for development without hardware)
	if os.Getenv("NEXUS_MOCK_DEVICE") == "1" {
		a.logger.Info("using mock device (NEXUS_MOCK_DEVICE=1)")
		mockDevice := device.NewMockDevice()
		// Auto-connect the mock device
		if err := mockDevice.Connect(a.ctx); err != nil {
			return fmt.Errorf("failed to connect mock device: %w", err)
		}
		a.device = mockDevice
	} else {
		deviceConfig := device.ConnectionConfig{
			VendorID:         0x1b1c, // Corsair
			ProductID:        0x1b8e, // iCUE Nexus
			ReconnectRetries: 10,
			ReconnectDelay:   5 * time.Second,
		}
		a.device = device.NewNexusDevice(a.logger, deviceConfig)
		a.logger.Info("device created")
	}

	// 4. Create API server
	apiAddr := fmt.Sprintf("127.0.0.1:%d", a.apiPort)
	a.apiServer = api.NewServer(apiAddr, a.cfg, a.device, a.logger)
	a.logger.Info("API server created", "addr", apiAddr)

	// Register zone config manager with API server so endpoints can read/write configs.
	a.apiServer.SetZoneConfigManager(a.zoneCfg)
	a.logger.Info("zone config manager registered with API server")

	// 5. Create zone manager — loads from DB, falling back to YAML on first run.
	if a.layoutPath == "" {
		a.layoutPath = resolveLayoutPath()
	}
	a.zoneManager, err = zone.NewManager(a.ctx, a.logger, a.store, a.layoutPath)
	if err != nil {
		return fmt.Errorf("failed to create zone manager: %w", err)
	}
	a.logger.Info("zone manager initialized",
		"pages", len(a.zoneManager.GetConfig().Pages),
		"current_page", a.zoneManager.GetConfig().Pages[0].Name)

	// 6. Resolve plugins directory.
	// Priority: explicit flag > NEXUS_PLUGINS_DIR env > sibling to exe > XDG user data > /usr/lib/nexus-open/plugins (system package install).
	if a.pluginsDir == "" {
		if env := os.Getenv("NEXUS_PLUGINS_DIR"); env != "" {
			a.pluginsDir = env
		} else if exePath, err := os.Executable(); err == nil {
			sibling := filepath.Join(filepath.Dir(exePath), "plugins")
			xdgData := filepath.Join(os.Getenv("XDG_DATA_HOME"), "nexus-open", "plugins")
			if os.Getenv("XDG_DATA_HOME") == "" {
				xdgData = filepath.Join(os.Getenv("HOME"), ".local", "share", "nexus-open", "plugins")
			}
			systemData := "/usr/lib/nexus-open/plugins"
			switch {
			case dirHasPlugins(sibling):
				a.pluginsDir = sibling
			case dirHasPlugins(xdgData):
				a.pluginsDir = xdgData
			case dirHasPlugins(systemData):
				a.pluginsDir = systemData
			default:
				// Fall back to sibling even if absent — binary validation will
				// surface a clear error per zone rather than failing at startup.
				a.pluginsDir = sibling
			}
		}
	}
	a.logger.Info("plugins directory", "path", a.pluginsDir)

	// 7. Create module sampler
	a.zoneSampler = zone.NewSampler(a.ctx, a.logger, a.zoneManager, a.zoneCfg, a.pluginsDir)
	a.logger.Info("zone sampler created")

	// On page change, just broadcast new page state. All plugins run
	// continuously across all pages (started in Start()), so no sampler
	// restart is needed — payloads are already current on arrival.
	a.zoneManager.SetOnPageChange(func(pageIndex int) error {
		go a.apiServer.BroadcastPageState()
		return nil
	})

	// Restart the sampler for a single zone when a tap cycles its module.
	a.zoneManager.SetOnZoneCycle(func(zoneConfig zone.ZoneConfig) error {
		return a.zoneSampler.RestartZone(zoneConfig)
	})

	// Register zone sampler as per-zone config notifier so API updates affect live modules.
	a.apiServer.SetZoneConfigNotifier(a.zoneSampler)
	a.logger.Info("zone config notifier registered with API server")

	// Register zone sampler as status provider so /api/zones/:id/status works.
	a.apiServer.SetZoneStatusProvider(a.zoneSampler)

	// Register zone sampler as plugin catalog provider so /api/plugins works.
	a.apiServer.SetPluginCatalog(a.zoneSampler)

	// Wire zone manager for swipe simulation, tap simulation, navigation, and layout editing.
	a.apiServer.SetSwipeSimulator(a.zoneManager)
	a.apiServer.SetZoneTapper(a.zoneManager)
	a.apiServer.SetDetailFrameProvider(a.zoneManager)
	a.apiServer.SetFrameProvider(a.zoneManager)
	a.zoneManager.SetDetailStateCallback(func(active bool) {
		a.apiServer.BroadcastDetailState(active, zone.DetailCloseX*2, zone.DetailCloseY*2)
	})
	a.apiServer.SetNavigator(a.zoneManager)
	a.apiServer.SetLayoutStore(a.store)
	a.apiServer.SetLayoutReloader(a.zoneManager)

	// Propagate display config from settings into the zone manager theme,
	// both immediately on startup and on every subsequent Flutter UI save.
	applySettingsTheme := func(cfg settings.Config) {
		// Theme is defined entirely in the layout YAML (including per-zone
		// ThemeOverride accents). Settings only controls the background image.
		if cfg.BackgroundImage != "" && cfg.BackgroundImage != settings.DefaultBackgroundImage {
			if dir, err := os.UserConfigDir(); err == nil {
				imgPath := dir + "/nexus-open/images/" + cfg.BackgroundImage
				if err := a.zoneManager.SetBackground(imgPath); err != nil {
					a.logger.Warn("failed to set background image", "path", imgPath, "error", err)
				}
			}
		} else {
			a.zoneManager.SetBackground("") //nolint:errcheck
		}
	}

	// Apply saved settings immediately so hardware reflects stored config on startup.
	applySettingsTheme(a.cfg.Get())

	// Watch for future saves from the Flutter UI.
	settingsCh := make(chan settings.Config, 4)
	a.cfg.Watch(settingsCh)
	go func() {
		for cfg := range settingsCh {
			applySettingsTheme(cfg)
			a.logger.Debug("theme updated from settings",
				"bg", cfg.BackgroundColor,
				"fg", cfg.TextColor,
			)
		}
	}()

	// 7. Create touch handler and wire detail tap support.
	a.zoneManager.SetPluginLookup(a.zoneSampler)
	a.touchHandler = touch.NewHandler(a.logger, a.device, a.zoneManager)
	a.logger.Info("touch handler created")

	return nil
}

// start begins operation of all components.
func (a *App) start() error {
	a.logger.Debug("starting application components")

	// Start the API server first so Flutter and health checks are available
	// immediately — device connect can take several seconds on a slow replug.
	a.wg.Add(1)
	go func() {
		defer a.wg.Done()
		if err := a.apiServer.Start(); err != nil {
			a.logger.Error("API server error", "error", err)
		}
	}()
	a.logger.Info("API server started")

	// Start module sampler
	if err := a.zoneSampler.Start(); err != nil {
		return fmt.Errorf("failed to start zone sampler: %w", err)
	}
	a.logger.Info("zone sampler started")

	// Start touch handler
	if err := a.touchHandler.Start(a.ctx); err != nil {
		return fmt.Errorf("failed to start touch handler: %w", err)
	}
	a.logger.Info("touch handler started")

	// Start zone rendering loop
	a.wg.Add(1)
	go a.renderLoop()
	a.logger.Info("zone rendering started")

	// Attempt device connect after everything else is running. If it fails,
	// the watcher retries every 3s so the device can be plugged in any time.
	if err := a.device.Connect(a.ctx); err != nil {
		a.logger.Warn("failed to connect to device", "error", err)
		a.apiServer.SetLastConnectError(err)
		a.startDeviceWatcher()
	}

	return nil
}

// startDeviceWatcher polls for the device in the background until it connects.
// Called when the initial connect attempt fails (device not plugged in yet).
// Once connected, the device's own monitorConnection goroutine takes over.
func (a *App) startDeviceWatcher() {
	a.wg.Add(1)
	go func() {
		defer a.wg.Done()

		const pollInterval = 3 * time.Second
		ticker := time.NewTicker(pollInterval)
		defer ticker.Stop()

		a.logger.Info("device watcher started — waiting for device to be plugged in")

		for {
			select {
			case <-a.ctx.Done():
				return
			case <-ticker.C:
				if a.device.IsConnected() {
					// Already connected (monitorConnection handled a replug).
					return
				}
				if err := a.device.Connect(a.ctx); err != nil {
					a.logger.Debug("device not available, will retry", "error", err)
					a.apiServer.SetLastConnectError(err)
				} else {
					a.logger.Info("device connected")
					a.apiServer.SetLastConnectError(nil)
					return
				}
			}
		}
	}()
}

// dirHasPlugins returns true only when path is a directory that contains at
// least one regular executable file. An empty or stale directory does not
// qualify — this prevents a leftover ~/.local/share/nexus-open/plugins/ from
// shadowing the system install at /usr/lib/nexus-open/plugins/.
func dirHasPlugins(path string) bool {
	entries, err := os.ReadDir(path)
	if err != nil {
		return false
	}
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		if info, err := e.Info(); err == nil && info.Mode()&0111 != 0 {
			return true
		}
	}
	return false
}

// resolveLayoutPath finds the fallback zone layout YAML using the same
// priority order as plugin resolution: XDG user install > system package
// install (/usr/share) > dev repo path relative to the executable.
func resolveLayoutPath() string {
	const layoutFile = "configs/layouts/multi-page.yaml"

	xdgData := os.Getenv("XDG_DATA_HOME")
	if xdgData == "" {
		xdgData = filepath.Join(os.Getenv("HOME"), ".local", "share")
	}
	candidates := []string{
		filepath.Join(xdgData, "nexus-open", layoutFile),
		filepath.Join("/usr/share/nexus-open", layoutFile),
	}
	if exe, err := os.Executable(); err == nil {
		candidates = append(candidates,
			filepath.Join(filepath.Dir(exe), "..", layoutFile), // dev: repo root
		)
	}
	for _, p := range candidates {
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}
	return layoutFile // last resort: relative path (dev CWD)
}
