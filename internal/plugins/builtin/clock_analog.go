package builtin

import (
	"image"
	"image/color"
	"math"
	"time"

	"github.com/fogleman/gg"

	"github.com/mantonx/nexus-open/internal/design"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// sampleAnalog renders the analog face to RawFrame.
func (m *ClockPlugin) sampleAnalog(now time.Time) (plugin.Payload, error) {
	w, h := m.zoneW, m.zoneH
	if w == 0 || h == 0 {
		w, h = 160, 48
	}
	return plugin.Payload{
		Primary:  now.Format("3:04 PM"),
		RawFrame: m.drawAnalogFace(now, w, h),
		Severity: plugin.SeverityOK,
		TTL:       2 * time.Second,
		Timestamp: now,
	}, nil
}

// drawAnalogFace renders a hybrid analog+digital clock into a width×height RGBA buffer.
// The clock face is left-aligned and sized to fill the full zone height.
// A digital time readout occupies the remaining horizontal space on the right.
func (m *ClockPlugin) drawAnalogFace(now time.Time, width, height int) []byte {
	format := m.format
	dc := gg.NewContext(width, height)
	dc.SetColor(design.ScreenBg)
	dc.Clear()

	const pad = 2.0
	radius := float64(height)/2 - pad
	cx := radius + pad
	cy := float64(height) / 2

	// Face fill.
	dc.DrawCircle(cx, cy, radius)
	dc.SetRGBA(0, 0, 0, 0.55)
	dc.Fill()

	// Rim.
	dc.DrawCircle(cx, cy, radius)
	dc.SetRGBA(1, 1, 1, 0.75)
	dc.SetLineWidth(1.0)
	dc.Stroke()

	// Cardinal tick marks at 12, 3, 6, 9 — thick and high-contrast at this scale.
	for i := 0; i < 4; i++ {
		angle := float64(i)/4*2*math.Pi - math.Pi/2
		x1 := cx + math.Cos(angle)*(radius-1.0)
		y1 := cy + math.Sin(angle)*(radius-1.0)
		x2 := cx + math.Cos(angle)*(radius-5.0)
		y2 := cy + math.Sin(angle)*(radius-5.0)
		dc.DrawLine(x1, y1, x2, y2)
		dc.SetRGBA(1, 1, 1, 1.0)
		dc.SetLineWidth(1.5)
		dc.Stroke()
	}

	hr := float64(now.Hour()%12) + float64(now.Minute())/60
	min := float64(now.Minute()) + float64(now.Second())/60
	sec := float64(now.Second())

	// Hour hand — short and thick.
	drawHand(dc, cx, cy, hr/12*2*math.Pi-math.Pi/2, radius*0.52, 2.2, color.RGBA{255, 255, 255, 240})
	// Minute hand — long and medium.
	drawHand(dc, cx, cy, min/60*2*math.Pi-math.Pi/2, radius*0.78, 1.4, color.RGBA{220, 220, 220, 220})
	// Second hand — accent red, thin, extends slightly past centre.
	drawHandWithTail(dc, cx, cy, sec/60*2*math.Pi-math.Pi/2, radius*0.82, radius*0.20, 0.8, color.RGBA{220, 60, 60, 255})

	// Centre dot.
	dc.DrawCircle(cx, cy, 1.8)
	dc.SetColor(design.Value)
	dc.Fill()

	// Digital readout — right of the analog face.
	leftEdge := cx + radius + 4
	available := float64(width) - leftEdge
	if available >= 20 {
		midX := leftEdge + available/2
		H := float64(height)

		var timeStr, subStr string
		if format == ClockFormat24Hour {
			timeStr = now.Format("15:04")
			subStr = now.Format(":05")
		} else {
			timeStr = now.Format("3:04")
			subStr = now.Format("PM")
		}

		// Primary time in the label face (10pt) — fits the ~22px upper half.
		if m.labelFace != nil {
			dc.SetFontFace(m.labelFace)
		}
		setRGBA(dc, design.Value, 1)
		dc.DrawStringAnchored(timeStr, midX, H*0.55, 0.5, 0)

		// Sub-string (AM/PM or seconds) in unit face (13pt is too tall; use label).
		setRGBA(dc, design.Unit, 1)
		dc.DrawStringAnchored(subStr, midX, H*0.88, 0.5, 0)
	}

	img := dc.Image().(*image.RGBA)
	out := make([]byte, len(img.Pix))
	copy(out, img.Pix)
	return out
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

// drawHandWithTail draws a clock hand with a short counterweight tail past centre.
func drawHandWithTail(dc *gg.Context, cx, cy, angle, length, tail, width float64, col color.RGBA) {
	x1 := cx - math.Cos(angle)*tail
	y1 := cy - math.Sin(angle)*tail
	x2 := cx + math.Cos(angle)*length
	y2 := cy + math.Sin(angle)*length
	dc.DrawLine(x1, y1, x2, y2)
	dc.SetColor(col)
	dc.SetLineWidth(width)
	dc.SetLineCapRound()
	dc.Stroke()
}
