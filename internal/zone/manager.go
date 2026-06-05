package zone

import (
	"context"
	"fmt"
	"image"
	"log/slog"
	"math"
	"os"
	"sync"
	"time"

	"gopkg.in/yaml.v3"

	"github.com/mantonx/nexus-next/pkg/module"
)

const liveSwipePreviewThreshold = 0.24

// Manager manages zones, their renderers, and lifecycle
type Manager struct {
	logger      *slog.Logger
	config      *Config
	configPath  string
	currentPage int
	themeMu     sync.RWMutex // guards config.Theme live updates

	// Zone state
	zones      map[string]*Zone
	renderers  map[string]*Renderer
	payloads   map[string]*module.Payload
	payloadsMu sync.RWMutex

	// Compositor for current page
	compositor *Compositor

	// Transition state
	transition   *TransitionState
	transitionMu sync.RWMutex
	lastFrame    *image.RGBA
	lastFrameMu  sync.Mutex

	// Live swipe tracking
	liveSwipeActive        bool
	liveSwipeProgress      float32
	liveSwipeLeft          bool
	liveSwipeMu            sync.RWMutex
	lastSwipeFinalize      time.Time
	lastSwipeDirLeft       bool
	liveSwipeTargetFrame   *image.RGBA
	liveSwipePreviewActive bool

	// Pre-rendered page cache for instant transitions
	pageCache   map[int]*image.RGBA
	pageCacheMu sync.RWMutex

	// Page change callback
	onPageChange func(pageIndex int) error

	// Lifecycle
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// Zone represents a single zone instance
type Zone struct {
	Config   ZoneConfig
	Renderer *Renderer
	Module   string // Module identifier (for now, just string)
}

// NewManager creates a new zone manager
func NewManager(ctx context.Context, logger *slog.Logger, configPath string) (*Manager, error) {
	// Load configuration
	config, err := LoadConfig(configPath)
	if err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
	}

	// Validate configuration
	if err := config.Validate(); err != nil {
		return nil, fmt.Errorf("invalid config: %w", err)
	}

	ctx, cancel := context.WithCancel(ctx)

	m := &Manager{
		logger:      logger,
		config:      config,
		configPath:  configPath,
		currentPage: 0,
		zones:       make(map[string]*Zone),
		renderers:   make(map[string]*Renderer),
		payloads:    make(map[string]*module.Payload),
		transition:  NewTransitionState(),
		pageCache:   make(map[int]*image.RGBA),
		ctx:         ctx,
		cancel:      cancel,
	}

	// Initialize zones for current page
	if err := m.initializePage(); err != nil {
		return nil, fmt.Errorf("failed to initialize page: %w", err)
	}

	logger.Info("zone manager initialized",
		"pages", len(config.Pages),
		"current_page", config.Pages[m.currentPage].Name,
		"zones", len(m.zones))

	// Pre-render adjacent pages on startup for instant first swipe
	go m.preRenderAdjacentPages()

	return m, nil
}

// LoadConfig loads a zone configuration from YAML
func LoadConfig(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("failed to read config file: %w", err)
	}

	var config Config
	if err := yaml.Unmarshal(data, &config); err != nil {
		return nil, fmt.Errorf("failed to parse config: %w", err)
	}

	// Set default theme if not specified
	if config.Theme.Bg == "" {
		config.Theme = DefaultTheme()
	}

	return &config, nil
}

