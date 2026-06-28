// Package zone manages display zones, plugin sampling, and page transitions.
package zone

import (
	"context"
	"fmt"
	"log/slog"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/mantonx/nexus-open/internal/plugins/builtin"
	pluginhost "github.com/mantonx/nexus-open/internal/plugins/host"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// ZoneStatus represents the current health of a single zone's plugin.
type ZoneStatus struct {
	Status string // "ok" | "error" | "timeout" | "loading"
	Error  string // non-empty when Status is "error" or "timeout"
}

// Sampler manages periodic sampling of modules and updating zone payloads
type Sampler struct {
	logger            *slog.Logger
	manager           *Manager
	pluginHost        pluginhost.PluginHost
	zoneCfg           *ConfigManager
	pluginsDir        string // absolute path where exec: plugins live
	modules           map[string]plugin.Plugin        // zoneID -> plugin instance
	builtins          map[string]func() plugin.Plugin // Built-in factories — called per zone so each zone gets its own instance
	cancelFuncs       map[string]context.CancelFunc
	mu                sync.RWMutex
	ctx               context.Context
	cancel            context.CancelFunc
	wg                sync.WaitGroup
	zoneStartTimes    map[string]time.Time
	firstSampleLogged map[string]bool
	pluginSpec        map[string]string
	zoneWidths        map[string]int            // zoneID -> pixel width, for injecting into Configure
	triggerChannels   map[string]chan struct{}   // zoneID -> trigger channel for immediate sampling
	zoneErrors        map[string]ZoneStatus     // zoneID -> last known status
	zoneErrorsMu      sync.RWMutex

	// crashBackoffStart is the initial backoff duration for plugin crash restarts.
	// Defaults to crashBackoffInit (1s). Overridable in tests to avoid real waits.
	crashBackoffStart time.Duration
}

// NewSampler creates a new plugin sampler. pluginsDir is the absolute path to
// the directory containing exec: plugin binaries. Pass "" to use the default
// (sibling plugins/ directory next to the running executable).
func NewSampler(ctx context.Context, logger *slog.Logger, manager *Manager, zoneCfg *ConfigManager, pluginsDir string) *Sampler {
	ctx, cancel := context.WithCancel(ctx)

	s := &Sampler{
		logger:            logger,
		manager:           manager,
		zoneCfg:           zoneCfg,
		pluginsDir:        pluginsDir,
		pluginHost:        pluginhost.NewHost(logger),
		modules:           make(map[string]plugin.Plugin),
		builtins:          make(map[string]func() plugin.Plugin),
		cancelFuncs:       make(map[string]context.CancelFunc),
		ctx:               ctx,
		cancel:            cancel,
		zoneStartTimes:    make(map[string]time.Time),
		firstSampleLogged: make(map[string]bool),
		pluginSpec:        make(map[string]string),
		zoneWidths:        make(map[string]int),
		triggerChannels:   make(map[string]chan struct{}),
		zoneErrors:        make(map[string]ZoneStatus),
		crashBackoffStart: crashBackoffInit,
	}

	// Register built-in factories — each zone gets its own instance via the factory,
	// preventing Configure() calls on one zone from stomping another zone's state.
	s.builtins["clock"] = func() plugin.Plugin { return builtin.NewClock() }
	s.builtins["clock24"] = func() plugin.Plugin { return builtin.NewClockWithFormat(builtin.ClockFormat24Hour) }
	s.builtins["placeholder"] = func() plugin.Plugin { return builtin.NewPlaceholder("Loading...") }

	return s
}

// normalizePluginID converts legacy path-form exec IDs to the canonical short form.
//
//	exec:./plugins/cpu-temp/nexus-cpu-temp  →  exec:nexus-cpu-temp
//	exec:nexus-cpu-temp                     →  exec:nexus-cpu-temp  (unchanged)
//	builtin:clock                           →  builtin:clock        (unchanged)
func NormalizePluginID(id string) string {
	if !strings.HasPrefix(id, "exec:") {
		return id
	}
	rel := strings.TrimPrefix(id, "exec:")
	name := filepath.Base(rel)
	return "exec:" + name
}

// resolvePluginPath converts an exec: spec into an absolute binary path and
// confirms it stays inside pluginsDir. Returns ("", error) if the spec is
// absolute, contains directory traversal, or escapes the plugins directory.
//
//	exec:nexus-cpu-temp  →  <pluginsDir>/nexus-cpu-temp
func (s *Sampler) resolvePluginPath(spec string) (string, error) {
	name := strings.TrimPrefix(spec, "exec:")

	// Absolute paths are never allowed — they bypass the plugins directory entirely.
	if filepath.IsAbs(name) {
		return "", fmt.Errorf("plugin spec must not be an absolute path: %q", spec)
	}

	// Only a bare binary name is accepted — no subdirectory components.
	if strings.ContainsRune(name, '/') {
		return "", fmt.Errorf("plugin spec must not contain path separators: %q", spec)
	}

	return filepath.Join(s.pluginsDir, name), nil
}

// Start begins sampling all zones across all pages. Keeping every plugin
// running at all times means page switches are instant — no subprocess
// teardown/restart on swipe, so payloads are always current when a page
// becomes visible.
func (s *Sampler) Start() error {
	s.logger.Info("starting plugin sampler")

	config := s.manager.GetConfig()

	seen := make(map[string]bool)
	for _, page := range config.Pages {
		for _, zoneConfig := range page.Zones {
			if seen[zoneConfig.ID] {
				continue
			}
			seen[zoneConfig.ID] = true
			if err := s.startZoneSampling(zoneConfig); err != nil {
				s.logger.Error("failed to start zone sampling",
					"zone_id", zoneConfig.ID,
					"plugin", zoneConfig.Plugin,
					"error", err)
				s.markZoneLaunchFailed(zoneConfig.ID, err)
			}
		}
	}

	return nil
}

// startZoneSampling starts sampling for a single zone
func (s *Sampler) startZoneSampling(zoneConfig ZoneConfig) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Parse plugin specification, normalising legacy path forms to the short canonical form.
	pluginSpec := NormalizePluginID(zoneConfig.Plugin)
	var mod plugin.Plugin
	var err error

	if strings.HasPrefix(pluginSpec, "builtin:") {
		// Built-in plugin — call the factory to get a fresh instance per zone.
		modName := strings.TrimPrefix(pluginSpec, "builtin:")
		factory, ok := s.builtins[modName]
		if !ok {
			return fmt.Errorf("unknown built-in plugin: %s", modName)
		}
		mod = factory()
		s.logger.Info("using built-in plugin", "zone_id", zoneConfig.ID, "plugin", modName)
	} else if strings.HasPrefix(pluginSpec, "exec:") {
		// External plugin
		var modPath string
		modPath, err = s.resolvePluginPath(pluginSpec)
		if err != nil {
			return fmt.Errorf("failed to launch plugin: %w", err)
		}
		mod, err = s.pluginHost.LaunchPlugin(s.ctx, zoneConfig.ID, modPath)
		if err != nil {
			return fmt.Errorf("failed to launch plugin: %w", err)
		}
		s.logger.Info("launched plugin", "zone_id", zoneConfig.ID, "path", modPath)
	} else {
		return fmt.Errorf("invalid plugin specification: %s", pluginSpec)
	}

	// Store plugin
	s.modules[zoneConfig.ID] = mod
	s.zoneStartTimes[zoneConfig.ID] = time.Now()
	s.firstSampleLogged[zoneConfig.ID] = false
	s.pluginSpec[zoneConfig.ID] = pluginSpec
	s.zoneWidths[zoneConfig.ID] = zoneConfig.Width

	// Apply stored zone config to the plugin before sampling starts.
	s.applyInitialZoneConfig(zoneConfig, pluginSpec, mod)

	// Create trigger channel for immediate sampling
	s.triggerChannels[zoneConfig.ID] = make(chan struct{}, 1)

	// Start sampling goroutine
	ctx, cancel := context.WithCancel(s.ctx)
	s.cancelFuncs[zoneConfig.ID] = cancel

	s.wg.Add(1)
	go s.sampleLoop(ctx, zoneConfig.ID, mod, time.Duration(zoneConfig.RefreshMs)*time.Millisecond)

	return nil
}

