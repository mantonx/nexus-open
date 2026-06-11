package zone

import (
	"context"
	"fmt"
	"time"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

const (
	crashBackoffInit = 1 * time.Second
	crashBackoffMax  = 30 * time.Second
)

// handlePluginCrash evicts the dead subprocess and relaunches it after a
// backoff delay. Returns the new plugin.Plugin on success, or nil if the
// context was cancelled before the plugin could be restarted.
func (s *Sampler) handlePluginCrash(ctx context.Context, zoneID string, backoff *time.Duration, interval time.Duration) plugin.Plugin {
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

	modPath, err := s.resolvePluginPath(spec)
	if err != nil {
		s.logger.Error("plugin restart failed", "zone_id", zoneID, "error", err)
		s.markZoneLaunchFailed(zoneID, err)
		return nil
	}
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
	s.sampleOnce(ctx, zoneID, mod, interval) //nolint:errcheck
	return mod
}

// sampleTimeout returns a per-zone deadline: half the refresh interval, clamped
// to [5s, 30s]. Fast zones keep the original 5s ceiling; slow zones (e.g. the
// 5-minute weather zone) get enough headroom for real network calls.
func sampleTimeout(interval time.Duration) time.Duration {
	t := interval / 2
	if t < 5*time.Second {
		t = 5 * time.Second
	}
	if t > 30*time.Second {
		t = 30 * time.Second
	}
	return t
}

func (s *Sampler) sampleOnce(parentCtx context.Context, zoneID string, mod plugin.Plugin, interval time.Duration) (dead bool) {
	ctx, cancel := context.WithTimeout(parentCtx, sampleTimeout(interval))
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
