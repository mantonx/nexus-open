package instruments

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"nexus-open/internal/settings"
)

// Registry manages a collection of instruments and provides aggregated data
type Registry struct {
	logger     *slog.Logger
	cfg        *config.Manager
	instruments map[string]Instrument
	mu         sync.RWMutex

	// Individual instruments
	cpuTemp *CPUTemp
	gpuTemp *GPUTemp
	network *Network
	weather *Weather

	// Current aggregated data
	data SystemData

	// Lifecycle
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// NewRegistry creates a new instrument registry
func NewRegistry(logger *slog.Logger, cfg *config.Manager) *Registry {
	return &Registry{
		logger:      logger,
		cfg:         cfg,
		instruments: make(map[string]Instrument),
	}
}

// Initialize creates and registers all instruments
func (r *Registry) Initialize() error {
	r.mu.Lock()
	defer r.mu.Unlock()

	// Create instruments
	r.cpuTemp = NewCPUTemp(r.logger, 5*time.Second)
	r.gpuTemp = NewGPUTemp(r.logger, 5*time.Second)
	r.network = NewNetwork(r.logger, 1*time.Second)
	r.weather = NewWeather(r.logger, 10*time.Minute)

	// Register them
	r.instruments[r.cpuTemp.Name()] = r.cpuTemp
	r.instruments[r.gpuTemp.Name()] = r.gpuTemp
	r.instruments[r.network.Name()] = r.network
	r.instruments[r.weather.Name()] = r.weather

	// Note: Weather configuration is now handled per-zone via zone config system
	// Each zone running the weather module will have its own config (location, unit)

	r.logger.Info("instruments initialized", "count", len(r.instruments))
	return nil
}

// Start begins collecting data from all instruments
func (r *Registry) Start(ctx context.Context) error {
	r.mu.Lock()
	if r.cancel != nil {
		r.mu.Unlock()
		return fmt.Errorf("registry already started")
	}
	r.ctx, r.cancel = context.WithCancel(ctx)
	r.mu.Unlock()

	// Start all instruments
	for name, inst := range r.instruments {
		if err := inst.Start(r.ctx); err != nil {
			r.logger.Error("failed to start instrument", "name", name, "error", err)
			return fmt.Errorf("failed to start %s: %w", name, err)
		}
		r.logger.Debug("instrument started", "name", name)
	}

	// Start aggregation loop
	r.wg.Add(1)
	go r.aggregationLoop()

	// Start config watcher
	r.wg.Add(1)
	go r.watchConfig()

	r.logger.Info("instrument registry started")
	return nil
}

// Stop gracefully stops all instruments
func (r *Registry) Stop() error {
	r.mu.Lock()
	if r.cancel == nil {
		r.mu.Unlock()
		return nil
	}
	r.cancel()
	r.mu.Unlock()

	// Stop all instruments
	for name, inst := range r.instruments {
		if err := inst.Stop(); err != nil {
			r.logger.Error("error stopping instrument", "name", name, "error", err)
		}
	}

	// Wait for goroutines
	r.wg.Wait()

	r.logger.Info("instrument registry stopped")
	return nil
}

// GetData returns the current aggregated system data
func (r *Registry) GetData() SystemData {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.data
}

// aggregationLoop periodically collects data from all instruments
func (r *Registry) aggregationLoop() {
	defer r.wg.Done()

	ticker := time.NewTicker(1 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-r.ctx.Done():
			return
		case <-ticker.C:
			r.aggregateData()
		}
	}
}

func (r *Registry) aggregateData() {
	data := SystemData{
		Timestamp: time.Now(),
		Temperature: TemperatureData{
			CPU: r.cpuTemp.GetCurrent(),
			GPU: r.gpuTemp.GetCurrent(),
		},
		Network: r.network.GetCurrent(),
		Weather: r.weather.GetCurrent(),
		CPULoad: 0, // TODO: Add CPU load monitoring
	}

	r.mu.Lock()
	r.data = data
	r.mu.Unlock()
}

// watchConfig monitors configuration changes and updates instruments accordingly
func (r *Registry) watchConfig() {
	defer r.wg.Done()

	// Create channel for config updates
	ch := make(chan config.Config, 1)
	r.cfg.Watch(ch)

	for {
		select {
		case <-r.ctx.Done():
			return
		case cfg := <-ch:
			r.logger.Debug("config changed")
			// UI config changes (colors, fonts) don't affect instruments
			// Module-specific configs (location, unit) are handled per-zone
			_ = cfg // Suppress unused warning
		}
	}
}