// sampleLoop periodically samples a plugin and updates the zone.
// For exec: plugins it also detects subprocess crashes and relaunches with
// exponential backoff (1s → 2s → 4s → … capped at 30s, reset on success).
func (s *Sampler) sampleLoop(ctx context.Context, zoneID string, mod plugin.Plugin, interval time.Duration) {
	defer s.wg.Done()

	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	s.logger.Debug("starting sample loop", "zone_id", zoneID, "interval", interval)

	// Sample immediately on start.
	s.sampleOnce(ctx, zoneID, mod, interval) //nolint:errcheck // initial sample; can't restart here

	// Get the trigger channel for this zone.
	s.mu.RLock()
	triggerCh := s.triggerChannels[zoneID]
	s.mu.RUnlock()

	// isExec is true for subprocess plugins — only those can crash or hang.
	s.mu.RLock()
	isExec := strings.HasPrefix(s.pluginSpec[zoneID], "exec:")
	s.mu.RUnlock()

	backoff := s.crashBackoffStart

	for {
		select {
		case <-ctx.Done():
			s.logger.Debug("sample loop stopped", "zone_id", zoneID)
			return

		case <-ticker.C:
			dead := s.sampleOnce(ctx, zoneID, mod, interval)

			// Treat a hung (timeout) plugin the same as a crashed one: evict
			// and restart. IsAlive also catches crashes the timeout path misses.
			if isExec && (dead || !s.pluginHost.IsAlive(zoneID)) {
				mod = s.handlePluginCrash(ctx, zoneID, &backoff, interval)
				if mod == nil {
					return // context cancelled during restart
				}
			} else {
				backoff = crashBackoffInit // reset on healthy tick
			}

		case <-triggerCh:
			s.logger.Debug("immediate sample triggered", "zone_id", zoneID)
			s.sampleOnce(ctx, zoneID, mod, interval) //nolint:errcheck // trigger path; crash caught next tick
		}
	}
}

