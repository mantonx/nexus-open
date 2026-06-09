package zone

import (
	"image"
	"image/color"
	"image/draw"
	"log/slog"
	"math"
	"strings"

	"github.com/fogleman/gg"
	"golang.org/x/image/font"

	"github.com/mantonx/nexus-open/internal/design"
	"github.com/mantonx/nexus-open/internal/fonts"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

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
		r.drawProgressBar(dc, payload.Progress, textColor)
	}

	return dc.Image().(*image.RGBA), nil
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

// drawGraph dispatches to the appropriate graph renderer based on GraphType.
func (r *Renderer) drawGraph(dc *gg.Context, payload plugin.Payload, col color.RGBA) {
	switch payload.GraphType {
	case plugin.GraphTypeSegmented:
		r.drawSegmented(dc, payload.Spark, payload.Severity)
		return
	case plugin.GraphTypeBarThresh:
		r.drawBarThresh(dc, payload.Spark, payload.Severity)
		return
	}
	r.drawSparkline(dc, payload, col)
}

// drawSparkline draws the Corsair-inspired graph: bottom-anchored gradient fill,
// glow halo composited under a crisp 1.5px line.
func (r *Renderer) drawSparkline(dc *gg.Context, payload plugin.Payload, col color.RGBA) {
	data := payload.Spark
	n := len(data)
	if n < 2 {
		return
	}

	// Normalise if requested.
	vals := make([]float32, n)
	copy(vals, data)
	if payload.NormalizeGraph {
		var mx float32
		for _, v := range vals {
			if v > mx {
				mx = v
			}
		}
		if mx > 0 {
			for i := range vals {
				vals[i] /= mx
			}
		}
	}
	for i, v := range vals {
		vals[i] = clampFloat(v)
	}

	H := float64(r.height)
	W := float64(r.width)
	xStep := W / float64(n-1)

	// Graph is confined to the bottom portion of the zone so it never
	// overlaps the label at the top. graphTop is the highest y the line can reach.
	const graphTop = 32.0
	graphH := H - graphTop
	yOf := func(v float32) float64 { return H - float64(v)*graphH }

	ar := float64(col.R) / 255
	ag := float64(col.G) / 255
	ab := float64(col.B) / 255

	// ── Layer A: bottom-anchored gradient fill ────────────────────────────────
	// Find the y-range of the line so we can anchor the gradient to it.
	minY := H
	lineYs := make([]float64, n)
	for i, v := range vals {
		lineYs[i] = yOf(v)
		if lineYs[i] < minY {
			minY = lineYs[i]
		}
	}
	// Build a clipping mask matching the graph polygon, then paint gradient rows.
	clipDC := gg.NewContext(r.width, r.height)
	clipDC.MoveTo(0, H)
	for i := range vals {
		clipDC.LineTo(float64(i)*xStep, lineYs[i])
	}
	clipDC.LineTo(float64(n-1)*xStep, H)
	clipDC.ClosePath()
	clipDC.SetRGB(1, 1, 1)
	clipDC.Fill()
	mask := clipDC.Image().(*image.RGBA)

	// Paint gradient rows into a temp layer, then mask-composite onto dc.
	// Gradient is anchored to the bottom of the zone — always glows near the baseline
	// regardless of how high the line sits. The polygon mask clips anything above the line.
	const glowBand = 10.0 // px above baseline the fill covers — stays below the label area
	gradImg := image.NewRGBA(image.Rect(0, 0, r.width, r.height))
	fillStart := int(H - glowBand)
	if fillStart < int(minY) {
		fillStart = int(minY)
	}
	for py := fillStart; py < r.height; py++ {
		distFromBaseline := H - float64(py)
		t := 1.0 - (distFromBaseline / glowBand)
		if t < 0 {
			t = 0
		}
		alpha := math.Pow(t, 1.2) * 120
		if alpha > 120 {
			alpha = 120
		}
		a8 := uint8(alpha)
		cr := uint8(float64(col.R) * float64(a8) / 255)
		cg := uint8(float64(col.G) * float64(a8) / 255)
		cb := uint8(float64(col.B) * float64(a8) / 255)
		for px := 0; px < r.width; px++ {
			if mask.RGBAAt(px, py).R > 0 {
				gradImg.SetRGBA(px, py, color.RGBA{R: cr, G: cg, B: cb, A: a8})
			}
		}
	}
	// Composite gradient fill onto main context.
	draw.Draw(dc.Image().(*image.RGBA), dc.Image().Bounds(), gradImg, image.Point{}, draw.Over)

	// ── Layer B: glow halo ────────────────────────────────────────────────────
	// Render a 3px line onto a blank layer, box-blur it, composite at ~50% opacity.
	glowDC := gg.NewContext(r.width, r.height)
	glowDC.MoveTo(0, lineYs[0])
	for i := 1; i < n; i++ {
		glowDC.LineTo(float64(i)*xStep, lineYs[i])
	}
	glowDC.SetRGBA(ar, ag, ab, 1.0)
	glowDC.SetLineWidth(2.0)
	glowDC.SetLineCapRound()
	glowDC.SetLineJoinRound()
	glowDC.Stroke()
	glowLayer := glowDC.Image().(*image.RGBA)
	boxBlur(glowLayer, 2)
	blendAlpha(dc.Image().(*image.RGBA), glowLayer, 0.40)

	// ── Layer C: crisp line ───────────────────────────────────────────────────
	dc.MoveTo(0, lineYs[0])
	for i := 1; i < n; i++ {
		dc.LineTo(float64(i)*xStep, lineYs[i])
	}
	dc.SetRGBA(ar, ag, ab, 0.95)
	dc.SetLineWidth(1.5)
	dc.SetLineCapRound()
	dc.SetLineJoinRound()
	dc.Stroke()
}

