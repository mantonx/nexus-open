// Package plugin defines the interface and types for Nexus Open plugins.
package plugin

import "time"

// FieldType names the data type of a config field, used by the Flutter UI to
// render the appropriate input widget.
type FieldType string

const (
	FieldTypeString FieldType = "string"
	FieldTypeEnum   FieldType = "enum"
	FieldTypeInt    FieldType = "int"
	FieldTypeBool   FieldType = "bool"
	FieldTypeColor  FieldType = "color"
)

// FieldOption is a single selectable value for an enum field.
type FieldOption struct {
	Value string `json:"value"`
	Label string `json:"label"`
}

// ShowIfCondition hides a field unless another field's current value matches.
// Both key and not_eq may be set: field is visible when cfg[Key] != NotEq.
type ShowIfCondition struct {
	Key   string `json:"key"`
	NotEq string `json:"not_eq"`
}

// ConfigField describes one configurable parameter of a plugin.
type ConfigField struct {
	Key     string           `json:"key"`
	Label   string           `json:"label"`
	Type    FieldType        `json:"type"`
	Default any              `json:"default,omitempty"`
	Options []FieldOption    `json:"options,omitempty"` // enum only
	Min     *int             `json:"min,omitempty"`     // int only
	Max     *int             `json:"max,omitempty"`     // int only
	Help    string           `json:"help,omitempty"`
	ShowIf  *ShowIfCondition `json:"show_if,omitempty"`
}

// ConfigSchema is the full schema for a plugin's configurable fields.
type ConfigSchema struct {
	Fields []ConfigField `json:"fields"`
}

// Descriptor contains metadata about a plugin.
type Descriptor struct {
	Name        string       `json:"name"`         // Human-readable name (e.g., "CPU Temperature")
	Version     string       `json:"version"`      // Semantic version (e.g., "1.0.0")
	Author      string       `json:"author"`       // Author name or organization
	Description string       `json:"description"`  // Brief description of functionality
	Icon        string       `json:"icon"`         // Default icon identifier (Font Awesome or emoji)
	RefreshMs   int          `json:"refresh_ms"`   // Recommended refresh interval in milliseconds
	HasGraph    bool         `json:"has_graph,omitempty"` // True if this plugin renders a sparkline/graph
	Schema      ConfigSchema `json:"config_schema"` // Declared configurable fields
}

// Payload represents data returned by a module to be rendered in a zone.
type Payload struct {
	// Title - Optional zone header (usually omitted for space)
	Title string `json:"title,omitempty"`

	// Primary - Main value displayed (14-16px bold)
	// Examples: "42°C", "↓58 MB/s", "Now Playing"
	Primary string `json:"primary"`

	// Secondary - Subtext or context (10px, muted color)
	// Examples: "Load 31%", "Albany ☀️", "Radiohead"
	Secondary string `json:"secondary,omitempty"`

	// Spark - Sparkline data (normalized 0.0-1.0, max 60 points)
	// Rendered as small bars/line at bottom of zone
	Spark []float32 `json:"spark,omitempty"`

	// GraphType - How to render sparkline data: "sparkline", "bar", "area"
	// Defaults to "sparkline" if empty
	GraphType GraphType `json:"graph_type,omitempty"`

	// Severity - Visual severity indicator: "ok", "warn", "crit"
	// Affects primary text color
	Severity Severity `json:"severity,omitempty"`

	// TTL - Cache lifetime
	// Host will re-use this payload until TTL expires
	TTL time.Duration `json:"ttl,omitempty"`

	// Icon - Icon override (Font Awesome name or emoji)
	Icon string `json:"icon,omitempty"`

	// Progress - Progress bar value (0.0-1.0)
	// Rendered as horizontal bar (for media playback, etc.)
	Progress float32 `json:"progress,omitempty"`

	// Timestamp - When this payload was generated
	Timestamp time.Time `json:"timestamp,omitempty"`

	// LineSpacing - Spacing between lines for multi-line Primary text (in pixels)
	// Defaults to 24 if not specified. Use higher values (e.g., 28-30) for more breathing room
	LineSpacing int `json:"line_spacing,omitempty"`

	// LabelPosition - Where to position the secondary label relative to primary
	// Defaults to "below" if not specified. Options: "below", "right"
	LabelPosition LabelPosition `json:"label_position,omitempty"`

	// LabelOffsetX - Horizontal offset for label positioning (in pixels)
	// Positive moves right, negative moves left. Applied after base positioning.
	LabelOffsetX int `json:"label_offset_x,omitempty"`

	// LabelOffsetY - Vertical offset for label positioning (in pixels)
	// Positive moves down, negative moves up. Applied after base positioning.
	LabelOffsetY int `json:"label_offset_y,omitempty"`

	// NormalizeGraph - If true, graph data is normalized to fill from baseline
	// Set to true for graphs where relative changes matter (network bandwidth)
	// Set to false for graphs where absolute values matter (temperatures)
	// Defaults to false (no normalization, show absolute 0-1 values)
	NormalizeGraph bool `json:"normalize_graph,omitempty"`

	// GraphBgOpacity - Background fill opacity for graphs (0-100)
	// 0 = fully transparent, 100 = fully opaque
	// If not set, uses theme default (typically very low for subtlety)
	GraphBgOpacity int `json:"graph_bg_opacity,omitempty"`

	// GraphLineOpacity - Line opacity for graphs (0-100)
	// 0 = fully transparent, 100 = fully opaque
	// If not set, uses theme default (typically low for subtlety)
	GraphLineOpacity int `json:"graph_line_opacity,omitempty"`

	// RawFrame - Pre-rendered RGBA pixel data (width×height×4 bytes, row-major).
	// When set, the renderer skips all text/graph layout and blits these pixels
	// directly. Width and height must match the zone dimensions exactly.
	// Primary may be empty when RawFrame is set.
	RawFrame []byte `json:"raw_frame,omitempty"`
}

