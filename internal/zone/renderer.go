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

	// Render sparkline/graph (bottom-aligned)
	if len(payload.Spark) > 0 {
		r.drawGraph(img, payload.Spark, payload.GraphType, primaryColor)
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

// drawGraph renders a graph at the bottom of the zone based on the specified type
func (r *Renderer) drawGraph(img *image.RGBA, data []float32, graphType module.GraphType, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	// Default to sparkline if not specified
	if graphType == "" {
		graphType = module.GraphTypeSparkline
	}

	switch graphType {
	case module.GraphTypeBar:
		r.drawBarGraph(img, data, col)
	case module.GraphTypeArea:
		r.drawAreaGraph(img, data, col)
	default: // module.GraphTypeSparkline
		r.drawSparkline(img, data, col)
	}
}

// drawSparkline renders a line graph at the bottom of the zone
func (r *Renderer) drawSparkline(img *image.RGBA, data []float32, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	const sparkHeight = 16
	const paddingH = 4
	const paddingV = 2

	availableWidth := r.width - (2 * paddingH)
	yBase := r.height - paddingV

	// More opaque for better visibility
	sparkColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 255}

	// Draw thicker line (2 pixels)
	lineThickness := 2

	// Draw line connecting points
	for i := 0; i < len(data)-1; i++ {
		val1 := data[i]
		val2 := data[i+1]

		// Clamp values
		if val1 < 0 {
			val1 = 0
		}
		if val1 > 1 {
			val1 = 1
		}
		if val2 < 0 {
			val2 = 0
		}
		if val2 > 1 {
			val2 = 1
		}

		// Calculate x positions
		x1 := paddingH + (i * availableWidth / len(data))
		x2 := paddingH + ((i + 1) * availableWidth / len(data))

		// Calculate y positions (inverted, higher value = lower y)
		y1 := yBase - int(float32(sparkHeight)*val1)
		y2 := yBase - int(float32(sparkHeight)*val2)

		// Draw thick line by drawing multiple parallel lines
		for offset := -lineThickness/2; offset <= lineThickness/2; offset++ {
			r.drawLine2D(img, x1, y1+offset, x2, y2+offset, sparkColor)
		}
	}
}

// drawLine2D draws a line between two points (Bresenham's algorithm)
func (r *Renderer) drawLine2D(img *image.RGBA, x1, y1, x2, y2 int, col color.RGBA) {
	dx := abs(x2 - x1)
	dy := abs(y2 - y1)
	sx := -1
	if x1 < x2 {
		sx = 1
	}
	sy := -1
	if y1 < y2 {
		sy = 1
	}
	err := dx - dy

	for {
		if x1 >= 0 && x1 < r.width && y1 >= 0 && y1 < r.height {
			img.Set(x1, y1, col)
		}

		if x1 == x2 && y1 == y2 {
			break
		}

		e2 := 2 * err
		if e2 > -dy {
			err -= dy
			x1 += sx
		}
		if e2 < dx {
			err += dx
			y1 += sy
		}
	}
}

func abs(x int) int {
	if x < 0 {
		return -x
	}
	return x
}

// drawBarGraph renders vertical bars at the bottom of the zone
func (r *Renderer) drawBarGraph(img *image.RGBA, data []float32, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	const sparkHeight = 16
	const paddingH = 4
	const paddingV = 2

	// Calculate bar width
	availableWidth := r.width - (2 * paddingH)
	barWidth := availableWidth / len(data)
	if barWidth < 1 {
		barWidth = 1
	}

	// Draw bars from bottom
	yBase := r.height - paddingV

	// More opaque for better visibility
	sparkColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 220}

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
		for py := yBase - barHeight; py < yBase; py++ {
			for px := x; px < x+barWidth-1 && px < r.width-paddingH; px++ {
				img.Set(px, py, sparkColor)
			}
		}
	}
}

// drawAreaGraph renders a filled area graph at the bottom of the zone
func (r *Renderer) drawAreaGraph(img *image.RGBA, data []float32, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	const sparkHeight = 16
	const paddingH = 4
	const paddingV = 2

	availableWidth := r.width - (2 * paddingH)
	yBase := r.height - paddingV

	// More opaque fill for better visibility
	fillColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 150}
	// Fully opaque line for definition
	lineColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 255}

	// Draw filled area and top line
	for i := 0; i < len(data); i++ {
		value := data[i]

		// Clamp values
		if value < 0 {
			value = 0
		}
		if value > 1 {
			value = 1
		}

		x := paddingH + (i * availableWidth / len(data))
		y := yBase - int(float32(sparkHeight)*value)

		// Fill column from bottom to value height
		for py := y; py < yBase; py++ {
			if x >= 0 && x < r.width && py >= 0 && py < r.height {
				img.Set(x, py, fillColor)
			}
		}
	}

	// Draw top line connecting points (on top of fill for definition)
	for i := 0; i < len(data)-1; i++ {
		val1 := data[i]
		val2 := data[i+1]

		// Clamp values
		if val1 < 0 {
			val1 = 0
		}
		if val1 > 1 {
			val1 = 1
		}
		if val2 < 0 {
			val2 = 0
		}
		if val2 > 1 {
			val2 = 1
		}

		x1 := paddingH + (i * availableWidth / len(data))
		x2 := paddingH + ((i + 1) * availableWidth / len(data))
		y1 := yBase - int(float32(sparkHeight)*val1)
		y2 := yBase - int(float32(sparkHeight)*val2)

		r.drawLine2D(img, x1, y1, x2, y2, lineColor)
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