// drawSegmented draws a fixed-width row of rounded segments. Filled segments
// are colored by value threshold; unfilled (future) slots are dim #2a2a2a.
// The row always spans the zone width using the data length as slot count
// (min 12 slots so the row is never sparse-looking).
func (r *Renderer) drawSegmented(dc *gg.Context, data []float32, sev plugin.Severity) {
	if len(data) == 0 {
		return
	}
	const segH = 5.0
	const gap = 2.0
	const rr = 1.5
	const minSlots = 12
	W := float64(r.width)
	H := float64(r.height)
	total := len(data)
	if total < minSlots {
		total = minSlots
	}
	segW := math.Max(4.0, (W-gap*float64(total-1))/float64(total))
	y := H - segH - 1
	dim := color.RGBA{R: 0x2a, G: 0x2a, B: 0x2a, A: 0xff}

	for i := 0; i < total; i++ {
		x := float64(i) * (segW + gap)
		var col color.RGBA
		if i < len(data) {
			v := data[i]
			switch {
			case v >= 0.85:
				col = design.Crit
			case v >= 0.6:
				col = design.Warn
			default:
				col = design.Ok
			}
		} else {
			col = dim
		}
		dc.SetColor(col)
		dc.DrawRoundedRectangle(x, y, segW, segH, rr)
		dc.Fill()
	}
}

