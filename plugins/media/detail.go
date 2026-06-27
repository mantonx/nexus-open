package main

import (
	_ "embed"
	"fmt"
	"image"
	"image/color"
	_ "image/jpeg"
	_ "image/png"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/fogleman/gg"
	"github.com/golang/freetype/truetype"
	"golang.org/x/image/draw"
	"golang.org/x/image/font"
	"golang.org/x/image/font/gofont/gobold"
	"golang.org/x/image/font/gofont/goregular"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

//go:embed assets/fonts/FontAwesome-Solid.ttf
var faSolidTTF []byte

func fetchArt(artURL string, size int) (image.Image, error) {
	if artURL == "" {
		return nil, nil
	}

	var r io.ReadCloser
	if path, ok := strings.CutPrefix(artURL, "file://"); ok {
		f, err := os.Open(path)
		if err != nil {
			return nil, nil
		}
		r = f
	} else {
		client := &http.Client{Timeout: 3 * time.Second}
		resp, err := client.Get(artURL)
		if err != nil || resp.StatusCode != 200 {
			if resp != nil {
				resp.Body.Close()
			}
			return nil, nil
		}
		r = resp.Body
	}
	defer r.Close()

	src, _, err := image.Decode(r)
	if err != nil {
		return nil, nil
	}

	dst := image.NewRGBA(image.Rect(0, 0, size, size))
	draw.BiLinear.Scale(dst, dst.Bounds(), src, src.Bounds(), draw.Over, nil)
	return dst, nil
}

func (m *MediaPlugin) OnTap() (plugin.DetailPayload, error) {
	m.mu.Lock()
	track := m.lastTrack
	m.mu.Unlock()

	if track == nil {
		return plugin.DetailPayload{Title: "Media"}, nil
	}

	artURL := track.ArtURL
	if artURL == "" && track.Title != "" {
		artURL = tmdb.posterURL(track.Title)
	}
	art, _ := fetchArt(artURL, 48)

	frame, err := renderDetailFrame(track, art)
	if err != nil {
		return plugin.DetailPayload{}, err
	}
	return plugin.DetailPayload{
		Title:    track.Title + " — " + track.Artist,
		RawFrame: frame,
	}, nil
}

func renderDetailFrame(t *TrackInfo, art image.Image) ([]byte, error) {
	const (
		w = 640
		h = 48

		artW     = 48.0
		gap      = 7.0
		contentX = artW + gap // ~55

		dividerX  = 400.0 // vertical separator
		rightX    = 410.0 // right panel content start
		rightMaxX = 626.0 // right panel content end (before × hint)

		padX = 10.0

		titleSize = 14.0
		metaSize  = 10.5
		rightSize = 10.5
		iconSize  = 10.0

		barH = 3.0
		barY = float64(h) - barH - 3
	)

	bg      := color.RGBA{R: 0x14, G: 0x16, B: 0x1A, A: 0xFF}
	panelBg := color.RGBA{R: bg.R + 18, G: bg.G + 18, B: bg.B + 18, A: 0xFF} // bg lightened ~8%
	accent  := color.RGBA{R: 0x4C, G: 0x9F, B: 0xFF, A: 0xFF}
	white   := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF}
	dim     := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xB3}
	dimmer  := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x77}
	barBg   := color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x33}

	dc := gg.NewContext(w, h)
	dc.SetColor(panelBg)
	dc.Clear()

	// Load fonts.
	titleFace, err := loadTTFFace(gobold.TTF, titleSize)
	if err != nil {
		return nil, fmt.Errorf("load title font: %w", err)
	}
	defer titleFace.Close()

	metaFace, err := loadTTFFace(goregular.TTF, metaSize)
	if err != nil {
		return nil, fmt.Errorf("load meta font: %w", err)
	}
	defer metaFace.Close()
	rightFace := metaFace

	faFace, err := loadTTFFace(faSolidTTF, iconSize)
	if err != nil {
		return nil, fmt.Errorf("load FA font: %w", err)
	}
	defer faFace.Close()

	// Album art — 48×48 flush left, with a smooth gradient fade over the right
	// 12px so the art blends into the panel background rather than hard-cutting.
	// When no art is available, shift content left to avoid a blank gap.
	hasArt := art != nil
	if hasArt {
		dc.DrawImage(art, 0, 0)
		const fadeW = 12
		for i := range fadeW {
			alpha := uint8(float64(i+1) / float64(fadeW) * 0xCC)
			dc.SetColor(color.RGBA{R: bg.R, G: bg.G, B: bg.B, A: alpha})
			dc.DrawRectangle(float64(artW-fadeW+i), 0, 1, float64(h))
			dc.Fill()
		}
	}

	// × hint top-right — visual affordance for long-press dismiss.
	dc.SetFontFace(faFace)
	dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x44})
	dc.DrawStringAnchored(string(rune(0xf00d)), float64(w)-padX, 11, 1.0, 0.5)

	// Vertical divider — same color as the main screen zone gutters: RGB(80,84,92).
	// Stops before the progress bar row so it doesn't cross it.
	dividerEnd := int(barY) - 3
	for y := range dividerEnd {
		t := float64(y) / float64(dividerEnd-1)
		dist := t - 0.5
		a := 1.0 - (dist*dist)*3.5
		if a < 0.12 {
			a = 0.12
		}
		alpha := uint8(a * 180)
		dc.SetColor(color.RGBA{R: 80, G: 84, B: 92, A: alpha})
		dc.SetPixel(int(dividerX), y)
	}

	// ── Left panel: title, artist, progress ──────────────────────────────────

	// Shift content left when there's no album art so it doesn't start mid-screen.
	leftX := contentX
	if !hasArt {
		leftX = 10
	}
	leftMaxX := dividerX - gap

	hasProgress := t.Length > 0
	titleY := 16.0
	metaY := 29.0
	if !hasProgress {
		titleY = 24.0
		metaY = 38.0
	}

	dc.SetFontFace(titleFace)
	dc.SetColor(white)
	dc.DrawString(truncate(dc, t.Title, leftMaxX-leftX), leftX, titleY)

	// Only show the meta row if there's something meaningful to show.
	meta := ""
	switch {
	case t.Artist != "" && t.Player != "":
		meta = t.Artist + "  ·  " + t.Player
	case t.Artist != "":
		meta = t.Artist
	case t.Player != "":
		meta = t.Player
	}
	if meta != "" {
		dc.SetFontFace(metaFace)
		dc.SetColor(dim)
		dc.DrawString(truncate(dc, meta, leftMaxX-leftX), leftX, metaY)
	}

	// Progress bar — only rendered when the player reports a non-zero length.
	// Firefox and some streams expose position/length as empty strings which
	// parse to zero; showing 0:00 – 0:00 is misleading, so we skip it entirely.
	if t.Length > 0 {
		posStr := formatDuration(t.Position)
		lenStr := formatDuration(t.Length)

		dc.SetFontFace(metaFace)
		posW, _ := dc.MeasureString(posStr)
		lenW, _ := dc.MeasureString(lenStr)

		posX := leftX
		lenX := rightMaxX - lenW
		barX := posX + posW + gap
		barW := lenX - gap - barX

		dc.SetColor(dim)
		dc.DrawString(posStr, posX, float64(h)-5)
		dc.DrawString(lenStr, lenX, float64(h)-5)

		if barW > 0 {
			dc.SetColor(barBg)
			dc.DrawRoundedRectangle(barX, barY, barW, barH, barH/2)
			dc.Fill()

			filled := barW * float64(t.Position) / float64(t.Length)
			if filled > barW {
				filled = barW
			}
			if filled > 0 {
				dc.SetColor(accent)
				dc.DrawRoundedRectangle(barX, barY, filled, barH, barH/2)
				dc.Fill()
			}
		}
	}

	// ── Right panel: context-aware layout ────────────────────────────────────
	//
	// Music with album:  album name top, status below — grouped near center.
	// Music without album (streams/radio): status vertically centered.
	// Video/movie: title is already in left panel; right shows duration format
	//   in H:MM:SS when length > 1 hour, and status centered.

	hasAlbum := t.Album != ""

	// Status icon and label.
	statusIcon := rune(0xf04b) // ▶
	statusLabel := "Playing"
	switch t.Status {
	case "Paused":
		statusIcon = rune(0xf04c) // ⏸
		statusLabel = "Paused"
	case "Stopped":
		statusIcon = rune(0xf04d) // ■
		statusLabel = "Stopped"
	}

	var albumY, statusY float64
	if hasAlbum {
		albumY = 16
		statusY = 30
	} else if hasProgress {
		// Center between top and the progress bar row (~y=39).
		statusY = barY / 2
	} else {
		// No bar, no album — single line centered in 48px.
		statusY = 28.0
	}

	if hasAlbum {
		dc.SetFontFace(rightFace)
		dc.SetColor(dim)
		dc.DrawString(truncate(dc, t.Album, rightMaxX-rightX), rightX, albumY)
	}

	// Status: accent FA icon then dimmer label.
	dc.SetFontFace(faFace)
	dc.SetColor(accent)
	iconStr := string(statusIcon)
	iconW, _ := dc.MeasureString(iconStr)
	dc.DrawString(iconStr, rightX, statusY)

	dc.SetFontFace(rightFace)
	dc.SetColor(dimmer)
	dc.DrawString(statusLabel, rightX+iconW+5, statusY)

	// Pixel extraction.
	img := dc.Image()
	pix := make([]byte, w*h*4)
	for y := range h {
		for x := range w {
			r, g, b, a := img.At(x, y).RGBA()
			base := (y*w + x) * 4
			pix[base] = uint8(r >> 8)
			pix[base+1] = uint8(g >> 8)
			pix[base+2] = uint8(b >> 8)
			pix[base+3] = uint8(a >> 8)
		}
	}
	return pix, nil
}

// formatDuration converts microseconds to M:SS or H:MM:SS for content over an hour.
func formatDuration(us int64) string {
	s := us / 1_000_000
	h := s / 3600
	m := (s % 3600) / 60
	sec := s % 60
	if h > 0 {
		return fmt.Sprintf("%d:%02d:%02d", h, m, sec)
	}
	return fmt.Sprintf("%d:%02d", m, sec)
}

func truncate(dc *gg.Context, s string, maxW float64) string {
	w, _ := dc.MeasureString(s)
	if w <= maxW {
		return s
	}
	ellipsis := "…"
	for len(s) > 0 {
		runes := []rune(s)
		s = string(runes[:len(runes)-1])
		w, _ = dc.MeasureString(s + ellipsis)
		if w <= maxW {
			return s + ellipsis
		}
	}
	return ellipsis
}

func loadTTFFace(data []byte, size float64) (font.Face, error) {
	f, err := truetype.Parse(data)
	if err != nil {
		return nil, err
	}
	return truetype.NewFace(f, &truetype.Options{Size: size, DPI: 72, Hinting: font.HintingFull}), nil
}

