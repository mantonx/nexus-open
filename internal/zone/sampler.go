package zone

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"sync"
	"time"

	"nexus-open/internal/modules/builtin"
	"nexus-open/internal/plugin"
	"nexus-open/internal/zoneconfig"
	"nexus-open/pkg/module"
)

// Sampler manages periodic sampling of modules and updating zone payloads
type Sampler struct {
	logger            *slog.Logger
	manager           *Manager
	pluginHost        *plugin.Host
	zoneCfg           *zoneconfig.Manager
	modules           map[string]module.Module // zoneID -> module instance
	builtins          map[string]module.Module // Built-in modules by name
	cancelFuncs       map[string]context.CancelFunc
	mu                sync.RWMutex
	ctx               context.Context
	cancel            context.CancelFunc
	wg                sync.WaitGroup
	zoneStartTimes    map[string]time.Time
	firstSampleLogged map[string]bool
	moduleSpec        map[string]string
	firstSampleMu     sync.Mutex
	triggerChannels   map[string]chan struct{} // zoneID -> trigger channel for immediate sampling
}

// NewSampler creates a new module sampler
func NewSampler(ctx context.Context, logger *slog.Logger, manager *Manager, zoneCfg *zoneconfig.Manager) *Sampler {
	ctx, cancel := context.WithCancel(ctx)

	s := &Sampler{
		logger:            logger,
		manager:           manager,
		zoneCfg:           zoneCfg,
		pluginHost:        plugin.NewHost(logger),
		modules:           make(map[string]module.Module),
		builtins:          make(map[string]module.Module),
		cancelFuncs:       make(map[string]context.CancelFunc),
		ctx:               ctx,
		cancel:            cancel,
		zoneStartTimes:    make(map[string]time.Time),
		firstSampleLogged: make(map[string]bool),
		moduleSpec:        make(map[string]string),
		triggerChannels:   make(map[string]chan struct{}),
	}

	// Register built-in modules
	s.builtins["clock"] = builtin.NewClock()
	s.builtins["clock24"] = builtin.NewClockWithFormat(builtin.ClockFormat24Hour)
	s.builtins["placeholder"] = builtin.NewPlaceholder("Loading...")

	return s
}

// Start begins sampling all modules configured in the zone manager
func (s *Sampler) Start() error {
	s.logger.Info("starting module sampler")

	// Get all zones and their module configurations
	config := s.manager.GetConfig()
	currentPage := s.manager.GetCurrentPage()

	if currentPage >= len(config.Pages) {
		return fmt.Errorf("invalid page index: %d", currentPage)
	}

	page := config.Pages[currentPage]

	// Start sampling for each zone on the current page
	for _, zoneConfig := range page.Zones {
		if err := s.startZoneSampling(zoneConfig); err != nil {
			s.logger.Error("failed to start zone sampling",
				"zone_id", zoneConfig.ID,
				"module", zoneConfig.Module,
				"error", err)
			// Continue with other zones
		}
	}

	return nil
}

// startZoneSampling starts sampling for a single zone
func (s *Sampler) startZoneSampling(zoneConfig ZoneConfig) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Parse module specification
	moduleSpec := zoneConfig.Module
	var mod module.Module
	var err error

	if strings.HasPrefix(moduleSpec, "builtin:") {
		// Built-in module
		modName := strings.TrimPrefix(moduleSpec, "builtin:")
		var ok bool
		mod, ok = s.builtins[modName]
		if !ok {
			return fmt.Errorf("unknown built-in module: %s", modName)
		}
		s.logger.Info("using built-in module", "zone_id", zoneConfig.ID, "module", modName)
	} else if strings.HasPrefix(moduleSpec, "exec:") {
		// External plugin module
		modPath := strings.TrimPrefix(moduleSpec, "exec:")
		mod, err = s.pluginHost.LaunchModule(s.ctx, zoneConfig.ID, modPath)
		if err != nil {
			return fmt.Errorf("failed to launch plugin: %w", err)
		}
		s.logger.Info("launched plugin module", "zone_id", zoneConfig.ID, "path", modPath)
	} else {
		return fmt.Errorf("invalid module specification: %s", moduleSpec)
	}

	// Store module
	s.modules[zoneConfig.ID] = mod
	s.zoneStartTimes[zoneConfig.ID] = time.Now()
	s.firstSampleLogged[zoneConfig.ID] = false
	s.moduleSpec[zoneConfig.ID] = moduleSpec

	// Send zone config to module (if available and module supports it)
	if s.zoneCfg != nil {
		if config := s.zoneCfg.Get(zoneConfig.ID, moduleSpec); config != nil {
			if notifier, ok := module.SupportsConfigNotification(mod); ok {
				if err := notifier.OnConfigChanged(config); err != nil {
					s.logger.Warn("failed to apply initial zone config",
						"zone_id", zoneConfig.ID,
						"error", err)
				} else {
					s.logger.Info("applied initial zone config",
						"zone_id", zoneConfig.ID,
						"config", config)
				}
			}
		}
	}

	// Create trigger channel for immediate sampling
	s.triggerChannels[zoneConfig.ID] = make(chan struct{}, 1)

	// Start sampling goroutine
	ctx, cancel := context.WithCancel(s.ctx)
	s.cancelFuncs[zoneConfig.ID] = cancel

	// Apply initial configuration if available
	s.applyInitialZoneConfig(zoneConfig.ID, moduleSpec, mod)

	s.wg.Add(1)
	go s.sampleLoop(ctx, zoneConfig.ID, mod, time.Duration(zoneConfig.RefreshMs)*time.Millisecond)

	return nil
}