// drawBarThresh draws a single horizontal fill bar with tick marks at the warn
// (70%) and crit (90%) thresholds. Bar colour follows the payload severity.
func (r *Renderer) drawBarThresh(dc *gg.Context, data []float32, sev plugin.Severity) {
	if len(data) == 0 {
		return
	}
	v := data[len(data)-1]
	const barH = 4.0
	const rr = 2.0
	W := float64(r.width)
	H := float64(r.height)
	y := H - barH - 1

	// Track.
	dc.SetColor(design.Divider)
	dc.DrawRoundedRectangle(0, y, W-2, barH, rr)
	dc.Fill()

	// Fill — colour by severity.
	var fillCol color.RGBA
	switch sev {
	case plugin.SeverityWarn:
		fillCol = design.Warn
	case plugin.SeverityCrit:
		fillCol = design.Crit
	default:
		fillCol = design.OkBar
	}
	filled := (W - 2) * float64(clampFloat(v))
	if filled > 0 {
		dc.SetColor(fillCol)
		dc.DrawRoundedRectangle(0, y, filled, barH, rr)
		dc.Fill()
	}

	// Threshold tick at 70% (warn) and 90% (crit).
	drawTick := func(pct float64, col color.RGBA) {
		x := (W - 2) * pct
		dc.SetColor(col)
		dc.DrawRectangle(x, y-1, 1, barH+2)
		dc.Fill()
	}
	// Mock: warn tick is neutral (#cfcfcf), crit tick is crit red.
	drawTick(0.70, color.RGBA{R: 0xcf, G: 0xcf, B: 0xcf, A: 0xff})
	drawTick(0.90, design.Crit)
}

// drawComboGraph draws load columns (fill=#262626) from loadSpark, then overlays
// the temperature sparkline in textColor. Graph occupies the bottom ~34px of the
// zone, leaving the top 14px for the inline label+value header.
func (r *Renderer) drawComboGraph(dc *gg.Context, payload plugin.Payload, textColor color.RGBA) {
	const graphTop = 14.0
	W := float64(r.width)
	H := float64(r.height)
	graphH := H - graphTop

	// Load columns — 4px wide with 2px gaps, anchored to bottom.
	load := payload.LoadSpark
	if len(load) > 0 {
		const colW = 4.0
		const colGap = 2.0
		col := color.RGBA{R: 0x26, G: 0x26, B: 0x26, A: 0xff}
		for i, v := range load {
			h := graphH * float64(clampFloat(v))
			if h < 1 {
				h = 1
			}
			x := r.padL + float64(i)*(colW+colGap)
			if x+colW > W {
				break
			}
			dc.SetColor(col)
			dc.DrawRectangle(x, H-h, colW, h)
			dc.Fill()
		}
	}

	// Temperature sparkline overlay.
	spark := payload.Spark
	if len(spark) >= 2 {
		n := len(spark)
		xStep := W / float64(n-1)
		yOf := func(v float32) float64 { return H - float64(v)*graphH }
		ar := float64(textColor.R) / 255
		ag := float64(textColor.G) / 255
		ab := float64(textColor.B) / 255
		dc.MoveTo(0, yOf(spark[0]))
		for i := 1; i < n; i++ {
			dc.LineTo(float64(i)*xStep, yOf(spark[i]))
		}
		dc.SetRGBA(ar, ag, ab, 0.95)
		dc.SetLineWidth(1.2)
		dc.SetLineCapRound()
		dc.SetLineJoinRound()
		dc.Stroke()
	}
}

// drawComboContent draws the inline header: label left-anchored and
// value+unit right-anchored, both on the same baseline (y=13).
// This matches the mock: "CPU" left, "71°" right, all within the top 14px.
func (r *Renderer) drawComboContent(dc *gg.Context, label, value, valueUnit string, textColor color.RGBA) {
	const headerY = 13.0
	W := float64(r.width)
	padL := r.padL
	padR := r.padR

	lr := float64(design.Label.R) / 255
	lg := float64(design.Label.G) / 255
	lb := float64(design.Label.B) / 255
	tr := float64(textColor.R) / 255
	tg := float64(textColor.G) / 255
	tb := float64(textColor.B) / 255

	// Label left.
	if r.labelFace != nil {
		dc.SetFontFace(r.labelFace)
	}
	dc.SetRGB(lr, lg, lb)
	dc.DrawString(label, padL, headerY)

	// Value+unit right-anchored.
	// Mock font-size=12 for this compact header value — use unitFace as proxy.
	if r.unitFace != nil {
		dc.SetFontFace(r.unitFace)
	}
	display := value
	if valueUnit != "" {
		display = value + valueUnit
	}
	vw, _ := dc.MeasureString(display)
	dc.SetRGB(tr, tg, tb)
	dc.DrawString(display, W-padR-vw, headerY)
}

