package zone

import (
	"image"
	"image/color"
	"image/draw"
	"log/slog"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"

	"nexus-open/internal/fonts"
	"nexus-open/pkg/module"
)

// Renderer renders a single zone from a Payload
type Renderer struct {
	logger *slog.Logger
	theme  Theme
	width  int
	height int
	align  Alignment

	fontManager   *fonts.Manager
	primaryFace   font.Face
	secondaryFace font.Face
	iconFace      font.Face
}

// NewRenderer creates a new zone renderer
func NewRenderer(logger *slog.Logger, theme Theme, width, height int, align Alignment) *Renderer {
	r := &Renderer{
		logger: logger,
		theme:  theme,
		width:  width,
		height: height,
		align:  align,
	}

	fontManager := fonts.NewManager(logger)
	r.fontManager = fontManager

	if face, _, err := fontManager.LoadBestAvailableFont(14); err == nil {
		r.primaryFace = face
	} else {
		logger.Warn("failed to load primary font, using fallback", "error", err)
		r.primaryFace = basicfont.Face7x13
	}

	if face, _, err := fontManager.LoadBestAvailableFont(11); err == nil {
		r.secondaryFace = face
	} else {
		logger.Warn("failed to load secondary font, using fallback", "error", err)
		r.secondaryFace = basicfont.Face7x13
	}

	if iconFace, err := fontManager.GetFace("FontAwesome-Solid", 14); err == nil {
		r.iconFace = iconFace
	} else {
		logger.Warn("failed to load icon font, icons may not render", "error", err)
	}

	return r
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
		iconGlyph := r.resolveIcon(payload.Icon)
		r.drawLine(img, payload.Primary, iconGlyph, primaryColor, paddingH, paddingV+14, r.primaryFace)
	}

	// Render secondary text (below primary)
	if payload.Secondary != "" {
		mutedColor := r.theme.GetMutedColor()
		r.drawLine(img, payload.Secondary, "", mutedColor, paddingH, paddingV+26, r.secondaryFace)
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

var faIconGlyphs = map[string]string{
	"cloud":         "\uf0c2",
	"cpu":           "\uf2db",
	"microchip":     "\uf2db",
	"network-wired": "\uf6ff",
	"hand-wave":     "\ue1d8",
}

func (r *Renderer) resolveIcon(icon string) string {
	if icon == "" {
		return ""
	}
	runes := []rune(icon)
	if len(runes) == 1 {
		return icon
	}
	if glyph, ok := faIconGlyphs[icon]; ok {
		return glyph
	}
	return ""
}

// drawLine renders optional icon + text respecting alignment.
func (r *Renderer) drawLine(img *image.RGBA, text string, icon string, col color.RGBA, paddingH, y int, face font.Face) {
	if face == nil {
		face = basicfont.Face7x13
	}

	iconWidth := 0
	if icon != "" && r.iconFace != nil {
		iconWidth = font.MeasureString(r.iconFace, icon).Ceil()
	}

	textWidth := font.MeasureString(face, text).Ceil()
	if textWidth == 0 {
		textWidth = 0
	}
	spacing := 0
	if textWidth > 0 && iconWidth > 0 {
		spacing = 4
	}
	lineWidth := iconWidth + spacing + textWidth

	startX := paddingH
	switch r.align {
	case AlignCenter:
		startX = (r.width - lineWidth) / 2
	case AlignRight:
		startX = r.width - lineWidth - paddingH
	}

	x := startX
	if icon != "" && r.iconFace != nil && iconWidth > 0 {
		r.drawIconGlyph(img, icon, col, x, y, r.iconFace)
		x += iconWidth + spacing
	}

	r.drawRawText(img, text, col, x, y, face)
}

func (r *Renderer) drawIconGlyph(img *image.RGBA, glyph string, col color.RGBA, baselineX, baselineY int, face font.Face) {
	if face == nil || glyph == "" {
		return
	}
	// Align icon baseline with primary text baseline (empirical offset)
	point := fixed.Point26_6{
		X: fixed.I(baselineX),
		Y: fixed.I(baselineY + 2),
	}
	drawer := &font.Drawer{
		Dst:  img,
		Src:  image.NewUniform(col),
		Face: face,
		Dot:  point,
	}
	drawer.DrawString(glyph)
}

func (r *Renderer) drawRawText(img *image.RGBA, text string, col color.RGBA, x, y int, face font.Face) {
	if face == nil {
		face = basicfont.Face7x13
	}

	drawer := &font.Drawer{
		Dst:  img,
		Src:  image.NewUniform(col),
		Face: face,
		Dot: fixed.Point26_6{
			X: fixed.I(x),
			Y: fixed.I(y),
		},
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
