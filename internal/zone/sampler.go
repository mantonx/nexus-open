// Package zone manages display zones, plugin sampling, and page transitions.
package zone

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/mantonx/nexus-next/internal/plugins/builtin"
	pluginhost "github.com/mantonx/nexus-next/internal/plugins/host"
	"github.com/mantonx/nexus-next/pkg/plugin"
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
	modules           map[string]plugin.Plugin // zoneID -> plugin instance
	builtins          map[string]plugin.Plugin // Built-in plugins by name
	cancelFuncs       map[string]context.CancelFunc
	mu                sync.RWMutex
	ctx               context.Context
	cancel            context.CancelFunc
	wg                sync.WaitGroup
	zoneStartTimes    map[string]time.Time
	firstSampleLogged map[string]bool
	pluginSpec        map[string]string
	triggerChannels   map[string]chan struct{} // zoneID -> trigger channel for immediate sampling
	zoneErrors        map[string]ZoneStatus   // zoneID -> last known status
	zoneErrorsMu      sync.RWMutex
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
		builtins:          make(map[string]plugin.Plugin),
		cancelFuncs:       make(map[string]context.CancelFunc),
		ctx:               ctx,
		cancel:            cancel,
		zoneStartTimes:    make(map[string]time.Time),
		firstSampleLogged: make(map[string]bool),
		pluginSpec:        make(map[string]string),
		triggerChannels:   make(map[string]chan struct{}),
		zoneErrors:        make(map[string]ZoneStatus),
	}

	// Register built-in modules
	s.builtins["clock"] = builtin.NewClock()
	s.builtins["clock24"] = builtin.NewClockWithFormat(builtin.ClockFormat24Hour)
	s.builtins["placeholder"] = builtin.NewPlaceholder("Loading...")

	return s
}

// resolvePluginPath converts an exec: spec like "exec:./plugins/cpu-temp/cpu-temp"
// into an absolute path using pluginsDir. Paths that are already absolute are
// returned unchanged. The "./plugins/" prefix is treated as a conventional
// relative marker; everything after it is appended to pluginsDir.
func (s *Sampler) resolvePluginPath(spec string) string {
	rel := strings.TrimPrefix(spec, "exec:")
	if filepath.IsAbs(rel) {
		return rel
	}
	// Strip the conventional ./plugins/ prefix and anchor to pluginsDir.
	stripped := strings.TrimPrefix(rel, "./plugins/")
	return filepath.Join(s.pluginsDir, stripped)
}

// Start begins sampling all modules configured in the zone manager
func (s *Sampler) Start() error {
	s.logger.Info("starting plugin sampler")

	// Get all zones and their plugin configurations
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
				"plugin", zoneConfig.Plugin,
				"error", err)
			s.markZoneLaunchFailed(zoneConfig.ID, err)
		}
	}

	return nil
}

// startZoneSampling starts sampling for a single zone
func (s *Sampler) startZoneSampling(zoneConfig ZoneConfig) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Parse plugin specification
	pluginSpec := zoneConfig.Plugin
	var mod plugin.Plugin
	var err error

	if strings.HasPrefix(pluginSpec, "builtin:") {
		// Built-in plugin
		modName := strings.TrimPrefix(pluginSpec, "builtin:")
		var ok bool
		mod, ok = s.builtins[modName]
		if !ok {
			return fmt.Errorf("unknown built-in plugin: %s", modName)
		}
		s.logger.Info("using built-in plugin", "zone_id", zoneConfig.ID, "plugin", modName)
	} else if strings.HasPrefix(pluginSpec, "exec:") {
		// External plugin
		modPath := s.resolvePluginPath(pluginSpec)
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

	// Apply stored zone config to the plugin before sampling starts.
	s.applyInitialZoneConfig(zoneConfig.ID, pluginSpec, mod)

	// Create trigger channel for immediate sampling
	s.triggerChannels[zoneConfig.ID] = make(chan struct{}, 1)

	// Start sampling goroutine
	ctx, cancel := context.WithCancel(s.ctx)
	s.cancelFuncs[zoneConfig.ID] = cancel

	s.wg.Add(1)
	go s.sampleLoop(ctx, zoneConfig.ID, mod, time.Duration(zoneConfig.RefreshMs)*time.Millisecond)

	return nil
}

const (
	crashBackoffInit = 1 * time.Second
	crashBackoffMax  = 30 * time.Second
)

