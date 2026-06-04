// Package app provides the main application orchestration and dependency injection.
package app

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"sync"
	"time"

	"github.com/mantonx/nexus-next/internal/api"
	"github.com/mantonx/nexus-next/internal/device"
	settings "github.com/mantonx/nexus-next/internal/settings"
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

		// Stop components in reverse order
		shutdownErr = a.stop()

		// Wait for all goroutines to finish
		a.wg.Wait()

		a.logger.Info("shutdown complete")
	})

	return shutdownErr
}

// initialize sets up all application components.
func (a *App) initialize() error {
	a.logger.Debug("initializing application components")

	// 1. Load configuration
	var err error
	a.cfg, err = settings.NewManager(a.configPath)
	if err != nil {
		return fmt.Errorf("failed to create config manager: %w", err)
	}
	a.logger.Info("configuration loaded")

	// 2. Load zone configuration manager
	a.zoneCfg, err = zone.NewConfigManager("")
	if err != nil {
		return fmt.Errorf("failed to create zone config manager: %w", err)
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

	// Register page change callback to restart sampler on page switch
	a.zoneManager.SetOnPageChange(a.zoneSampler.RestartForPage)

	// Register zone sampler as per-zone config notifier so API updates affect live modules.
	a.apiServer.SetZoneConfigNotifier(a.zoneSampler)
	a.logger.Info("zone config notifier registered with API server")

	// 7. Create touch handler
	a.touchHandler = touch.NewHandler(a.logger, a.device, a.zoneManager)
	a.logger.Info("touch handler created")

	return nil
}

// start begins operation of all components.
func (a *App) start() error {
	a.logger.Debug("starting application components")

	// Connect to device
	if err := a.device.Connect(a.ctx); err != nil {
		a.logger.Warn("failed to connect to device", "error", err)
		// Don't fail - device will retry connection
	}

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

	// Start API server in background
	a.wg.Add(1)
	go func() {
		defer a.wg.Done()
		if err := a.apiServer.Start(); err != nil {
			a.logger.Error("API server error", "error", err)
		}
	}()
	a.logger.Info("API server started")

	return nil
}

// renderLoop continuously renders frames and sends them to the device
func (a *App) renderLoop() {
	defer a.wg.Done()

	const targetFPS = 30
	frameDuration := time.Second / targetFPS
	ticker := time.NewTicker(frameDuration)
	defer ticker.Stop()

	a.logger.Info("render loop started", "fps", targetFPS)

	for {
		select {
		case <-a.ctx.Done():
			a.logger.Info("render loop stopped")
			return

		case <-ticker.C:
			// Render frame using zone manager
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
		}
	}
}

// stop halts all running components.
func (a *App) stop() error {
	a.logger.Debug("stopping application components")

	// Stop zone sampler
	if a.zoneSampler != nil {
		a.zoneSampler.Stop()
	}

	// Stop API server
	if a.apiServer != nil {
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := a.apiServer.Shutdown(shutdownCtx); err != nil {
			a.logger.Error("error shutting down API server", "error", err)
		}
	}

	// Stop device connection last
	if a.device != nil {
		if err := a.device.Disconnect(); err != nil {
			a.logger.Error("error disconnecting device", "error", err)
		}
	}

	return nil
}
