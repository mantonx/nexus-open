package zone

import (
	"fmt"
	"image"
	"time"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

const (
	detailTimeout  = 10 * time.Second
	detailDebounce = 600 * time.Millisecond
)

// SetDetailStateCallback registers a function called whenever the detail overlay
// is shown or hidden. Safe to call before Start.
func (m *Manager) SetDetailStateCallback(fn func(active bool)) {
	m.onDetailState = fn
}

// ShowDetail renders payload into a detail overlay and starts the slide-up transition.
func (m *Manager) ShowDetail(payload plugin.DetailPayload) {
	m.configMu.RLock()
	theme := m.config.Theme
	m.configMu.RUnlock()

	frame := RenderDetailFrame(m.logger, payload, theme)

	// Snapshot lastFrame before acquiring detailMu to avoid lock-order deadlock
	// with RenderFrame (which holds lastFrameMu then acquires detailMu).
	m.lastFrameMu.Lock()
	oldFrame := m.lastFrame
	m.lastFrameMu.Unlock()
	if oldFrame == nil {
		oldFrame = image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	}

	m.detailMu.Lock()
	defer m.detailMu.Unlock()

	m.detailFrame = frame
	m.detailActive = true
	m.detailZoneID = payload.ZoneID
	m.detailShownAt = time.Now()
	m.detailTransition.Duration = 200 * time.Millisecond
	m.detailTransition.Start(TransitionSlideUp, oldFrame, frame, 1)

	// Auto-dismiss after timeout.
	if m.detailTimer != nil {
		m.detailTimer.Stop()
	}
	m.detailTimer = time.AfterFunc(detailTimeout, func() {
		m.ClearDetail()
	})

	if m.onDetailState != nil {
		go m.onDetailState(true)
	}
}

// updateDetailFrame re-renders payload into detailFrame without triggering
// a slide transition. Used by the live-refresh loop.
func (m *Manager) updateDetailFrame(payload plugin.DetailPayload) {
	m.configMu.RLock()
	theme := m.config.Theme
	m.configMu.RUnlock()

	frame := RenderDetailFrame(m.logger, payload, theme)

	m.detailMu.Lock()
	defer m.detailMu.Unlock()
	if m.detailActive {
		m.detailFrame = frame
	}
}

// StartDetailRefresh begins polling tapper every interval while the detail
// overlay is active, updating the frame in place. Call immediately after
// ShowDetail. A previous refresh goroutine (if any) is cancelled first.
func (m *Manager) StartDetailRefresh(zoneID string, tapper plugin.Tapper, interval time.Duration) {
	m.detailMu.Lock()
	if m.detailRefreshStop != nil {
		close(m.detailRefreshStop)
	}
	stop := make(chan struct{})
	m.detailRefreshStop = stop
	m.detailMu.Unlock()

	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()
		for {
			select {
			case <-stop:
				return
			case <-ticker.C:
				m.detailMu.Lock()
				active := m.detailActive
				m.detailMu.Unlock()
				if !active {
					return
				}
				detail, err := tapper.OnTap()
				if err != nil {
					continue
				}
				detail.ZoneID = zoneID
				m.updateDetailFrame(detail)
			}
		}
	}()
}

// ClearDetail dismisses the detail overlay with a slide-down transition.
func (m *Manager) ClearDetail() {
	m.waitForRipple()

	// Snapshot lastFrame before acquiring detailMu — same lock-order rule as ShowDetail.
	m.lastFrameMu.Lock()
	pageFrame := m.lastFrame
	m.lastFrameMu.Unlock()
	if pageFrame == nil {
		pageFrame = image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	}

	m.detailMu.Lock()
	defer m.detailMu.Unlock()

	if !m.detailActive {
		return
	}

	// Ignore dismiss requests within the debounce window — prevents the finger
	// lift from the opening tap from immediately closing the overlay.
	if time.Since(m.detailShownAt) < detailDebounce {
		return
	}

	if m.detailTimer != nil {
		m.detailTimer.Stop()
		m.detailTimer = nil
	}

	m.detailTransition.Duration = 180 * time.Millisecond
	m.detailTransition.Start(TransitionSlideDown, m.detailFrame, pageFrame, -1)

	// Mark inactive — RenderFrame will still render the slide-down transition.
	m.detailActive = false
	m.detailZoneID = ""

	if m.detailRefreshStop != nil {
		close(m.detailRefreshStop)
		m.detailRefreshStop = nil
	}

	if m.onDetailState != nil {
		go m.onDetailState(false)
	}
}

