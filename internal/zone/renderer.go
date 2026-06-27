package zone

import (
	"image"
	"image/color"
	"log/slog"
	"strings"
	"time"

	"github.com/fogleman/gg"
	"golang.org/x/image/font"

	"github.com/mantonx/nexus-open/internal/design"
	"github.com/mantonx/nexus-open/internal/fonts"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

const (
	marqueeScrollPx   = 30.0            // pixels per second
	marqueePauseStart = 3 * time.Second
	marqueePauseEnd   = 2 * time.Second
)

// marqueePhase tracks which part of the scroll cycle a text slot is in.
type marqueePhase int

const (
	marqueePhasingStart marqueePhase = iota // showing ellipsis, waiting to scroll
	marqueePhaseScrolling                   // scrolling right-to-left
	marqueePhasingEnd                       // tail visible, waiting to reset
)

// marqueeState holds the scroll animation state for a single text slot.
type marqueeState struct {
	text     string
	phase    marqueePhase
	offset   float64   // current scroll offset in pixels
	phaseAt  time.Time // when the current phase started
	lastTick time.Time
}

// advance updates offset/phase based on elapsed time and full text width.
func (s *marqueeState) advance(fullW, maxW float64) {
	now := time.Now()
	if s.lastTick.IsZero() {
		s.lastTick = now
		s.phaseAt = now
		return
	}
	dt := now.Sub(s.lastTick).Seconds()
	s.lastTick = now

	switch s.phase {
	case marqueePhasingStart:
		if now.Sub(s.phaseAt) >= marqueePauseStart {
			s.phase = marqueePhaseScrolling
			s.phaseAt = now
		}
	case marqueePhaseScrolling:
		s.offset += marqueeScrollPx * dt
		maxScroll := fullW - maxW
		if s.offset >= maxScroll {
			s.offset = maxScroll
			s.phase = marqueePhasingEnd
			s.phaseAt = now
		}
	case marqueePhasingEnd:
		if now.Sub(s.phaseAt) >= marqueePauseEnd {
			s.offset = 0
			s.phase = marqueePhasingStart
			s.phaseAt = now
		}
	}
}

// Renderer renders a single zone from a Payload using fogleman/gg.
type Renderer struct {
	logger  *slog.Logger
	theme   Theme
	width   int
	height  int
	align   Alignment

	primaryFace   font.Face // value text — design.SizeValue (22pt)
	unitFace      font.Face // unit suffix — design.SizeUnit (11pt)
	labelFace     font.Face // zone label — design.SizeLabel (10pt)
	multiLineFace font.Face // multi-line values (network speeds)
	secondaryFace font.Face // kept for legacy callers; same as labelFace
	iconFace      font.Face // FontAwesome fallback (may be nil)

	primarySize   float64
	secondarySize float64

	// Layout baselines pre-scaled to zone width (set in NewRenderer / reloadFaces).
	padL       float64
	padR       float64
	labelY     float64 // secondary label baseline
	valueY     float64 // primary value baseline (split layout)
	valueYSolo float64 // primary value baseline (single-line, no label)
	iconSize   float64 // icon font size
	multiSize  float64 // multi-line font size

	// Per-slot marquee scroll state. Keys are "primary" and "secondary".
	marquee map[string]*marqueeState
}

// IsAnimating returns true if any text slot is actively scrolling or in its
// end pause. The start pause (showing static ellipsis) does not require
// continuous re-renders — the frame is static until scrolling begins.
func (r *Renderer) IsAnimating() bool {
	for _, s := range r.marquee {
		if s.phase != marqueePhasingStart {
			return true
		}
	}
	return false
}

// UpdateTheme replaces the renderer's theme for all subsequent Render calls.
func (r *Renderer) UpdateTheme(theme Theme) {
	r.theme = theme
	basePrimary := float64(theme.FontSizePrimary)
	if basePrimary == 0 {
		basePrimary = float64(design.SizeValue)
	}
	baseSecondary := float64(theme.FontSizeSecondary)
	if baseSecondary == 0 {
		baseSecondary = float64(design.SizeLabel)
	}
	r.primarySize = basePrimary
	r.secondarySize = baseSecondary
	r.reloadFaces()
}

// NewRenderer creates a new zone renderer.
func NewRenderer(logger *slog.Logger, theme Theme, width, height int, align Alignment) *Renderer {
	r := &Renderer{
		logger: logger,
		theme:  theme,
		width:  width,
		height: height,
		align:  align,
	}

	// Font sizes are fixed to design tokens — the 48px display height is constant
	// regardless of zone width. Truncation handles overflow, not font shrinkage.
	basePrimary := float64(theme.FontSizePrimary)
	if basePrimary == 0 {
		basePrimary = float64(design.SizeValue)
	}
	baseSecondary := float64(theme.FontSizeSecondary)
	if baseSecondary == 0 {
		baseSecondary = float64(design.SizeLabel)
	}

	r.primarySize = basePrimary
	r.secondarySize = baseSecondary
	r.multiSize = 8.5
	r.iconSize = 18

	// Baselines from design tokens (native 640×48 coordinate space).
	r.padL = 8
	r.padR = 6
	r.labelY = float64(design.LabelBaselineY) // 15
	r.valueY = float64(design.ValueBaselineY) // 38
	r.valueYSolo = 32                         // centred when no label — between 15 and 38

	r.marquee = make(map[string]*marqueeState)
	r.reloadFaces()
	return r
}

// reloadFaces loads font faces via fonts.Manager, which already applies
// HintingFull internally via truetype.NewFace.
func (r *Renderer) reloadFaces() {
	fm := fonts.NewManager(r.logger)

	load := func(size float64) font.Face {
		if face, _, err := fm.LoadBestAvailableFont(size); err == nil {
			return face
		}
		return nil
	}

	r.primaryFace = load(r.primarySize)

	// Unit and label sizes are fixed to spec tokens, not scaled from theme.
	r.unitFace = load(float64(design.SizeUnit))   // 11pt
	r.labelFace = load(float64(design.SizeLabel)) // 10pt
	r.secondaryFace = r.labelFace                 // alias for legacy callers

	multiSize := r.multiSize
	if multiSize == 0 {
		multiSize = 12
	}
	r.multiLineFace = load(multiSize)

	iconSize := r.iconSize
	if iconSize == 0 {
		iconSize = 18
	}
	if face, err := fm.GetFace("FontAwesome-Solid", iconSize); err == nil {
		r.iconFace = face
	}
}

// ── Public API ────────────────────────────────────────────────────────────────

// Render renders a payload into a zone-sized RGBA image.
func (r *Renderer) Render(payload plugin.Payload) (*image.RGBA, error) {
	if err := payload.Validate(); err != nil {
		return nil, err
	}

	// Pre-rendered frame: blit raw RGBA pixels directly, skipping all layout.
	if len(payload.RawFrame) == r.width*r.height*4 {
		img := image.NewRGBA(image.Rect(0, 0, r.width, r.height))
		copy(img.Pix, payload.RawFrame)
		return img, nil
	}

	dc := gg.NewContext(r.width, r.height)

	// Layer 1: solid background.
	bg := r.theme.GetBgColor()
	dc.SetColor(bg)
	dc.Clear()

	// Parse payload.
	primary := strings.Split(payload.Primary, "\n")
	isMulti := len(primary) > 1

	// Text colour: severity drives warn/crit; neutral value colour for OK state.
	var textColor color.RGBA
	switch payload.Severity {
	case plugin.SeverityWarn, plugin.SeverityCrit:
		textColor = r.severityColor(payload.Severity)
	default:
		textColor = design.Value
	}

	// Combo layout: label+value inline at top, full-height graph below.
	if payload.GraphType == plugin.GraphTypeCombo {
		r.drawComboGraph(dc, payload, textColor)
		r.drawComboContent(dc, payload.Secondary, payload.Value, payload.ValueUnit, textColor)
	} else {
		// Layer 2: graph fill + line (if spark data present).
		if len(payload.Spark) > 0 {
			r.drawGraph(dc, payload, design.Info)
		}

		// hasGraph triggers the split layout (value moves up to make room).
		// segmented and bar_thresh live in the bottom strip — value stays at y=38.
		bottomStripOnly := payload.GraphType == plugin.GraphTypeSegmented ||
			payload.GraphType == plugin.GraphTypeBarThresh
		hasGraph := len(payload.Spark) > 0 && !bottomStripOnly

		// Layer 3b: caption (rate readout etc.) — info-blue, small, between label and sparkline.
		// When a caption is present with a graph, draw only the label+caption; skip
		// the normal value layout so the caption IS the data readout.
		if payload.Caption != "" && hasGraph {
			r.drawLabel(dc, payload.Secondary)
			r.drawCaption(dc, payload.Caption)
		} else {
			// Layer 4: text content.
			r.drawContent(dc, primary, isMulti, payload.Secondary, payload.Icon, payload.Value, payload.ValueUnit, textColor, hasGraph)
		}
	}

	// Layer 5: progress bar (optional, bottom edge).
	if payload.Progress > 0 {
		r.drawProgressBar(dc, payload.Progress, r.theme.GetAccentColor())
	}

	// Layer 6: dog-ear affordance — top-right corner fold indicating a tappable zone.
	if payload.Expandable {
		r.drawDogEar(dc)
	}

	return dc.Image().(*image.RGBA), nil
}

// drawDogEar draws a folded-corner affordance in the top-right of the zone.
// The triangle is accent blue with a bright hypotenuse highlight and a dark
// inner crease to give the illusion of a physical page fold.
func (r *Renderer) drawDogEar(dc *gg.Context) {
	const size = 17.0
	w := float64(r.width)

	// Main triangle.
	dc.NewSubPath()
	dc.MoveTo(w-size, 0)
	dc.LineTo(w, 0)
	dc.LineTo(w, size)
	dc.ClosePath()
	dc.SetColor(design.Info)
	dc.Fill()

	// Dark inner crease — 1px inside the hypotenuse.
	dc.SetRGBA(
		float64(design.Info.R)/255*0.3,
		float64(design.Info.G)/255*0.3,
		float64(design.Info.B)/255*0.3,
		0.9,
	)
	dc.SetLineWidth(1)
	dc.DrawLine(w-size+2, 0, w, size-2)
	dc.Stroke()

	// Bright highlight along the hypotenuse.
	dc.SetRGBA(0.78, 0.87, 1.0, 0.75)
	dc.SetLineWidth(1)
	dc.DrawLine(w-size, 0, w, size)
	dc.Stroke()
}

// ── Layer renderers ───────────────────────────────────────────────────────────

// drawLabel renders the zone label at the standard top-left position (y=15).
func (r *Renderer) drawLabel(dc *gg.Context, text string) {
	if text == "" {
		return
	}
	if r.labelFace != nil {
		dc.SetFontFace(r.labelFace)
	}
	lr := float64(design.Label.R) / 255
	lg := float64(design.Label.G) / 255
	lb := float64(design.Label.B) / 255
	dc.SetRGB(lr, lg, lb)
	dc.DrawString(text, r.padL, r.labelY)
}

// drawCaption renders a small info-blue annotation line between the label (y=15)
// and the sparkline (y≈32). Used for rate readouts like "↓222K ↑221K".
func (r *Renderer) drawCaption(dc *gg.Context, text string) {
	if r.multiLineFace != nil {
		dc.SetFontFace(r.multiLineFace)
	}
	vr := float64(design.Value.R) / 255
	vg := float64(design.Value.G) / 255
	vb := float64(design.Value.B) / 255
	dc.SetRGB(vr, vg, vb)
	dc.DrawString(text, r.padL, 27)
}


// drawContent draws the primary value, optional icon, and secondary label.
// When value+valueUnit are non-empty they are rendered as two runs at different
// sizes/colours; otherwise primary[0] is rendered as a single fused string.
func (r *Renderer) drawContent(dc *gg.Context, primary []string, isMulti bool,
	secondary, icon, value, valueUnit string, textColor color.RGBA, hasGraph bool) {

	padL := r.padL
	padR := r.padR

	tr := float64(textColor.R) / 255
	tg := float64(textColor.G) / 255
	tb := float64(textColor.B) / 255

	// Label colour: token label grey, independent of severity.
	lr := float64(design.Label.R) / 255
	lg := float64(design.Label.G) / 255
	lb := float64(design.Label.B) / 255

	W := float64(r.width)
	H := float64(r.height)

	// Split layout: used when there is a label+graph, OR when the primary
	// contains multiple lines (multi-source plugins).
	if (hasGraph && secondary != "") || isMulti {
		r.drawSplitLayout(dc, primary, isMulti, secondary, icon, value, valueUnit, tr, tg, tb, lr, lg, lb, W, H)
		return
	}

	// Single-line layout (no graph, or no label).
	if r.primaryFace != nil {
		dc.SetFontFace(r.primaryFace)
	}

	// Display-only zones (clock etc): full vertical + horizontal centring.
	isDisplayOnly := secondary == "" && !hasGraph
	if isDisplayOnly && r.align == AlignCenter {
		text := r.truncate(primary[0], W-padL*2, dc)
		dc.SetRGB(tr, tg, tb)
		dc.DrawStringAnchored(text, W/2, H/2, 0.5, 0.5)
		return
	}

	// Standard layout: label near top-left, value at bottom.
	if secondary != "" {
		if r.labelFace != nil {
			dc.SetFontFace(r.labelFace)
		}
		dc.SetRGB(lr, lg, lb)
		r.drawMarquee(dc, "secondary", secondary, padL, r.labelY, W-padL-padR)
		if r.primaryFace != nil {
			dc.SetFontFace(r.primaryFace)
		}
	}

	valueBaseline := r.valueY
	if secondary == "" {
		valueBaseline = r.valueYSolo
	}

	x := padL

	if icon != "" && r.iconFace != nil && r.align != AlignCenter {
		glyph := resolveIconGlyph(icon)
		if glyph != "" {
			dc.SetFontFace(r.iconFace)
			dc.SetRGB(tr, tg, tb)
			dc.DrawString(glyph, x, valueBaseline)
			gw, _ := dc.MeasureString(glyph)
			x += gw + 2
			if r.primaryFace != nil {
				dc.SetFontFace(r.primaryFace)
			}
		}
	}

	if value != "" {
		r.drawValueUnit(dc, value, valueUnit, x, valueBaseline, textColor, hasGraph)
	} else {
		maxW := W - padL - padR
		dc.SetRGB(tr, tg, tb)
		r.drawMarquee(dc, "primary", primary[0], x, valueBaseline, maxW)
	}
}

// drawValueUnit renders value in the primary face (textColor) then valueUnit
// immediately after in the unit face (design.Unit colour, smaller).
func (r *Renderer) drawValueUnit(dc *gg.Context, value, valueUnit string, x, baseline float64, textColor color.RGBA, hasGraph bool) {
	tr := float64(textColor.R) / 255
	tg := float64(textColor.G) / 255
	tb := float64(textColor.B) / 255

	if r.primaryFace != nil {
		dc.SetFontFace(r.primaryFace)
	}
	vw, vh := dc.MeasureString(value)

	if hasGraph {
		unitW := 0.0
		if valueUnit != "" && r.unitFace != nil {
			dc.SetFontFace(r.unitFace)
			unitW, _ = dc.MeasureString(valueUnit)
			dc.SetFontFace(r.primaryFace)
		}
		dc.SetRGBA(0, 0, 0, 0.28)
		dc.DrawRectangle(x-2, baseline-vh, vw+unitW+4, vh+2)
		dc.Fill()
	}

	if r.primaryFace != nil {
		dc.SetFontFace(r.primaryFace)
	}
	dc.SetRGB(tr, tg, tb)
	dc.DrawString(value, x, baseline)

	if valueUnit != "" {
		ur := float64(design.Unit.R) / 255
		ug := float64(design.Unit.G) / 255
		ub := float64(design.Unit.B) / 255
		if r.unitFace != nil {
			dc.SetFontFace(r.unitFace)
		}
		dc.SetRGB(ur, ug, ub)
		dc.DrawString(valueUnit, x+vw+2, baseline)
	}
}

// drawProgressBar draws a 2px bar at the bottom edge.
func (r *Renderer) drawProgressBar(dc *gg.Context, progress float32, col color.RGBA) {
	if progress <= 0 {
		return
	}
	if progress > 1 {
		progress = 1
	}
	const h = 2.0
	const padH = 4.0
	W := float64(r.width)
	H := float64(r.height)
	filled := (W - padH*2) * float64(progress)

	// Track.
	dc.SetRGBA(float64(col.R)/255, float64(col.G)/255, float64(col.B)/255, 0.2)
	dc.DrawRectangle(padH, H-h-2, W-padH*2, h)
	dc.Fill()

	// Fill.
	dc.SetRGB(float64(col.R)/255, float64(col.G)/255, float64(col.B)/255)
	dc.DrawRectangle(padH, H-h-2, filled, h)
	dc.Fill()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// drawSpaced draws text with extra letter-spacing (extraPx pixels between glyphs).
// The current draw colour must already be set on dc before calling.
// drawSplitLayout renders a zone with:
//   - Label vertically centred on the LEFT third of the zone
//   - Value (+ icon) right-anchored on the RIGHT two-thirds
//
// Works for both single-line and multi-line (network) values.
func (r *Renderer) drawSplitLayout(dc *gg.Context, primary []string, isMulti bool,
	secondary, icon, value, valueUnit string,
	tr, tg, tb, lr, lg, lb float64,
	W, H float64) {

	padL := r.padL
	padR := r.padR
	labelY := r.labelY
	valueY := r.valueY

	// All split-layout zones: label top-left, icon+value below.
	if r.labelFace != nil {
		dc.SetFontFace(r.labelFace)
	}
	label := r.truncateSpaced(dc, secondary, W-padL-padR, 1.0)
	dc.SetRGB(lr, lg, lb)
	r.drawSpaced(dc, label, padL, labelY, 1.0)

	if isMulti {
		// Network: two speed lines right-anchored, spaced within the lower half.
		if r.multiLineFace != nil {
			dc.SetFontFace(r.multiLineFace)
		}
		rightX := W - padR - 8
		spacing := (H - labelY) / 3
		baselines := []float64{labelY + spacing, labelY + spacing*2}
		for i, line := range primary {
			if i >= 2 {
				break
			}
			text := r.truncate(line, W-padL-padR, dc)
			dc.SetRGB(tr, tg, tb)
			dc.DrawStringAnchored(text, rightX, baselines[i], 1.0, 0)
		}
		return
	}

	// Single line: icon + value centred horizontally below the label.
	var iw float64
	var glyph string
	if icon != "" && r.iconFace != nil {
		glyph = resolveIconGlyph(icon)
		if glyph != "" {
			dc.SetFontFace(r.iconFace)
			iw, _ = dc.MeasureString(glyph)
		}
	}

	const iconGap = 4.0

	if value != "" {
		// Measure value+unit group to centre it.
		if r.primaryFace != nil {
			dc.SetFontFace(r.primaryFace)
		}
		vw, _ := dc.MeasureString(value)
		unitW := 0.0
		if valueUnit != "" && r.unitFace != nil {
			dc.SetFontFace(r.unitFace)
			unitW, _ = dc.MeasureString(valueUnit)
		}
		groupW := iw + iconGap + vw + unitW
		startX := (W - groupW) / 2
		if glyph != "" && r.iconFace != nil {
			dc.SetFontFace(r.iconFace)
			dc.SetRGB(tr, tg, tb)
			dc.DrawString(glyph, startX, valueY)
		}
		textColor := color.RGBA{R: uint8(tr * 255), G: uint8(tg * 255), B: uint8(tb * 255), A: 255}
		r.drawValueUnit(dc, value, valueUnit, startX+iw+iconGap, valueY, textColor, true)
		return
	}

	// Fallback: fused primary string.
	if r.primaryFace != nil {
		dc.SetFontFace(r.primaryFace)
	}
	valueText := r.truncate(primary[0], W-padL*2, dc)
	vw, _ := dc.MeasureString(valueText)
	groupW := iw + iconGap + vw
	startX := (W - groupW) / 2
	if glyph != "" && r.iconFace != nil {
		dc.SetFontFace(r.iconFace)
		dc.SetRGB(tr, tg, tb)
		dc.DrawString(glyph, startX, valueY)
	}
	if r.primaryFace != nil {
		dc.SetFontFace(r.primaryFace)
	}
	dc.SetRGB(tr, tg, tb)
	dc.DrawString(valueText, startX+iw+iconGap, valueY)
}

func (r *Renderer) drawSpaced(dc *gg.Context, text string, x, y, extraPx float64) {
	for _, ch := range text {
		s := string(ch)
		dc.DrawString(s, x, y)
		w, _ := dc.MeasureString(s)
		x += w + extraPx
	}
}

// measureSpaced returns the total pixel width of text rendered with drawSpaced.
func (r *Renderer) measureSpaced(dc *gg.Context, text string, extraPx float64) float64 {
	var total float64
	runes := []rune(text)
	for i, ch := range runes {
		w, _ := dc.MeasureString(string(ch))
		total += w
		if i < len(runes)-1 {
			total += extraPx
		}
	}
	return total
}

// truncateSpaced clips text accounting for per-character spacing, appending "…" if needed.
func (r *Renderer) truncateSpaced(dc *gg.Context, text string, maxWidth, extraPx float64) string {
	if r.measureSpaced(dc, text, extraPx) <= maxWidth {
		return text
	}
	ellipsis := "…"
	ew, _ := dc.MeasureString(ellipsis)
	runes := []rune(text)
	for len(runes) > 0 {
		runes = runes[:len(runes)-1]
		if r.measureSpaced(dc, string(runes), extraPx)+ew <= maxWidth {
			return string(runes) + ellipsis
		}
	}
	return ellipsis
}

// truncate clips text to maxWidth pixels using the current dc font, appending
// "…" if needed.
func (r *Renderer) truncate(text string, maxWidth float64, dc *gg.Context) string {
	if r.primaryFace == nil {
		return text
	}
	w, _ := dc.MeasureString(text)
	if w <= maxWidth {
		return text
	}
	ellipsis := "…"
	ew, _ := dc.MeasureString(ellipsis)
	runes := []rune(text)
	for len(runes) > 0 {
		runes = runes[:len(runes)-1]
		w, _ = dc.MeasureString(string(runes))
		if w+ew <= maxWidth {
			return string(runes) + ellipsis
		}
	}
	return ellipsis
}

// drawMarquee draws text at (x, y) clipped to maxW pixels wide.
// If the text fits, it draws normally and clears any scroll state for the slot.
// If it overflows, it shows the ellipsis during the start pause, then scrolls
// the full text, then pauses at the end before resetting.
// slot is a stable key ("primary" or "secondary") identifying this text position.
func (r *Renderer) drawMarquee(dc *gg.Context, slot, text string, x, y, maxW float64) {
	fullW, _ := dc.MeasureString(text)
	if fullW <= maxW {
		delete(r.marquee, slot)
		dc.DrawString(text, x, y)
		return
	}

	// Get or create scroll state; reset if text changed.
	s, ok := r.marquee[slot]
	if !ok || s.text != text {
		s = &marqueeState{text: text}
		r.marquee[slot] = s
	}
	s.advance(fullW, maxW)

	if s.phase == marqueePhasingStart {
		// Show truncated text with ellipsis during the initial pause.
		dc.DrawString(r.truncate(text, maxW, dc), x, y)
		return
	}

	// Scroll and end-pause phases: clip to [x, x+maxW] and draw full text
	// shifted left by the current offset. DrawRectangle+Clip uses the current
	// path as a clip mask; ResetClip removes it afterwards.
	dc.DrawRectangle(x, 0, maxW, float64(r.height))
	dc.Clip()
	dc.DrawString(text, x-s.offset, y)
	dc.ResetClip()
}

// severityColor maps payload severity to a text colour.
func (r *Renderer) severityColor(sev plugin.Severity) color.RGBA {
	switch sev {
	case plugin.SeverityWarn:
		return SeverityColorWarn
	case plugin.SeverityCrit:
		return SeverityColorCrit
	default:
		return r.theme.GetFgColor()
	}
}

// resolveIconGlyph returns icon if it is a single Unicode codepoint, otherwise "".
// Plugins are responsible for passing raw FA codepoints — the core does not
// maintain a name-to-glyph mapping.
func resolveIconGlyph(icon string) string {
	if len([]rune(icon)) == 1 {
		return icon
	}
	return ""
}

// clampFloat clamps a float32 to [0, 1].
func clampFloat(v float32) float32 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}