// ZoneStatus returns the last known health status for a zone.
// Returns {Status: "loading"} if no sample has completed yet.
func (s *Sampler) GetZoneStatus(zoneID string) ZoneStatus {
	s.zoneErrorsMu.RLock()
	defer s.zoneErrorsMu.RUnlock()
	if st, ok := s.zoneErrors[zoneID]; ok {
		return st
	}
	return ZoneStatus{Status: "loading"}
}

// GetPlugin returns the live plugin instance for zoneID, if one is loaded.
func (s *Sampler) GetPlugin(zoneID string) (plugin.Plugin, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	p, ok := s.modules[zoneID]
	return p, ok
}

// AllZoneStatuses returns a snapshot of all zone statuses.
func (s *Sampler) AllZoneStatuses() map[string]ZoneStatus {
	s.zoneErrorsMu.RLock()
	defer s.zoneErrorsMu.RUnlock()
	out := make(map[string]ZoneStatus, len(s.zoneErrors))
	for k, v := range s.zoneErrors {
		out[k] = v
	}
	return out
}

// Stop stops all sampling
func (s *Sampler) Stop() {
	s.logger.Info("stopping plugin sampler")

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

	// Kill all external plugin subprocesses now that no goroutine holds a
	// reference. Must happen after wg.Wait so we don't race a goroutine
	// mid-LaunchPlugin or mid-Sample.
	if h, ok := s.pluginHost.(interface{ StopAll() }); ok {
		h.StopAll()
	}
}

// RestartZone stops and restarts sampling for a single zone with a new plugin spec.
// Used when a tap action cycles the zone to its next plugin choice.
func (s *Sampler) RestartZone(zoneConfig ZoneConfig) error {
	s.mu.Lock()
	if cancel, ok := s.cancelFuncs[zoneConfig.ID]; ok {
		cancel()
		delete(s.cancelFuncs, zoneConfig.ID)
		delete(s.modules, zoneConfig.ID)
		delete(s.pluginSpec, zoneConfig.ID)
	}
	s.mu.Unlock()

	// Evict the old subprocess so it doesn't linger until global StopAll.
	s.pluginHost.Evict(zoneConfig.ID)

	return s.startZoneSampling(zoneConfig)
}

