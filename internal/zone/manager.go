package zone

import (
	"bytes"
	"context"
	"fmt"
	"image"
	"image/draw"
	"image/gif"
	_ "image/jpeg"
	_ "image/png"
	"log/slog"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/nfnt/resize"
	"gopkg.in/yaml.v3"

	"github.com/mantonx/nexus-next/pkg/module"
)

// liveSwipePreviewThreshold is the drag progress at which the target page
// frame is swapped in. 0 = show immediately on first drag update.
const liveSwipePreviewThreshold = 0.0

// Manager manages zones, their renderers, and lifecycle.
//
// Implementation is split across four files for readability:
//   manager.go       — struct, NewManager, lifecycle (Start/Stop/Reload)
//   manager_page.go  — page/config/navigation/cache methods
//   manager_render.go — payload/theme/compositing/frame methods
//   manager_swipe.go  — live swipe and transition methods
type Manager struct {
	logger     *slog.Logger
	config     *Config
	configPath string
	currentPage int
	themeMu    sync.RWMutex // guards config.Theme live updates

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
	liveSwipeBoundary      bool // true when swiping into a page boundary (rubber-band)
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

	// Zone cycle callback — called when a tap action advances a zone to the next module choice.
	onZoneCycle func(zoneConfig ZoneConfig) error

	// Tracks the current choice index per zone for cycling (zoneID → choice index).
	choiceIndex   map[string]int
	choiceIndexMu sync.Mutex

	// Lifecycle
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// Zone represents a single zone instance.
type Zone struct {
	Config   ZoneConfig
	Renderer *Renderer
	Plugin   string
}

// NewManager creates a new zone manager.
func NewManager(ctx context.Context, logger *slog.Logger, configPath string) (*Manager, error) {
	config, err := LoadConfig(configPath)
	if err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
	}

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
		choiceIndex: make(map[string]int),
		ctx:         ctx,
		cancel:      cancel,
	}

	if err := m.initializePage(); err != nil {
		return nil, fmt.Errorf("failed to initialize page: %w", err)
	}

	logger.Info("zone manager initialized",
		"pages", len(config.Pages),
		"current_page", config.Pages[m.currentPage].Name,
		"zones", len(m.zones))

	go m.preRenderAdjacentPages()

	return m, nil
}

// LoadConfig loads a zone configuration from YAML.
func LoadConfig(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("failed to read config file: %w", err)
	}

	var config Config
	if err := yaml.Unmarshal(data, &config); err != nil {
		return nil, fmt.Errorf("failed to parse config: %w", err)
	}

	if config.Theme.Bg == "" {
		config.Theme = DefaultTheme()
	}

	return &config, nil
}

// Start starts the zone manager.
func (m *Manager) Start() error {
	m.logger.Info("zone manager started")
	return nil
}

// Stop stops the zone manager.
func (m *Manager) Stop() error {
	m.logger.Info("stopping zone manager")
	m.cancel()
	m.wg.Wait()
	m.logger.Info("zone manager stopped")
	return nil
}

// ReloadFromConfig replaces the running layout with the given config and
// re-initialises the current page. Used by the layout editor so changes take
// effect immediately without restarting the application.
func (m *Manager) ReloadFromConfig(config *Config) error {
	if err := config.Validate(); err != nil {
		return fmt.Errorf("invalid config: %w", err)
	}

	m.config = config

	// Clamp currentPage to new page count.
	if m.currentPage >= len(config.Pages) {
		m.currentPage = 0
	}

	if err := m.initializePage(); err != nil {
		return fmt.Errorf("failed to reinitialize page: %w", err)
	}

	// Invalidate the page cache — old frames are stale after a layout change.
	m.pageCacheMu.Lock()
	m.pageCache = make(map[int]*image.RGBA)
	m.pageCacheMu.Unlock()

	go m.preRenderAdjacentPages()
	m.logger.Info("layout reloaded from config",
		"pages", len(config.Pages),
		"current_page", config.Pages[m.currentPage].Name)
	return nil
}

// SetBackground loads an image or GIF from disk and sets it as the background
// layer on the current compositor. Passing an empty path clears the background.
// Supported formats: PNG, JPEG, GIF (animated GIFs play at their own frame rate).
func (m *Manager) SetBackground(path string) error {
	if path == "" {
		m.compositor.ClearBackground()
		m.logger.Info("background cleared")
		return nil
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("failed to read background image: %w", err)
	}

	if strings.ToLower(strings.TrimPrefix(path[strings.LastIndex(path, "."):], ".")) == "gif" {
		g, err := gif.DecodeAll(bytes.NewReader(data))
		if err != nil {
			return fmt.Errorf("failed to decode GIF: %w", err)
		}
		// Resize each frame to 640×48 if necessary.
		for i, frame := range g.Image {
			if frame.Bounds().Dx() != DisplayWidth || frame.Bounds().Dy() != DisplayHeight {
				resized := resize.Resize(DisplayWidth, DisplayHeight, frame, resize.Lanczos3)
				dst := image.NewPaletted(image.Rect(0, 0, DisplayWidth, DisplayHeight), frame.Palette)
				draw.FloydSteinberg.Draw(dst, dst.Bounds(), resized, image.Point{})
				g.Image[i] = dst
			}
		}
		m.compositor.SetBackgroundGIF(g)
		m.logger.Info("background GIF set", "path", path, "frames", len(g.Image))
		return nil
	}

	img, _, err := image.Decode(bytes.NewReader(data))
	if err != nil {
		return fmt.Errorf("failed to decode background image: %w", err)
	}

	resized := resize.Resize(DisplayWidth, DisplayHeight, img, resize.Lanczos3)
	dst := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	draw.Draw(dst, dst.Bounds(), resized, image.Point{}, draw.Src)
	m.compositor.SetBackground(dst)
	m.logger.Info("background image set", "path", path)
	return nil
}

// Reload reloads the configuration from disk.
func (m *Manager) Reload() error {
	config, err := LoadConfig(m.configPath)
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	if err := config.Validate(); err != nil {
		return fmt.Errorf("invalid config: %w", err)
	}

	m.config = config

	if err := m.initializePage(); err != nil {
		return fmt.Errorf("failed to reinitialize page: %w", err)
	}

	m.logger.Info("configuration reloaded", "path", m.configPath)

	return nil
}
