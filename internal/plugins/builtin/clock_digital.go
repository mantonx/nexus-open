package builtin

import (
	"image"
	"time"

	"github.com/fogleman/gg"

	"github.com/mantonx/nexus-open/internal/design"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// sampleDigital renders all digital faces to RawFrame.
func (m *ClockPlugin) sampleDigital(now time.Time) (plugin.Payload, error) {
	w, h := m.zoneW, m.zoneH
	if w == 0 || h == 0 {
		w, h = 160, 48
	}

	// Phase-lock the blink to wall-clock seconds so sampler jitter can't desync it.
	colonVisible := !m.blinkColon || now.Second()%2 == 0

	frame := m.drawDigitalFace(now, w, h, colonVisible)

	return plugin.Payload{
		Primary:   now.Format("3:04 PM"), // non-empty for logging/debug
		RawFrame:  frame,
		Severity:  plugin.SeverityOK,
		TTL:       2 * time.Second,
		Icon:      "clock",
		Timestamp: now,
	}, nil
}

// drawDigitalFace renders the appropriate digital layout into a width×height RGBA buffer.
func (m *ClockPlugin) drawDigitalFace(now time.Time, width, height int, colonVisible bool) []byte {
	dc := gg.NewContext(width, height)
	dc.SetColor(design.ScreenBg)
	dc.Clear()

	switch m.face {
	case ClockFaceTimeOnly:
		m.drawTimeOnly(dc, now, width, colonVisible)
	case ClockFaceWithSeconds:
		m.drawTimeWithSeconds(dc, now, width, height, colonVisible)
	case ClockFaceDateFirst:
		m.drawDateFirst(dc, now, width, colonVisible)
	default: // ClockFaceDigital
		m.drawDigital(dc, now, width, colonVisible)
	}

	img := dc.Image().(*image.RGBA)
	out := make([]byte, len(img.Pix))
	copy(out, img.Pix)
	return out
}

// drawDigital renders "HH:MM [AM/PM]" at the value baseline, with date label above.
func (m *ClockPlugin) drawDigital(dc *gg.Context, now time.Time, w int, colonVisible bool) {
	m.drawTimeStr(dc, now, float64(w), float64(design.ValueBaselineY), colonVisible)
	m.drawDateLabel(dc, now.Format("Mon, Jan 02"), float64(w))
}

// drawTimeOnly renders the time string vertically centred (no date label).
func (m *ClockPlugin) drawTimeOnly(dc *gg.Context, now time.Time, w int, colonVisible bool) {
	h := float64(m.zoneH)
	if h == 0 {
		h = 48
	}
	baseline := h/2 + 8 // approximate cap-height centre for SizeValue
	m.drawTimeStr(dc, now, float64(w), baseline, colonVisible)
}

// drawTimeWithSeconds renders "HH:MM:SS [AM/PM]" with date label above.
func (m *ClockPlugin) drawTimeWithSeconds(dc *gg.Context, now time.Time, w, _ int, colonVisible bool) {
	m.drawTimeStrSeconds(dc, now, float64(w), colonVisible)
	m.drawDateLabel(dc, now.Format("Mon, Jan 02"), float64(w))
}

// drawDateFirst renders the date large at top (primary) and time small below.
func (m *ClockPlugin) drawDateFirst(dc *gg.Context, now time.Time, w int, colonVisible bool) {
	W := float64(w)

	if m.primaryFace != nil {
		dc.SetFontFace(m.primaryFace)
	}
	setRGBA(dc, design.Value, 1)
	dc.DrawStringAnchored(now.Format("Mon, Jan 02"), W/2, float64(design.LabelBaselineY), 0.5, 0)

	if m.labelFace != nil {
		dc.SetFontFace(m.labelFace)
	}
	setRGBA(dc, design.Label, 1)
	h12, suffix := m.timeParts(now, colonVisible)
	dc.DrawStringAnchored(h12+suffix, W/2, float64(design.ValueBaselineY), 0.5, 0)
}

// drawTimeStr renders "HH:MM [AM/PM]" with the colon at a fixed pixel position.
// Hours, colon, and minutes are drawn as separate calls so the minutes never
// shift when the colon blinks — the colon is invisible (alpha=0) rather than replaced.
func (m *ClockPlugin) drawTimeStr(dc *gg.Context, now time.Time, W, baseline float64, colonVisible bool) {
	if m.primaryFace == nil {
		return
	}
	dc.SetFontFace(m.primaryFace)

	left, right, suffix := m.timeSegments(now)

	lw, _ := dc.MeasureString(left)
	cw, _ := dc.MeasureString(":")
	rw, _ := dc.MeasureString("00") // canonical width — minutes are always 2 digits

	var suffixW float64
	if suffix != "" && m.unitFace != nil {
		dc.SetFontFace(m.unitFace)
		suffixW, _ = dc.MeasureString(suffix)
		dc.SetFontFace(m.primaryFace)
	}

	startX := (W - (lw + cw + rw + suffixW)) / 2

	colonAlpha := 1.0
	if !colonVisible {
		colonAlpha = 0
	}

	setRGBA(dc, design.Value, 1)
	dc.DrawString(left, startX, baseline)

	setRGBA(dc, design.Value, colonAlpha)
	dc.DrawString(":", startX+lw, baseline)

	setRGBA(dc, design.Value, 1)
	dc.DrawString(right, startX+lw+cw, baseline)

	if suffix != "" && m.unitFace != nil {
		dc.SetFontFace(m.unitFace)
		setRGBA(dc, design.Unit, 1)
		dc.DrawString(suffix, startX+lw+cw+rw, baseline)
	}
}

// drawTimeStrSeconds renders "HH:MM:SS [AM/PM]" with fixed-position colons.
func (m *ClockPlugin) drawTimeStrSeconds(dc *gg.Context, now time.Time, W float64, colonVisible bool) {
	if m.primaryFace == nil {
		return
	}
	dc.SetFontFace(m.primaryFace)

	left, mid, right, suffix := m.timeSegmentsSec(now)

	lw, _ := dc.MeasureString(left)
	cw, _ := dc.MeasureString(":")
	mw, _ := dc.MeasureString("00")
	rw, _ := dc.MeasureString("00")

	var suffixW float64
	if suffix != "" && m.unitFace != nil {
		dc.SetFontFace(m.unitFace)
		suffixW, _ = dc.MeasureString(suffix)
		dc.SetFontFace(m.primaryFace)
	}

	startX := (W - (lw + cw + mw + cw + rw + suffixW)) / 2
	baseline := float64(design.ValueBaselineY)

	colonAlpha := 1.0
	if !colonVisible {
		colonAlpha = 0
	}

	setRGBA(dc, design.Value, 1)
	dc.DrawString(left, startX, baseline)

	setRGBA(dc, design.Value, colonAlpha)
	dc.DrawString(":", startX+lw, baseline)

	setRGBA(dc, design.Value, 1)
	dc.DrawString(mid, startX+lw+cw, baseline)

	setRGBA(dc, design.Value, colonAlpha)
	dc.DrawString(":", startX+lw+cw+mw, baseline)

	setRGBA(dc, design.Value, 1)
	dc.DrawString(right, startX+lw+cw+mw+cw, baseline)

	if suffix != "" && m.unitFace != nil {
		dc.SetFontFace(m.unitFace)
		setRGBA(dc, design.Unit, 1)
		dc.DrawString(suffix, startX+lw+cw+mw+cw+rw, baseline)
	}
}

// drawDateLabel draws the date string centred at the label baseline (y=15).
func (m *ClockPlugin) drawDateLabel(dc *gg.Context, text string, W float64) {
	if m.labelFace == nil {
		return
	}
	dc.SetFontFace(m.labelFace)
	setRGBA(dc, design.Label, 1)
	dc.DrawStringAnchored(text, W/2, float64(design.LabelBaselineY), 0.5, 0)
}

// timeSegments returns the three drawable pieces of an HH:MM string.
func (m *ClockPlugin) timeSegments(t time.Time) (left, right, suffix string) {
	if m.format == ClockFormat24Hour {
		return t.Format("15"), t.Format("04"), ""
	}
	return t.Format("3"), t.Format("04"), t.Format(" PM")
}

// timeSegmentsSec returns the four drawable pieces of an HH:MM:SS string.
func (m *ClockPlugin) timeSegmentsSec(t time.Time) (left, mid, right, suffix string) {
	if m.format == ClockFormat24Hour {
		return t.Format("15"), t.Format("04"), t.Format("05"), ""
	}
	return t.Format("3"), t.Format("04"), t.Format("05"), t.Format(" PM")
}

// timeParts returns a fused time string and suffix for faces that render the
// time in a single DrawString call (e.g. the small time line in drawDateFirst).
func (m *ClockPlugin) timeParts(t time.Time, colonVisible bool) (digits, suffix string) {
	colon := ":"
	if !colonVisible {
		colon = " "
	}
	if m.format == ClockFormat24Hour {
		return t.Format("15") + colon + t.Format("04"), ""
	}
	return t.Format("3") + colon + t.Format("04"), t.Format(" PM")
}
