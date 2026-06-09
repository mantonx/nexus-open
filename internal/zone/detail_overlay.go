package zone

import (
	"image"
	"image/color"
	"log/slog"

	"github.com/mantonx/nexus-open/pkg/plugin"
)


// DetailCloseX/Y are the hardware pixel centre of the close (✕) glyph.
const (
	DetailCloseX = DisplayWidth - 10
	DetailCloseY = 10
)

// RenderDetailFrame blits the pre-rendered pixel buffer from payload.RawFrame
// onto a 640×48 RGBA image. If RawFrame is absent or the wrong size, a plain
// error frame is returned so the display always shows something.
func RenderDetailFrame(_ *slog.Logger, payload plugin.DetailPayload, theme Theme) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))

	const stride = DisplayWidth * 4
	if len(payload.RawFrame) == stride*DisplayHeight {
		copy(img.Pix, payload.RawFrame)
		return img
	}

	// Fallback: solid background with an error tint so a broken plugin is visible.
	bg := theme.GetBgColor()
	errColor := color.RGBA{R: uint8(float64(bg.R) + (255-float64(bg.R))*0.15),
		G: bg.G, B: bg.B, A: 255}
	for i := 0; i < len(img.Pix); i += 4 {
		img.Pix[i] = errColor.R
		img.Pix[i+1] = errColor.G
		img.Pix[i+2] = errColor.B
		img.Pix[i+3] = 255
	}
	return img
}
