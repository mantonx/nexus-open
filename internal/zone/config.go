// Package zone implements the zone-based layout system for the display.
package zone

import (
	"fmt"
	"image/color"
)

const (
	// MaxZonesPerPage is the maximum number of zones allowed on a single page.
	MaxZonesPerPage = 6
	// DisplayWidthPx is the total pixel width of the Nexus display strip.
	DisplayWidthPx = 640
	// MinZoneWidthPx is the minimum pixel width of any zone.
	MinZoneWidthPx = 80
)

// Config represents the complete layout configuration
type Config struct {
	Name    string  `yaml:"name" json:"name"`       // Layout name
	Version string  `yaml:"version" json:"version"` // Layout version
	Theme   Theme   `yaml:"theme" json:"theme"`     // Global theme
	Pages   []Page  `yaml:"pages" json:"pages"`     // Pages (swipeable layouts)
	Nav     NavConfig `yaml:"navigation,omitempty" json:"navigation,omitempty"` // Navigation settings
}

// Page represents a single page of zones
type Page struct {
	Name  string      `yaml:"name" json:"name"`   // Page name
	Zones []ZoneConfig `yaml:"zones" json:"zones"` // Zone configurations
}

// PageInfo is a lightweight page descriptor sent to the Flutter preview UI.
type PageInfo struct {
	Name  string     `json:"name"`
	Zones []ZoneInfo `json:"zones"`
}

// ZoneInfo is a lightweight zone descriptor sent to the Flutter preview UI.
type ZoneInfo struct {
	ID    string `json:"id"`
	Width int    `json:"width"`
	OnTap string `json:"on_tap,omitempty"`
}

// ZoneConfig represents configuration for a single zone
type ZoneConfig struct {
	ID            string         `yaml:"id" json:"id"`                                           // Unique zone identifier
	Width         int            `yaml:"width" json:"width"`                                     // Zone width in pixels
	X             int            `yaml:"x,omitempty" json:"x,omitempty"`                         // X offset (auto-computed if 0)
	Plugin        string         `yaml:"plugin" json:"plugin"`                                   // Plugin endpoint (builtin:name or exec:path)
	RefreshMs     int            `yaml:"refresh_ms" json:"refresh_ms"`                           // Sampling interval
	Align         Alignment      `yaml:"align,omitempty" json:"align,omitempty"`                 // Text alignment
	ThemeOverride *Theme         `yaml:"theme_override,omitempty" json:"theme_override,omitempty"` // Per-zone theme
	PluginConfig  map[string]any `yaml:"plugin_config,omitempty" json:"plugin_config,omitempty"` // Per-zone plugin configuration
	Choices       []string       `yaml:"choices,omitempty" json:"choices,omitempty"`             // Plugin choices for cycling
	OnTap         TapAction      `yaml:"on_tap,omitempty" json:"on_tap,omitempty"`               // Tap action
}

// Theme represents visual styling
type Theme struct {
	Bg                     string `yaml:"bg" json:"bg"`                                                           // Background color (hex)
	Fg                     string `yaml:"fg" json:"fg"`                                                           // Foreground color (hex)
	Muted                  string `yaml:"muted" json:"muted"`                                                     // Muted text color (hex)
	Accent                 string `yaml:"accent" json:"accent"`                                                   // Accent color (hex)
	ZoneBg                 string `yaml:"zone_bg,omitempty" json:"zone_bg,omitempty"`                            // Zone background color (hex)
	GutterPx               int    `yaml:"gutter_px,omitempty" json:"gutter_px,omitempty"`                        // Gutter width
	FontSizePrimary        int    `yaml:"font_size_primary,omitempty" json:"font_size_primary,omitempty"`        // Primary text size
	FontSizeSecondary      int    `yaml:"font_size_secondary,omitempty" json:"font_size_secondary,omitempty"`    // Secondary text size
	GraphBgOpacity         int    `yaml:"graph_bg_opacity,omitempty" json:"graph_bg_opacity,omitempty"`          // Graph background opacity (0-100)
	GraphLineOpacity       int    `yaml:"graph_line_opacity,omitempty" json:"graph_line_opacity,omitempty"`      // Graph line opacity (0-100)
}

