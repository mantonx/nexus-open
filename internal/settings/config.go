// Package config provides configuration management backed by SQLite.
//
// The Manager reads and writes UI preferences (colours, locale, display config)
// using the shared [store.DB]. On first run it imports any existing config.yaml
// so users upgrading from a YAML-based installation don't lose their settings.
package config

import (
	"errors"
	"fmt"
	"image/color"
	"log/slog"
	"path/filepath"
	"regexp"
	"strings"
	"sync"

	"github.com/mantonx/nexus-open/internal/store"
)

// Manager handles configuration loading, validation, and watching.
// The zero value is invalid — use [NewManager].
type Manager struct {
	mu         sync.RWMutex
	cfg        *Config
	store      *store.DB
	watchers   []chan<- Config
	IsFirstRun bool // true when the DB was freshly created on this launch
}

// Config holds the application configuration (UI settings only).
// Module-specific configs are managed per-zone via zone config system.
// openapi:schema Config
type Config struct {
	// openapi:description Background color in hex format
	// openapi:example #000000
	BackgroundColor string `mapstructure:"background_color" json:"background_color"`
	// openapi:description Background image filename
	// openapi:example background.png
	BackgroundImage string `mapstructure:"background_image" json:"background_image"`
	// openapi:description Text color in hex format
	// openapi:example #FFFFFF
	TextColor string `mapstructure:"text_color" json:"text_color"`
	// openapi:description List of custom image paths
	ImagePaths []string `mapstructure:"image_paths" json:"image_paths"`
	// openapi:description Display-specific configuration
	Display DisplayConfig `mapstructure:"display" json:"display"`
}

// DisplayConfig holds display-specific configuration.
// openapi:schema DisplayConfig
type DisplayConfig struct {
	// openapi:description Font family name
	// openapi:example GoRegular
	FontFamily string `mapstructure:"font_family" json:"font_family"`
	// openapi:description Base font size in points
	// openapi:example 11.0
	FontSize float64 `mapstructure:"font_size" json:"font_size"`
	// openapi:description Time display font size in points
	// openapi:example 14.0
	TimeFontSize float64 `mapstructure:"time_font_size" json:"time_font_size"`
	// openapi:description Layout style
	// openapi:enum dashboard minimalist compact balanced
	// openapi:example dashboard
	Layout string `mapstructure:"layout" json:"layout"`
	// openapi:description Date format string
	// openapi:example MM/DD/YYYY
	DateFormat string `mapstructure:"date_format" json:"date_format"`
}

// Default configuration values.
const (
	DefaultBackgroundColor = "#000000"
	DefaultBackgroundImage = "background.png"
	DefaultTextColor       = "#FFFFFF"
	DefaultFontFamily      = "GoRegular"
	DefaultFontSize        = 11.0
	DefaultTimeFontSize    = 14.0
	DefaultLayout          = "dashboard"
	DefaultDateFormat      = "MM/DD/YYYY"
)

var (
	ErrInvalidColor = errors.New("invalid color: must be hex color (e.g. #FFFFFF)")
)

var hexColorRegex = regexp.MustCompile(`^#[0-9A-Fa-f]{6}$`)

// NewManager creates a Manager backed by the given store.
// It loads the current config from the store (applying defaults for any
// missing keys) and marks IsFirstRun if the store was newly created.
func NewManager(s *store.DB, logger *slog.Logger) (*Manager, error) {
	if logger == nil {
		logger = slog.Default()
	}

	m := &Manager{
		store:      s,
		watchers:   make([]chan<- Config, 0),
		IsFirstRun: s.IsFirstRun(),
	}

	cfg, err := m.loadFromStore()
	if err != nil {
		return nil, fmt.Errorf("settings: load: %w", err)
	}
	m.cfg = cfg

	logger.Info("settings loaded from store")
	return m, nil
}

// NewManagerFromPath is a convenience constructor that opens its own store at
// the given path (or the default path when empty). Callers that already have a
// *store.DB should use NewManager instead.
func NewManagerFromPath(path string, logger *slog.Logger) (*Manager, error) {
	s, err := store.Open(path, logger)
	if err != nil {
		return nil, err
	}
	return NewManager(s, logger)
}

// Get returns a copy of the current configuration (thread-safe).
func (m *Manager) Get() Config {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return *m.cfg
}

// Update atomically validates, persists, and broadcasts new configuration.
func (m *Manager) Update(cfg Config) error {
	if err := cfg.Validate(); err != nil {
		return err
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	m.cfg = &cfg

	if err := m.saveToStore(cfg); err != nil {
		return fmt.Errorf("settings: save: %w", err)
	}

	m.notifyWatchers(cfg)
	return nil
}

// Watch registers a channel to receive configuration updates.
func (m *Manager) Watch(ch chan<- Config) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.watchers = append(m.watchers, ch)
}

// ── Store I/O ─────────────────────────────────────────────────────────────────

