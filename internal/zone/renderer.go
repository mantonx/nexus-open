package zone

import (
	"image"
	"image/color"
	"image/draw"
	"log/slog"
	"strings"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"

	"github.com/mantonx/nexus-next/internal/fonts"
	"github.com/mantonx/nexus-next/pkg/module"
)

// Renderer renders a single zone from a Payload
type Renderer struct {
	logger *slog.Logger
	theme  Theme
	width  int
	height int
	align  Alignment

	fontManager     *fonts.Manager
	primaryFace     font.Face
	multiLineFace   font.Face // Smaller font for multi-line content (16pt)
	secondaryFace   font.Face
	iconFace        font.Face
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

	// Load primary font at configured size (default 24pt for modern dashboard)
	primarySize := float64(theme.FontSizePrimary)
	if primarySize == 0 {
		primarySize = 24 // Default to modern dashboard size
	}
	if face, _, err := fontManager.LoadBestAvailableFont(primarySize); err == nil {
		r.primaryFace = face
	} else {
		logger.Warn("failed to load primary font, using fallback", "error", err)
		r.primaryFace = basicfont.Face7x13
	}

	// Load multi-line font (12pt - fits two lines in 48px height with labels)
	if face, _, err := fontManager.LoadBestAvailableFont(12); err == nil {
		r.multiLineFace = face
	} else {
		logger.Warn("failed to load multi-line font, using fallback", "error", err)
		r.multiLineFace = basicfont.Face7x13
	}

	// Load secondary font at configured size (default 9pt for modern dashboard)
	secondarySize := float64(theme.FontSizeSecondary)
	if secondarySize == 0 {
		secondarySize = 9 // Default to modern dashboard size
	}
	if face, _, err := fontManager.LoadBestAvailableFont(secondarySize); err == nil {
		r.secondaryFace = face
	} else {
		logger.Warn("failed to load secondary font, using fallback", "error", err)
		r.secondaryFace = basicfont.Face7x13
	}

	// Load icon font at 18pt (proportional to primary)
	if iconFace, err := fontManager.GetFace("FontAwesome-Solid", 18); err == nil {
		r.iconFace = iconFace
	} else {
		logger.Warn("failed to load icon font, icons may not render", "error", err)
	}

	return r
}

// ContentModel represents the parsed content from a payload
type ContentModel struct {
	Icon             *IconContent
	PrimaryLines     []string
	Secondary        string
	GraphData        []float32
	GraphType        module.GraphType
	LineSpacing      int                  // Spacing between lines for multi-line Primary text
	LabelPosition    module.LabelPosition // Where to position the secondary label
	LabelOffsetX     int                  // Horizontal offset for label (pixels)
	LabelOffsetY     int                  // Vertical offset for label (pixels)
	NormalizeGraph   bool                 // Whether to normalize graph data to fill from baseline
	GraphBgOpacity   int                  // Per-module background opacity override (0-100)
	GraphLineOpacity int                  // Per-module line opacity override (0-100)
}

// IconContent represents an icon to be rendered
type IconContent struct {
	Glyph string
}

// Measurements holds measured dimensions of content elements
type Measurements struct {
	IconWidth  int
	IconHeight int

	PrimaryWidths  []int // One per line
	PrimaryHeight  int   // Total height with line spacing
	LineHeight     int   // Height of a single line
	LineSpacing    int   // Spacing between line baselines

	SecondaryWidth  int
	SecondaryHeight int

	TotalContentHeight int // Everything stacked
}

// LayoutBox represents a positioned rectangular region
type LayoutBox struct {
	X, Y          int
	Width, Height int
}

// CalculatedLayout holds the calculated positions for all content
type CalculatedLayout struct {
	IconBox      *LayoutBox
	PrimaryBoxes []LayoutBox // One per line
	SecondaryBox *LayoutBox
}

// parsePayload converts a module payload into a content model
func (r *Renderer) parsePayload(payload module.Payload) ContentModel {
	model := ContentModel{
		PrimaryLines:     strings.Split(payload.Primary, "\n"),
		Secondary:        payload.Secondary,
		GraphData:        payload.Spark,
		GraphType:        payload.GraphType,
		LineSpacing:      payload.LineSpacing,
		LabelPosition:    payload.LabelPosition,
		LabelOffsetX:     payload.LabelOffsetX,
		LabelOffsetY:     payload.LabelOffsetY,
		NormalizeGraph:   payload.NormalizeGraph,
		GraphBgOpacity:   payload.GraphBgOpacity,
		GraphLineOpacity: payload.GraphLineOpacity,
	}

	// Add icon if meaningful
	if r.shouldShowIcon(payload) {
		glyph := r.resolveIcon(payload.Icon)
		if glyph != "" {
			model.Icon = &IconContent{Glyph: glyph}
		}
	}

	return model
}

