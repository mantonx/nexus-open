// Package app provides the main application orchestration and dependency injection.
package app

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"nexus-open/internal/api"
	"nexus-open/internal/config"
	"nexus-open/internal/device"
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

	// Components
	cfg       *config.Manager
	device    device.Device
	apiServer *api.Server
	// display     *display.Renderer
	// instruments *instruments.Registry

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
	a.cfg, err = config.NewManager(a.configPath)
	if err != nil {
		return fmt.Errorf("failed to create config manager: %w", err)
	}
	a.logger.Info("configuration loaded")

	// 2. Create device connection
	deviceConfig := device.ConnectionConfig{
		VendorID:         0x1b1c, // Corsair
		ProductID:        0x1b8e, // iCUE Nexus
		ReconnectRetries: 10,
		ReconnectDelay:   5 * time.Second,
	}
	a.device = device.NewNexusDevice(a.logger, deviceConfig)
	a.logger.Info("device created")

	// 3. Create API server
	apiAddr := fmt.Sprintf(":%d", a.apiPort)
	a.apiServer = api.NewServer(apiAddr, a.cfg, a.logger)
	a.logger.Info("API server created", "addr", apiAddr)

	// TODO: Initialize remaining components
	// 4. Set up display renderer
	// 5. Initialize instruments

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

	// Start API server in background
	a.wg.Add(1)
	go func() {
		defer a.wg.Done()
		if err := a.apiServer.Start(); err != nil {
			a.logger.Error("API server error", "error", err)
		}
	}()

	// TODO: Start remaining components
	// 1. Start instrument data collection
	// 2. Start display update loop
	// 3. Start configuration watcher

	return nil
}

// stop halts all running components.
func (a *App) stop() error {
	a.logger.Debug("stopping application components")

	// Stop API server first
	if a.apiServer != nil {
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := a.apiServer.Shutdown(shutdownCtx); err != nil {
			a.logger.Error("error shutting down API server", "error", err)
		}
	}

	// Stop device connection
	if a.device != nil {
		if err := a.device.Disconnect(); err != nil {
			a.logger.Error("error disconnecting device", "error", err)
		}
	}

	// TODO: Stop remaining components in reverse order
	// 1. Stop display updates
	// 2. Stop instruments

	return nil
}