// DefaultTheme returns the default dark theme
func DefaultTheme() Theme {
	return Theme{
		Bg:                "#000000",
		Fg:                "#EAEAEA",
		Muted:             "#B8BDC2",
		Accent:            "#00C8FF",
		ZoneBg:            "#000000",
		GutterPx:          2,
		FontSizePrimary:   24,
		FontSizeSecondary: 9,
		GraphBgOpacity:    0, // new renderer uses gradient fill — this is unused
		GraphLineOpacity:  0, // new renderer uses fixed 0.95 line opacity
	}
}

// ParseColor converts hex color string to color.RGBA
func (t *Theme) ParseColor(hex string) (color.RGBA, error) {
	if len(hex) != 7 || hex[0] != '#' {
		return color.RGBA{}, fmt.Errorf("invalid color format: %s (expected #RRGGBB)", hex)
	}

	var r, g, b uint8
	_, err := fmt.Sscanf(hex, "#%02x%02x%02x", &r, &g, &b)
	if err != nil {
		return color.RGBA{}, fmt.Errorf("failed to parse color %s: %w", hex, err)
	}

	return color.RGBA{R: r, G: g, B: b, A: 255}, nil
}

// GetBgColor returns the background color as color.RGBA
func (t *Theme) GetBgColor() color.RGBA {
	c, _ := t.ParseColor(t.Bg)
	return c
}

// GetFgColor returns the foreground color as color.RGBA
func (t *Theme) GetFgColor() color.RGBA {
	c, _ := t.ParseColor(t.Fg)
	return c
}

// GetMutedColor returns the muted color as color.RGBA
func (t *Theme) GetMutedColor() color.RGBA {
	c, _ := t.ParseColor(t.Muted)
	return c
}

// GetAccentColor returns the accent color as color.RGBA
func (t *Theme) GetAccentColor() color.RGBA {
	c, _ := t.ParseColor(t.Accent)
	return c
}

// GetZoneBgColor returns the zone background color as color.RGBA
func (t *Theme) GetZoneBgColor() color.RGBA {
	if t.ZoneBg == "" {
		// Fallback to main background if not set
		return t.GetBgColor()
	}
	c, _ := t.ParseColor(t.ZoneBg)
	return c
}

// NavConfig represents navigation settings
type NavConfig struct {
	SwipeEnabled       bool `yaml:"swipe_enabled" json:"swipe_enabled"`
	AutoRotate         bool `yaml:"auto_rotate" json:"auto_rotate"`
	AutoRotateIntervalS int `yaml:"auto_rotate_interval_s,omitempty" json:"auto_rotate_interval_s,omitempty"`
}

// Alignment options for zone content
type Alignment string

const (
	AlignLeft   Alignment = "left"
	AlignCenter Alignment = "center"
	AlignRight  Alignment = "right"
)

// TapAction defines what happens when a zone is tapped
type TapAction string

const (
	TapActionNone   TapAction = "none"
	TapActionCycle  TapAction = "cycle"  // Cycle through plugin choices
	TapActionDetail TapAction = "detail" // Show plugin detail overlay
)

// Severity display colors — fixed values that match the Corsair design language.
var (
	SeverityColorWarn = color.RGBA{R: 255, G: 176, B: 32, A: 255}
	SeverityColorCrit = color.RGBA{R: 255, G: 68, B: 68, A: 255}
)

// Validate checks if the configuration is valid
func (c *Config) Validate() error {
	if c.Name == "" {
		return fmt.Errorf("config name is required")
	}

	if len(c.Pages) == 0 {
		return fmt.Errorf("at least one page is required")
	}

	for i, page := range c.Pages {
		if err := page.Validate(); err != nil {
			return fmt.Errorf("page %d (%s): %w", i, page.Name, err)
		}
	}

	return nil
}