// measureContent measures all content elements and calculates total dimensions
func (r *Renderer) measureContent(model ContentModel) Measurements {
	m := Measurements{}

	// Measure icon
	if model.Icon != nil && r.iconFace != nil {
		m.IconWidth = font.MeasureString(r.iconFace, model.Icon.Glyph).Ceil()
		m.IconHeight = 18 // Icon font size
	}

	// Determine which font to use based on number of lines
	isMultiLine := len(model.PrimaryLines) > 1
	primaryFont := r.primaryFace
	var lineHeight, lineSpacing int

	if isMultiLine {
		// Use smaller font for multi-line (12pt fits 2 lines in 48px with labels)
		primaryFont = r.multiLineFace
		lineHeight = 12
		// Use custom line spacing from module, or default to 18
		if model.LineSpacing > 0 {
			lineSpacing = model.LineSpacing
		} else {
			lineSpacing = 18 // Default spacing between baselines for multi-line
		}
	} else {
		// Use large font for single line (24pt)
		primaryFont = r.primaryFace
		lineHeight = 24
		lineSpacing = 24
	}

	// Measure primary lines with appropriate font
	for _, line := range model.PrimaryLines {
		if primaryFont != nil {
			width := font.MeasureString(primaryFont, line).Ceil()
			m.PrimaryWidths = append(m.PrimaryWidths, width)
		}
	}

	// Store line metrics for layout calculation
	m.LineHeight = lineHeight
	m.LineSpacing = lineSpacing

	// Calculate total primary height
	if len(model.PrimaryLines) > 0 {
		m.PrimaryHeight = len(model.PrimaryLines) * lineSpacing
		// Adjust for last line (no spacing after it)
		m.PrimaryHeight -= (lineSpacing - lineHeight)
	}

	// Measure secondary
	if model.Secondary != "" && r.secondaryFace != nil {
		m.SecondaryWidth = font.MeasureString(r.secondaryFace, model.Secondary).Ceil()
		m.SecondaryHeight = 10 // Secondary font size
	}

	// Calculate total content height
	// Use a fixed standard height for all modules to ensure consistent vertical alignment
	// This makes graphs and all content align at the same vertical position
	const standardContentHeight = 38 // Fixed height for consistent centering (24pt primary + 4px gap + 10pt secondary)
	m.TotalContentHeight = standardContentHeight

	return m
}

