// Package config provides configuration management with validation and watching.
package config

import (
	"errors"
	"fmt"
	"image/color"
	"os"
	"path/filepath"
	"regexp"
	"sync"

	"github.com/spf13/viper"
)

// Manager handles configuration loading, validation, and watching.
type Manager struct {
	mu       sync.RWMutex
	cfg      *Config
	path     string
	watchers []chan<- Config
}

// Config holds the application configuration.
// openapi:schema Config
type Config struct {
	// openapi:description Location for weather and timezone (city, state or coordinates)
	// openapi:example Jersey City, NJ
	Location string `mapstructure:"location" json:"location"`
	// openapi:description Time format (12h or 24h)
	// openapi:enum 12h 24h
	// openapi:example 12h
	TimeFormat string `mapstructure:"time_format" json:"time_format"`
	// openapi:description Unit system (metric or imperial)
	// openapi:enum metric imperial
	// openapi:example imperial
	Unit string `mapstructure:"unit" json:"unit"`
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

// DisplayConfig holds display-specific configuration
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
	// openapi:description Layout style (dashboard, minimalist, compact, balanced)
	// openapi:enum dashboard minimalist compact balanced
	// openapi:example dashboard
	Layout string `mapstructure:"layout" json:"layout"`
}

// Default configuration values.
const (
	DefaultLocation        = "Jersey City, NJ"
	DefaultTimeFormat      = "12h"
	DefaultUnit            = "imperial"
	DefaultBackgroundColor = "#000000"
	DefaultBackgroundImage = "background.png"
	DefaultTextColor       = "#FFFFFF"
	DefaultFontFamily      = "GoRegular"
	DefaultFontSize        = 11.0
	DefaultTimeFontSize    = 14.0
	DefaultLayout          = "dashboard"
)

// Validation constants.
const (
	TimeFormat12Hour = "12h"
	TimeFormat24Hour = "24h"
	UnitMetric       = "metric"
	UnitImperial     = "imperial"
)

var (
	ErrInvalidTimeFormat = errors.New("invalid time format: must be 12h or 24h")
	ErrInvalidUnit       = errors.New("invalid unit: must be metric or imperial")
	ErrInvalidColor      = errors.New("invalid color: must be hex color (e.g. #FFFFFF)")
	ErrInvalidLocation   = errors.New("invalid location: cannot be empty")
)

var hexColorRegex = regexp.MustCompile(`^#[0-9A-Fa-f]{6}$`)

// NewManager creates a new configuration manager.
func NewManager(path string) (*Manager, error) {
	if path == "" {
		// Use default config path
		configDir, err := os.UserConfigDir()
		if err != nil {
			return nil, fmt.Errorf("failed to get config dir: %w", err)
		}
		path = filepath.Join(configDir, "nexus-open", "config.yaml")
	}

	m := &Manager{
		path:     path,
		watchers: make([]chan<- Config, 0),
	}

	// Load initial configuration
	if err := m.Load(); err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
	}

	return m, nil
}

// Load reads configuration from file or creates default if not exists.
func (m *Manager) Load() error {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Create default config if file doesn't exist
	if _, err := os.Stat(m.path); os.IsNotExist(err) {
		if err := m.createDefaultConfig(); err != nil {
			return err
		}
	}

	// Load configuration
	viper.SetConfigFile(m.path)
	viper.SetConfigType("yaml")

	if err := viper.ReadInConfig(); err != nil {
		return fmt.Errorf("failed to read config: %w", err)
	}

	var cfg Config
	if err := viper.Unmarshal(&cfg); err != nil {
		return fmt.Errorf("failed to unmarshal config: %w", err)
	}

	// Apply defaults for missing display config values
	if cfg.Display.FontFamily == "" {
		cfg.Display.FontFamily = DefaultFontFamily
	}
	if cfg.Display.FontSize == 0 {
		cfg.Display.FontSize = DefaultFontSize
	}
	if cfg.Display.TimeFontSize == 0 {
		cfg.Display.TimeFontSize = DefaultTimeFontSize
	}
	if cfg.Display.Layout == "" {
		cfg.Display.Layout = DefaultLayout
	}

	// Validate
	if err := cfg.Validate(); err != nil {
		return fmt.Errorf("invalid config: %w", err)
	}

	m.cfg = &cfg
	return nil
}

