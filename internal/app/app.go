// Package app provides the main application orchestration and dependency injection.
package app

import (
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"image/png"
	"log/slog"
	"os"
	"sync"
	"time"

	"github.com/mantonx/nexus-next/internal/api"
	"github.com/mantonx/nexus-next/internal/device"
	settings "github.com/mantonx/nexus-next/internal/settings"
	"github.com/mantonx/nexus-next/internal/store"
	"github.com/mantonx/nexus-next/internal/touch"
	"github.com/mantonx/nexus-next/internal/zone"
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
	select {
	case <-ctx.Done():
		a.logger.Info("context canceled")
		return ctx.Err()
	case <-a.shutdownCh:
		a.logger.Info("shutdown requested")
		return nil
	}
}

// Shutdown gracefully stops all application components.
func (a *App) Shutdown() error {
	var shutdownErr error

	a.shutdownOnce.Do(func() {
		a.logger.Info("initiating shutdown")
		close(a.shutdownCh)
		a.cancel()

		// Stop sampler and API server (does not close HID device yet)
		a.zoneSampler.Stop()
		if a.apiServer != nil {
			shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			a.apiServer.Shutdown(shutdownCtx) //nolint:errcheck
		}

		// Wait for render loop and watcher to exit before closing the HID
		// device — they may be mid-write and still hold a reference.
		a.wg.Wait()

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

	// On first run also import legacy zone-configs.yaml.
	if a.store.IsFirstRun() {
		if dir, e := os.UserConfigDir(); e == nil {
			legacyZone := dir + "/nexus-open/zone-configs.yaml"
			if err := a.zoneCfg.ImportFromYAML(legacyZone); err != nil {
				a.logger.Warn("zone config: yaml import failed (continuing)", "error", err)
			}
		}
	}
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
	apiAddr := fmt.Sprintf(":%d", a.apiPort)
	a.apiServer = api.NewServer(apiAddr, a.cfg, a.device, a.logger)
	a.logger.Info("API server created", "addr", apiAddr)

	// Register zone config manager with API server so endpoints can read/write configs.
	a.apiServer.SetZoneConfigManager(a.zoneCfg)
	a.logger.Info("zone config manager registered with API server")

	// 5. Create zone manager
	if a.layoutPath == "" {
		a.layoutPath = "configs/layouts/multi-page.yaml"
	}
	a.zoneManager, err = zone.NewManager(a.ctx, a.logger, a.layoutPath)
	if err != nil {
		return fmt.Errorf("failed to create zone manager: %w", err)
	}
	a.logger.Info("zone manager initialized",
		"pages", len(a.zoneManager.GetConfig().Pages),
		"current_page", a.zoneManager.GetConfig().Pages[0].Name)

	// 6. Create module sampler
	a.zoneSampler = zone.NewSampler(a.ctx, a.logger, a.zoneManager, a.zoneCfg)
	a.logger.Info("zone sampler created")

	// Register page change callback: restart sampler and broadcast new page state.
	// BroadcastPageState is fired in its own goroutine — it acquires the hub
	// lock, and doing that on the same goroutine as sampler restart adds latency
	// that can push past test timing budgets when modules are slow to launch.
	a.zoneManager.SetOnPageChange(func(pageIndex int) error {
		if err := a.zoneSampler.RestartForPage(pageIndex); err != nil {
			return err
		}
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

	// Wire zone manager for swipe simulation, navigation, and layout editing.
	a.apiServer.SetSwipeSimulator(a.zoneManager)
	a.apiServer.SetNavigator(a.zoneManager)
	a.apiServer.SetLayoutStore(a.store)
	a.apiServer.SetLayoutReloader(a.zoneManager)

	// On first DB run, seed the layout tables from the YAML layout file so
	// the editor has something to show immediately.
	if a.store.IsFirstRun() {
		if hasLayout, _ := a.store.HasLayout(); !hasLayout {
			if err := importLayoutFromYAML(a.store, a.zoneManager.GetConfig(), a.logger); err != nil {
				a.logger.Warn("layout import from YAML failed (continuing with YAML)", "error", err)
			}
		}
	}

	// Propagate display config from settings into the zone manager theme,
	// both immediately on startup and on every subsequent Flutter UI save.
	applySettingsTheme := func(cfg settings.Config) {
		current := a.zoneManager.GetConfig().Theme
		current.Bg = cfg.BackgroundColor
		current.Fg = cfg.TextColor
		// Accent is NOT overwritten from TextColor — it stays as the zone theme
		// accent (cyan by default) which drives graph colours and severity tints.
		if cfg.Display.FontSize > 0 {
			current.FontSizePrimary = int(cfg.Display.FontSize)
		}
		if cfg.Display.TimeFontSize > 0 {
			current.FontSizeSecondary = int(cfg.Display.TimeFontSize)
		}
		a.zoneManager.UpdateTheme(current)

		// Apply background image/GIF if configured.
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

	// 7. Create touch handler
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

// renderLoop continuously renders frames and sends them to the device.
// Every 3rd frame (~10 FPS) is also broadcast to WebSocket clients as a base64 PNG.
func (a *App) renderLoop() {
	defer a.wg.Done()

	const targetFPS = 30
	frameDuration := time.Second / targetFPS
	ticker := time.NewTicker(frameDuration)
	defer ticker.Stop()

	a.logger.Info("render loop started", "fps", targetFPS)

	var frameCount int
	wsHub := a.apiServer.Hub()

	for {
		select {
		case <-a.ctx.Done():
			a.logger.Info("render loop stopped")
			return

		case <-ticker.C:
			frame, err := a.zoneManager.RenderFrame()
			if err != nil {
				a.logger.Error("failed to render frame", "error", err)
				continue
			}

			// Send to device if connected
			if a.device.IsConnected() {
				if err := a.device.SendFrame(a.ctx, frame.Pix); err != nil {
					a.logger.Debug("failed to send frame", "error", err)
				}
			}

			// During transitions broadcast every frame (30fps) so the WS
			// analyser and Flutter preview see the full motion. Otherwise
			// subsample to every 3rd frame (~10fps) to keep bandwidth low.
			frameCount++
			if a.zoneManager.IsTransitioning() || frameCount%3 == 0 {
				var buf bytes.Buffer
				if err := png.Encode(&buf, frame); err == nil {
					encoded := base64.StdEncoding.EncodeToString(buf.Bytes())
					wsHub.Broadcast(api.WSMessage{Type: "frame", Data: encoded})
				}
			}
		}
	}
}

// stop is no longer used — shutdown logic moved to Shutdown() to ensure
// goroutines exit before the HID handle is closed.
func (a *App) stop() error { return nil }
