package zone

import (
	"fmt"
	"image"
	"time"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

// initializePage sets up zones and renderers for the current page.
// Preserves existing payloads so there's no Loading flash on page switch.
//
// payloadsMu is held for the duration so UpdatePayload (which checks m.zones)
// cannot observe a partially-rebuilt map while we swap it out.
func (m *Manager) initializePage() error {
	if m.currentPage >= len(m.config.Pages) {
		return fmt.Errorf("invalid page index: %d", m.currentPage)
	}

	page := &m.config.Pages[m.currentPage]

	m.payloadsMu.Lock()
	defer m.payloadsMu.Unlock()

	m.zones = make(map[string]*Zone)
	m.renderers = make(map[string]*Renderer)

	page.ComputeOffsets()

	for _, zoneConfig := range page.Zones {
		theme := m.config.Theme
		if zoneConfig.ThemeOverride != nil {
			theme = mergeTheme(theme, *zoneConfig.ThemeOverride)
		}

		renderer := NewRenderer(m.logger, theme, zoneConfig.Width, DisplayHeight, zoneConfig.Align)

		m.zones[zoneConfig.ID] = &Zone{
			Config:   zoneConfig,
			Renderer: renderer,
			Plugin:   zoneConfig.Plugin,
		}
		m.renderers[zoneConfig.ID] = renderer

		if _, exists := m.payloads[zoneConfig.ID]; !exists {
			m.payloads[zoneConfig.ID] = &plugin.Payload{
				Primary:   "—",
				Severity:  plugin.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		m.logger.Debug("zone initialized",
			"id", zoneConfig.ID,
			"width", zoneConfig.Width,
			"x", zoneConfig.X,
			"plugin", zoneConfig.Plugin)
	}

	m.compositor = NewCompositor(m.logger, m.config.Theme, page)

	m.lastFrameMu.Lock()
	m.frameDirty = true
	m.lastFrameMu.Unlock()

	return nil
}

// mergeTheme merges an override theme into a base theme.
func mergeTheme(base, override Theme) Theme {
	result := base

	if override.Bg != "" {
		result.Bg = override.Bg
	}
	if override.Fg != "" {
		result.Fg = override.Fg
	}
	if override.Muted != "" {
		result.Muted = override.Muted
	}
	if override.Accent != "" {
		result.Accent = override.Accent
	}
	if override.GutterPx > 0 {
		result.GutterPx = override.GutterPx
	}
	if override.FontSizePrimary > 0 {
		result.FontSizePrimary = override.FontSizePrimary
	}
	if override.FontSizeSecondary > 0 {
		result.FontSizeSecondary = override.FontSizeSecondary
	}

	return result
}

// GetConfig returns the current configuration pointer. The pointer is stable
// for the duration of the current config — callers must not mutate it.
func (m *Manager) GetConfig() *Config {
	m.configMu.RLock()
	defer m.configMu.RUnlock()
	return m.config
}

// GetCurrentPage returns the current page index.
func (m *Manager) GetCurrentPage() int {
	return m.currentPage
}

// NumPages returns the total number of pages.
func (m *Manager) NumPages() int {
	return len(m.config.Pages)
}

// GetPageInfos returns lightweight page + zone descriptors for the Flutter preview UI.
func (m *Manager) GetPageInfos() []PageInfo {
	pages := make([]PageInfo, len(m.config.Pages))
	for i, p := range m.config.Pages {
		zones := make([]ZoneInfo, len(p.Zones))
		for j, z := range p.Zones {
			zones[j] = ZoneInfo{ID: z.ID, Width: z.Width}
		}
		pages[i] = PageInfo{Name: p.Name, Zones: zones}
	}
	return pages
}

// GetZones returns all zones for the current page.
func (m *Manager) GetZones() map[string]*Zone {
	return m.zones
}

// SetOnPageChange sets a callback to be called when the page changes.
func (m *Manager) SetOnPageChange(callback func(pageIndex int) error) {
	m.onPageChange = callback
}

// SetOnZoneCycle sets a callback invoked when a tap action cycles a zone's plugin.
func (m *Manager) SetOnZoneCycle(callback func(zoneConfig ZoneConfig) error) {
	m.onZoneCycle = callback
}

// CycleZonePlugin advances the zone to its next plugin choice and notifies the
// sampler via the onZoneCycle callback.
func (m *Manager) CycleZonePlugin(zoneID string) error {
	var found *ZoneConfig
	for pi := range m.config.Pages {
		for zi := range m.config.Pages[pi].Zones {
			if m.config.Pages[pi].Zones[zi].ID == zoneID {
				found = &m.config.Pages[pi].Zones[zi]
				break
			}
		}
		if found != nil {
			break
		}
	}
	if found == nil {
		return fmt.Errorf("zone %q not found", zoneID)
	}
	if len(found.Choices) == 0 {
		return nil
	}

	m.choiceIndexMu.Lock()
	idx := (m.choiceIndex[zoneID] + 1) % len(found.Choices)
	m.choiceIndex[zoneID] = idx
	m.choiceIndexMu.Unlock()

	found.Plugin = found.Choices[idx]
	m.logger.Info("cycling zone plugin", "zone", zoneID, "plugin", found.Plugin, "choice", idx)

	if m.onZoneCycle != nil {
		return m.onZoneCycle(*found)
	}
	return nil
}

// SwitchPage switches to a different page with a slide-left transition.
func (m *Manager) SwitchPage(pageIndex int) error {
	return m.SwitchPageWithTransition(pageIndex, TransitionSlideLeft, 1)
}

// SwitchPageWithTransition switches to a different page with a specified transition.
func (m *Manager) SwitchPageWithTransition(pageIndex int, transitionType TransitionType, direction int) error {
	if pageIndex < 0 || pageIndex >= len(m.config.Pages) {
		return fmt.Errorf("invalid page index: %d (have %d pages)", pageIndex, len(m.config.Pages))
	}

	if pageIndex == m.currentPage {
		m.logger.Debug("already on target page", "page", pageIndex)
		return nil
	}

	m.liveSwipeMu.Lock()
	m.liveSwipeActive = false
	m.liveSwipeProgress = 0
	m.liveSwipeMu.Unlock()

	m.logger.Info("🔄 STARTING PAGE SWITCH",
		"from_page", m.currentPage,
		"from_name", m.config.Pages[m.currentPage].Name,
		"to_page", pageIndex,
		"to_name", m.config.Pages[pageIndex].Name,
		"transition", transitionType,
		"direction", direction)

	m.lastFrameMu.Lock()
	oldFrame := m.lastFrame
	m.lastFrameMu.Unlock()

	if oldFrame == nil {
		m.logger.Warn("⚠️ NO OLD FRAME for transition")
	}

	oldPage := m.currentPage
	m.currentPage = pageIndex

	if err := m.initializePage(); err != nil {
		m.currentPage = oldPage
		return fmt.Errorf("failed to initialize page: %w", err)
	}

	// Always render fresh after a page switch — the cache may have been built
	// before this page's renderers were initialised with the correct themes.
	// The cache is only used for the live swipe preview, not for the final frame.
	m.pageCacheMu.Lock()
	delete(m.pageCache, pageIndex)
	m.pageCacheMu.Unlock()

	var err error
	newFrame, err := m.renderImmediateFrameForCurrentPage()
	if err != nil {
		return fmt.Errorf("failed to render new frame: %w", err)
	}

	if oldFrame != nil && transitionType != TransitionNone {
		m.transitionMu.Lock()
		m.transition.Start(transitionType, oldFrame, newFrame, direction)
		m.transitionMu.Unlock()
		m.logger.Info("🎬 TRANSITION STARTED",
			"type", transitionType,
			"active", m.transition.Active,
			"duration_ms", m.transition.Duration.Milliseconds())
	} else {
		m.logger.Info("⏭️ NO TRANSITION (immediate switch)")
	}

	m.logger.Info("✅ PAGE SWITCH COMPLETE",
		"page", pageIndex,
		"name", m.config.Pages[pageIndex].Name)

	if m.onPageChange != nil {
		go func() {
			if err := m.onPageChange(pageIndex); err != nil {
				m.logger.Error("page change callback failed", "error", err)
			}
		}()
	}

	go m.preRenderAdjacentPages()

	m.transitionMu.RLock()
	postManualActive := m.transition.Active && m.transition.IsManual()
	postManualProgress := 0.0
	if postManualActive {
		postManualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("switch finalize state",
		"current_page", m.currentPage,
		"target_page", pageIndex,
		"transition_manual_active", postManualActive,
		"transition_manual_progress_pct", int(postManualProgress*100))

	return nil
}

// NextPage switches to the next page with a slide-left transition.
func (m *Manager) NextPage() error {
	nextPage := (m.currentPage + 1) % len(m.config.Pages)
	return m.SwitchPageWithTransition(nextPage, TransitionSlideLeft, 1)
}

// PrevPage switches to the previous page with a slide-right transition.
func (m *Manager) PrevPage() error {
	prevPage := m.currentPage - 1
	if prevPage < 0 {
		prevPage = len(m.config.Pages) - 1
	}
	return m.SwitchPageWithTransition(prevPage, TransitionSlideRight, -1)
}

// getCachedPageFrame returns a pre-rendered frame for the given page, or nil.
func (m *Manager) getCachedPageFrame(pageIndex int) *image.RGBA {
	m.pageCacheMu.RLock()
	defer m.pageCacheMu.RUnlock()
	return m.pageCache[pageIndex]
}

// preRenderAdjacentPages pre-renders the next and previous pages in the background.
func (m *Manager) preRenderAdjacentPages() {
	nextPage := (m.currentPage + 1) % len(m.config.Pages)
	m.preRenderPage(nextPage)

	prevPage := m.currentPage - 1
	if prevPage < 0 {
		prevPage = len(m.config.Pages) - 1
	}
	m.preRenderPage(prevPage)
}

// preRenderPage renders a specific page and caches the result.
func (m *Manager) preRenderPage(pageIndex int) {
	if pageIndex < 0 || pageIndex >= len(m.config.Pages) {
		return
	}
	if pageIndex == m.currentPage {
		return
	}

	start := time.Now()
	m.logger.Debug("pre-rendering page", "page", pageIndex, "name", m.config.Pages[pageIndex].Name)

	// Deep-copy the zone slice so ComputeOffsets (called below) doesn't race
	// with concurrent preRenderPage goroutines writing X on the shared slice.
	srcPage := m.config.Pages[pageIndex]
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
		theme := m.config.Theme
		if zoneConfig.ThemeOverride != nil {
			theme = mergeTheme(theme, *zoneConfig.ThemeOverride)
		}
		renderer := NewRenderer(m.logger, theme, zoneConfig.Width, DisplayHeight, zoneConfig.Align)

		payload, ok := m.payloads[zoneConfig.ID]
		if !ok {
			payload = &plugin.Payload{
				Primary:   "—",
				Severity:  plugin.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		img, err := renderer.Render(*payload)
		if err != nil {
			continue
		}
		zoneImages[zoneConfig.ID] = img
	}

	m.configMu.RLock()
	theme := m.config.Theme
	m.configMu.RUnlock()

	compositor := NewCompositor(m.logger, theme, &page)
	frame, err := compositor.Composite(nil, zoneImages, theme)
	if err != nil {
		m.logger.Error("failed to pre-render page", "page", pageIndex, "error", err)
		return
	}

	m.pageCacheMu.Lock()
	m.pageCache[pageIndex] = frame
	cacheSize := len(m.pageCache)
	m.pageCacheMu.Unlock()

	m.logger.Debug("page pre-rendered and cached",
		"page", pageIndex,
		"zones", len(page.Zones),
		"duration_ms", time.Since(start).Milliseconds(),
		"cache_entries", cacheSize)
}