// calculateLayout calculates the position of all content elements
func (r *Renderer) calculateLayout(model ContentModel, measurements Measurements) CalculatedLayout {
	const paddingH = 16
	const paddingV = 8
	const iconTextSpacing = 8
	const primarySecondarySpacing = 4  // For "below" positioning
	const labelRightSpacing = 8        // For "right" positioning - base spacing between values and label

	layout := CalculatedLayout{}

	hasIcon := model.Icon != nil

	// Calculate content starting X position
	contentStartX := paddingH
	if hasIcon {
		// Icon flows first on the left
		iconCenterY := r.height / 2
		layout.IconBox = &LayoutBox{
			X:      paddingH,
			Y:      iconCenterY - measurements.IconHeight/2,
			Width:  measurements.IconWidth,
			Height: measurements.IconHeight,
		}

		// Text flows after icon with spacing
		contentStartX = paddingH + measurements.IconWidth + iconTextSpacing
	}

	// Vertically center the entire text block
	contentCenterY := r.height / 2
	textBlockStartY := contentCenterY - measurements.TotalContentHeight/2

	// Position primary lines
	currentY := textBlockStartY
	for i := range model.PrimaryLines {
		var lineX int
		lineWidth := 0
		if i < len(measurements.PrimaryWidths) {
			lineWidth = measurements.PrimaryWidths[i]
		}

		// Determine horizontal alignment
		if hasIcon || r.align == AlignLeft {
			// Left-aligned (or after icon)
			lineX = contentStartX
		} else if r.align == AlignCenter {
			// Center-aligned
			lineX = (r.width - lineWidth) / 2
		} else {
			// Right-aligned
			lineX = r.width - lineWidth - paddingH
		}

		layout.PrimaryBoxes = append(layout.PrimaryBoxes, LayoutBox{
			X:      lineX,
			Y:      currentY,
			Width:  lineWidth,
			Height: measurements.LineHeight,
		})

		currentY += measurements.LineSpacing
	}

	// Position secondary label based on LabelPosition
	if model.Secondary != "" {
		var secX, secY int

		if model.LabelPosition == module.LabelPositionRight {
			// Position label to the right of primary text
			// Find the widest primary line to position label after it
			maxPrimaryWidth := 0
			for _, width := range measurements.PrimaryWidths {
				if width > maxPrimaryWidth {
					maxPrimaryWidth = width
				}
			}

			// Position label right after the widest primary line
			// Module specifies exact spacing via LabelOffsetX
			secX = contentStartX + maxPrimaryWidth

			// Debug logging for label positioning
			if model.Secondary == "Network" {
				r.logger.Debug("network label positioning",
					"contentStartX", contentStartX,
					"maxPrimaryWidth", maxPrimaryWidth,
					"labelOffsetX", model.LabelOffsetX,
					"finalSecX", secX+model.LabelOffsetX,
				)
			}

			// Vertically center with primary text block
			secY = textBlockStartY + (measurements.PrimaryHeight-measurements.SecondaryHeight)/2
		} else {
			// Default: position below primary
			// (currentY already has spacing from last line)
			currentY = currentY - measurements.LineSpacing + measurements.LineHeight + primarySecondarySpacing
			secY = currentY

			// Match alignment with primary
			if hasIcon || r.align == AlignLeft {
				secX = contentStartX
			} else if r.align == AlignCenter {
				secX = (r.width - measurements.SecondaryWidth) / 2
			} else {
				secX = r.width - measurements.SecondaryWidth - paddingH
			}
		}

		// Apply custom offsets from module
		layout.SecondaryBox = &LayoutBox{
			X:      secX + model.LabelOffsetX,
			Y:      secY + model.LabelOffsetY,
			Width:  measurements.SecondaryWidth,
			Height: 10,
		}
	}

	return layout
}

// Render renders a payload to an image buffer with flexible layout system
func (r *Renderer) Render(payload module.Payload) (*image.RGBA, error) {
	// Validate payload
	if err := payload.Validate(); err != nil {
		return nil, err
	}

	// Create image buffer
	img := image.NewRGBA(image.Rect(0, 0, r.width, r.height))

	// Step 1: Fill main background
	draw.Draw(img, img.Bounds(), &image.Uniform{r.theme.GetBgColor()}, image.Point{}, draw.Src)

	// Step 2: Draw subtle zone background for visual separation
	r.drawZoneBackground(img)

	// Step 3: Parse payload into content model
	model := r.parsePayload(payload)

	// Step 4: Draw graph as atmospheric background (if present)
	if len(model.GraphData) > 0 {
		primaryColor := r.getSeverityColor(payload.Severity)
		r.drawBackgroundGraph(img, model.GraphData, model.GraphType, primaryColor, model.NormalizeGraph, model.GraphBgOpacity, model.GraphLineOpacity)
	}

	// Step 5: Measure and calculate layout
	measurements := r.measureContent(model)
	layout := r.calculateLayout(model, measurements)

	// Step 6: Get colors
	primaryColor := r.getSeverityColor(payload.Severity)
	secondaryColor := r.theme.GetMutedColor()

	// Step 7: Render icon at calculated position
	if layout.IconBox != nil && model.Icon != nil {
		// Add baseline offset for proper font rendering
		r.drawIconGlyph(img, model.Icon.Glyph, primaryColor,
			layout.IconBox.X, layout.IconBox.Y+14, r.iconFace)
	}

	// Step 8: Render primary lines at calculated positions
	// Use multi-line font if there are multiple lines
	primaryFont := r.primaryFace
	baselineOffset := 24 // For 24pt font
	if len(model.PrimaryLines) > 1 {
		primaryFont = r.multiLineFace
		baselineOffset = 16 // For 16pt font
	}

	for i, box := range layout.PrimaryBoxes {
		if i < len(model.PrimaryLines) {
			// Add baseline offset for proper font rendering
			r.drawRawText(img, model.PrimaryLines[i], primaryColor,
				box.X, box.Y+baselineOffset, primaryFont)
		}
	}

	// Step 9: Render secondary at calculated position
	if layout.SecondaryBox != nil {
		// Add baseline offset for proper font rendering
		r.drawRawText(img, model.Secondary, secondaryColor,
			layout.SecondaryBox.X, layout.SecondaryBox.Y+10, r.secondaryFace)
	}

	// Step 10: Render progress bar if present (bottom-aligned)
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
	// Align icon baseline with primary text baseline (adjusted for better alignment)
	point := fixed.Point26_6{
		X: fixed.I(baselineX),
		Y: fixed.I(baselineY),
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
	case module.GraphTypeLine:
		r.drawLineGraph(img, data, col)
	default: // module.GraphTypeSparkline
		r.drawSparkline(img, data, col)
	}
}