// RestartForPage restarts sampling for a new page.
// Old zone goroutines are cancelled and new ones start immediately —
// we don't wait for old goroutines to exit so there's no blocking delay.
func (s *Sampler) RestartForPage(pageIndex int) error {
	s.logger.Info("restarting sampler for new page", "page", pageIndex)

	config := s.manager.GetConfig()
	if pageIndex >= len(config.Pages) {
		return fmt.Errorf("invalid page index: %d", pageIndex)
	}

	// Cancel old goroutines — they'll stop asynchronously.
	s.mu.Lock()
	for zoneID, cancel := range s.cancelFuncs {
		cancel()
		delete(s.cancelFuncs, zoneID)
		delete(s.modules, zoneID)
		delete(s.zoneStartTimes, zoneID)
		delete(s.firstSampleLogged, zoneID)
		delete(s.pluginSpec, zoneID)
	}
	s.mu.Unlock()

	// Start new zone goroutines immediately without waiting for old ones to exit.
	page := config.Pages[pageIndex]
	for _, zoneConfig := range page.Zones {
		if err := s.startZoneSampling(zoneConfig); err != nil {
			s.logger.Error("failed to restart zone sampling",
				"zone_id", zoneConfig.ID,
				"error", err)
			s.markZoneLaunchFailed(zoneConfig.ID, err)
		}
	}

	return nil
}

// applyInitialZoneConfig delivers the stored zone config to the plugin before sampling starts.
// Zone pixel dimensions are injected under the reserved keys declared in pkg/plugin
// so plugins like the analog clock can size their RawFrame output correctly.
func (s *Sampler) applyInitialZoneConfig(zoneConfig ZoneConfig, pluginSpec string, mod plugin.Plugin) {
	if s.zoneCfg == nil {
		return
	}

	cfg := s.zoneCfg.Get(zoneConfig.ID, pluginSpec)
	if len(cfg) == 0 {
		cfg = make(map[string]any)
	}

	cfg[plugin.ConfigKeyZoneWidth] = zoneConfig.Width
	cfg[plugin.ConfigKeyZoneHeight] = DisplayHeight

	if err := mod.Configure(cfg); err != nil {
		s.logger.Warn("failed to apply initial zone config",
			"zone_id", zoneConfig.ID,
			"plugin", pluginSpec,
			"error", err)
		return
	}

	s.logger.Info("applied initial zone config",
		"zone_id", zoneConfig.ID,
		"plugin", pluginSpec,
		"config", cfg)
}

// BroadcastConfigChange delivers a global config update to all loaded plugins.
// Kept for the settings-level config path; zone-specific config goes through
// BroadcastZoneConfigChange.
func (s *Sampler) BroadcastConfigChange(config map[string]any) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var toResample []string
	for zoneID, mod := range s.modules {
		if err := mod.Configure(config); err != nil {
			s.logger.Error("plugin Configure failed",
				"zone_id", zoneID,
				"plugin", s.pluginSpec[zoneID],
				"error", err)
		} else {
			toResample = append(toResample, zoneID)
		}
	}

	for _, zoneID := range toResample {
		if ch, ok := s.triggerChannels[zoneID]; ok {
			select {
			case ch <- struct{}{}:
			default:
			}
		}
	}
}

// BroadcastZoneConfigChange delivers a config update to a specific zone's plugin.
func (s *Sampler) BroadcastZoneConfigChange(zoneID string, config map[string]any) error {
	s.mu.RLock()
	defer s.mu.RUnlock()

	mod, exists := s.modules[zoneID]
	if !exists {
		return fmt.Errorf("zone %q not found", zoneID)
	}

	if w, ok := s.zoneWidths[zoneID]; ok {
		config[plugin.ConfigKeyZoneWidth] = w
		config[plugin.ConfigKeyZoneHeight] = DisplayHeight
	}

	if err := mod.Configure(config); err != nil {
		s.logger.Error("zone Configure failed",
			"zone_id", zoneID,
			"plugin", s.pluginSpec[zoneID],
			"error", err)
		return fmt.Errorf("Configure failed: %w", err)
	}

	s.logger.Info("zone config updated",
		"zone_id", zoneID,
		"plugin", s.pluginSpec[zoneID],
		"config", config)

	if ch, ok := s.triggerChannels[zoneID]; ok {
		select {
		case ch <- struct{}{}:
		default:
		}
	}

	return nil
}