// boxBlur applies a fast separable box blur of the given radius to img in-place.
func boxBlur(img *image.RGBA, radius int) {
	w, h := img.Bounds().Dx(), img.Bounds().Dy()
	tmp := image.NewRGBA(img.Bounds())

	// Horizontal pass.
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			var rr, gg, bb, aa int
			cnt := 0
			for dx := -radius; dx <= radius; dx++ {
				nx := x + dx
				if nx < 0 || nx >= w {
					continue
				}
				c := img.RGBAAt(nx, y)
				rr += int(c.R); gg += int(c.G); bb += int(c.B); aa += int(c.A)
				cnt++
			}
			if cnt > 0 {
				tmp.SetRGBA(x, y, color.RGBA{R: uint8(rr / cnt), G: uint8(gg / cnt), B: uint8(bb / cnt), A: uint8(aa / cnt)})
			}
		}
	}

	// Vertical pass.
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			var rr, gg, bb, aa int
			cnt := 0
			for dy := -radius; dy <= radius; dy++ {
				ny := y + dy
				if ny < 0 || ny >= h {
					continue
				}
				c := tmp.RGBAAt(x, ny)
				rr += int(c.R); gg += int(c.G); bb += int(c.B); aa += int(c.A)
				cnt++
			}
			if cnt > 0 {
				img.SetRGBA(x, y, color.RGBA{R: uint8(rr / cnt), G: uint8(gg / cnt), B: uint8(bb / cnt), A: uint8(aa / cnt)})
			}
		}
	}
}

// blendAlpha composites src onto dst at the given opacity (0–1).
func blendAlpha(dst, src *image.RGBA, opacity float64) {
	b := dst.Bounds()
	for y := b.Min.Y; y < b.Max.Y; y++ {
		for x := b.Min.X; x < b.Max.X; x++ {
			s := src.RGBAAt(x, y)
			if s.A == 0 {
				continue
			}
			d := dst.RGBAAt(x, y)
			sa := float64(s.A) * opacity / 255.0
			da := float64(d.A) / 255.0
			oa := sa + da*(1-sa)
			if oa == 0 {
				continue
			}
			dst.SetRGBA(x, y, color.RGBA{
				R: uint8((float64(s.R)*sa + float64(d.R)*da*(1-sa)) / oa),
				G: uint8((float64(s.G)*sa + float64(d.G)*da*(1-sa)) / oa),
				B: uint8((float64(s.B)*sa + float64(d.B)*da*(1-sa)) / oa),
				A: uint8(oa * 255),
			})
		}
	}
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
		label := r.truncateSpaced(dc, secondary, W-padL-padR, 1.0)
		dc.SetRGB(lr, lg, lb)
		r.drawSpaced(dc, label, padL, r.labelY, 1.0)
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
		var valueX float64
		ax := 0.0
		switch r.align {
		case AlignCenter:
			valueX = W / 2
			ax = 0.5
		case AlignRight:
			valueX = W - padR
			ax = 1.0
		default:
			valueX = x
		}
		maxW := W - padL - padR
		text := r.truncate(primary[0], maxW, dc)
		if hasGraph {
			tw, _ := dc.MeasureString(text)
			scrX := valueX
			switch ax {
			case 0.5:
				scrX = valueX - tw/2
			case 1.0:
				scrX = valueX - tw
			}
			dc.SetRGBA(0, 0, 0, 0.28)
			dc.DrawRectangle(scrX-2, valueBaseline-22, tw+4, 24)
			dc.Fill()
		}
		dc.SetRGB(tr, tg, tb)
		dc.DrawStringAnchored(text, valueX, valueBaseline, ax, 0)
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