func (m *Manager) loadFromStore() (*Config, error) {
	settings, err := m.store.GetSettings()
	if err != nil {
		return nil, err
	}

	cfg := &Config{
		BackgroundColor: getOrDefault(settings, "background_color", DefaultBackgroundColor),
		BackgroundImage: getOrDefault(settings, "background_image", DefaultBackgroundImage),
		TextColor:       getOrDefault(settings, "text_color", DefaultTextColor),
		Display: DisplayConfig{
			FontFamily:   getOrDefault(settings, "display.font_family", DefaultFontFamily),
			Layout:       getOrDefault(settings, "display.layout", DefaultLayout),
			DateFormat:   getOrDefault(settings, "display.date_format", DefaultDateFormat),
		},
	}

	if v := parseFloat(settings["display.font_size"]); v > 0 {
		cfg.Display.FontSize = v
	} else {
		cfg.Display.FontSize = DefaultFontSize
	}
	if v := parseFloat(settings["display.time_font_size"]); v > 0 {
		cfg.Display.TimeFontSize = v
	} else {
		cfg.Display.TimeFontSize = DefaultTimeFontSize
	}

	// Validate and apply defaults for any missing/invalid values.
	if !isValidHexColor(cfg.BackgroundColor) {
		cfg.BackgroundColor = DefaultBackgroundColor
	}
	if !isValidHexColor(cfg.TextColor) {
		cfg.TextColor = DefaultTextColor
	}

	return cfg, nil
}

func (m *Manager) saveToStore(cfg Config) error {
	return m.store.SetSettings(map[string]string{
		"background_color":        cfg.BackgroundColor,
		"background_image":        cfg.BackgroundImage,
		"text_color":              cfg.TextColor,
		"display.font_family":     cfg.Display.FontFamily,
		"display.font_size":       formatFloat(cfg.Display.FontSize),
		"display.time_font_size":  formatFloat(cfg.Display.TimeFontSize),
		"display.layout":          cfg.Display.Layout,
		"display.date_format":     cfg.Display.DateFormat,
	})
}

// ImportFromYAML imports settings from a legacy config.yaml file into the
// store. Called once on first run. Silently succeeds if the YAML file doesn't
// exist (fresh install, not an upgrade).
func (m *Manager) ImportFromYAML(yamlPath string, logger *slog.Logger) error {
	legacy, err := loadYAMLConfig(yamlPath)
	if err != nil {
		// File doesn't exist → nothing to import.
		return nil
	}

	if logger != nil {
		logger.Info("settings: importing legacy config.yaml", "path", yamlPath)
	}

	if err := m.Update(*legacy); err != nil {
		return fmt.Errorf("settings: import yaml: %w", err)
	}
	return nil
}

// ── Validation ────────────────────────────────────────────────────────────────

// Validate checks if the configuration is valid.
func (c *Config) Validate() error {
	if !isValidHexColor(c.TextColor) {
		return fmt.Errorf("%w: text_color=%s", ErrInvalidColor, c.TextColor)
	}
	if !isValidHexColor(c.BackgroundColor) {
		return fmt.Errorf("%w: background_color=%s", ErrInvalidColor, c.BackgroundColor)
	}
	if err := isBareFilename(c.BackgroundImage); c.BackgroundImage != "" && err != nil {
		return fmt.Errorf("invalid background_image: %w", err)
	}
	for _, p := range c.ImagePaths {
		if err := isBareFilename(p); err != nil {
			return fmt.Errorf("invalid image path %q: %w", p, err)
		}
	}
	return nil
}

// isBareFilename rejects anything that is not a plain filename: no slashes,
// no null bytes, no path separators. This confines file fields to their
// expected directory and prevents directory traversal.
func isBareFilename(name string) error {
	if name != filepath.Base(name) || strings.ContainsAny(name, "/\\\x00") {
		return fmt.Errorf("must be a plain filename with no path separators")
	}
	return nil
}

// GetTextColor parses and returns the text color.
func (c *Config) GetTextColor() (color.Color, error) {
	return parseHexColor(c.TextColor)
}

// GetBackgroundColor parses and returns the background color.
func (c *Config) GetBackgroundColor() (color.Color, error) {
	return parseHexColor(c.BackgroundColor)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

func (m *Manager) notifyWatchers(cfg Config) {
	for _, ch := range m.watchers {
		select {
		case ch <- cfg:
		default:
		}
	}
}

func isValidHexColor(s string) bool {
	return hexColorRegex.MatchString(s)
}

func parseHexColor(hex string) (color.Color, error) {
	if !isValidHexColor(hex) {
		return nil, ErrInvalidColor
	}
	var r, g, b uint8
	if _, err := fmt.Sscanf(hex, "#%02x%02x%02x", &r, &g, &b); err != nil {
		return nil, err
	}
	return color.RGBA{R: r, G: g, B: b, A: 255}, nil
}

func getOrDefault(m map[string]string, key, def string) string {
	if v, ok := m[key]; ok && v != "" {
		return v
	}
	return def
}

func parseFloat(s string) float64 {
	var f float64
	_, _ = fmt.Sscanf(s, "%f", &f)
	return f
}

func formatFloat(f float64) string {
	return fmt.Sprintf("%g", f)
}