// initializePage initializes zones for the current page
func (m *Manager) initializePage() error {
	if m.currentPage >= len(m.config.Pages) {
		return fmt.Errorf("invalid page index: %d", m.currentPage)
	}

	page := &m.config.Pages[m.currentPage]

	// Clear existing zones
	m.zones = make(map[string]*Zone)
	m.renderers = make(map[string]*Renderer)

	// Compute zone offsets
	page.ComputeOffsets()

	// Create zones and renderers
	for _, zoneConfig := range page.Zones {
		theme := m.config.Theme
		if zoneConfig.ThemeOverride != nil {
			// Merge theme override
			theme = mergeTheme(theme, *zoneConfig.ThemeOverride)
		}

		renderer := NewRenderer(
			m.logger,
			theme,
			zoneConfig.Width,
			DisplayHeight,
			zoneConfig.Align,
		)

		zone := &Zone{
			Config:   zoneConfig,
			Renderer: renderer,
			Module:   zoneConfig.Module,
		}

		m.zones[zoneConfig.ID] = zone
		m.renderers[zoneConfig.ID] = renderer

		// Initialize with placeholder payload
		m.payloads[zoneConfig.ID] = &module.Payload{
			Primary:   "—",
			Secondary: "Loading...",
			Severity:  module.SeverityOK,
			Timestamp: time.Now(),
		}

		m.logger.Debug("zone initialized",
			"id", zoneConfig.ID,
			"width", zoneConfig.Width,
			"x", zoneConfig.X,
			"module", zoneConfig.Module)
	}

	// Create compositor for this page
	m.compositor = NewCompositor(m.logger, m.config.Theme, page)

	return nil
}

// mergeTheme merges an override theme into a base theme
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

