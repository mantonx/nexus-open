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

	if _, ok := m.zones[zoneID]; !ok {
		return fmt.Errorf("zone not found: %s", zoneID)
	}

	if err := payload.Validate(); err != nil {
		return fmt.Errorf("invalid payload: %w", err)
	}

	if payload.Timestamp.IsZero() {
		payload.Timestamp = time.Now()
	}

	m.payloads[zoneID] = &payload

	// Snapshot config pointer once under read-lock before walking pages.
	m.configMu.RLock()
	cfg := m.config
	m.configMu.RUnlock()

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
}

// RenderFrame renders the current frame (transition or live zones composited).
func (m *Manager) RenderFrame() (*image.RGBA, error) {
	m.configMu.RLock()
	theme := m.config.Theme
	m.configMu.RUnlock()

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

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	frame, err := m.renderZoneImages(theme)
	if err != nil {
		return nil, err
	}

	m.lastFrameMu.Lock()
	m.lastFrame = frame
	m.lastFrameMu.Unlock()

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

		zoneImages[zoneID] = img
	}

	frame, err := m.compositor.Composite(zoneImages, theme)
	if err != nil {
		return nil, fmt.Errorf("failed to composite frame: %w", err)
	}
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
		// Use live renderer if available (has correct ThemeOverride already applied).
		var renderer *Renderer
		if r, ok := m.renderers[zoneConfig.ID]; ok {
			renderer = r
		} else {
			theme := cfg.Theme
			if zoneConfig.ThemeOverride != nil {
				theme = mergeTheme(theme, *zoneConfig.ThemeOverride)
			}
			renderer = NewRenderer(m.logger, theme, zoneConfig.Width, DisplayHeight, zoneConfig.Align)
		}
		payload, ok := m.payloads[zoneConfig.ID]
		if !ok {
			payload = &plugin.Payload{Primary: "—", Severity: plugin.SeverityOK, Timestamp: time.Now()}
		}
		if img, err := renderer.Render(*payload); err == nil {
			zoneImages[zoneConfig.ID] = img
		}
	}

	compositor := NewCompositor(m.logger, cfg.Theme, &page)
	return compositor.Composite(zoneImages, cfg.Theme)
}
