package zone

import (
	"image"
	"image/color"
	"image/draw"
	"math"

	"github.com/fogleman/gg"
	"github.com/mantonx/nexus-open/internal/design"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

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