// UpdatePayload updates the payload for a zone
func (m *Manager) UpdatePayload(zoneID string, payload module.Payload) error {
	m.payloadsMu.Lock()
	defer m.payloadsMu.Unlock()

	if _, ok := m.zones[zoneID]; !ok {
		return fmt.Errorf("zone not found: %s", zoneID)
	}

	// Validate payload
	if err := payload.Validate(); err != nil {
		return fmt.Errorf("invalid payload: %w", err)
	}

	// Set timestamp if not set
	if payload.Timestamp.IsZero() {
		payload.Timestamp = time.Now()
	}

	m.payloads[zoneID] = &payload

	// Invalidate pre-rendered cache since data has changed
	m.pageCacheMu.Lock()
	invalidated := make([]int, 0, 2)
	for pageIndex, page := range m.config.Pages {
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

	// Trigger background re-rendering of adjacent pages
	go m.preRenderAdjacentPages()

	m.logger.Debug("payload updated",
		"zone_id", zoneID,
		"primary", payload.Primary,
		"severity", payload.Severity)

	return nil
}

// UpdateTheme applies a new theme to all subsequent rendered frames.
// Safe to call from any goroutine; the change takes effect on the next frame.
func (m *Manager) UpdateTheme(theme Theme) {
	m.themeMu.Lock()
	m.config.Theme = theme
	m.themeMu.Unlock()
}

// RenderFrame renders the current frame (all zones composited)
func (m *Manager) RenderFrame() (*image.RGBA, error) {
	// Snapshot the theme once so all render calls in this frame are consistent
	// and we avoid holding themeMu across other locks.
	m.themeMu.RLock()
	theme := m.config.Theme
	m.themeMu.RUnlock()

	// Check if transition is active
	m.transitionMu.RLock()
	if m.transition.Active && !m.transition.IsComplete() {
		frame := m.transition.Render()
		progress := m.transition.GetProgress()
		m.transitionMu.RUnlock()
		m.logger.Debug("🎬 RENDERING TRANSITION", "progress", int(progress*100))
		return frame, nil
	}
	if m.transition.Active && m.transition.IsComplete() {
		m.logger.Debug("🏁 TRANSITION COMPLETE - deactivating")
	}
	m.transitionMu.RUnlock()

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	// Render each zone
	zoneImages := make(map[string]*image.RGBA)

	for zoneID, zone := range m.zones {
		payload, ok := m.payloads[zoneID]
		if !ok {
			// Use placeholder
			payload = &module.Payload{
				Primary:   "—",
				Severity:  module.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		// Check if payload is expired
		if payload.IsExpired() {
			m.logger.Warn("payload expired", "zone_id", zoneID, "age", time.Since(payload.Timestamp))
			payload = &module.Payload{
				Primary:   "—",
				Secondary: "Stale",
				Severity:  module.SeverityWarn,
				Timestamp: time.Now(),
			}
		}

		img, err := zone.Renderer.Render(*payload)
		if err != nil {
			m.logger.Error("failed to render zone", "zone_id", zoneID, "error", err)
			// Use error placeholder
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

	// Composite zones into display
	frame, err := m.compositor.Composite(zoneImages)
	if err != nil {
		return nil, fmt.Errorf("failed to composite frame: %w", err)
	}

	// Store frame for potential transitions
	m.lastFrameMu.Lock()
	m.lastFrame = frame
	m.lastFrameMu.Unlock()

	return frame, nil
}

// GetLastFrame returns a copy of the most recently rendered frame, or nil if no frame
// has been rendered yet. The copy is safe to read after the call returns.
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

// renderImmediateFrameForCurrentPage renders the current page ignoring transition state.
// It takes its own theme snapshot so concurrent UpdateTheme calls are safe.
// Used for pre-rendering or when we need an up-to-date frame while a transition is in progress.
func (m *Manager) renderImmediateFrameForCurrentPage() (*image.RGBA, error) {
	start := time.Now()
	success := false
	defer func() {
		m.logger.Debug("render immediate frame",
			"page", m.currentPage,
			"duration_ms", time.Since(start).Milliseconds(),
			"success", success)
	}()

	m.themeMu.RLock()
	theme := m.config.Theme
	m.themeMu.RUnlock()

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	zoneImages := make(map[string]*image.RGBA)

	for zoneID, zone := range m.zones {
		payload, ok := m.payloads[zoneID]
		if !ok || payload == nil {
			payload = &module.Payload{
				Primary:   "—",
				Secondary: "Loading...",
				Severity:  module.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		if payload.IsExpired() {
			m.logger.Warn("payload expired", "zone_id", zoneID, "age", time.Since(payload.Timestamp))
			payload = &module.Payload{
				Primary:   "—",
				Secondary: "Stale",
				Severity:  module.SeverityWarn,
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

	frame, err := m.compositor.Composite(zoneImages)
	if err != nil {
		return nil, fmt.Errorf("failed to composite frame: %w", err)
	}

	success = true
	return frame, nil
}

func (m *Manager) getCachedPageFrame(pageIndex int) *image.RGBA {
	m.pageCacheMu.RLock()
	defer m.pageCacheMu.RUnlock()
	return m.pageCache[pageIndex]
}

// SwitchPage switches to a different page with optional transition
func (m *Manager) SwitchPage(pageIndex int) error {
	return m.SwitchPageWithTransition(pageIndex, TransitionSlideLeft, 1)
}

// SwitchPageWithTransition switches to a different page with a specified transition
func (m *Manager) SwitchPageWithTransition(pageIndex int, transitionType TransitionType, direction int) error {
	if pageIndex < 0 || pageIndex >= len(m.config.Pages) {
		return fmt.Errorf("invalid page index: %d (have %d pages)", pageIndex, len(m.config.Pages))
	}

	if pageIndex == m.currentPage {
		m.logger.Debug("already on target page", "page", pageIndex)
		return nil // Already on this page
	}

	// Clear live swipe state if we're completing a swipe
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

	// Capture current frame for transition
	m.lastFrameMu.Lock()
	oldFrame := m.lastFrame
	m.lastFrameMu.Unlock()

	if oldFrame == nil {
		m.logger.Warn("⚠️ NO OLD FRAME for transition")
	}

	// Store old page index
	oldPage := m.currentPage

	// Switch to new page
	m.currentPage = pageIndex

	if err := m.initializePage(); err != nil {
		m.currentPage = oldPage // Revert on error
		return fmt.Errorf("failed to initialize page: %w", err)
	}

	// Try to get pre-rendered frame from cache first
	m.pageCacheMu.RLock()
	newFrame, cached := m.pageCache[pageIndex]
	m.pageCacheMu.RUnlock()

	if !cached {
		// Not in cache, render it now (this is the slow path we want to avoid)
		m.logger.Debug("page not pre-rendered, rendering now", "page", pageIndex)
		var err error
		newFrame, err = m.renderImmediateFrameForCurrentPage()
		if err != nil {
			return fmt.Errorf("failed to render new frame: %w", err)
		}
	} else {
		m.logger.Debug("using pre-rendered page from cache", "page", pageIndex)
	}

	// Start transition if we have both frames
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

	// Notify page change callback if set (run asynchronously to not block transition)
	if m.onPageChange != nil {
		go func() {
			if err := m.onPageChange(pageIndex); err != nil {
				m.logger.Error("page change callback failed", "error", err)
			}
		}()
	}

	// Pre-render adjacent pages in background for instant next transition
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

// preRenderAdjacentPages pre-renders the next and previous pages in the background
func (m *Manager) preRenderAdjacentPages() {
	// Pre-render next page
	nextPage := (m.currentPage + 1) % len(m.config.Pages)
	m.preRenderPage(nextPage)

	// Pre-render previous page
	prevPage := m.currentPage - 1
	if prevPage < 0 {
		prevPage = len(m.config.Pages) - 1
	}
	m.preRenderPage(prevPage)
}

// preRenderPage renders a specific page and caches it
func (m *Manager) preRenderPage(pageIndex int) {
	if pageIndex < 0 || pageIndex >= len(m.config.Pages) {
		return
	}

	// Don't re-render current page
	if pageIndex == m.currentPage {
		return
	}

	start := time.Now()
	m.logger.Debug("pre-rendering page", "page", pageIndex, "name", m.config.Pages[pageIndex].Name)

	// Temporarily render this page's zones
	page := m.config.Pages[pageIndex]
	zoneImages := make(map[string]*image.RGBA)

	m.payloadsMu.RLock()
	defer m.payloadsMu.RUnlock()

	for _, zoneConfig := range page.Zones {
		// Create temporary renderer for this zone
		theme := m.config.Theme
		if zoneConfig.ThemeOverride != nil {
			theme = mergeTheme(theme, *zoneConfig.ThemeOverride)
		}

		renderer := NewRenderer(
			m.logger,
			theme,
			zoneConfig.Width,
			DisplayHeight,
			zoneConfig.Align,
		)

		// Get payload if available
		payload, ok := m.payloads[zoneConfig.ID]
		if !ok {
			payload = &module.Payload{
				Primary:   "—",
				Severity:  module.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		img, err := renderer.Render(*payload)
		if err != nil {
			continue
		}
		zoneImages[zoneConfig.ID] = img
	}

	// Create temporary compositor for this page layout
	compositor := NewCompositor(m.logger, m.config.Theme, &page)
	frame, err := compositor.Composite(zoneImages)
	if err != nil {
		m.logger.Error("failed to pre-render page", "page", pageIndex, "error", err)
		return
	}

	// Cache the rendered frame
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

// NextPage switches to the next page (wraps around) with slide left transition
func (m *Manager) NextPage() error {
	nextPage := (m.currentPage + 1) % len(m.config.Pages)
	return m.SwitchPageWithTransition(nextPage, TransitionSlideLeft, 1)
}

// PrevPage switches to the previous page (wraps around) with slide right transition
func (m *Manager) PrevPage() error {
	prevPage := m.currentPage - 1
	if prevPage < 0 {
		prevPage = len(m.config.Pages) - 1
	}
	return m.SwitchPageWithTransition(prevPage, TransitionSlideRight, -1)
}

// Reload reloads the configuration from disk
func (m *Manager) Reload() error {
	config, err := LoadConfig(m.configPath)
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	if err := config.Validate(); err != nil {
		return fmt.Errorf("invalid config: %w", err)
	}

	m.config = config

	// Re-initialize current page
	if err := m.initializePage(); err != nil {
		return fmt.Errorf("failed to reinitialize page: %w", err)
	}

	m.logger.Info("configuration reloaded", "path", m.configPath)

	return nil
}

// Start starts the zone manager (for future use with module sampling)
func (m *Manager) Start() error {
	m.logger.Info("zone manager started")
	return nil
}

// Stop stops the zone manager
func (m *Manager) Stop() error {
	m.logger.Info("stopping zone manager")
	m.cancel()
	m.wg.Wait()
	m.logger.Info("zone manager stopped")
	return nil
}

// GetConfig returns the current configuration
func (m *Manager) GetConfig() *Config {
	return m.config
}

// GetCurrentPage returns the current page index
func (m *Manager) GetCurrentPage() int {
	return m.currentPage
}

// SetOnPageChange sets a callback to be called when the page changes
func (m *Manager) SetOnPageChange(callback func(pageIndex int) error) {
	m.onPageChange = callback
}

// GetZones returns all zones for the current page
func (m *Manager) GetZones() map[string]*Zone {
	return m.zones
}

// UpdateLiveSwipe updates the live swipe progress for interactive transitions.
// This is called continuously while the user's finger is moving during a swipe.
func (m *Manager) UpdateLiveSwipe(progress float32, isLeft bool) error {
	rawProgress := progress
	if progress < 0 {
		progress = 0
	} else if progress > 1 {
		progress = 1
	}

	m.transitionMu.RLock()
	transitionActive := m.transition.Active
	var transitionProgress float64
	if transitionActive && m.transition.Duration > 0 {
		elapsed := time.Since(m.transition.StartTime)
		transitionProgress = math.Min(1, float64(elapsed)/float64(m.transition.Duration))
	}
	m.transitionMu.RUnlock()

	m.liveSwipeMu.Lock()
	defer m.liveSwipeMu.Unlock()

	// If not already active, start the live swipe
	if !m.liveSwipeActive {
		m.liveSwipeActive = true
		m.liveSwipeLeft = isLeft

		// Capture current frame as the "old" frame
		m.lastFrameMu.Lock()
		oldFrame := m.lastFrame
		m.lastFrameMu.Unlock()

		if oldFrame == nil {
			// Render current frame if we don't have one
			frame, err := m.RenderFrame()
			if err != nil {
				m.logger.Error("failed to render current frame", "error", err)
				return err
			}
			oldFrame = frame
		}

		// Get the target page index
		targetPage := m.currentPage
		if isLeft {
			targetPage = (m.currentPage + 1) % len(m.config.Pages)
		} else {
			targetPage = (m.currentPage - 1 + len(m.config.Pages)) % len(m.config.Pages)
		}

		// Try to use fresh cached frame for live swipe preview (deferred engagement)
		var targetFrame *image.RGBA
		m.pageCacheMu.RLock()
		cachedFrame, hasCached := m.pageCache[targetPage]
		m.pageCacheMu.RUnlock()

		if hasCached {
			targetFrame = cachedFrame
			m.logger.Debug("cached target frame ready for live swipe", "page", targetPage)
		} else {
			m.logger.Debug("no cached target frame, using placeholder until ready", "page", targetPage)
		}

		previewActive := false
		previewThreshold := liveSwipePreviewThreshold
		newFrame := oldFrame
		if targetFrame != nil && float64(progress) >= previewThreshold {
			newFrame = targetFrame
			previewActive = true
		}

		m.liveSwipeTargetFrame = targetFrame
		m.liveSwipePreviewActive = previewActive

		// Start manual transition with live progress control
		direction := 1
		if !isLeft {
			direction = -1
		}

		transitionType := TransitionSlideLeft
		if !isLeft {
			transitionType = TransitionSlideRight
		}

		m.transitionMu.Lock()
		m.transition.StartManual(transitionType, oldFrame, newFrame, direction)
		m.transition.SetManualProgress(float64(progress))
		m.transitionMu.Unlock()

		m.logger.Debug("live swipe started",
			"progress", int(progress*100),
			"direction", map[bool]string{true: "left", false: "right"}[isLeft],
			"target_frame_ready", targetFrame != nil,
			"preview_active", previewActive,
			"preview_threshold_pct", int(previewThreshold*100))
	}

	// Update progress
	m.liveSwipeProgress = progress

	// Update transition progress by adjusting the start time
	previewLog := ""
	m.transitionMu.Lock()
	if m.transition.Active {
		m.transition.SetManualProgress(float64(progress))
		if m.transition.IsManual() {
			targetFrame := m.liveSwipeTargetFrame
			if targetFrame != nil {
				if !m.liveSwipePreviewActive && float64(progress) >= liveSwipePreviewThreshold {
					if m.transition.NewFrame != targetFrame {
						m.transition.NewFrame = targetFrame
					}
					m.liveSwipePreviewActive = true
					previewLog = "engaged"
				} else if m.liveSwipePreviewActive && float64(progress) < liveSwipePreviewThreshold {
					if m.transition.NewFrame != m.transition.OldFrame {
						m.transition.NewFrame = m.transition.OldFrame
					}
					m.liveSwipePreviewActive = false
					previewLog = "reverted"
				}
			}
		}
	}
	manualActive := false
	manualProgress := 0.0
	manualAnimating := false
	manualTarget := 0.0
	manualDuration := time.Duration(0)
	if m.transition.Active && m.transition.IsManual() {
		manualActive = true
		manualProgress = m.transition.ManualProgress()
		manualAnimating = m.transition.manualAnimating
		manualTarget = m.transition.manualTo
		manualDuration = m.transition.manualDuration
	}
	m.transitionMu.Unlock()

	if previewLog != "" {
		m.logger.Debug("swipe preview frame "+previewLog,
			"progress_pct", int(progress*100),
			"threshold_pct", int(liveSwipePreviewThreshold*100))
	}

	progressPct := int(transitionProgress * 100)
	if manualActive {
		progressPct = int(manualProgress * 100)
	}

	m.logger.Debug("swipe progress updated",
		"raw_progress", rawProgress,
		"clamped_progress", progress,
		"direction", map[bool]string{true: "left", false: "right"}[isLeft],
		"transition_active", transitionActive,
		"transition_progress_pct", progressPct,
		"transition_manual", manualActive,
		"transition_manual_animating", manualAnimating,
		"transition_manual_target_pct", int(manualTarget*100),
		"transition_manual_duration_ms", manualDuration.Milliseconds())

	return nil
}

// FinalizeLiveSwipe commits an in-progress live swipe and smoothly completes the transition.
// This is called when a swipe gesture is determined to commit (change pages).
// It updates the current page and adjusts the ongoing transition to complete smoothly.
func (m *Manager) FinalizeLiveSwipe(progress float32, velocity float32, isLeft bool) error {
	m.liveSwipeMu.Lock()

	if progress < 0 {
		progress = 0
	} else if progress > 1 {
		progress = 1
	}

	if !m.liveSwipeActive {
		// Fallback: if we somehow finalize without an active swipe, use provided direction
		m.liveSwipeLeft = isLeft
		m.liveSwipeProgress = progress
		m.lastSwipeFinalize = time.Now()
		m.lastSwipeDirLeft = isLeft
		m.liveSwipeMu.Unlock()
		return nil // No active swipe to finalize
	}

	// Use the direction captured when the live swipe started to avoid jitter flipping pages.
	swipeLeft := m.liveSwipeLeft

	now := time.Now()
	var sinceLastFinalize time.Duration
	if !m.lastSwipeFinalize.IsZero() {
		sinceLastFinalize = now.Sub(m.lastSwipeFinalize)
	}
	m.lastSwipeFinalize = now
	m.lastSwipeDirLeft = swipeLeft

	m.transitionMu.RLock()
	transitionActive := m.transition.Active
	transitionType := m.transition.Type
	transitionDirection := m.transition.Direction
	transitionDuration := m.transition.Duration
	transitionStart := m.transition.StartTime
	manualActive := transitionActive && m.transition.IsManual()
	manualProgress := 0.0
	manualAnimating := false
	manualTarget := 0.0
	manualDuration := time.Duration(0)
	if manualActive {
		manualProgress = m.transition.ManualProgress()
		manualAnimating = m.transition.manualAnimating
		manualTarget = m.transition.manualTo
		manualDuration = m.transition.manualDuration
	}
	m.transitionMu.RUnlock()

	var transitionProgress float64
	if transitionDuration > 0 {
		transitionProgress = math.Min(1, float64(time.Since(transitionStart))/float64(transitionDuration))
	}

	m.liveSwipeProgress = progress
	currentProgress := progress
	m.logger.Debug("finalizing live swipe",
		"current_progress", int(currentProgress*100),
		"velocity_px_s", int(velocity),
		"direction", map[bool]string{true: "left", false: "right"}[swipeLeft],
		"since_last_finalize_ms", sinceLastFinalize.Milliseconds(),
		"transition_active", transitionActive,
		"transition_type", transitionType,
		"transition_direction", transitionDirection,
		"transition_progress_pct", int(transitionProgress*100),
		"transition_manual", manualActive,
		"transition_manual_progress_pct", int(manualProgress*100),
		"transition_manual_animating", manualAnimating,
		"transition_manual_target_pct", int(manualTarget*100),
		"transition_manual_duration_ms", manualDuration.Milliseconds())

	// Calculate target page
	var targetPage int
	if swipeLeft {
		targetPage = (m.currentPage + 1) % len(m.config.Pages)
	} else {
		targetPage = m.currentPage - 1
		if targetPage < 0 {
			targetPage = len(m.config.Pages) - 1
		}
	}

	// Update current page immediately
	oldPage := m.currentPage
	m.currentPage = targetPage

	// Calculate remaining distance and adjust duration based on velocity
	remainingDistance := 1.0 - currentProgress
	if remainingDistance < 0 {
		remainingDistance = 0
	}

	// Base duration derived from remaining distance so we continue at a similar perceived speed.
	const (
		minDuration     = 35 * time.Millisecond
		distanceStretch = 90 * time.Millisecond
	)

	additional := time.Duration(float32(distanceStretch) * remainingDistance)
	finalDuration := minDuration + additional

	// Adjust duration based on velocity (faster swipe = shorter finish, slower swipe = longer finish)
	switch {
	case velocity >= 360:
		finalDuration = time.Duration(float64(finalDuration) * 0.5)
	case velocity >= 240:
		finalDuration = time.Duration(float64(finalDuration) * 0.7)
	case velocity >= 170:
		finalDuration = time.Duration(float64(finalDuration) * 0.82)
	case velocity < 120:
		finalDuration = time.Duration(float64(finalDuration) * 1.3)
	}

	// Clamp to sensible bounds so we never snap nor drag too long.
	if finalDuration < 40*time.Millisecond {
		finalDuration = 40 * time.Millisecond
	}
	if finalDuration > 200*time.Millisecond {
		finalDuration = 200 * time.Millisecond
	}

	m.logger.Info("✅ PAGE SWITCH (momentum)",
		"from", oldPage,
		"to", targetPage,
		"remaining", int(remainingDistance*100),
		"duration_ms", finalDuration.Milliseconds())

	// Adjust the transition to complete smoothly from current progress
	m.transitionMu.Lock()
	// Set start time so that current progress is maintained and it completes in finalDuration
	if m.transition.Active {
		targetFrame := m.liveSwipeTargetFrame
		if targetFrame != nil && m.transition.IsManual() {
			if m.transition.NewFrame != targetFrame {
				m.transition.NewFrame = targetFrame
				m.logger.Debug("transition preview forced for finalize",
					"progress_pct", int(currentProgress*100))
			}
			m.liveSwipePreviewActive = true
		}
		if m.transition.IsManual() {
			m.logger.Debug("transition finalize manual",
				"current_progress_pct", int(float64(currentProgress)*100),
				"duration_ms", finalDuration.Milliseconds())
			m.transition.FinalizeManual(finalDuration)
		} else {
			m.logger.Debug("transition finalize timed",
				"current_progress_pct", int(float64(currentProgress)*100),
				"duration_ms", finalDuration.Milliseconds())
			m.transition.Duration = finalDuration
			m.transition.StartTime = time.Now().Add(-time.Duration(currentProgress * float32(finalDuration)))
		}
	}
	m.transitionMu.Unlock()

	// Mark live swipe as no longer active (transition will complete naturally)
	m.liveSwipeActive = false
	m.liveSwipeProgress = 0
	m.liveSwipeTargetFrame = nil
	m.liveSwipePreviewActive = false
	m.liveSwipeMu.Unlock()

	// Initialize the new page (zones, etc)
	if err := m.initializePage(); err != nil {
		m.logger.Error("failed to initialize page after swipe", "error", err)
		// Don't revert - animation is already running
	}

	// Ensure the transition finishes with the real target frame (cache may have been stale)
	targetFrame := m.getCachedPageFrame(targetPage)
	if targetFrame == nil {
		m.logger.Debug("swipe finalize cache miss", "page", targetPage)
		frame, err := m.renderImmediateFrameForCurrentPage()
		if err != nil {
			m.logger.Error("failed to render target page for swipe completion", "page", targetPage, "error", err)
		} else {
			targetFrame = frame
			m.pageCacheMu.Lock()
			m.pageCache[targetPage] = frame
			m.pageCacheMu.Unlock()
		}
	} else {
		m.logger.Debug("swipe finalize using cached frame", "page", targetPage)
	}

	if targetFrame != nil {
		m.transitionMu.Lock()
		if m.transition.Active {
			m.transition.NewFrame = targetFrame
		}
		m.transitionMu.Unlock()
	}

	// Notify page change callback if set (async)
	if m.onPageChange != nil {
		go func() {
			if err := m.onPageChange(targetPage); err != nil {
				m.logger.Error("page change callback failed", "error", err)
			}
		}()
	}

	// Pre-render adjacent pages in background
	go m.preRenderAdjacentPages()

	m.transitionMu.RLock()
	postManualActive := m.transition.Active && m.transition.IsManual()
	postManualProgress := 0.0
	if postManualActive {
		postManualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("finalize complete",
		"current_page", m.currentPage,
		"target_page", targetPage,
		"transition_manual_active", postManualActive,
		"transition_manual_progress_pct", int(postManualProgress*100))

	return nil
}

// CancelLiveSwipe cancels an in-progress live swipe and snaps back to the current page.
func (m *Manager) CancelLiveSwipe() error {
	m.liveSwipeMu.Lock()
	defer m.liveSwipeMu.Unlock()

	if !m.liveSwipeActive {
		return nil // Nothing to cancel
	}

	currentProgress := m.liveSwipeProgress
	swipeLeft := m.liveSwipeLeft
	m.transitionMu.RLock()
	transitionActive := m.transition.Active
	transitionType := m.transition.Type
	transitionDirection := m.transition.Direction
	manualActive := transitionActive && m.transition.IsManual()
	manualProgress := 0.0
	manualAnimating := false
	manualTarget := 0.0
	manualDuration := time.Duration(0)
	if manualActive {
		manualProgress = m.transition.ManualProgress()
		manualAnimating = m.transition.manualAnimating
		manualTarget = m.transition.manualTo
		manualDuration = m.transition.manualDuration
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("canceling live swipe",
		"progress_pct", int(currentProgress*100),
		"direction", map[bool]string{true: "left", false: "right"}[swipeLeft],
		"transition_active", transitionActive,
		"transition_type", transitionType,
		"transition_direction", transitionDirection,
		"transition_manual", manualActive,
		"transition_manual_progress_pct", int(manualProgress*100),
		"transition_manual_animating", manualAnimating,
		"transition_manual_target_pct", int(manualTarget*100),
		"transition_manual_duration_ms", manualDuration.Milliseconds())

	// Stop the transition
	m.transitionMu.Lock()
	if m.transition.Active {
		if m.transition.IsManual() {
			decay := time.Duration(120*float64(currentProgress)) * time.Millisecond
			if decay < 50*time.Millisecond {
				decay = 50 * time.Millisecond
			}
			m.transition.AnimateManualTo(0, decay)
			m.logger.Debug("swipe cancel animate back",
				"duration_ms", decay.Milliseconds(),
				"start_progress_pct", int(manualProgress*100),
				"target_progress_pct", 0)
		} else {
			m.transition.Active = false
		}
	}
	m.transitionMu.Unlock()

	m.liveSwipeActive = false
	m.liveSwipeProgress = 0
	m.liveSwipeTargetFrame = nil
	m.liveSwipePreviewActive = false

	m.transitionMu.RLock()
	postManualActive := m.transition.Active && m.transition.IsManual()
	postManualProgress := 0.0
	if postManualActive {
		postManualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("cancel complete",
		"current_page", m.currentPage,
		"transition_manual_active", postManualActive,
		"transition_manual_progress_pct", int(postManualProgress*100))

	return nil
}