// sampleLoop periodically samples a plugin and updates the zone.
// For exec: plugins it also detects subprocess crashes and relaunches with
// exponential backoff (1s → 2s → 4s → … capped at 30s, reset on success).
func (s *Sampler) sampleLoop(ctx context.Context, zoneID string, mod plugin.Plugin, interval time.Duration) {
	defer s.wg.Done()

	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	s.logger.Debug("starting sample loop", "zone_id", zoneID, "interval", interval)

	// Sample immediately on start.
	s.sampleOnce(ctx, zoneID, mod) //nolint:errcheck // initial sample; can't restart here

	// Get the trigger channel for this zone.
	s.mu.RLock()
	triggerCh := s.triggerChannels[zoneID]
	s.mu.RUnlock()

	// isExec is true for subprocess plugins — only those can crash or hang.
	s.mu.RLock()
	isExec := strings.HasPrefix(s.pluginSpec[zoneID], "exec:")
	s.mu.RUnlock()

	backoff := crashBackoffInit

	for {
		select {
		case <-ctx.Done():
			s.logger.Debug("sample loop stopped", "zone_id", zoneID)
			return

		case <-ticker.C:
			dead := s.sampleOnce(ctx, zoneID, mod)

			// Treat a hung (timeout) plugin the same as a crashed one: evict
			// and restart. IsAlive also catches crashes the timeout path misses.
			if isExec && (dead || !s.pluginHost.IsAlive(zoneID)) {
				mod = s.handlePluginCrash(ctx, zoneID, &backoff)
				if mod == nil {
					return // context cancelled during restart
				}
			} else {
				backoff = crashBackoffInit // reset on healthy tick
			}

		case <-triggerCh:
			s.logger.Debug("immediate sample triggered", "zone_id", zoneID)
			s.sampleOnce(ctx, zoneID, mod) //nolint:errcheck // trigger path; crash caught next tick
		}
	}
}

// handlePluginCrash evicts the dead subprocess and relaunches it after a
// backoff delay. Returns the new plugin.Plugin on success, or nil if the
// context was cancelled before the plugin could be restarted.
func (s *Sampler) handlePluginCrash(ctx context.Context, zoneID string, backoff *time.Duration) plugin.Plugin {
	s.logger.Warn("plugin subprocess exited unexpectedly, scheduling restart",
		"zone_id", zoneID,
		"backoff", backoff.String())

	s.pluginHost.Evict(zoneID)
	s.markZoneLaunchFailed(zoneID, fmt.Errorf("plugin crashed; restarting in %s", backoff.String()))

	// Wait for backoff period or cancellation.
	select {
	case <-ctx.Done():
		return nil
	case <-time.After(*backoff):
	}

	// Grow backoff for next crash, capped at max.
	*backoff *= 2
	if *backoff > crashBackoffMax {
		*backoff = crashBackoffMax
	}

	// Re-read the plugin path from the spec stored at start time.
	s.mu.RLock()
	spec := s.pluginSpec[zoneID]
	s.mu.RUnlock()

	modPath := s.resolvePluginPath(spec)
	mod, err := s.pluginHost.LaunchPlugin(ctx, zoneID, modPath)
	if err != nil {
		s.logger.Error("plugin restart failed", "zone_id", zoneID, "error", err)
		s.markZoneLaunchFailed(zoneID, err)
		// Return a nil mod so the caller exits — the zone stays in error state
		// until the user reloads or a page change triggers RestartForPage.
		return nil
	}

	// Update the modules map so BroadcastConfigChange finds the new instance.
	s.mu.Lock()
	s.modules[zoneID] = mod
	s.mu.Unlock()

	s.logger.Info("plugin restarted successfully", "zone_id", zoneID, "path", modPath)
	s.setZoneStatus(zoneID, ZoneStatus{Status: "loading"})

	// Prime the new process with an immediate sample (crash on this prime
	// is caught by IsAlive on the next regular tick).
	s.sampleOnce(ctx, zoneID, mod) //nolint:errcheck
	return mod
}

// sampleOnce samples a plugin once and updates the zone.
// Returns true if the plugin should be treated as dead (timeout or hard error)
// and the caller should evict and restart it.
func (s *Sampler) sampleOnce(parentCtx context.Context, zoneID string, mod plugin.Plugin) (dead bool) {
	ctx, cancel := context.WithTimeout(parentCtx, 5*time.Second)
	defer cancel()

	type result struct {
		payload plugin.Payload
		err     error
	}
	resultCh := make(chan result, 1)

	go func() {
		payload, err := mod.Sample()
		resultCh <- result{payload, err}
	}()

	select {
	case <-ctx.Done():
		if parentCtx.Err() != nil {
			// Parent context cancelled (page change) — not a plugin fault.
			s.logger.Debug("plugin sample cancelled", "zone_id", zoneID)
			return false
		}
		// 5s timeout — the subprocess is hung. Treat it as dead so the
		// caller evicts it and restarts with backoff.
		s.logger.Warn("plugin sample timeout, evicting", "zone_id", zoneID)
		s.setZoneStatus(zoneID, ZoneStatus{Status: "timeout", Error: "plugin hung; restarting"})
		s.manager.UpdatePayload(zoneID, plugin.Payload{ //nolint:errcheck
			Primary:   "Timeout",
			Secondary: "Restarting…",
			Severity:  plugin.SeverityWarn,
			Timestamp: time.Now(),
		})
		return true

	case res := <-resultCh:
		if res.err != nil {
			s.logger.Error("plugin sample failed", "zone_id", zoneID, "error", res.err)
			s.setZoneStatus(zoneID, ZoneStatus{Status: "error", Error: res.err.Error()})
			s.manager.UpdatePayload(zoneID, plugin.Payload{ //nolint:errcheck
				Primary:   "Error",
				Secondary: res.err.Error(),
				Severity:  plugin.SeverityCrit,
				Timestamp: time.Now(),
			})
			return false // error ≠ crash; IsAlive check covers actual subprocess death
		}

		if parentCtx.Err() != nil {
			s.logger.Debug("skipping update after cancellation", "zone_id", zoneID)
			return false
		}

		if err := s.manager.UpdatePayload(zoneID, res.payload); err != nil {
			s.logger.Error("failed to update payload", "zone_id", zoneID, "error", err)
		}

		s.setZoneStatus(zoneID, ZoneStatus{Status: "ok"})
		s.recordFirstSample(zoneID)
		return false
	}
}

