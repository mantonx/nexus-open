package zone

import (
	"image"
	"image/color"
	"image/draw"
	"log/slog"
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

// Composite renders all zones and composites them into a single 640x48 image
func (c *Compositor) Composite(zoneImages map[string]*image.RGBA) (*image.RGBA, error) {
	// Create display buffer
	display := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))

	// Fill with background color
	bgColor := c.theme.GetBgColor()
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
		if i < len(c.page.Zones)-1 && c.theme.GutterPx > 0 {
			c.drawGutter(display, zone.X+zone.Width)
		}
	}

	return display, nil
}

// drawGutter draws a vertical gutter at the specified X position
func (c *Compositor) drawGutter(img *image.RGBA, x int) {
	gutterColor := c.theme.GetMutedColor()
	gutterColor.A = 60 // Semi-transparent

	for gx := 0; gx < c.theme.GutterPx; gx++ {
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

	// Draw centered text
	// For now, just draw a simple indicator
	// TODO: Use proper text rendering
	centerX := width / 2
	centerY := height / 2

	// Draw a small indicator
	for y := centerY - 2; y < centerY+2; y++ {
		for x := centerX - 4; x < centerX+4; x++ {
			if x >= 0 && x < width && y >= 0 && y < height {
				img.Set(x, y, fgColor)
			}
		}
	}

	return img
}
