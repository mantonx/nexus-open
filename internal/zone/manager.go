package zone

import (
	"bytes"
	"context"
	"encoding/json"
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

	"github.com/mantonx/nexus-open/internal/store"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// liveSwipePreviewThreshold is the drag progress at which the target page
// frame is swapped in. 0 = show immediately on first drag update.
const liveSwipePreviewThreshold = 0.0

// PluginLookup is satisfied by Sampler. Manager uses it to resolve a zone's
// live plugin instance without importing the sampler package directly.
type PluginLookup interface {
	GetPlugin(zoneID string) (plugin.Plugin, bool)
}

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
	db         *store.DB // nil when running from YAML only (e.g. cmd binaries)
	currentPage int
	configMu   sync.RWMutex // guards m.config pointer reads/writes

	// Zone state
	zones      map[string]*Zone
	renderers  map[string]*Renderer
	payloads   map[string]*plugin.Payload
	payloadsMu sync.RWMutex

	// Compositor for current page
	compositor *Compositor

	// Transition state
	transition   *TransitionState
	transitionMu sync.RWMutex
	lastFrame    *image.RGBA
	frameDirty   bool // true when lastFrame is stale and must be re-composited
	lastFrameMu  sync.Mutex

	// Double-buffer for the compositor output: frameBufs[0] and [1] are
	// pre-allocated once and reused. frameBufIdx is the index of the buffer
	// that is currently being written (back buffer); the other is lastFrame
	// (front buffer). Both are 640×48 RGBA = 122,880 bytes each.
	frameBufs   [2]*image.RGBA
	frameBufIdx int

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

	// Zone cycle callback — called when a tap action advances a zone to the next plugin choice.
	onZoneCycle func(zoneConfig ZoneConfig) error

	// Detail state callback — called when the detail overlay is shown or hidden.
	onDetailState func(active bool)

	// Tracks the current choice index per zone for cycling (zoneID → choice index).
	choiceIndex   map[string]int
	choiceIndexMu sync.Mutex

	// Detail overlay — shown when a Tapper zone is tapped.
	detailActive     bool
	detailShownAt    time.Time
	detailFrame      *image.RGBA
	detailTransition TransitionState
	detailTimer      *time.Timer
	detailMu         sync.Mutex

	// Plugin lookup — set after construction via SetPluginLookup.
	// Kept as an interface so the zone package does not import the sampler.
	pluginLookup PluginLookup

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

// ClearDetail dismisses the detail overlay with a slide-down transition.
func (m *Manager) ClearDetail() {
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

	if m.onDetailState != nil {
		go m.onDetailState(false)
	}
}

// IsShowingDetail reports whether the detail overlay is active or animating IN.
// Returns false during the slide-down dismiss transition so taps pass through.
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

// detailCacheTTL is how long a DB-persisted DetailPayload is considered fresh.
// Longer than the plugin's in-memory TTL because the point of DB persistence
// is to survive restarts, not to drive high-frequency refreshes.
const detailCacheTTL = 30 * time.Minute

// HandleZoneTap looks up the plugin for zoneID, calls OnTap if it implements
// plugin.Tapper, and shows the result as a detail overlay. Safe to call from
// any goroutine.
//
// Cache strategy (DB-backed):
//  1. Load cached DetailPayload from DB.
//  2. If cache is fresh (< detailCacheTTL), show it immediately — done.
//  3. If cache is stale, show it immediately for responsiveness, then fetch
//     fresh data in the background and update the overlay + DB.
//  4. If no cache, fetch synchronously, show, and save to DB.
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

	// Try DB cache first.
	if m.db != nil {
		cached, fetchedAt, err := m.db.LoadPayloadCache(zoneID)
		if err == nil && len(cached) > 0 {
			var detail plugin.DetailPayload
			if json.Unmarshal(cached, &detail) == nil {
				detail.ZoneID = zoneID
				age := time.Since(time.Unix(fetchedAt, 0))
				if age < detailCacheTTL {
					// Fresh — serve immediately.
					m.ShowDetail(detail)
					return nil
				}
				// Stale — serve immediately, revalidate in background.
				m.ShowDetail(detail)
				go m.revalidateDetailCache(zoneID, tapper)
				return nil
			}
		}
	}

	// No usable cache — fetch synchronously.
	detail, err := tapper.OnTap()
	if err == plugin.ErrNotTapper {
		return fmt.Errorf("HandleZoneTap: plugin for zone %q returned ErrNotTapper", zoneID)
	}
	if err != nil {
		return fmt.Errorf("HandleZoneTap: OnTap error for zone %q: %w", zoneID, err)
	}
	detail.ZoneID = zoneID
	m.ShowDetail(detail)
	m.saveDetailCache(zoneID, detail)
	return nil
}

