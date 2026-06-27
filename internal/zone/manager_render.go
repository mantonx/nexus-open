package zone

import (
	"fmt"
	"image"
	"time"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

// UpdatePayload updates the rendered data for a zone and invalidates the page cache.
func (m *Manager) UpdatePayload(zoneID string, payload plugin.Payload) error {
	m.payloadsMu.Lock()
	defer m.payloadsMu.Unlock()

	// Accept payloads for any zone that exists in any page — not just the
	// current page's m.zones — because all plugins run simultaneously.
	m.configMu.RLock()
	cfg := m.config
	m.configMu.RUnlock()
	knownZone := false
	for _, page := range cfg.Pages {
		for _, z := range page.Zones {
			if z.ID == zoneID {
				knownZone = true
				break
			}
		}
		if knownZone {
			break
		}
	}
	if !knownZone {
		return fmt.Errorf("zone not found: %s", zoneID)
	}

	if err := payload.Validate(); err != nil {
		return fmt.Errorf("invalid payload: %w", err)
	}

	if payload.Timestamp.IsZero() {
		payload.Timestamp = time.Now()
	}

	m.payloads[zoneID] = &payload

	m.lastFrameMu.Lock()
	m.frameDirty = true
	m.lastFrameMu.Unlock()

	// Invalidate cached frames for every page that contains this zone.
	m.pageCacheMu.Lock()
	invalidated := make([]int, 0, 2)
	for pageIndex, page := range cfg.Pages {
		for _, zoneCfg := range page.Zones {
			if zoneCfg.ID == zoneID {
				if _, ok := m.pageCache[pageIndex]; ok {
					delete(m.pageCache, pageIndex)
					invalidated = append(invalidated, pageIndex)
				}
				break
			}
		}
	}
	m.pageCacheMu.Unlock()
	m.logger.Debug("page cache invalidated", "zone_id", zoneID, "pages", invalidated)

	go m.preRenderAdjacentPages()

	m.logger.Debug("payload updated",
		"zone_id", zoneID,
		"primary", payload.Primary,
		"severity", payload.Severity)

	return nil
}

// UpdateTheme applies a new theme to all subsequent rendered frames.
// Per-zone ThemeOverrides (accent colour, font size, etc.) are re-merged on top
// of the incoming global theme so they are never stomped by a settings save.
func (m *Manager) UpdateTheme(theme Theme) {
	m.configMu.Lock()
	m.config.Theme = theme
	for id, r := range m.renderers {
		zoneTheme := theme
		if zone, ok := m.zones[id]; ok && zone.Config.ThemeOverride != nil {
			zoneTheme = mergeTheme(theme, *zone.Config.ThemeOverride)
		}
		r.UpdateTheme(zoneTheme)
	}
	m.configMu.Unlock()

	// Invalidate the entire page cache — stale frames would show the old theme.
	m.pageCacheMu.Lock()
	m.pageCache = make(map[int]*image.RGBA)
	m.pageCacheMu.Unlock()

	m.lastFrameMu.Lock()
	m.frameDirty = true
	m.lastFrameMu.Unlock()
}

// RenderFrame renders the current frame (transition or live zones composited).
// When no payload, theme, or page has changed since the last render, it returns
// the cached lastFrame directly — skipping allocation and the full compositor
// pipeline. This keeps idle CPU and GC pressure near zero at 30fps.
func (m *Manager) RenderFrame() (*image.RGBA, error) {
	m.configMu.RLock()
	theme := m.config.Theme
	m.configMu.RUnlock()

	// Detail overlay takes priority — either animating in/out or fully shown.
	m.detailMu.Lock()
	if m.detailTransition.Active && !m.detailTransition.IsComplete() {
		frame := m.detailTransition.Render()
		m.detailMu.Unlock()
		m.logger.Debug("rendering detail transition")
		m.compositeRipple(frame)
		return frame, nil
	}
	if m.detailActive {
		frame := m.detailFrame
		m.detailMu.Unlock()
		m.logger.Debug("rendering detail overlay")
		return frame, nil
	}
	m.detailMu.Unlock()

	m.transitionMu.RLock()
	if m.transition.Active && !m.transition.IsComplete() {
		frame := m.transition.Render()
		progress := m.transition.GetProgress()
		m.transitionMu.RUnlock()
		m.logger.Debug("rendering transition", "progress", int(progress*100))
		return frame, nil
	}
	if m.transition.Active && m.transition.IsComplete() {
		m.logger.Debug("transition complete")
	}
	m.transitionMu.RUnlock()

	// Fast path: nothing changed since last render — return cached frame.
	// Skip cache when a ripple or marquee animation is active.
	m.rippleMu.Lock()
	rippleActive := m.ripple.active
	m.rippleMu.Unlock()

	m.configMu.RLock()
	var marqueeActive bool
	for _, z := range m.zones {
		if z.Renderer.IsAnimating() {
			marqueeActive = true
			break
		}
	}
	m.configMu.RUnlock()

	m.lastFrameMu.Lock()
	if !m.frameDirty && !rippleActive && !marqueeActive && m.lastFrame != nil {
		cached := m.lastFrame
		m.lastFrameMu.Unlock()
		return cached, nil
	}
	m.lastFrameMu.Unlock()

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	frame, err := m.renderZoneImages(theme)
	if err != nil {
		return nil, err
	}

	// Composite tap ripple on top of the finished frame; keep dirty while active.
	if m.compositeRipple(frame) {
		m.lastFrameMu.Lock()
		m.lastFrame = frame
		// Leave frameDirty true — render loop will re-composite next tick.
		m.lastFrameMu.Unlock()
	} else {
		m.lastFrameMu.Lock()
		m.lastFrame = frame
		m.frameDirty = false
		m.lastFrameMu.Unlock()
	}

	return frame, nil
}

// GetLastFrame returns a copy of the most recently rendered frame, or nil.
func (m *Manager) GetLastFrame() *image.RGBA {
	m.lastFrameMu.Lock()
	defer m.lastFrameMu.Unlock()
	if m.lastFrame == nil {
		return nil
	}
	cp := *m.lastFrame
	cp.Pix = make([]byte, len(m.lastFrame.Pix))
	copy(cp.Pix, m.lastFrame.Pix)
	return &cp
}

// IsTransitioning reports whether a page transition is currently in progress.
// Used by the render loop to decide whether to broadcast at full rate (30fps).
func (m *Manager) IsTransitioning() bool {
	m.transitionMu.RLock()
	defer m.transitionMu.RUnlock()
	return m.transition.Active && !m.transition.IsComplete()
}

// renderImmediateFrameForCurrentPage renders the current page ignoring transition state.
// Used for pre-rendering and when a fresh frame is needed mid-transition.
func (m *Manager) renderImmediateFrameForCurrentPage() (*image.RGBA, error) {
	start := time.Now()
	success := false
	defer func() {
		m.logger.Debug("render immediate frame",
			"page", m.currentPage,
			"duration_ms", time.Since(start).Milliseconds(),
			"success", success)
	}()

	m.configMu.RLock()
	theme := m.config.Theme
	m.configMu.RUnlock()

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	frame, err := m.renderZoneImages(theme)
	if err != nil {
		return nil, err
	}

	success = true
	return frame, nil
}

// renderZoneImages renders all current-page zones into a composited frame.
// It writes into the pre-allocated back buffer (frameBufs[frameBufIdx]) and
// flips the index so the next call writes into the other buffer.
// Callers must hold m.payloadsMu.RLock before calling.
func (m *Manager) renderZoneImages(theme Theme) (*image.RGBA, error) {
	zoneImages := make(map[string]*image.RGBA, len(m.zones))

	for zoneID, zone := range m.zones {
		payload, ok := m.payloads[zoneID]
		if !ok || payload == nil {
			payload = &plugin.Payload{
				Primary:   "—",
				Secondary: "Loading...",
				Severity:  plugin.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		if payload.IsExpired() {
			m.logger.Warn("payload expired", "zone_id", zoneID, "age", time.Since(payload.Timestamp))
			payload = &plugin.Payload{
				Primary:   "—",
				Secondary: "Stale",
				Severity:  plugin.SeverityWarn,
				Timestamp: time.Now(),
			}
		}

		// Re-use the cached image if the payload hasn't changed and the zone
		// isn't actively animating (marquee scrolling). This prevents adjacent
		// zones (e.g. the clock) from being re-rendered every tick just because
		// a different zone's marquee is active.
		if zone.cachedImg != nil &&
			zone.cachedPayload == payload.Timestamp &&
			!zone.Renderer.IsAnimating() {
			zoneImages[zoneID] = zone.cachedImg
			continue
		}

		img, err := zone.Renderer.Render(*payload)
		if err != nil {
			m.logger.Error("failed to render zone", "zone_id", zoneID, "error", err)
			img = RenderPlaceholder(
				zone.Config.Width,
				DisplayHeight,
				"Error",
				theme.GetBgColor(),
				theme.GetFgColor(),
			)
		}

		zone.cachedImg = img
		zone.cachedPayload = payload.Timestamp
		zoneImages[zoneID] = img
	}

	dst := m.frameBufs[m.frameBufIdx]
	frame, err := m.compositor.Composite(dst, zoneImages, theme)
	if err != nil {
		return nil, fmt.Errorf("failed to composite frame: %w", err)
	}
	m.frameBufIdx ^= 1
	return frame, nil
}

// renderPageFrame renders any page by index using current payloads.
// Used to warm the cache for pages that aren't currently displayed.
func (m *Manager) renderPageFrame(pageIndex int) (*image.RGBA, error) {
	m.configMu.RLock()
	cfg := m.config
	m.configMu.RUnlock()

	if pageIndex < 0 || pageIndex >= len(cfg.Pages) {
		return nil, fmt.Errorf("invalid page index: %d", pageIndex)
	}

	srcPage := cfg.Pages[pageIndex]
	page := srcPage
	page.Zones = make([]ZoneConfig, len(srcPage.Zones))
	copy(page.Zones, srcPage.Zones)
	page.ComputeOffsets()

	zoneImages := make(map[string]*image.RGBA)

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	for _, zoneConfig := range page.Zones {
		// Always build a fresh renderer — reusing the live renderer would race
		// with the main render loop since freetype's GlyphBuf is not thread-safe.
		theme := cfg.Theme
		if zoneConfig.ThemeOverride != nil {
			theme = mergeTheme(theme, *zoneConfig.ThemeOverride)
		}
		renderer := NewRenderer(m.logger, theme, zoneConfig.Width, DisplayHeight, zoneConfig.Align)
		payload, ok := m.payloads[zoneConfig.ID]
		if !ok {
			payload = &plugin.Payload{Primary: "—", Severity: plugin.SeverityOK, Timestamp: time.Now()}
		}
		if img, err := renderer.Render(*payload); err == nil {
			zoneImages[zoneConfig.ID] = img
		}
	}

	compositor := NewCompositor(m.logger, cfg.Theme, &page)
	return compositor.Composite(nil, zoneImages, cfg.Theme)
}
