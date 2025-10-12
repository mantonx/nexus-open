package zone

import (
	"context"
	"fmt"
	"image"
	"log/slog"
	"os"
	"sync"
	"time"

	"gopkg.in/yaml.v3"

	"nexus-open/pkg/module"
)

// Manager manages zones, their renderers, and lifecycle
type Manager struct {
	logger     *slog.Logger
	config     *Config
	configPath string
	currentPage int

	// Zone state
	zones      map[string]*Zone
	renderers  map[string]*Renderer
	payloads   map[string]*module.Payload
	payloadsMu sync.RWMutex

	// Compositor for current page
	compositor *Compositor

	// Transition state
	transition     *TransitionState
	transitionMu   sync.RWMutex
	lastFrame      *image.RGBA
	lastFrameMu    sync.Mutex

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

	m.logger.Debug("payload updated",
		"zone_id", zoneID,
		"primary", payload.Primary,
		"severity", payload.Severity)

	return nil
}

// RenderFrame renders the current frame (all zones composited)
func (m *Manager) RenderFrame() (*image.RGBA, error) {
	// Check if transition is active
	m.transitionMu.RLock()
	if m.transition.Active && !m.transition.IsComplete() {
		frame := m.transition.Render()
		m.transitionMu.RUnlock()
		return frame, nil
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
				m.config.Theme.GetBgColor(),
				m.config.Theme.GetFgColor(),
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
		return nil // Already on this page
	}

	// Capture current frame for transition
	m.lastFrameMu.Lock()
	oldFrame := m.lastFrame
	m.lastFrameMu.Unlock()

	// Store old page index
	oldPage := m.currentPage

	// Switch to new page
	m.currentPage = pageIndex

	if err := m.initializePage(); err != nil {
		m.currentPage = oldPage // Revert on error
		return fmt.Errorf("failed to initialize page: %w", err)
	}

	// Render new page frame (without transition)
	m.payloadsMu.RLock()
	zoneImages := make(map[string]*image.RGBA)
	for zoneID, zone := range m.zones {
		payload, ok := m.payloads[zoneID]
		if !ok {
			payload = &module.Payload{
				Primary:   "—",
				Severity:  module.SeverityOK,
				Timestamp: time.Now(),
			}
		}

		img, _ := zone.Renderer.Render(*payload)
		zoneImages[zoneID] = img
	}
	m.payloadsMu.RUnlock()

	newFrame, err := m.compositor.Composite(zoneImages)
	if err != nil {
		return fmt.Errorf("failed to composite new frame: %w", err)
	}

	// Start transition if we have both frames
	if oldFrame != nil && transitionType != TransitionNone {
		m.transitionMu.Lock()
		m.transition.Start(transitionType, oldFrame, newFrame, direction)
		m.transitionMu.Unlock()
	}

	m.logger.Info("switched to page",
		"index", pageIndex,
		"name", m.config.Pages[pageIndex].Name,
		"transition", transitionType)

	return nil
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

// GetZones returns all zones for the current page
func (m *Manager) GetZones() map[string]*Zone {
	return m.zones
}