// revalidateDetailCache fetches a fresh DetailPayload and updates the overlay
// and DB cache. Called in a goroutine when a stale cache entry was served.
func (m *Manager) revalidateDetailCache(zoneID string, tapper plugin.Tapper) {
	detail, err := tapper.OnTap()
	if err != nil {
		m.logger.Warn("detail cache revalidation failed", "zone", zoneID, "error", err)
		return
	}
	detail.ZoneID = zoneID
	m.ShowDetail(detail)
	m.saveDetailCache(zoneID, detail)
}

// saveDetailCache serialises detail and writes it to the DB cache.
func (m *Manager) saveDetailCache(zoneID string, detail plugin.DetailPayload) {
	if m.db == nil {
		return
	}
	raw, err := json.Marshal(detail)
	if err != nil {
		m.logger.Warn("detail cache marshal failed", "zone", zoneID, "error", err)
		return
	}
	if err := m.db.SavePayloadCache(zoneID, detail.ZoneID, raw, time.Now().Unix()); err != nil {
		m.logger.Warn("detail cache save failed", "zone", zoneID, "error", err)
	}
}

// HandleZoneTapAtX resolves the zone at hardware X coordinate x (0–639) on the
// current page and executes its OnTap action — identical to what the hardware
// touch handler does. Used by the debug tap API so the Flutter preview can
// drive the same code path with mouse clicks.
func (m *Manager) HandleZoneTapAtX(x int) error {
	m.configMu.RLock()
	cfg := m.config
	pageIdx := m.currentPage
	m.configMu.RUnlock()

	if pageIdx >= len(cfg.Pages) {
		return fmt.Errorf("tap: page index %d out of range", pageIdx)
	}
	page := cfg.Pages[pageIdx]
	page.ComputeOffsets()

	for _, z := range page.Zones {
		if x >= z.X && x < z.X+z.Width {
			switch z.OnTap {
			case TapActionCycle:
				return m.CycleZonePlugin(z.ID)
			case TapActionDetail:
				return m.HandleZoneTap(z.ID)
			}
			return nil
		}
	}
	return nil // tap outside all zones — not an error
}

