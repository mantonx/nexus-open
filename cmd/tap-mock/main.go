// tap-mock generates three 640×48 PNG mock frames showing:
//  1. mock_idle.png       — normal display with the touch-dot affordance on the weather zone
//  2. mock_tap_flash.png  — tap ripple at peak (~50ms), composited over the idle frame
//  3. mock_transition.png — slide-up mid-point (~50% progress) between idle and detail
//
// Usage:
//
//	go run ./cmd/tap-mock [-token TOKEN] [-out DIR]
//
// The live frame is fetched from /api/debug/frame (requires the daemon to be
// running). TOKEN defaults to reading ~/.config/nexus-open/token.
package main

import (
	"flag"
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"math"
	"net/http"
	"os"
	"path/filepath"

	"github.com/fogleman/gg"
	"github.com/mantonx/nexus-open/internal/design"
)

func main() {
	tokenFlag := flag.String("token", "", "API capability token (default: ~/.config/nexus-open/token)")
	outDir := flag.String("out", ".", "Output directory for PNG files")
	flag.Parse()

	token := *tokenFlag
	if token == "" {
		home, _ := os.UserHomeDir()
		data, err := os.ReadFile(filepath.Join(home, ".config/nexus-open/token"))
		if err != nil {
			fmt.Fprintf(os.Stderr, "error reading token: %v\n", err)
			os.Exit(1)
		}
		token = string(data)
	}

	base, err := fetchFrame(token)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error fetching frame: %v\n", err)
		os.Exit(1)
	}

	// Weather zone is slot 1 (0-indexed). Slot x-origin = slot*(SlotWidth+SlotGap).
	// design.SlotWidth=100, design.SlotGap=8 → slot 1 starts at x=108.
	weatherSlotOriginX := design.SlotWidth + design.SlotGap // 108
	tapX := float64(weatherSlotOriginX + design.TouchDotX)  // 108+92 = 200
	tapY := float64(design.TouchDotY)                       // 42

	detail, err := fetchDetailFrame(token)
	if err != nil {
		fmt.Fprintf(os.Stderr, "warning: could not fetch detail frame (%v), using synthetic\n", err)
		detail = renderSyntheticDetail()
	}

	idle := renderIdle(base, tapX, tapY)
	flash := renderTapFlash(idle, tapX, tapY)
	trans := renderTransition(idle, detail)

	for name, img := range map[string]*image.RGBA{
		"mock_idle.png":       idle,
		"mock_tap_flash.png":  flash,
		"mock_transition.png": trans,
	} {
		path := filepath.Join(*outDir, name)
		if err := writePNG(path, img); err != nil {
			fmt.Fprintf(os.Stderr, "error writing %s: %v\n", path, err)
			os.Exit(1)
		}
		fmt.Printf("wrote %s\n", path)
	}
}

// fetchFrame fetches the live 640×48 display frame from the daemon.
func fetchFrame(token string) (*image.RGBA, error) {
	req, _ := http.NewRequest("GET", "http://localhost:1985/api/debug/frame", nil)
	req.Header.Set("X-Nexus-Token", token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("GET /api/debug/frame: %w", err)
	}
	defer resp.Body.Close() //nolint:errcheck
	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("GET /api/debug/frame: status %d", resp.StatusCode)
	}
	img, err := png.Decode(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("decode PNG: %w", err)
	}
	rgba := image.NewRGBA(img.Bounds())
	draw.Draw(rgba, rgba.Bounds(), img, image.Point{}, draw.Src)
	return rgba, nil
}

// renderIdle draws the touch-dot affordance on the weather zone.
func renderIdle(base *image.RGBA, dotX, dotY float64) *image.RGBA {
	out := cloneRGBA(base)
	dc := gg.NewContextForRGBA(out)
	dc.SetColor(design.TouchDot)
	dc.DrawCircle(dotX, dotY, design.TouchDotRadius)
	dc.Fill()
	return out
}

// renderTapFlash composites a ripple ring at peak expansion (~t=50ms).
// Renders as a thin expanding ring with a soft outer glow — not a filled disc,
// so the zone content stays legible underneath.
func renderTapFlash(base *image.RGBA, cx, cy float64) *image.RGBA {
	const (
		ringRadius = 22.0 // px — ring centre radius at peak
		ringWidth  = 4.0  // stroke width of the ring
		glowRadius = 10.0 // soft outer halo radius
		ringAlpha  = 0.75 // ring stroke opacity
		glowAlpha  = 0.25 // outer glow opacity
	)
	out := cloneRGBA(base)

	accent := color.RGBA{R: 0x4C, G: 0x9F, B: 0xFF, A: 0xFF}
	ar := float64(accent.R) / 255
	ag := float64(accent.G) / 255
	ab := float64(accent.B) / 255

	// Paint onto a blank overlay, then alpha-composite onto the cloned base.
	overlay := image.NewRGBA(out.Bounds())
	ov := gg.NewContextForRGBA(overlay)

	// Outer glow — concentric stroked circles expanding outward from the ring,
	// each progressively wider and more transparent. Strokes produce annuli
	// naturally without any fill bleeding into the centre.
	innerGlow := ringRadius + ringWidth/2
	for step := 0.0; step < glowRadius; step += 1.0 {
		t := step / glowRadius // 0=ring edge, 1=outer edge
		a := glowAlpha * (1.0 - t)
		ov.SetRGBA(ar, ag, ab, a)
		ov.SetLineWidth(1.5)
		ov.DrawCircle(cx, cy, innerGlow+step)
		ov.Stroke()
	}

	// Ring stroke.
	ov.SetRGBA(ar, ag, ab, ringAlpha)
	ov.SetLineWidth(ringWidth)
	ov.DrawCircle(cx, cy, ringRadius)
	ov.Stroke()

	draw.Draw(out, out.Bounds(), overlay, image.Point{}, draw.Over)

	return out
}