// Get returns a copy of the current configuration (thread-safe).
func (m *Manager) Get() Config {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return *m.cfg
}

// Update atomically updates the configuration and saves to disk.
func (m *Manager) Update(cfg Config) error {
	// Validate first
	if err := cfg.Validate(); err != nil {
		return err
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	m.cfg = &cfg

	// Save to disk
	if err := m.save(); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}

	// Notify watchers
	m.notifyWatchers(*m.cfg)

	return nil
}

// Watch registers a channel to receive configuration updates.
func (m *Manager) Watch(ch chan<- Config) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.watchers = append(m.watchers, ch)
}

// save writes current configuration to disk (caller must hold lock).
func (m *Manager) save() error {
	// Ensure directory exists
	if err := os.MkdirAll(filepath.Dir(m.path), 0755); err != nil {
		return err
	}

	viper.SetConfigFile(m.path)
	viper.SetConfigType("yaml")

	// Set values
	viper.Set("location", m.cfg.Location)
	viper.Set("time_format", m.cfg.TimeFormat)
	viper.Set("unit", m.cfg.Unit)
	viper.Set("background_color", m.cfg.BackgroundColor)
	viper.Set("background_image", m.cfg.BackgroundImage)
	viper.Set("text_color", m.cfg.TextColor)
	viper.Set("image_paths", m.cfg.ImagePaths)
	viper.Set("display", m.cfg.Display)

	return viper.WriteConfig()
}

// createDefaultConfig creates a new configuration file with defaults.
func (m *Manager) createDefaultConfig() error {
	defaultCfg := &Config{
		Location:        DefaultLocation,
		TimeFormat:      DefaultTimeFormat,
		Unit:            DefaultUnit,
		BackgroundColor: DefaultBackgroundColor,
		BackgroundImage: DefaultBackgroundImage,
		TextColor:       DefaultTextColor,
		ImagePaths:      []string{},
		Display: DisplayConfig{
			FontFamily:   DefaultFontFamily,
			FontSize:     DefaultFontSize,
			TimeFontSize: DefaultTimeFontSize,
			Layout:       DefaultLayout,
		},
	}

	m.cfg = defaultCfg
	return m.save()
}

// notifyWatchers sends config updates to all registered watchers.
func (m *Manager) notifyWatchers(cfg Config) {
	for _, ch := range m.watchers {
		select {
		case ch <- cfg:
		default:
			// Don't block if watcher is slow
		}
	}
}

// Validate checks if the configuration is valid.
func (c *Config) Validate() error {
	// Validate time format
	if c.TimeFormat != TimeFormat12Hour && c.TimeFormat != TimeFormat24Hour {
		return ErrInvalidTimeFormat
	}

	// Validate unit
	if c.Unit != UnitMetric && c.Unit != UnitImperial {
		return ErrInvalidUnit
	}

	// Validate colors
	if !isValidHexColor(c.TextColor) {
		return fmt.Errorf("%w: text_color=%s", ErrInvalidColor, c.TextColor)
	}
	if !isValidHexColor(c.BackgroundColor) {
		return fmt.Errorf("%w: background_color=%s", ErrInvalidColor, c.BackgroundColor)
	}

	// Validate location
	if c.Location == "" {
		return ErrInvalidLocation
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

// isValidHexColor checks if a string is a valid hex color.
func isValidHexColor(s string) bool {
	return hexColorRegex.MatchString(s)
}

// parseHexColor converts hex color string to color.Color.
func parseHexColor(hex string) (color.Color, error) {
	if !isValidHexColor(hex) {
		return nil, ErrInvalidColor
	}

	var r, g, b uint8
	_, err := fmt.Sscanf(hex, "#%02x%02x%02x", &r, &g, &b)
	if err != nil {
		return nil, err
	}

	return color.RGBA{R: r, G: g, B: b, A: 255}, nil
}