// setZoneStatus records the current health of a zone.
func (s *Sampler) setZoneStatus(zoneID string, status ZoneStatus) {
	s.zoneErrorsMu.Lock()
	s.zoneErrors[zoneID] = status
	s.zoneErrorsMu.Unlock()
}

// markZoneLaunchFailed records an error status and pushes a visible error
// payload so the display shows something useful instead of staying blank.
func (s *Sampler) markZoneLaunchFailed(zoneID string, err error) {
	s.setZoneStatus(zoneID, ZoneStatus{Status: "error", Error: err.Error()})
	s.manager.UpdatePayload(zoneID, plugin.Payload{ //nolint:errcheck
		Primary:   "Error",
		Secondary: err.Error(),
		Severity:  plugin.SeverityCrit,
		Timestamp: time.Now(),
	})
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

// CatalogEntry is one entry in the plugin catalog returned by GetCatalog.
type CatalogEntry struct {
	ID         string            `json:"id"`   // e.g. "builtin:clock" or "exec:cpu-temp"
	Kind       string            `json:"kind"` // "builtin" or "exec"
	Descriptor plugin.Descriptor `json:"descriptor"`
}

// GetCatalog returns metadata for all available plugins: running builtins (from
// the loaded modules map) plus every executable found in pluginsDir.
// Exec plugins not currently loaded are launched briefly to retrieve their Descriptor.
func (s *Sampler) GetCatalog() []CatalogEntry {
	seen := make(map[string]bool)
	var entries []CatalogEntry

	// Collect builtins from loaded modules.
	s.mu.RLock()
	for zoneID, mod := range s.modules {
		spec := s.pluginSpec[zoneID]
		if !strings.HasPrefix(spec, "builtin:") || seen[spec] {
			continue
		}
		seen[spec] = true
		if desc, err := mod.Describe(); err == nil {
			entries = append(entries, CatalogEntry{ID: spec, Kind: "builtin", Descriptor: desc})
		}
	}
	s.mu.RUnlock()

	// Always include static builtins even if no zone is using them right now.
	for _, id := range []string{"builtin:clock", "builtin:placeholder", "builtin:debug"} {
		if seen[id] {
			continue
		}
		seen[id] = true
		var mod plugin.Plugin
		switch id {
		case "builtin:clock":
			mod = builtin.NewClock()
		case "builtin:placeholder":
			mod = builtin.NewPlaceholder("")
		case "builtin:debug":
			mod = builtin.NewDebug("catalog", 640)
		}
		if desc, err := mod.Describe(); err == nil {
			entries = append(entries, CatalogEntry{ID: id, Kind: "builtin", Descriptor: desc})
		}
	}

	// Scan pluginsDir for exec plugin binaries.
	if s.pluginsDir != "" {
		dirEntries, err := os.ReadDir(s.pluginsDir)
		if err == nil {
			for _, de := range dirEntries {
				if !de.IsDir() {
					continue
				}
				name := de.Name()
				binPath := filepath.Join(s.pluginsDir, name, name)
				id := "exec:" + name
				if seen[id] {
					continue
				}

				desc, err := pluginhost.DescribePlugin(binPath)
				if err != nil {
					s.logger.Debug("catalog: describe failed", "plugin", name, "error", err)
					continue
				}
				seen[id] = true
				entries = append(entries, CatalogEntry{ID: id, Kind: "exec", Descriptor: desc})
			}
		}
	}

	return entries
}

func (s *Sampler) recordFirstSample(zoneID string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.firstSampleLogged[zoneID] {
		return
	}

	start, ok := s.zoneStartTimes[zoneID]
	if !ok {
		return
	}

	latency := time.Since(start)
	pluginSpec := s.pluginSpec[zoneID]
	s.firstSampleLogged[zoneID] = true
	if latency < 0 {
		latency = 0
	}

	s.logger.Info("zone first payload",
		"zone_id", zoneID,
		"latency_ms", latency.Milliseconds(),
		"plugin", pluginSpec)
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
func (s *Sampler) applyInitialZoneConfig(zoneID, pluginSpec string, mod plugin.Plugin) {
	if s.zoneCfg == nil {
		return
	}

	cfg := s.zoneCfg.Get(zoneID, pluginSpec)
	if len(cfg) == 0 {
		return
	}

	if err := mod.Configure(cfg); err != nil {
		s.logger.Warn("failed to apply initial zone config",
			"zone_id", zoneID,
			"plugin", pluginSpec,
			"error", err)
		return
	}

	s.logger.Info("applied initial zone config",
		"zone_id", zoneID,
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