// sampleLoop periodically samples a module and updates the zone
func (s *Sampler) sampleLoop(ctx context.Context, zoneID string, mod module.Module, interval time.Duration) {
	defer s.wg.Done()

	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	s.logger.Debug("starting sample loop", "zone_id", zoneID, "interval", interval)

	// Sample immediately
	s.sampleOnce(ctx, zoneID, mod)

	// Get the trigger channel for this zone
	s.mu.RLock()
	triggerCh := s.triggerChannels[zoneID]
	s.mu.RUnlock()

	for {
		select {
		case <-ctx.Done():
			s.logger.Debug("sample loop stopped", "zone_id", zoneID)
			return
		case <-ticker.C:
			s.sampleOnce(ctx, zoneID, mod)
		case <-triggerCh:
			// Immediate sample triggered (e.g., by config change)
			s.logger.Debug("immediate sample triggered", "zone_id", zoneID)
			s.sampleOnce(ctx, zoneID, mod)
		}
	}
}

// sampleOnce samples a module once and updates the zone
func (s *Sampler) sampleOnce(parentCtx context.Context, zoneID string, mod module.Module) {
	// Sample with timeout (longer for network-dependent modules like weather)
	// Use parent context so cancellation is respected
	ctx, cancel := context.WithTimeout(parentCtx, 5*time.Second)
	defer cancel()

	// Create a channel to receive the result
	type result struct {
		payload module.Payload
		err     error
	}
	resultCh := make(chan result, 1)

	go func() {
		payload, err := mod.Sample()
		resultCh <- result{payload, err}
	}()

	select {
	case <-ctx.Done():
		// Check if it's a timeout or cancellation
		if parentCtx.Err() != nil {
			// Parent context was cancelled (page change) - silently stop
			s.logger.Debug("module sample cancelled", "zone_id", zoneID)
			return
		}
		// Timeout
		s.logger.Warn("module sample timeout", "zone_id", zoneID)
		// Update with error payload
		s.manager.UpdatePayload(zoneID, module.Payload{
			Primary:   "Timeout",
			Secondary: "Module slow",
			Severity:  module.SeverityWarn,
			Timestamp: time.Now(),
		})
	case res := <-resultCh:
		if res.err != nil {
			s.logger.Error("module sample failed", "zone_id", zoneID, "error", res.err)
			// Update with error payload
			s.manager.UpdatePayload(zoneID, module.Payload{
				Primary:   "Error",
				Secondary: res.err.Error(),
				Severity:  module.SeverityCrit,
				Timestamp: time.Now(),
			})
			return
		}

		// Check if context was cancelled before updating (avoid race)
		if parentCtx.Err() != nil {
			s.logger.Debug("skipping update after cancellation", "zone_id", zoneID)
			return
		}

		// Update zone with payload
		if err := s.manager.UpdatePayload(zoneID, res.payload); err != nil {
			s.logger.Error("failed to update payload", "zone_id", zoneID, "error", err)
		}

		s.recordFirstSample(zoneID)
	}
}

func (s *Sampler) recordFirstSample(zoneID string) {
	s.firstSampleMu.Lock()
	defer s.firstSampleMu.Unlock()

	if s.firstSampleLogged[zoneID] {
		return
	}

	start, ok := s.zoneStartTimes[zoneID]
	if !ok {
		return
	}

	latency := time.Since(start)
	moduleSpec := s.moduleSpec[zoneID]
	s.firstSampleLogged[zoneID] = true
	if latency < 0 {
		latency = 0
	}

	s.logger.Info("zone first payload",
		"zone_id", zoneID,
		"latency_ms", latency.Milliseconds(),
		"module", moduleSpec)
}

// Stop stops all sampling
func (s *Sampler) Stop() {
	s.logger.Info("stopping module sampler")

	s.mu.Lock()
	// Cancel all zone sampling loops
	for zoneID, cancel := range s.cancelFuncs {
		s.logger.Debug("stopping zone sampling", "zone_id", zoneID)
		cancel()
	}
	s.mu.Unlock()

	// Cancel main context
	s.cancel()

	// Wait for all goroutines to finish
	s.wg.Wait()

	// Shutdown plugin host
}

