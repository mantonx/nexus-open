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

	primaryFace   font.Face // scaled primary pt, HintingFull
	multiLineFace font.Face // scaled multi-line pt, HintingFull
	secondaryFace font.Face // scaled secondary pt, HintingFull
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
	scale := math.Min(1.0, float64(r.width)/refZoneWidth)
	basePrimary := float64(theme.FontSizePrimary)
	if basePrimary == 0 {
		basePrimary = 24
	}
	baseSecondary := float64(theme.FontSizeSecondary)
	if baseSecondary == 0 {
		baseSecondary = 9
	}
	r.primarySize = math.Max(10, math.Round(basePrimary*scale))
	r.secondarySize = math.Max(7, math.Round(baseSecondary*scale))
	r.reloadFaces()
}

// refZoneWidth is the zone width at which font sizes and baselines were originally
// tuned (4 zones across a 640px display). Narrower zones scale down proportionally.
const refZoneWidth = 160.0

// NewRenderer creates a new zone renderer.
func NewRenderer(logger *slog.Logger, theme Theme, width, height int, align Alignment) *Renderer {
	r := &Renderer{
		logger: logger,
		theme:  theme,
		width:  width,
		height: height,
		align:  align,
	}

	// Scale font sizes down for narrow zones; never scale up beyond the theme value.
	basePrimary := float64(theme.FontSizePrimary)
	if basePrimary == 0 {
		basePrimary = 24
	}
	baseSecondary := float64(theme.FontSizeSecondary)
	if baseSecondary == 0 {
		baseSecondary = 9
	}

	scale := math.Min(1.0, float64(width)/refZoneWidth)
	r.primarySize = math.Max(10, math.Round(basePrimary*scale))
	r.secondarySize = math.Max(7, math.Round(baseSecondary*scale))
	r.multiSize = math.Max(8, math.Round(12*scale))
	r.iconSize = math.Max(10, math.Round(18*scale))

	// Baselines tuned to the 48px zone height, scaled with zone width.
	// At refZoneWidth: padL=8, padR=6, labelY=12, valueY=36, valueYSolo=27.
	r.padL = math.Max(4, math.Round(8*scale))
	r.padR = math.Max(3, math.Round(6*scale))
	r.labelY = math.Round(float64(height) * 0.25)
	r.valueY = math.Round(float64(height) * 0.75)
	r.valueYSolo = math.Round(float64(height) * 0.5625) // ~27/48

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

	multiSize := r.multiSize
	if multiSize == 0 {
		multiSize = 12
	}
	r.multiLineFace = load(multiSize)

	// Secondary label: use at least 13pt for legibility; scale up from secondarySize.
	secLabelSize := math.Max(13, r.secondarySize+2)
	r.secondaryFace = load(secLabelSize)

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

	// Zone identity colour — comes from theme accent (set per-zone via ThemeOverride).
	accentColor := r.theme.GetAccentColor()
	// Text colour: accent colour always for OK state (zone identity is the accent);
	// severity override only for warn/crit states.
	var textColor color.RGBA
	switch payload.Severity {
	case plugin.SeverityWarn, plugin.SeverityCrit:
		textColor = r.severityColor(payload.Severity)
	default:
		textColor = accentColor
	}

	// Layer 2: zone tint (very subtle accent wash).
	r.drawTint(dc, accentColor, bg)

	// Layer 3: graph fill + line (if spark data present).
	if len(payload.Spark) > 0 {
		r.drawGraph(dc, payload, accentColor)
	}

	// Layer 4: text content.
	r.drawContent(dc, primary, isMulti, payload.Secondary, payload.Icon, textColor, len(payload.Spark) > 0)

	// Layer 5: progress bar (optional, bottom edge).
	if payload.Progress > 0 {
		r.drawProgressBar(dc, payload.Progress, textColor)
	}

	return dc.Image().(*image.RGBA), nil
}

// ── Layer renderers ───────────────────────────────────────────────────────────

