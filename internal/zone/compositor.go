package zone

import (
	"image"
	"image/color"
	"image/draw"
	"log/slog"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"
)

const (
	// DisplayWidth is the fixed width of the display
	DisplayWidth = 640
	// DisplayHeight is the fixed height of the display
	DisplayHeight = 48
)

// Compositor composites multiple zone renderers into a single display image
type Compositor struct {
	logger *slog.Logger
	theme  Theme
	page   *Page
}

// NewCompositor creates a new compositor for a page
func NewCompositor(logger *slog.Logger, theme Theme, page *Page) *Compositor {
	return &Compositor{
		logger: logger,
		theme:  theme,
		page:   page,
	}
}

// Composite renders all zones and composites them into a single 640x48 image.
// theme is passed in so live UpdateTheme calls are reflected immediately
// rather than using the stale copy stored at compositor creation time.
func (c *Compositor) Composite(zoneImages map[string]*image.RGBA, theme Theme) (*image.RGBA, error) {
	// Create display buffer
	display := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))

	// Fill with background color
	bgColor := theme.GetBgColor()
	draw.Draw(display, display.Bounds(), &image.Uniform{bgColor}, image.Point{}, draw.Src)

	// Ensure offsets are computed
	c.page.ComputeOffsets()

	// Composite each zone
	for i, zone := range c.page.Zones {
		zoneImg, ok := zoneImages[zone.ID]
		if !ok {
			c.logger.Warn("zone image not found", "zone_id", zone.ID, "index", i)
			continue
		}

		// Draw zone image at its offset
		destRect := image.Rect(zone.X, 0, zone.X+zone.Width, DisplayHeight)
		draw.Draw(display, destRect, zoneImg, image.Point{}, draw.Src)

		// Draw gutter (vertical separator) if not the last zone
		if i < len(c.page.Zones)-1 && theme.GutterPx > 0 {
			c.drawGutter(display, zone.X+zone.Width, theme)
		}
	}

	return display, nil
}

// drawGutter draws a vertical gutter at the specified X position
func (c *Compositor) drawGutter(img *image.RGBA, x int, theme Theme) {
	gutterColor := theme.GetMutedColor()
	gutterColor.A = 60 // Semi-transparent

	for gx := 0; gx < theme.GutterPx; gx++ {
		for y := 0; y < DisplayHeight; y++ {
			px := x + gx
			if px < DisplayWidth {
				img.Set(px, y, gutterColor)
			}
		}
	}
}

// RenderPlaceholder renders a placeholder zone (for loading or errors)
func RenderPlaceholder(width, height int, text string, bgColor, fgColor color.RGBA) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, width, height))
	draw.Draw(img, img.Bounds(), &image.Uniform{bgColor}, image.Point{}, draw.Src)

	if text == "" {
		return img
	}

	face := basicfont.Face7x13
	textWidth := len(text) * 7
	x := (width - textWidth) / 2
	if x < 2 {
		x = 2
	}
	// Baseline: vertically center within the zone (face ascent is 11px above baseline)
	y := (height + face.Metrics().Ascent.Ceil()) / 2

	d := &font.Drawer{
		Dst:  img,
		Src:  image.NewUniform(fgColor),
		Face: face,
		Dot:  fixed.P(x, y),
	}
	d.DrawString(text)

	return img
}