// RestartForPage restarts sampling for a new page
func (s *Sampler) RestartForPage(pageIndex int) error {
	s.logger.Info("restarting sampler for new page", "page", pageIndex)

	// Stop current sampling
	s.mu.Lock()
	for zoneID, cancel := range s.cancelFuncs {
		cancel()
		delete(s.cancelFuncs, zoneID)
		delete(s.modules, zoneID)
		delete(s.zoneStartTimes, zoneID)
		delete(s.firstSampleLogged, zoneID)
		delete(s.moduleSpec, zoneID)
	}
	s.mu.Unlock()

	// Wait for goroutines to stop
	s.wg.Wait()

	// Start sampling for new page
	config := s.manager.GetConfig()
	if pageIndex >= len(config.Pages) {
		return fmt.Errorf("invalid page index: %d", pageIndex)
	}

	page := config.Pages[pageIndex]
	for _, zoneConfig := range page.Zones {
		if err := s.startZoneSampling(zoneConfig); err != nil {
			s.logger.Error("failed to restart zone sampling",
				"zone_id", zoneConfig.ID,
				"error", err)
		}
	}

	return nil
}

// applyInitialZoneConfig notifies a module of its current configuration before sampling starts.
func (s *Sampler) applyInitialZoneConfig(zoneID, moduleSpec string, mod module.Module) {
	if s.zoneCfg == nil {
		return
	}

	notifier, ok := module.SupportsConfigNotification(mod)
	if !ok {
		return
	}

	config := s.zoneCfg.Get(zoneID, moduleSpec)
	if len(config) == 0 {
		return
	}

	if err := notifier.OnConfigChanged(config); err != nil {
		s.logger.Warn("failed to apply initial zone config",
			"zone_id", zoneID,
			"module", moduleSpec,
			"error", err)
		return
	}

	s.logger.Info("applied initial zone config",
		"zone_id", zoneID,
		"module", moduleSpec,
		"config", config)
}

// BroadcastConfigChange notifies all modules about a configuration change.
// Only modules implementing the ConfigNotifier interface will receive the notification.
func (s *Sampler) BroadcastConfigChange(config map[string]interface{}) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	s.logger.Info("broadcasting config change to modules", "module_count", len(s.modules))

	notified := 0
	skipped := 0
	var zonesToResample []string

	// Notify all loaded modules
	for zoneID, mod := range s.modules {
		if notifier, ok := module.SupportsConfigNotification(mod); ok {
			if err := notifier.OnConfigChanged(config); err != nil {
				s.logger.Error("module config notification failed",
					"zone_id", zoneID,
					"module", s.moduleSpec[zoneID],
					"error", err)
			} else {
				s.logger.Debug("module config notified",
					"zone_id", zoneID,
					"module", s.moduleSpec[zoneID])
				notified++
				zonesToResample = append(zonesToResample, zoneID)
			}
		} else {
			s.logger.Debug("module does not support config notifications",
				"zone_id", zoneID,
				"module", s.moduleSpec[zoneID])
			skipped++
		}
	}

	s.logger.Info("config broadcast complete",
		"notified", notified,
		"skipped", skipped,
		"total", len(s.modules))

	// Trigger immediate resampling for modules that were notified
	for _, zoneID := range zonesToResample {
		if triggerCh, ok := s.triggerChannels[zoneID]; ok {
			// Non-blocking send (channel has buffer of 1)
			select {
			case triggerCh <- struct{}{}:
				s.logger.Debug("triggered immediate resample", "zone_id", zoneID)
			default:
				// Channel already has a pending trigger, skip
			}
		}
	}
}

// BroadcastZoneConfigChange broadcasts a config change to a specific zone's module.
// Returns an error if the zone doesn't exist or doesn't support config notifications.
func (s *Sampler) BroadcastZoneConfigChange(zoneID string, config map[string]interface{}) error {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Find the module for this zone
	mod, exists := s.modules[zoneID]
	if !exists {
		return fmt.Errorf("zone %q not found", zoneID)
	}

	// Check if module supports config notifications
	notifier, ok := module.SupportsConfigNotification(mod)
	if !ok {
		return fmt.Errorf("module for zone %q does not support config notifications", zoneID)
	}

	// Send config to module
	if err := notifier.OnConfigChanged(config); err != nil {
		s.logger.Error("zone config notification failed",
			"zone_id", zoneID,
			"module", s.moduleSpec[zoneID],
			"error", err)
		return fmt.Errorf("config notification failed: %w", err)
	}

	s.logger.Info("zone config updated",
		"zone_id", zoneID,
		"module", s.moduleSpec[zoneID],
		"config", config)

	// Trigger immediate resampling
	if triggerCh, ok := s.triggerChannels[zoneID]; ok {
		select {
		case triggerCh <- struct{}{}:
			s.logger.Debug("triggered immediate resample", "zone_id", zoneID)
		default:
			// Channel already has a pending trigger, skip
		}
	}

	return nil
}