// drawTint draws a very subtle accent-colour wash over the entire zone.
// Alpha scales with background darkness so it stays perceptible on any bg.
func (r *Renderer) drawTint(dc *gg.Context, accent, bg color.RGBA) {
	lum := (uint16(bg.R)*299 + uint16(bg.G)*587 + uint16(bg.B)*114) / 1000
	alpha := 14.0 - float64(lum)*8.0/255.0
	if alpha < 4 {
		alpha = 4
	}
	dc.SetRGBA(
		float64(accent.R)/255,
		float64(accent.G)/255,
		float64(accent.B)/255,
		alpha/255,
	)
	dc.DrawRectangle(0, 0, float64(r.width), float64(r.height))
	dc.Fill()
}

// drawGraph draws the Corsair-inspired graph: bottom-anchored gradient fill,
// glow halo composited under a crisp 1.5px line.
func (r *Renderer) drawGraph(dc *gg.Context, payload plugin.Payload, col color.RGBA) {
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
// Text is monochromatic: label uses a dimmed version of textColor (55% brightness),
// value uses the full textColor. Horizontal alignment follows r.align.
func (r *Renderer) drawContent(dc *gg.Context, primary []string, isMulti bool,
	secondary, icon string, textColor color.RGBA, hasGraph bool) {

	padL := r.padL
	padR := r.padR

	tr := float64(textColor.R) / 255
	tg := float64(textColor.G) / 255
	tb := float64(textColor.B) / 255

	// Label: accent colour at 72% — readable without competing with the value.
	const labelDim = 0.72
	lr := tr * labelDim
	lg := tg * labelDim
	lb := tb * labelDim

	W := float64(r.width)
	H := float64(r.height)

	// Split layout: used when there is a label+graph, OR when the primary
	// contains multiple lines (multi-source plugins). Multi-line primaries
	// always use this path so both lines render at their fixed baselines.
	if (hasGraph && secondary != "") || isMulti {
		r.drawSplitLayout(dc, primary, isMulti, secondary, icon, tr, tg, tb, lr, lg, lb, W, H)
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

	// Fallback: label near top-left, value in middle.
	if secondary != "" {
		if r.secondaryFace != nil {
			dc.SetFontFace(r.secondaryFace)
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
	ax := 0.0

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

	var valueX float64
	switch r.align {
	case AlignCenter:
		valueX = W / 2
		ax = 0.5
	case AlignRight:
		valueX = W - padR
		ax = 1.0
	default:
		valueX = x
		ax = 0.0
	}

	maxW := W - padL - padR
	text := r.truncate(primary[0], maxW, dc)

	// Subtle dark scrim behind the value text so it reads over the gradient fill.
	// Kept faint so it doesn't look like a box — just darkens the gradient slightly.
	if hasGraph {
		tw, _ := dc.MeasureString(text)
		scrX := valueX
		switch ax {
		case 0.5:
			scrX = valueX - tw/2
		case 1.0:
			scrX = valueX - tw
		}
		scrX -= 2
		dc.SetRGBA(0, 0, 0, 0.28)
		dc.DrawRectangle(scrX, valueBaseline-22, tw+4, 24)
		dc.Fill()
	}

	dc.SetRGB(tr, tg, tb)
	dc.DrawStringAnchored(text, valueX, valueBaseline, ax, 0)
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
	secondary, icon string,
	tr, tg, tb, lr, lg, lb float64,
	W, H float64) {

	padL := r.padL
	padR := r.padR
	labelY := r.labelY
	valueY := r.valueY

	// All split-layout zones: label top-left, icon+value centred below.
	if r.secondaryFace != nil {
		dc.SetFontFace(r.secondaryFace)
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
		// Place two lines evenly in the lower 60% of the zone height.
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
	if r.primaryFace != nil {
		dc.SetFontFace(r.primaryFace)
	}
	valueText := r.truncate(primary[0], W-padL*2, dc)
	vw, _ := dc.MeasureString(valueText)

	var iw float64
	var glyph string
	if icon != "" && r.iconFace != nil {
		glyph = resolveIconGlyph(icon)
		if glyph != "" {
			dc.SetFontFace(r.iconFace)
			iw, _ = dc.MeasureString(glyph)
			dc.SetFontFace(r.primaryFace)
		}
	}

	const iconGap = 4.0
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