// NewManager creates a new zone manager.
//
// When db is non-nil the manager loads the layout from SQLite first; if the
// DB contains no pages it falls back to fallbackYAMLPath and seeds the DB so
// subsequent starts use the DB path. Pass nil for db (and any string for
// fallbackYAMLPath) to operate in YAML-only mode (used by the cmd/ binaries).
func NewManager(ctx context.Context, logger *slog.Logger, db *store.DB, fallbackYAMLPath string) (*Manager, error) {
	var config *Config
	var err error

	if db != nil {
		hasLayout, hlErr := db.HasLayout()
		if hlErr != nil {
			return nil, fmt.Errorf("zone: check db layout: %w", hlErr)
		}
		if !hasLayout {
			// Genuine first use or deliberate wipe — seed from the factory YAML.
			logger.Info("zone: db has no layout, seeding from YAML", "path", fallbackYAMLPath)
			config, err = LoadConfig(fallbackYAMLPath)
			if err != nil {
				return nil, fmt.Errorf("failed to load fallback YAML config: %w", err)
			}
			if seedErr := seedDBFromConfig(db, config, logger); seedErr != nil {
				// Non-fatal: log and continue with the YAML-loaded config.
				logger.Warn("zone: failed to seed db from YAML (continuing with YAML)", "error", seedErr)
			}
		} else {
			config, err = LoadConfigFromDB(db)
			if err != nil {
				return nil, fmt.Errorf("failed to load config from db: %w", err)
			}
		}
	} else {
		config, err = LoadConfig(fallbackYAMLPath)
		if err != nil {
			return nil, fmt.Errorf("failed to load config: %w", err)
		}
	}

	if err := config.Validate(); err != nil {
		return nil, fmt.Errorf("invalid config: %w", err)
	}

	ctx, cancel := context.WithCancel(ctx)

	m := &Manager{
		logger:      logger,
		config:      config,
		configPath:  fallbackYAMLPath,
		db:          db,
		currentPage: 0,
		zones:       make(map[string]*Zone),
		renderers:   make(map[string]*Renderer),
		payloads:    make(map[string]*plugin.Payload),
		transition:  NewTransitionState(),
		frameDirty:  true,
		frameBufs: [2]*image.RGBA{
			image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight)),
			image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight)),
		},
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

// seedDBFromConfig writes cfg into the layout tables in a single transaction.
// This is called once on first run to bootstrap the DB from the bundled YAML.
func seedDBFromConfig(db *store.DB, cfg *Config, logger *slog.Logger) error {
	if cfg == nil || len(cfg.Pages) == 0 {
		return nil
	}

	pages := make([]store.StoredPage, len(cfg.Pages))
	zoneMap := make(map[int64][]store.StoredZone, len(cfg.Pages))

	for i, p := range cfg.Pages {
		pageID := int64(i + 1)
		pages[i] = store.StoredPage{ID: pageID, Name: p.Name, Ord: i}

		for j, z := range p.Zones {
			sz := store.StoredZone{
				ID:        z.ID,
				PageID:    pageID,
				Ord:       j,
				WidthPx:   z.Width,
				Plugin:    z.Plugin,
				RefreshMs: z.RefreshMs,
				Align:     string(z.Align),
				OnTap:     string(z.OnTap),
				Choices:   z.Choices,
			}
			if z.ThemeOverride != nil {
				sz.ThemeJSON = map[string]any{
					"accent": z.ThemeOverride.Accent,
					"bg":     z.ThemeOverride.Bg,
					"fg":     z.ThemeOverride.Fg,
				}
			}
			zoneMap[pageID] = append(zoneMap[pageID], sz)
		}
	}

	if err := db.ImportLayout(pages, zoneMap); err != nil {
		return err
	}
	logger.Info("zone: layout seeded into db", "pages", len(pages))
	return nil
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

	m.configMu.Lock()
	m.config = config
	m.configMu.Unlock()

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

	// Restart the sampler for the current page so newly added zones are picked
	// up immediately without requiring a manual page switch.
	if m.onPageChange != nil {
		go func() {
			if err := m.onPageChange(m.currentPage); err != nil {
				m.logger.Error("layout reload: page change callback failed", "error", err)
			}
		}()
	}

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

// Reload reloads the configuration. When a DB is available it reloads from
// the database; otherwise it falls back to the YAML file on disk.
func (m *Manager) Reload() error {
	var config *Config
	var err error

	if m.db != nil {
		config, err = LoadConfigFromDB(m.db)
		if err != nil {
			return fmt.Errorf("failed to reload config from db: %w", err)
		}
	} else {
		config, err = LoadConfig(m.configPath)
		if err != nil {
			return fmt.Errorf("failed to reload config from yaml: %w", err)
		}
	}

	if err := config.Validate(); err != nil {
		return fmt.Errorf("invalid config: %w", err)
	}

	m.config = config

	if err := m.initializePage(); err != nil {
		return fmt.Errorf("failed to reinitialize page: %w", err)
	}

	m.logger.Info("configuration reloaded", "db", m.db != nil, "path", m.configPath)

	return nil
}