// fetchDetailFrame fetches the pre-rendered detail overlay from the daemon.
func fetchDetailFrame(token string) (*image.RGBA, error) {
	req, _ := http.NewRequest("GET", "http://localhost:1985/api/debug/render-detail", nil)
	req.Header.Set("X-Nexus-Token", token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("GET /api/debug/render-detail: %w", err)
	}
	defer resp.Body.Close() //nolint:errcheck
	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("GET /api/debug/render-detail: status %d", resp.StatusCode)
	}
	img, err := png.Decode(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("decode PNG: %w", err)
	}
	rgba := image.NewRGBA(img.Bounds())
	draw.Draw(rgba, rgba.Bounds(), img, image.Point{}, draw.Src)
	return rgba, nil
}

// renderTransition renders a 50% slide-up transition between idle and detail.
func renderTransition(idle, detail *image.RGBA) *image.RGBA {
	progress := 0.5
	offset := int(math.Round(float64(design.DisplayHeight) * (1.0 - progress)))

	out := image.NewRGBA(image.Rect(0, 0, design.DisplayWidth, design.DisplayHeight))
	// Idle frame — visible in the top portion.
	draw.Draw(out, image.Rect(0, 0, design.DisplayWidth, offset), idle, image.Point{}, draw.Src)
	// Detail frame — sliding up from the bottom.
	draw.Draw(out,
		image.Rect(0, offset, design.DisplayWidth, design.DisplayHeight),
		detail,
		image.Point{Y: offset},
		draw.Src,
	)

	return out
}

// renderSyntheticDetail produces a minimal 640×48 "detail" frame that looks
// like a 7-day forecast panel — enough for the mock without a network call.
func renderSyntheticDetail() *image.RGBA {
	const (
		w    = design.DisplayWidth
		h    = design.DisplayHeight
		colW = w / 7
	)

	bg := color.RGBA{R: 0x14, G: 0x16, B: 0x1A, A: 0xFF}
	panelBg := lerpColor(bg, color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF}, 0.08)
	accent := color.RGBA{R: 0x4C, G: 0x9F, B: 0xFF, A: 0xFF}
	dim := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x33}
	labelCol := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xB3}
	tempHi := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF}
	tempLo := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x73}

	days := []struct {
		label string
		icon  string // FA solid codepoints rendered as placeholder boxes
		hi    string
		lo    string
	}{
		{"Today", "☀", "90°F", "74"},
		{"Fri", "⛅", "87°F", "71"},
		{"Sat", "🌧", "78°F", "65"},
		{"Sun", "☀", "85°F", "68"},
		{"Mon", "☁", "82°F", "66"},
		{"Tue", "⛅", "84°F", "69"},
		{"Wed", "☀", "88°F", "72"},
	}

	dc := gg.NewContext(w, h)
	dc.SetColor(panelBg)
	dc.Clear()

	for i, day := range days {
		x := float64(i*colW) + float64(colW)/2.0
		isToday := i == 0

		if isToday {
			dc.SetColor(lerpColor(panelBg, accent, 0.13))
			dc.DrawRoundedRectangle(float64(i*colW)+2, 2, float64(colW)-4, float64(h)-4, 4)
			dc.Fill()
		}

		if i > 0 {
			dc.SetColor(dim)
			dc.DrawLine(float64(i*colW), 6, float64(i*colW), float64(h)-6)
			dc.SetLineWidth(1)
			dc.Stroke()
		}

		// Day label
		if isToday {
			dc.SetColor(accent)
		} else {
			dc.SetColor(labelCol)
		}
		dc.DrawStringAnchored(day.label, x, 4, 0.5, 1.0)

		// Icon (emoji fallback — real impl uses FA glyphs)
		dc.DrawStringAnchored(day.icon, x, 18, 0.5, 1.0)

		// Temps
		dc.SetColor(tempHi)
		dc.DrawStringAnchored(day.hi, x-4, 40, 1.0, 0.0)
		dc.SetColor(tempLo)
		dc.DrawStringAnchored(day.lo, x+4, 40, 0.0, 0.0)
	}

	// Close button
	dc.SetColor(color.RGBA{R: 0x00, G: 0x00, B: 0x00, A: 0x80})
	dc.DrawCircle(float64(design.DisplayWidth-10), 10, 9)
	dc.Fill()
	dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF})
	dc.DrawStringAnchored("✕", float64(design.DisplayWidth-10), 10, 0.5, 0.5)

	out := image.NewRGBA(image.Rect(0, 0, w, h))
	draw.Draw(out, out.Bounds(), dc.Image(), image.Point{}, draw.Src)
	return out
}

func cloneRGBA(src *image.RGBA) *image.RGBA {
	dst := image.NewRGBA(src.Bounds())
	copy(dst.Pix, src.Pix)
	return dst
}

func writePNG(path string, img image.Image) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	if err := png.Encode(f, img); err != nil {
		_ = f.Close()
		return err
	}
	return f.Close()
}

func lerpColor(a, b color.RGBA, t float64) color.RGBA {
	lerp := func(x, y uint8) uint8 { return uint8(float64(x) + (float64(y)-float64(x))*t) }
	return color.RGBA{R: lerp(a.R, b.R), G: lerp(a.G, b.G), B: lerp(a.B, b.B), A: 0xFF}
}
