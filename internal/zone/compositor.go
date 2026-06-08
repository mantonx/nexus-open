package zone

import (
	"image"
	"image/color"
	"image/draw"
	"image/gif"
	"log/slog"
	"sync"
	"time"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"
)

const (
	DisplayWidth  = 640
	DisplayHeight = 48
)

// bgLayer holds the current background — either a static image or an animated GIF.
type bgLayer struct {
	static  *image.RGBA // non-nil for static images
	anim    *gif.GIF    // non-nil for animated GIFs
	frameIdx  int
	nextFrame time.Time
	mu        sync.Mutex
}

// currentFrame returns the RGBA to draw as the background, advancing GIF frames
// if their delay has elapsed.
func (b *bgLayer) currentFrame() *image.RGBA {
	b.mu.Lock()
	defer b.mu.Unlock()

	if b.static != nil {
		return b.static
	}
	if b.anim == nil || len(b.anim.Image) == 0 {
		return nil
	}

	now := time.Now()
	if now.After(b.nextFrame) {
		b.frameIdx = (b.frameIdx + 1) % len(b.anim.Image)
		// GIF delay is in centiseconds
		delay := time.Duration(b.anim.Delay[b.frameIdx]) * 10 * time.Millisecond
		if delay < 16*time.Millisecond {
			delay = 100 * time.Millisecond // clamp minimum to ~10fps
		}
		b.nextFrame = now.Add(delay)
	}

	// Convert the paletted frame to RGBA
	src := b.anim.Image[b.frameIdx]
	dst := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	draw.Draw(dst, dst.Bounds(), src, src.Bounds().Min, draw.Src)
	return dst
}

// Compositor composites multiple zone renderers into a single display image.
type Compositor struct {
	logger *slog.Logger
	theme  Theme
	page   *Page
	bg     *bgLayer // nil = solid colour only
}

// NewCompositor creates a new compositor for a page.
func NewCompositor(logger *slog.Logger, theme Theme, page *Page) *Compositor {
	return &Compositor{
		logger: logger,
		theme:  theme,
		page:   page,
	}
}

// SetBackground sets a static image as the background layer.
func (c *Compositor) SetBackground(img *image.RGBA) {
	c.bg = &bgLayer{static: img}
}

// SetBackgroundGIF sets an animated GIF as the background layer.
// The GIF is decoded into paletted frames and played back at its own frame rate.
func (c *Compositor) SetBackgroundGIF(g *gif.GIF) {
	c.bg = &bgLayer{
		anim:      g,
		frameIdx:  0,
		nextFrame: time.Now(),
	}
}

// ClearBackground removes the background layer (reverts to solid bg colour).
func (c *Compositor) ClearBackground() {
	c.bg = nil
}

// Composite renders all zones and composites them into dst.
// If dst is nil a new 640×48 RGBA image is allocated. Pass a pre-allocated
// buffer to avoid allocation on the hot path.
// theme is passed in so live UpdateTheme calls are reflected immediately.
func (c *Compositor) Composite(dst *image.RGBA, zoneImages map[string]*image.RGBA, theme Theme) (*image.RGBA, error) {
	display := dst
	if display == nil {
		display = image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	}

	// Layer 1: solid background colour.
	bgColor := theme.GetBgColor()
	draw.Draw(display, display.Bounds(), &image.Uniform{bgColor}, image.Point{}, draw.Src)

	// Layer 2: background image / GIF frame (drawn over solid colour, under zones).
	if c.bg != nil {
		if frame := c.bg.currentFrame(); frame != nil {
			draw.Draw(display, display.Bounds(), frame, image.Point{}, draw.Over)
		}
	}

	// Layer 3: zone images — all zones first, gutters after.
	// Gutters must be drawn last; otherwise the next zone's draw.Draw overwrites them.
	// Zone X offsets were already computed by initializePage/preRenderPage — calling
	// ComputeOffsets here from concurrent goroutines races on Page.Zones[i].X.
	for i, zone := range c.page.Zones {
		zoneImg, ok := zoneImages[zone.ID]
		if !ok {
			c.logger.Warn("zone image not found", "zone_id", zone.ID, "index", i)
			continue
		}
		destRect := image.Rect(zone.X, 0, zone.X+zone.Width, DisplayHeight)
		draw.Draw(display, destRect, zoneImg, image.Point{}, draw.Over)
	}

	// Layer 4: zone separators drawn on top of everything.
	if theme.GutterPx > 0 {
		for i, zone := range c.page.Zones {
			if i < len(c.page.Zones)-1 {
				c.drawGutter(display, zone.X+zone.Width, theme)
			}
		}
	}

	return display, nil
}

func (c *Compositor) drawGutter(img *image.RGBA, x int, theme Theme) {
	// Draw a 1px separator: bright centre pixel, fading toward top and bottom.
	// This gives a subtle but clear division without a harsh hard edge.
	for y := 0; y < DisplayHeight; y++ {
		// Alpha peaks at the centre (y=24), fades to ~30 at edges.
		t := float64(y) / float64(DisplayHeight-1) // 0..1
		dist := t - 0.5                            // -0.5..0.5
		a := 1.0 - (dist*dist)*3.5                 // parabola: 1.0 centre, ~0.12 at edges
		if a < 0.12 {
			a = 0.12
		}
		alpha := uint8(a * 180) // max alpha ≈180 (~70%) — clearly visible on black
		col := color.RGBA{R: 80, G: 84, B: 92, A: alpha}
		if x < DisplayWidth {
			img.Set(x, y, col)
		}
	}
	// Second pixel for GutterPx>1 themes — fully transparent, just spacing.
	if theme.GutterPx > 1 && x+1 < DisplayWidth {
		img.Set(x+1, 0, color.RGBA{}) // no-op effectively, just reserve space
	}
}

// RenderPlaceholder renders a placeholder zone (for loading or errors).
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
