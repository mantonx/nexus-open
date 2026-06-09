// Package builtin contains built-in modules compiled into the host
package builtin

import (
	"image"
	"image/color"
	"math"
	"time"

	"github.com/fogleman/gg"
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
	zoneW      int // pixel width of this zone, injected via _zone_width
	zoneH      int // pixel height of this zone, injected via _zone_height
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

	// Phase-lock the blink to wall-clock seconds so sampler jitter can't desync it.
	colonVisible := !m.blinkColon || now.Second()%2 == 0

	timeVal, timeSuffix := m.formatTimeSplit(now, colonVisible, false)
	timeValSec, _ := m.formatTimeSplit(now, colonVisible, true)

	switch m.face {
	case ClockFaceTimeOnly:
		return plugin.Payload{
			Primary:   timeVal + timeSuffix,
			Value:     timeVal,
			ValueUnit: timeSuffix,
			Severity:  plugin.SeverityOK,
			TTL:       2 * time.Second,
			Icon:      "clock",
			Timestamp: now,
		}, nil

	case ClockFaceWithSeconds:
		return plugin.Payload{
			Primary:   timeValSec + timeSuffix,
			Value:     timeValSec,
			ValueUnit: timeSuffix,
			Secondary: now.Format("Mon, Jan 02"),
			Severity:  plugin.SeverityOK,
			TTL:       2 * time.Second,
			Icon:      "clock",
			Timestamp: now,
		}, nil

	case ClockFaceDateFirst:
		return plugin.Payload{
			Primary:   now.Format("Mon, Jan 02"),
			Secondary: timeVal + timeSuffix,
			Severity:  plugin.SeverityOK,
			TTL:       2 * time.Second,
			Icon:      "clock",
			Timestamp: now,
		}, nil

	default: // ClockFaceDigital
		return plugin.Payload{
			Primary:   timeVal + timeSuffix,
			Value:     timeVal,
			ValueUnit: timeSuffix,
			Secondary: now.Format("Mon, Jan 02"),
			Severity:  plugin.SeverityOK,
			TTL:       2 * time.Second,
			Icon:      "clock",
			Timestamp: now,
		}, nil
	}
}

// colonHidden uses a plain space for the "off" phase of the blink.
// U+2236 (∶ RATIO) was the first choice but GoRegular maps it to .notdef,
// producing a missing-glyph box. A regular space is universally supported
// and causes only a ~1px width shift at display sizes — far less jarring than a box.
const colonHidden = " "

// formatTimeSplit returns the digits and the AM/PM suffix as separate strings.
// For 24-hour format the suffix is always empty.
func (m *ClockPlugin) formatTimeSplit(t time.Time, showColon bool, withSeconds bool) (digits, suffix string) {
	colon := ":"
	if !showColon {
		colon = colonHidden
	}
	switch m.format {
	case ClockFormat24Hour:
		base := t.Format("15") + colon + t.Format("04")
		if withSeconds {
			return base + colon + t.Format("05"), ""
		}
		return base, ""
	default: // 12-hour
		base := t.Format("3") + colon + t.Format("04")
		if withSeconds {
			return base + colon + t.Format("05"), t.Format(" PM")
		}
		return base, t.Format(" PM")
	}
}

// ── Analog face ───────────────────────────────────────────────────────────────

func (m *ClockPlugin) sampleAnalog(now time.Time) (plugin.Payload, error) {
	w, h := m.zoneW, m.zoneH
	if w == 0 || h == 0 {
		w, h = 160, 48 // safe fallback until Configure delivers zone dimensions
	}
	frame := drawAnalogFace(now, w, h)
	return plugin.Payload{
		// Primary must be non-empty when RawFrame is absent; with RawFrame set it
		// is unused by the renderer but kept for logging/debug clarity.
		Primary:   now.Format("3:04 PM"),
		RawFrame:  frame,
		Severity:  plugin.SeverityOK,
		TTL:       2 * time.Second,
		Timestamp: now,
	}, nil
}

// drawAnalogFace renders an analog clock into a width×height RGBA pixel buffer.
// The clock is drawn as a circle centred and sized to fit the zone height, with
// hour marks, and hour/minute/second hands.
func drawAnalogFace(now time.Time, width, height int) []byte {
	dc := gg.NewContext(width, height)

	// Background: transparent black so the zone's own bg shows through.
	dc.SetColor(color.RGBA{0, 0, 0, 0})
	dc.Clear()

	// The clock circle is square: diameter = height minus padding on each side.
	const pad = 3.0
	diameter := float64(height) - pad*2
	radius := diameter / 2

	// Centre the circle horizontally and vertically.
	cx := float64(width) / 2
	cy := float64(height) / 2

	// Face fill — subtle semi-transparent dark circle.
	dc.DrawCircle(cx, cy, radius)
	dc.SetRGBA(0, 0, 0, 0.55)
	dc.Fill()

	// Rim.
	dc.DrawCircle(cx, cy, radius)
	dc.SetRGBA(1, 1, 1, 0.70)
	dc.SetLineWidth(0.8)
	dc.Stroke()

	// Hour tick marks — 12 short radial lines.
	for i := 0; i < 12; i++ {
		angle := float64(i)/12*2*math.Pi - math.Pi/2
		outer := radius - 1.5
		inner := radius - 4.5
		x1 := cx + math.Cos(angle)*outer
		y1 := cy + math.Sin(angle)*outer
		x2 := cx + math.Cos(angle)*inner
		y2 := cy + math.Sin(angle)*inner
		dc.DrawLine(x1, y1, x2, y2)
		dc.SetRGBA(1, 1, 1, 0.80)
		dc.SetLineWidth(0.8)
		dc.Stroke()
	}

	h := float64(now.Hour()%12) + float64(now.Minute())/60
	min := float64(now.Minute()) + float64(now.Second())/60
	sec := float64(now.Second())

	// Hour hand.
	drawHand(dc, cx, cy, h/12*2*math.Pi-math.Pi/2, radius*0.50, 1.8, color.RGBA{255, 255, 255, 230})
	// Minute hand.
	drawHand(dc, cx, cy, min/60*2*math.Pi-math.Pi/2, radius*0.72, 1.2, color.RGBA{255, 255, 255, 210})
	// Second hand — accent red, thin.
	drawHand(dc, cx, cy, sec/60*2*math.Pi-math.Pi/2, radius*0.80, 0.7, color.RGBA{220, 60, 60, 240})

	// Centre dot.
	dc.DrawCircle(cx, cy, 1.5)
	dc.SetRGBA(1, 1, 1, 0.9)
	dc.Fill()

	img := dc.Image().(*image.RGBA)
	return img.Pix
}

func drawHand(dc *gg.Context, cx, cy, angle, length, width float64, col color.RGBA) {
	x := cx + math.Cos(angle)*length
	y := cy + math.Sin(angle)*length
	dc.DrawLine(cx, cy, x, y)
	dc.SetColor(col)
	dc.SetLineWidth(width)
	dc.SetLineCapRound()
	dc.Stroke()
}

// ── Configuration ─────────────────────────────────────────────────────────────

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
	return nil
}