// HandleDetailTap routes a tap within the active detail overlay to the plugin.
// If the plugin implements DetailTapper, it calls OnDetailTap and dismisses only
// if the plugin returns false. If the plugin does not implement DetailTapper,
// the detail is dismissed unconditionally.
func (m *Manager) HandleDetailTap(x, y int) {
	m.detailMu.Lock()
	zoneID := m.detailZoneID
	active := m.detailActive
	m.detailMu.Unlock()

	if !active || zoneID == "" || m.pluginLookup == nil {
		m.ClearDetail()
		return
	}

	p, ok := m.pluginLookup.GetPlugin(zoneID)
	if !ok {
		m.ClearDetail()
		return
	}

	dt, ok := p.(plugin.DetailTapper)
	if !ok {
		m.ClearDetail()
		return
	}

	keep, err := dt.OnDetailTap(x, y)
	if err != nil {
		m.logger.Error("OnDetailTap error", "zone_id", zoneID, "error", err)
	}
	if !keep {
		m.ClearDetail()
	}
}

// IsShowingDetail reports whether the detail overlay is active or animating IN.
// Returns false during the slide-down dismiss transition so taps pass through.
// GetDetailFrame returns the most recently rendered detail frame, or nil if none.
// The caller receives a copy safe to read without holding any lock.
func (m *Manager) GetDetailFrame() *image.RGBA {
	m.detailMu.Lock()
	defer m.detailMu.Unlock()
	if m.detailFrame == nil {
		return nil
	}
	// Shallow copy: Pix slice is the only mutable field and we copy it.
	out := *m.detailFrame
	out.Pix = make([]uint8, len(m.detailFrame.Pix))
	copy(out.Pix, m.detailFrame.Pix)
	return &out
}

func (m *Manager) IsShowingDetail() bool {
	m.detailMu.Lock()
	defer m.detailMu.Unlock()
	if m.detailActive {
		return true
	}
	// Only block taps during slide-in (TransitionSlideUp), not slide-out.
	return m.detailTransition.Active &&
		m.detailTransition.Type == TransitionSlideUp &&
		!m.detailTransition.IsComplete()
}

// SetPluginLookup wires in the sampler so HandleZoneTap can resolve plugins.
func (m *Manager) SetPluginLookup(pl PluginLookup) {
	m.pluginLookup = pl
}

// HandleZoneTap looks up the plugin for zoneID, calls OnTap if it implements
// plugin.Tapper, and shows the result as a detail overlay. Safe to call from
// any goroutine.
//
// Detail payloads contain pre-rendered pixel frames (RawFrame) with ephemeral
// content baked in (playback position, live temperatures, etc.). They must
// always be fetched fresh — caching them produces stale visuals on re-tap.
// Plugins that need to avoid repeated expensive work (network calls, geocoding)
// maintain their own in-process cache and return quickly from OnTap.
func (m *Manager) HandleZoneTap(zoneID string) error {
	if m.pluginLookup == nil {
		return fmt.Errorf("HandleZoneTap: plugin lookup not set")
	}
	p, ok := m.pluginLookup.GetPlugin(zoneID)
	if !ok {
		return fmt.Errorf("HandleZoneTap: no plugin loaded for zone %q", zoneID)
	}
	tapper, ok := p.(plugin.Tapper)
	if !ok {
		return fmt.Errorf("HandleZoneTap: plugin for zone %q does not implement Tapper", zoneID)
	}

	// Respect the plugin's Expandable flag: if the last payload said the zone
	// is not expandable (e.g. music plugin with nothing playing), silently skip.
	m.payloadsMu.RLock()
	lastPayload := m.payloads[zoneID]
	m.payloadsMu.RUnlock()
	if lastPayload != nil && !lastPayload.Expandable {
		return nil
	}

	m.waitForRipple()

	detail, err := tapper.OnTap()
	if err == plugin.ErrNotTapper {
		return fmt.Errorf("HandleZoneTap: plugin for zone %q returned ErrNotTapper", zoneID)
	}
	if err != nil {
		return fmt.Errorf("HandleZoneTap: OnTap error for zone %q: %w", zoneID, err)
	}
	detail.ZoneID = zoneID
	m.ShowDetail(detail)
	m.StartDetailRefresh(zoneID, tapper, time.Second)
	return nil
}