// Severity levels for visual indication
type Severity string

const (
	SeverityOK   Severity = "ok"   // Normal operation (accent color)
	SeverityWarn Severity = "warn" // Warning threshold (yellow/orange)
	SeverityCrit Severity = "crit" // Critical state (red)
)

// GraphType specifies how sparkline data should be rendered
type GraphType string

const (
	GraphTypeSparkline GraphType = "sparkline" // Line graph (default)
	GraphTypeBar       GraphType = "bar"       // Vertical bars
	GraphTypeArea      GraphType = "area"      // Filled area under line
	GraphTypeLine      GraphType = "line"      // Thick gradient line with glow
)

// LabelPosition specifies where the secondary label should be positioned
type LabelPosition string

const (
	LabelPositionBelow LabelPosition = "below" // Below primary text (default)
	LabelPositionRight LabelPosition = "right" // To the right of primary text
)

// Validate checks if the payload meets requirements
func (p *Payload) Validate() error {
	if p.Primary == "" && len(p.RawFrame) == 0 {
		return ErrEmptyPrimary
	}

	if p.Severity != "" && p.Severity != SeverityOK && p.Severity != SeverityWarn && p.Severity != SeverityCrit {
		return ErrInvalidSeverity
	}

	if len(p.Spark) > 60 {
		return ErrSparkTooLong
	}

	for i, v := range p.Spark {
		if v < 0.0 || v > 1.0 {
			return &ErrSparkOutOfRange{Index: i, Value: v}
		}
	}

	if p.Progress < 0.0 || p.Progress > 1.0 {
		return ErrProgressOutOfRange
	}

	return nil
}

// IsExpired checks if the payload has exceeded its TTL
func (p *Payload) IsExpired() bool {
	if p.TTL == 0 {
		return false // No TTL means never expires
	}
	return time.Since(p.Timestamp) > p.TTL
}

// ConfigKeyZoneWidth and ConfigKeyZoneHeight are reserved keys injected into
// every plugin's Configure call by the sampler. Plugins may read these to
// adapt their rendering to the actual pixel dimensions of their zone.
// Do not declare these in ConfigSchema — they are host-injected, not user-editable.
const (
	ConfigKeyZoneWidth  = "_zone_width"
	ConfigKeyZoneHeight = "_zone_height"
)

// Plugin is the interface that all plugins must implement.
type Plugin interface {
	// Describe returns plugin metadata and config schema.
	Describe() (Descriptor, error)

	// Sample returns current data payload.
	Sample() (Payload, error)

	// Configure applies per-zone plugin configuration.
	// Called once at startup (with zone's stored config) and on every live edit.
	Configure(cfg map[string]any) error
}

// DetailPayload carries the pre-rendered detail overlay surfaced when a zone is tapped.
// The plugin renders its own 640×48 RGBA frame and returns it in RawFrame.
// Title is optional metadata (used for logging/accessibility; not rendered by core).
type DetailPayload struct {
	// ZoneID identifies which zone this detail belongs to.
	ZoneID string `json:"zone_id"`

	// Title is human-readable metadata about this detail view (e.g. "Jersey City — 7-Day Forecast").
	// Not rendered by the core; plugins may use it for logging or future accessibility support.
	Title string `json:"title,omitempty"`

	// RawFrame is a pre-rendered 640×48 RGBA pixel buffer (width×height×4 bytes, row-major).
	// The core blits this directly to the display. Plugins are responsible for all layout
	// and rendering. Use the gg (fogleman/gg) or image/draw packages to produce this.
	RawFrame []byte `json:"raw_frame,omitempty"`
}

// Tapper is an optional interface plugins may implement to handle zone taps.
// When a zone with on_tap:"detail" is tapped, the host type-asserts the plugin
// to Tapper and calls OnTap to retrieve the rich detail payload.
type Tapper interface {
	OnTap() (DetailPayload, error)
}