// drawSparkline renders a line graph at the bottom of the zone
func (r *Renderer) drawSparkline(img *image.RGBA, data []float32, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	const sparkHeight = 30  // Much taller for prominence
	const paddingH = 6
	const paddingV = 6      // More space at bottom

	availableWidth := r.width - (2 * paddingH)
	yBase := r.height - paddingV

	// Fully opaque and vibrant
	sparkColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 255}

	// Thicker line (4 pixels for smooth appearance)
	lineThickness := 4

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

	const sparkHeight = 30  // Taller bars
	const paddingH = 6
	const paddingV = 6
	const barSpacing = 1    // Small gap between bars

	// Calculate bar width
	availableWidth := r.width - (2 * paddingH)
	barWidth := (availableWidth - (len(data) * barSpacing)) / len(data)
	if barWidth < 2 {
		barWidth = 2  // Minimum 2px bars
	}

	// Draw bars from bottom
	yBase := r.height - paddingV

	// Fully opaque and vibrant
	sparkColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 255}

	for i, value := range data {
		if value < 0 {
			value = 0
		}
		if value > 1 {
			value = 1
		}

		barHeight := int(float32(sparkHeight) * value)
		x := paddingH + (i * (barWidth + barSpacing))

		// Draw bar
		for py := yBase - barHeight; py < yBase; py++ {
			for px := x; px < x+barWidth && px < r.width-paddingH; px++ {
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

	const sparkHeight = 30  // Taller area
	const paddingH = 6
	const paddingV = 6

	availableWidth := r.width - (2 * paddingH)
	yBase := r.height - paddingV

	// More prominent fill
	fillColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 180}
	// Fully opaque thick line for definition
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

// drawLineGraph renders a heartbeat-style line graph with ultra-thin crisp line
func (r *Renderer) drawLineGraph(img *image.RGBA, data []float32, col color.RGBA) {
	if len(data) == 0 {
		return
	}

	const sparkHeight = 30
	const paddingH = 6
	const paddingV = 6

	availableWidth := r.width - (2 * paddingH)
	yBase := r.height - paddingV

	// Draw a single 1-pixel line like an EKG monitor
	lineColor := color.RGBA{R: col.R, G: col.G, B: col.B, A: 255}

	// Only draw pixels, not lines - this ensures truly 1-pixel thickness
	for i := 0; i < len(data); i++ {
		val := clampFloat(data[i])
		x := paddingH + (i * availableWidth / len(data))
		y := yBase - int(float32(sparkHeight)*val)

		// Set single pixel
		if x >= 0 && x < r.width && y >= 0 && y < r.height {
			img.Set(x, y, lineColor)
		}
	}
}

// blendColors performs alpha blending
func (r *Renderer) blendColors(bg, fg color.RGBA) color.RGBA {
	alpha := float32(fg.A) / 255.0
	invAlpha := 1.0 - alpha

	return color.RGBA{
		R: uint8(float32(fg.R)*alpha + float32(bg.R)*invAlpha),
		G: uint8(float32(fg.G)*alpha + float32(bg.G)*invAlpha),
		B: uint8(float32(fg.B)*alpha + float32(bg.B)*invAlpha),
		A: 255,
	}
}

func clampFloat(v float32) float32 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
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

// Modern Dashboard Rendering Methods

// drawZoneBackground fills the zone with a subtle background color for visual separation
func (r *Renderer) drawZoneBackground(img *image.RGBA) {
	zoneBg := r.theme.GetZoneBgColor()
	draw.Draw(img, img.Bounds(), &image.Uniform{zoneBg}, image.Point{}, draw.Src)
}

// drawBackgroundGraph renders graph as full-height atmospheric background
func (r *Renderer) drawBackgroundGraph(img *image.RGBA, data []float32, graphType module.GraphType, col color.RGBA, normalize bool, moduleBgOpacity, moduleLineOpacity int) {
	if len(data) == 0 {
		return
	}

	const paddingH = 16
	const paddingV = 8

	// Full height minus vertical padding
	graphHeight := r.height - (2 * paddingV)
	availableWidth := r.width - (2 * paddingH)
	yBase := r.height - paddingV

	r.logger.Debug("graph position", "type", graphType, "width", r.width, "height", r.height, "img_bounds", img.Bounds(), "graphHeight", graphHeight, "paddingV", paddingV, "yBase", yBase, "normalize", normalize)

	// Get opacity values - prioritize per-module values, then theme, then defaults
	// Default to very subtle (3% fill, 8% line) unless overridden
	bgOpacity := moduleBgOpacity
	if bgOpacity == 0 {
		bgOpacity = r.theme.GraphBgOpacity
	}
	if bgOpacity == 0 {
		bgOpacity = 3  // Default: extremely subtle background
	}

	lineOpacity := moduleLineOpacity
	if lineOpacity == 0 {
		lineOpacity = r.theme.GraphLineOpacity
	}
	if lineOpacity == 0 {
		lineOpacity = 8  // Default: very subtle line
	}

	// Create very transparent fill color (default 8% opacity)
	fillColor := color.RGBA{
		R: col.R,
		G: col.G,
		B: col.B,
		A: uint8(bgOpacity * 255 / 100),
	}

	// Create semi-transparent line color (default 20% opacity)
	lineColor := color.RGBA{
		R: col.R,
		G: col.G,
		B: col.B,
		A: uint8(lineOpacity * 255 / 100),
	}

	// Render based on graph type - all rendered as atmospheric backgrounds
	switch graphType {
	case module.GraphTypeArea:
		r.drawBackgroundAreaGraph(img, data, fillColor, lineColor, paddingH, paddingV, graphHeight, availableWidth, yBase, normalize)
	case module.GraphTypeBar:
		r.drawBackgroundBarGraph(img, data, fillColor, paddingH, paddingV, graphHeight, availableWidth, yBase, normalize)
	case module.GraphTypeLine:
		r.drawBackgroundLineGraph(img, data, lineColor, paddingH, paddingV, graphHeight, availableWidth, yBase, normalize)
	default: // Sparkline
		r.drawBackgroundSparkline(img, data, lineColor, paddingH, paddingV, graphHeight, availableWidth, yBase, normalize)
	}
}

// drawBackgroundAreaGraph renders area graph as atmospheric background
func (r *Renderer) drawBackgroundAreaGraph(img *image.RGBA, data []float32,
	fillColor, lineColor color.RGBA, paddingH, paddingV, height, width, yBase int, normalize bool) {

	// Find min/max for normalization if requested
	minVal := float32(0.0)
	maxVal := float32(1.0)
	valRange := float32(1.0)

	if normalize {
		minVal = float32(1.0)
		maxVal = float32(0.0)
		for _, value := range data {
			val := clampFloat(value)
			if val < minVal {
				minVal = val
			}
			if val > maxVal {
				maxVal = val
			}
		}

		// Normalize range to fill from baseline
		valRange = maxVal - minVal
		if valRange < 0.01 {
			valRange = 1.0 // Avoid division by zero
		}
	}

	// First pass: Fill area with transparency
	for i := 0; i < len(data); i++ {
		val := clampFloat(data[i])
		normalizedVal := (val - minVal) / valRange
		x := paddingH + (i * width / len(data))
		y := yBase - int(float32(height)*normalizedVal)

		// Fill column from bottom to value with alpha blending
		for py := y; py <= yBase; py++ {
			if x >= 0 && x < r.width && py >= 0 && py < r.height {
				existing := img.At(x, py)
				blended := r.blendColors(existing.(color.RGBA), fillColor)
				img.Set(x, py, blended)
			}
		}
	}

	// Second pass: Draw top line
	for i := 0; i < len(data)-1; i++ {
		val1 := clampFloat(data[i])
		val2 := clampFloat(data[i+1])
		normalizedVal1 := (val1 - minVal) / valRange
		normalizedVal2 := (val2 - minVal) / valRange

		x1 := paddingH + (i * width / len(data))
		x2 := paddingH + ((i + 1) * width / len(data))
		y1 := yBase - int(float32(height)*normalizedVal1)
		y2 := yBase - int(float32(height)*normalizedVal2)

		r.drawBlendedLine(img, x1, y1, x2, y2, lineColor)
	}
}

// drawBackgroundBarGraph renders bars as atmospheric background
func (r *Renderer) drawBackgroundBarGraph(img *image.RGBA, data []float32,
	fillColor color.RGBA, paddingH, paddingV, height, width, yBase int, normalize bool) {

	const barSpacing = 1
	barWidth := (width - (len(data) * barSpacing)) / len(data)
	if barWidth < 2 {
		barWidth = 2
	}

	// Find min/max for normalization if requested
	minVal := float32(0.0)
	maxVal := float32(1.0)
	valRange := float32(1.0)

	if normalize {
		minVal = float32(1.0)
		maxVal = float32(0.0)
		for _, value := range data {
			val := clampFloat(value)
			if val < minVal {
				minVal = val
			}
			if val > maxVal {
				maxVal = val
			}
		}

		// Normalize range to fill from baseline
		valRange = maxVal - minVal
		if valRange < 0.01 {
			valRange = 1.0 // Avoid division by zero
		}
	}

	for i, value := range data {
		val := clampFloat(value)
		// Normalize to 0-1 range based on min/max in dataset (if normalize=true)
		normalizedVal := (val - minVal) / valRange
		barHeight := int(float32(height) * normalizedVal)
		x := paddingH + (i * (barWidth + barSpacing))

		// Draw bar with alpha blending - starts from yBase
		for py := yBase - barHeight; py <= yBase; py++ {
			for px := x; px < x+barWidth && px < r.width-paddingH; px++ {
				if px >= 0 && px < r.width && py >= 0 && py < r.height {
					existing := img.At(px, py)
					blended := r.blendColors(existing.(color.RGBA), fillColor)
					img.Set(px, py, blended)
				}
			}
		}
	}
}

// drawBackgroundLineGraph renders line graph as atmospheric background
func (r *Renderer) drawBackgroundLineGraph(img *image.RGBA, data []float32,
	lineColor color.RGBA, paddingH, paddingV, height, width, yBase int, normalize bool) {

	// Find min/max for normalization so graph fills from baseline
	minVal := float32(1.0)
	maxVal := float32(0.0)
	for _, value := range data {
		val := clampFloat(value)
		if val < minVal {
			minVal = val
		}
		if val > maxVal {
			maxVal = val
		}
	}

	// Normalize range to fill from baseline
	valRange := maxVal - minVal
	if valRange < 0.01 {
		valRange = 1.0 // Avoid division by zero
	}

	for i := 0; i < len(data)-1; i++ {
		val1 := clampFloat(data[i])
		val2 := clampFloat(data[i+1])
		normalizedVal1 := (val1 - minVal) / valRange
		normalizedVal2 := (val2 - minVal) / valRange

		x1 := paddingH + (i * width / len(data))
		x2 := paddingH + ((i + 1) * width / len(data))
		y1 := yBase - int(float32(height)*normalizedVal1)
		y2 := yBase - int(float32(height)*normalizedVal2)

		r.drawBlendedLine(img, x1, y1, x2, y2, lineColor)
	}

	// Draw baseline to ensure all line graphs are visually aligned at yBase
	for x := paddingH; x < paddingH+width; x++ {
		if x >= 0 && x < r.width && yBase >= 0 && yBase < r.height {
			existing := img.At(x, yBase)
			blended := r.blendColors(existing.(color.RGBA), lineColor)
			img.Set(x, yBase, blended)
		}
	}
}

// drawBackgroundSparkline renders sparkline as atmospheric background
func (r *Renderer) drawBackgroundSparkline(img *image.RGBA, data []float32,
	lineColor color.RGBA, paddingH, paddingV, height, width, yBase int, normalize bool) {

	// Find min/max for normalization so graph fills from baseline
	minVal := float32(1.0)
	maxVal := float32(0.0)
	for _, value := range data {
		val := clampFloat(value)
		if val < minVal {
			minVal = val
		}
		if val > maxVal {
			maxVal = val
		}
	}

	// Normalize range to fill from baseline
	valRange := maxVal - minVal
	if valRange < 0.01 {
		valRange = 1.0 // Avoid division by zero
	}

	// Thinner line for sparkline background (2 pixels instead of 4)
	lineThickness := 2

	for i := 0; i < len(data)-1; i++ {
		val1 := clampFloat(data[i])
		val2 := clampFloat(data[i+1])
		normalizedVal1 := (val1 - minVal) / valRange
		normalizedVal2 := (val2 - minVal) / valRange

		x1 := paddingH + (i * width / len(data))
		x2 := paddingH + ((i + 1) * width / len(data))
		y1 := yBase - int(float32(height)*normalizedVal1)
		y2 := yBase - int(float32(height)*normalizedVal2)

		// Draw thin line
		for offset := -lineThickness / 2; offset <= lineThickness/2; offset++ {
			r.drawBlendedLine(img, x1, y1+offset, x2, y2+offset, lineColor)
		}
	}

	// Draw baseline to ensure all sparklines are visually aligned at yBase
	for x := paddingH; x < paddingH+width; x++ {
		if x >= 0 && x < r.width && yBase >= 0 && yBase < r.height {
			existing := img.At(x, yBase)
			blended := r.blendColors(existing.(color.RGBA), lineColor)
			img.Set(x, yBase, blended)
		}
	}
}

// drawBlendedLine draws a line with alpha blending
func (r *Renderer) drawBlendedLine(img *image.RGBA, x1, y1, x2, y2 int, col color.RGBA) {
	// Bresenham's algorithm with alpha blending
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
			existing := img.At(x1, y1)
			blended := r.blendColors(existing.(color.RGBA), col)
			img.Set(x1, y1, blended)
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

// drawCenteredPrimaryText draws large hero text vertically centered
func (r *Renderer) drawCenteredPrimaryText(img *image.RGBA, text string, col color.RGBA, paddingH, paddingV int) {
	if r.primaryFace == nil {
		return
	}

	// Calculate vertical centering (approximately center of 48px height)
	// With 24pt font, baseline should be around y=28-30 for good centering
	baselineY := r.height/2 + 6

	// Calculate horizontal position based on alignment
	textWidth := font.MeasureString(r.primaryFace, text).Ceil()
	var x int
	switch r.align {
	case AlignCenter:
		x = (r.width - textWidth) / 2
	case AlignRight:
		x = r.width - textWidth - paddingH
	default: // AlignLeft
		x = paddingH
	}

	r.drawRawText(img, text, col, x, baselineY, r.primaryFace)
}

// drawSecondaryLabel draws small contextual label below primary
func (r *Renderer) drawSecondaryLabel(img *image.RGBA, text string, col color.RGBA, paddingH, paddingV int) {
	if r.secondaryFace == nil {
		return
	}

	// Position below primary text (around y=38-40)
	baselineY := r.height/2 + 16

	// Calculate horizontal position based on alignment
	textWidth := font.MeasureString(r.secondaryFace, text).Ceil()
	var x int
	switch r.align {
	case AlignCenter:
		x = (r.width - textWidth) / 2
	case AlignRight:
		x = r.width - textWidth - paddingH
	default: // AlignLeft
		x = paddingH
	}

	r.drawRawText(img, text, col, x, baselineY, r.secondaryFace)
}

// drawIcon draws icon with smart positioning (positioned in top-right corner)
func (r *Renderer) drawIcon(img *image.RGBA, glyph string, col color.RGBA, paddingH, paddingV int) {
	if r.iconFace == nil || glyph == "" {
		return
	}

	// Measure icon width to position from right edge
	iconWidth := font.MeasureString(r.iconFace, glyph).Ceil()

	// Position icon in top-right corner, above the text
	iconX := r.width - iconWidth - paddingH
	iconY := paddingV + 14 // Top area, with proper baseline

	r.drawIconGlyph(img, glyph, col, iconX, iconY, r.iconFace)
}

// shouldShowIcon determines if icon should be displayed based on smart visibility rules
func (r *Renderer) shouldShowIcon(payload module.Payload) bool {
	// Show icon if no graph data (graph tells the story)
	hasGraph := len(payload.Spark) > 0

	// Also show icon if it's semantically meaningful
	isMeaningful := payload.Icon == "cloud" ||
		payload.Icon == "cloud-rain" ||
		payload.Icon == "sun" ||
		payload.Severity != module.SeverityOK

	// Show if no graph OR if icon is meaningful
	return !hasGraph || isMeaningful
}
