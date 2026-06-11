// Package builtin contains built-in modules compiled into the host
package builtin

import (
	"image/color"
	"time"

	"github.com/fogleman/gg"
	"golang.org/x/image/font"

	"github.com/mantonx/nexus-open/internal/design"
	"github.com/mantonx/nexus-open/internal/fonts"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// ClockFace controls the visual style of the clock zone.
type ClockFace int

const (
	ClockFaceDigital    ClockFace = iota // "HH:MM [AM/PM]" primary, date secondary
	ClockFaceTimeOnly                    // time only, no secondary — fills zone
	ClockFaceWithSeconds                 // "HH:MM:SS [AM/PM]" primary, date secondary
	ClockFaceDateFirst                   // date primary (large), time secondary (small)
	ClockFaceAnalog                      // hand-drawn analog face rendered to RawFrame
)

// ClockFormat describes the hour convention for digital faces.
type ClockFormat int

const (
	ClockFormat12Hour ClockFormat = iota
	ClockFormat24Hour
)

// ClockPlugin displays current time and date.
type ClockPlugin struct {
	format     ClockFormat
	face       ClockFace
	blinkColon bool
	zoneW      int
	zoneH      int

	primaryFace font.Face // SizeValue (22pt) — loaded once in Configure
	unitFace    font.Face // SizeUnit (13pt)  — AM/PM suffix
	labelFace   font.Face // SizeLabel (10pt) — date line
}

// NewClock creates a new clock plugin with defaults.
func NewClock() *ClockPlugin {
	return NewClockWithFormat(ClockFormat12Hour)
}

// NewClockWithFormat creates a clock plugin with the requested hour format.
func NewClockWithFormat(format ClockFormat) *ClockPlugin {
	if format != ClockFormat24Hour {
		format = ClockFormat12Hour
	}
	return &ClockPlugin{format: format, face: ClockFaceDigital, blinkColon: true}
}

// NewClock24 returns a 24-hour clock plugin.
func NewClock24() *ClockPlugin {
	return NewClockWithFormat(ClockFormat24Hour)
}

// Describe returns plugin metadata.
func (m *ClockPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Clock",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays current time and date with configurable face style",
		Icon:        "clock",
		RefreshMs:   1000,
		Schema: plugin.ConfigSchema{
			Fields: []plugin.ConfigField{
				{
					Key:     "clock_face",
					Label:   "Face style",
					Type:    plugin.FieldTypeEnum,
					Default: "digital",
					Options: []plugin.FieldOption{
						{Value: "digital", Label: "Digital"},
						{Value: "time-only", Label: "Time only"},
						{Value: "time+seconds", Label: "With seconds"},
						{Value: "date-first", Label: "Date first"},
						{Value: "analog", Label: "Analog"},
					},
				},
				{
					Key:     "clock_format",
					Label:   "Hour format",
					Type:    plugin.FieldTypeEnum,
					Default: "12h",
					Options: []plugin.FieldOption{
						{Value: "12h", Label: "12-hour"},
						{Value: "24h", Label: "24-hour"},
					},
				},
				{
					Key:     "blink_colon",
					Label:   "Blink colon",
					Type:    plugin.FieldTypeBool,
					Default: true,
					ShowIf:  &plugin.ShowIfCondition{Key: "clock_face", NotEq: "analog"},
				},
			},
		},
	}, nil
}

// Sample returns the current time payload for the configured face.
func (m *ClockPlugin) Sample() (plugin.Payload, error) {
	now := time.Now()
	if m.face == ClockFaceAnalog {
		return m.sampleAnalog(now)
	}
	return m.sampleDigital(now)
}

// Configure applies zone-level plugin configuration.
func (m *ClockPlugin) Configure(cfg map[string]any) error {
	if v, ok := cfg[plugin.ConfigKeyZoneWidth].(int); ok {
		m.zoneW = v
	}
	if v, ok := cfg[plugin.ConfigKeyZoneHeight].(int); ok {
		m.zoneH = v
	}
	if v, ok := cfg["clock_face"].(string); ok {
		switch v {
		case "digital":
			m.face = ClockFaceDigital
		case "time-only":
			m.face = ClockFaceTimeOnly
		case "time+seconds":
			m.face = ClockFaceWithSeconds
		case "date-first":
			m.face = ClockFaceDateFirst
		case "analog":
			m.face = ClockFaceAnalog
		}
	}
	if v, ok := cfg["clock_format"].(string); ok {
		switch v {
		case "24h", "24hour", "24":
			m.format = ClockFormat24Hour
		case "12h", "12hour", "12":
			m.format = ClockFormat12Hour
		}
	}
	if v, ok := cfg["blink_colon"]; ok {
		switch val := v.(type) {
		case bool:
			m.blinkColon = val
		case string:
			m.blinkColon = val == "true" || val == "1" || val == "yes"
		}
	}
	m.loadFaces()
	return nil
}

// loadFaces loads the three font faces the clock needs. Called from Configure
// so faces are ready before the first Sample().
func (m *ClockPlugin) loadFaces() {
	fm := fonts.NewManager(nil)
	load := func(size float64) font.Face {
		if face, _, err := fm.LoadBestAvailableFont(size); err == nil {
			return face
		}
		return nil
	}
	m.primaryFace = load(float64(design.SizeValue))
	m.unitFace = load(float64(design.SizeUnit))
	m.labelFace = load(float64(design.SizeLabel))
}

// setRGBA sets the draw color from a design token with an alpha multiplier.
func setRGBA(dc *gg.Context, col color.RGBA, alpha float64) {
	dc.SetRGBA(
		float64(col.R)/255,
		float64(col.G)/255,
		float64(col.B)/255,
		float64(col.A)/255*alpha,
	)
}