// Validate checks if a page configuration is valid
func (p *Page) Validate() error {
	if p.Name == "" {
		return fmt.Errorf("page name is required")
	}

	if len(p.Zones) == 0 {
		return fmt.Errorf("at least one zone is required")
	}

	if len(p.Zones) > MaxZonesPerPage {
		return fmt.Errorf("page has %d zones, maximum is %d", len(p.Zones), MaxZonesPerPage)
	}

	totalWidth := 0
	for i, zone := range p.Zones {
		if err := zone.Validate(); err != nil {
			return fmt.Errorf("zone %d (%s): %w", i, zone.ID, err)
		}
		totalWidth += zone.Width
	}

	if totalWidth != DisplayWidthPx {
		return fmt.Errorf("zone widths must sum to %d, got %d", DisplayWidthPx, totalWidth)
	}

	return nil
}

// RedistributeWidths sets every zone's width to an equal share of totalPx,
// respecting the floorPx minimum. Remainder pixels are distributed one each
// to the leading zones so the total always equals totalPx exactly.
// Returns an error if the zones cannot fit within totalPx at floorPx each.
func (p *Page) RedistributeWidths(totalPx, floorPx int) error {
	n := len(p.Zones)
	if n == 0 {
		return nil
	}
	if n*floorPx > totalPx {
		return fmt.Errorf("cannot fit %d zones at %dpx floor into %dpx total", n, floorPx, totalPx)
	}
	base := totalPx / n
	extra := totalPx % n
	for i := range p.Zones {
		p.Zones[i].Width = base
		if i < extra {
			p.Zones[i].Width++
		}
	}
	return nil
}

// Validate checks if a zone configuration is valid
func (z *ZoneConfig) Validate() error {
	if z.ID == "" {
		return fmt.Errorf("zone id is required")
	}

	if z.Width < 80 {
		return fmt.Errorf("zone width must be at least 80px (got %d)", z.Width)
	}

	if z.Width > 640 {
		return fmt.Errorf("zone width must be at most 640px (got %d)", z.Width)
	}

	if z.Plugin == "" {
		return fmt.Errorf("zone plugin is required")
	}

	if z.RefreshMs < 100 {
		return fmt.Errorf("refresh interval must be at least 100ms (got %d)", z.RefreshMs)
	}

	if z.Align != "" && z.Align != AlignLeft && z.Align != AlignCenter && z.Align != AlignRight {
		return fmt.Errorf("invalid alignment: %s (must be left, center, or right)", z.Align)
	}

	// Default to center if not specified
	if z.Align == "" {
		z.Align = AlignCenter
	}

	return nil
}

// ComputeOffsets calculates X offsets for zones if not specified
func (p *Page) ComputeOffsets() {
	x := 0
	for i := range p.Zones {
		if p.Zones[i].X == 0 {
			p.Zones[i].X = x
		}
		x += p.Zones[i].Width
	}
}

// CopyConfig returns a deep copy of cfg without JSON round-tripping.
// Only the two reference types in ZoneConfig need explicit copying:
//   - PluginConfig map[string]any  — shallow map copy (values are JSON scalars)
//   - ThemeOverride *Theme         — copy the pointed-to struct
func CopyConfig(cfg *Config) *Config {
	if cfg == nil {
		return nil
	}
	out := *cfg // copy all value fields (Name, Version, Theme, Nav)
	out.Pages = make([]Page, len(cfg.Pages))
	for i, p := range cfg.Pages {
		np := Page{Name: p.Name}
		np.Zones = make([]ZoneConfig, len(p.Zones))
		for j, z := range p.Zones {
			nz := z // copy all value fields
			if z.PluginConfig != nil {
				nz.PluginConfig = make(map[string]any, len(z.PluginConfig))
				for k, v := range z.PluginConfig {
					nz.PluginConfig[k] = v
				}
			}
			if z.ThemeOverride != nil {
				t := *z.ThemeOverride
				nz.ThemeOverride = &t
			}
			if z.Choices != nil {
				nz.Choices = make([]string, len(z.Choices))
				copy(nz.Choices, z.Choices)
			}
			np.Zones[j] = nz
		}
		out.Pages[i] = np
	}
	return &out
}
