package zone

import (
	"image"
	"image/color"
	"log/slog"
	"math"

	"github.com/fogleman/gg"
	"golang.org/x/image/font"

	"github.com/mantonx/nexus-open/internal/fonts"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

const (
	detailColWidth  = DisplayWidth / 7 // ~91px per day column
	detailCloseIcon = ""         // FA6 xmark
)

// DetailCloseX/Y are the hardware pixel center of the close (✕) glyph.
// X: right-anchor at DisplayWidth-5, FA xmark at 13px is ~9px wide → center ≈ 630.
// Y: DrawStringAnchored y=10 with vertical anchor 0.5 → center is y=10.
const (
	DetailCloseX = DisplayWidth - 10
	DetailCloseY = 10
)

// RenderDetailFrame renders a DetailPayload into a full 640×48 overlay image.
// The close icon (✕) is drawn top-right as a visual dismissal cue.
func RenderDetailFrame(logger *slog.Logger, payload plugin.DetailPayload, theme Theme) *image.RGBA {
	dc := gg.NewContext(DisplayWidth, DisplayHeight)

	// Background: slightly lighter than zone bg so it reads as a distinct layer.
	bg := theme.GetBgColor()
	panelBg := lighten(bg, 0.12)
	dc.SetColor(panelBg)
	dc.Clear()

	fm := fonts.NewManager(logger)

	// Load faces — small sizes for the constrained 48px height.
	var dayFace, tempFace, iconFace font.Face
	if f, _, err := fm.LoadBestAvailableFont(9); err == nil {
		dayFace = f
	}
	if f, _, err := fm.LoadBestAvailableFont(11); err == nil {
		tempFace = f
	}
	if f, err := fm.GetFace("FontAwesome-Solid", 13); err == nil {
		iconFace = f
	}

	fg := theme.GetFgColor()
	accent := theme.GetAccentColor()

	// Close icon — top-right, dimmed.
	if iconFace != nil {
		dc.SetFontFace(iconFace)
		dc.SetRGBA(float64(fg.R)/255, float64(fg.G)/255, float64(fg.B)/255, 0.45)
		dc.DrawStringAnchored(detailCloseIcon, float64(DisplayWidth)-5, 10, 1.0, 0.5)
	}

	// Title — top-left, accent coloured, clipped so it doesn't overlap the close icon.
	if dayFace != nil && payload.Title != "" {
		dc.SetFontFace(dayFace)
		dc.SetColor(accent)
		dc.DrawStringAnchored(payload.Title, 4, 6, 0, 0.5)
	}

	// Forecast columns.
	for i, day := range payload.Forecast {
		if i >= 7 {
			break
		}
		drawForecastColumn(dc, day, i, dayFace, tempFace, iconFace, fg, accent)
	}

	return dc.Image().(*image.RGBA)
}

// drawForecastColumn draws a single day column at column index i.
func drawForecastColumn(
	dc *gg.Context,
	day plugin.DailyForecast,
	col int,
	dayFace, tempFace, iconFace font.Face,
	fg, accent color.RGBA,
) {
	x := float64(col*detailColWidth) + float64(detailColWidth)/2

	// Subtle column separator (skip the first).
	if col > 0 {
		dc.SetRGBA(float64(fg.R)/255, float64(fg.G)/255, float64(fg.B)/255, 0.12)
		dc.DrawLine(float64(col*detailColWidth), 14, float64(col*detailColWidth), float64(DisplayHeight)-2)
		dc.Stroke()
	}

	// Day label — top of column.
	if dayFace != nil {
		dc.SetFontFace(dayFace)
		label := day.Date
		if len(label) > 3 {
			label = label[:3]
		}
		isToday := day.Date == "Today"
		if isToday {
			dc.SetColor(accent)
		} else {
			dc.SetRGBA(float64(fg.R)/255, float64(fg.G)/255, float64(fg.B)/255, 0.75)
		}
		dc.DrawStringAnchored(label, x, 17, 0.5, 0.5)
	}

	// Weather icon — middle.
	if iconFace != nil {
		glyph := resolveIconGlyph(day.Icon)
		if glyph != "" {
			dc.SetFontFace(iconFace)
			dc.SetColor(accent)
			dc.DrawStringAnchored(glyph, x, 30, 0.5, 0.5)
		}
	}

	// High/low temps — bottom row.
	if tempFace != nil {
		dc.SetFontFace(tempFace)
		unit := "°"
		hi := formatTemp(day.TempHigh, unit)
		lo := formatTemp(day.TempLow, unit)

		// High in accent, low dimmed.
		hiW, _ := dc.MeasureString(hi)
		loW, _ := dc.MeasureString(lo)
		totalW := hiW + 3 + loW
		startX := x - totalW/2

		dc.SetColor(accent)
		dc.DrawString(hi, startX, float64(DisplayHeight)-4)
		dc.SetRGBA(float64(fg.R)/255, float64(fg.G)/255, float64(fg.B)/255, 0.55)
		dc.DrawString(lo, startX+hiW+3, float64(DisplayHeight)-4)
	}
}

func formatTemp(t float64, unit string) string {
	v := int(math.Round(t))
	s := ""
	if v >= 0 {
		s = itoa(v)
	} else {
		s = "-" + itoa(-v)
	}
	return s + unit
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	buf := [10]byte{}
	pos := len(buf)
	for n > 0 {
		pos--
		buf[pos] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[pos:])
}

// lighten brightens an RGBA colour by the given fraction (0–1).
func lighten(c color.RGBA, f float64) color.RGBA {
	clamp := func(v float64) uint8 {
		if v > 255 {
			return 255
		}
		return uint8(v)
	}
	return color.RGBA{
		R: clamp(float64(c.R) + (255-float64(c.R))*f),
		G: clamp(float64(c.G) + (255-float64(c.G))*f),
		B: clamp(float64(c.B) + (255-float64(c.B))*f),
		A: c.A,
	}
}
