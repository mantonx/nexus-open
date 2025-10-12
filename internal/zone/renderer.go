package zone

import (
	"image"
	"image/color"
	"image/draw"
	"log/slog"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"

	"nexus-open/pkg/module"
)

// Renderer renders a single zone from a Payload
type Renderer struct {
	logger *slog.Logger
	theme  Theme
	width  int
	height int
	align  Alignment
}

// NewRenderer creates a new zone renderer
func NewRenderer(logger *slog.Logger, theme Theme, width, height int, align Alignment) *Renderer {
	return &Renderer{
		logger: logger,
		theme:  theme,
		width:  width,
		height: height,
		align:  align,
	}
}

// Render renders a payload to an image buffer
func (r *Renderer) Render(payload module.Payload) (*image.RGBA, error) {
	// Validate payload
	if err := payload.Validate(); err != nil {
		return nil, err
	}

	// Create image buffer
	img := image.NewRGBA(image.Rect(0, 0, r.width, r.height))

	// Fill background
	draw.Draw(img, img.Bounds(), &image.Uniform{r.theme.GetBgColor()}, image.Point{}, draw.Src)

	// Get severity color
	primaryColor := r.getSeverityColor(payload.Severity)

	// Calculate text positions with padding
	const paddingH = 4
	const paddingV = 2

	// Render primary text (main value)
	if payload.Primary != "" {
		r.drawText(img, payload.Primary, primaryColor, paddingH, paddingV+14)
	}

	// Render secondary text (below primary)
	if payload.Secondary != "" {
		mutedColor := r.theme.GetMutedColor()
		r.drawText(img, payload.Secondary, mutedColor, paddingH, paddingV+26)
	}

	// Render sparkline (bottom-aligned)
	if len(payload.Spark) > 0 {
		r.drawSparkline(img, payload.Spark, primaryColor)
	}

	// Render progress bar (bottom-aligned)
	if payload.Progress > 0 {
		r.drawProgressBar(img, payload.Progress, primaryColor)
	}

	return img, nil
}

// drawText renders text at the specified position
func (r *Renderer) drawText(img *image.RGBA, text string, col color.RGBA, x, y int) {
	face := basicfont.Face7x13

	point := fixed.Point26_6{
		X: fixed.Int26_6(x * 64),
		Y: fixed.Int26_6(y * 64),
	}

	// Adjust for alignment
	textWidth := font.MeasureString(face, text).Ceil()

	switch r.align {
	case AlignCenter:
		point.X = fixed.Int26_6((r.width-textWidth)/2) * 64
	case AlignRight:
		point.X = fixed.Int26_6(r.width-textWidth-4) * 64
	case AlignLeft:
		// Already at left with padding
	}

	drawer := &font.Drawer{
		Dst:  img,
		Src:  image.NewUniform(col),
		Face: face,
		Dot:  point,
	}

	drawer.DrawString(text)
}

// drawSparkline renders a sparkline chart at the bottom of the zone
func (r *Renderer) drawSparkline(img *image.RGBA, data []float32, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	const sparkHeight = 8
	const paddingH = 4
	const paddingV = 2

	// Calculate bar width
	availableWidth := r.width - (2 * paddingH)
	barWidth := availableWidth / len(data)
	if barWidth < 1 {
		barWidth = 1
	}

	// Draw bars from bottom
	y1 := r.height - paddingV

	// Semi-transparent accent color
	sparkColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 150}

	for i, value := range data {
		if value < 0 {
			value = 0
		}
		if value > 1 {
			value = 1
		}

		barHeight := int(float32(sparkHeight) * value)
		x := paddingH + (i * barWidth)

		// Draw bar
		for py := y1 - barHeight; py < y1; py++ {
			for px := x; px < x+barWidth-1 && px < r.width-paddingH; px++ {
				img.Set(px, py, sparkColor)
			}
		}
	}
}

// drawProgressBar renders a progress bar at the bottom of the zone
func (r *Renderer) drawProgressBar(img *image.RGBA, progress float32, col color.RGBA) {
	const barHeight = 2
	const paddingH = 4
	const paddingV = 2

	if progress < 0 {
		progress = 0
	}
	if progress > 1 {
		progress = 1
	}

	availableWidth := r.width - (2 * paddingH)
	filledWidth := int(float32(availableWidth) * progress)

	y0 := r.height - paddingV - barHeight
	y1 := r.height - paddingV

	// Draw filled portion
	for y := y0; y < y1; y++ {
		for x := paddingH; x < paddingH+filledWidth; x++ {
			img.Set(x, y, col)
		}
	}

	// Draw unfilled portion (muted)
	mutedColor := r.theme.GetMutedColor()
	mutedColor.A = 80 // More transparent

	for y := y0; y < y1; y++ {
		for x := paddingH + filledWidth; x < r.width-paddingH; x++ {
			img.Set(x, y, mutedColor)
		}
	}
}

// getSeverityColor returns the appropriate color for a severity level
func (r *Renderer) getSeverityColor(severity module.Severity) color.RGBA {
	switch severity {
	case module.SeverityWarn:
		// Yellow/Orange
		return color.RGBA{R: 255, G: 176, B: 32, A: 255}
	case module.SeverityCrit:
		// Red
		return color.RGBA{R: 255, G: 68, B: 68, A: 255}
	default:
		// OK - use accent color
		return r.theme.GetAccentColor()
	}
}
