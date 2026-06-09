package main

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"image/color"
	"math"
	"net/http"
	"strings"
	"time"

	"github.com/fogleman/gg"
	"github.com/golang/freetype/truetype"
	"golang.org/x/image/font"
	"golang.org/x/image/font/gofont/goregular"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

//go:embed assets/fonts/FontAwesome-Solid.ttf
var faSolidTTF []byte

type dailyForecast struct {
	label string
	icon  string
	hi    float64
	lo    float64
}

// OnTap fetches the 7-day forecast and renders it as a 640×48 RGBA frame.
// Implements plugin.Tapper.
func (m *WeatherPlugin) OnTap() (plugin.DetailPayload, error) {
	m.mu.RLock()
	location := m.location
	unit := m.unit
	m.mu.RUnlock()

	lat, lon, err := m.getCityCoordinates(location)
	if err != nil {
		lat, lon = defaultLat, defaultLon
	}

	tempUnit := "celsius"
	if unit == "imperial" {
		tempUnit = "fahrenheit"
	}

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(fmt.Sprintf(openMeteoDailyURL, tempUnit, lat, lon))
	if err != nil {
		return plugin.DetailPayload{}, fmt.Errorf("fetch daily forecast: %w", err)
	}
	defer func() { _ = resp.Body.Close() }()

	var result struct {
		Daily struct {
			Time           []string  `json:"time"`
			WeatherCode    []int     `json:"weather_code"`
			TemperatureMax []float64 `json:"temperature_2m_max"`
			TemperatureMin []float64 `json:"temperature_2m_min"`
		} `json:"daily"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return plugin.DetailPayload{}, fmt.Errorf("decode daily forecast: %w", err)
	}

	days := result.Daily
	n := min(len(days.Time), 7)
	forecast := make([]dailyForecast, 0, n)
	for i := range n {
		label := days.Time[i]
		if t, err := time.Parse("2006-01-02", days.Time[i]); err == nil {
			if i == 0 {
				label = "Today"
			} else {
				label = t.Format("Mon")
			}
		}
		var code int
		if i < len(days.WeatherCode) {
			code = days.WeatherCode[i]
		}
		var hi, lo float64
		if i < len(days.TemperatureMax) {
			hi = days.TemperatureMax[i]
		}
		if i < len(days.TemperatureMin) {
			lo = days.TemperatureMin[i]
		}
		forecast = append(forecast, dailyForecast{
			label: label,
			icon:  weatherCodeToIcon(code, true),
			hi:    hi,
			lo:    lo,
		})
	}

	title := location
	if i := strings.Index(title, ","); i > 0 {
		title = strings.TrimSpace(title[:i])
	}

	frame, err := renderForecastFrame(forecast, unit)
	if err != nil {
		return plugin.DetailPayload{}, fmt.Errorf("render forecast frame: %w", err)
	}
	return plugin.DetailPayload{
		Title:    title + " — 7-Day Forecast",
		RawFrame: frame,
	}, nil
}

func renderForecastFrame(forecast []dailyForecast, unit string) ([]byte, error) {
	const (
		w         = 640
		h         = 48
		colW      = 640 / 7
		yLabel    = 4.0
		yIcon     = 17.0
		yTemp     = 40.0
		iconSize  = 12.0
		labelSize = 9.0
		tempSize  = 11.0
		closeX    = 630.0 // matches DetailCloseX in the core
		closeY    = 10.0  // matches DetailCloseY in the core
		closeSize = 11.0
	)
	closeIcon := string(rune(0xf00d)) // FA6 xmark — defined as var to survive encoding

	accent := color.RGBA{R: 0x4C, G: 0x9F, B: 0xFF, A: 0xFF}
	bg := color.RGBA{R: 0x14, G: 0x16, B: 0x1A, A: 0xFF}
	panelBg := lerpColor(bg, color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF}, 0.08)

	dc := gg.NewContext(w, h)
	dc.SetColor(panelBg)
	dc.Clear()

	faFace, err := loadTTFFace(faSolidTTF, iconSize)
	if err != nil {
		return nil, fmt.Errorf("load FA font: %w", err)
	}
	defer faFace.Close()

	labelFace, err := loadTTFFace(goregular.TTF, labelSize)
	if err != nil {
		return nil, fmt.Errorf("load label font: %w", err)
	}
	defer labelFace.Close()

	tempFace, err := loadTTFFace(goregular.TTF, tempSize)
	if err != nil {
		return nil, fmt.Errorf("load temp font: %w", err)
	}
	defer tempFace.Close()

	tempSuffix := "°F"
	if unit != "imperial" {
		tempSuffix = "°C"
	}

	for i, day := range forecast {
		x := float64(i*colW) + float64(colW)/2.0
		isToday := i == 0

		if isToday {
			dc.SetColor(lerpColor(panelBg, accent, 0.13))
			dc.DrawRoundedRectangle(float64(i*colW)+2, 2, float64(colW)-4, float64(h)-4, 4)
			dc.Fill()
		}

		if i > 0 {
			dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x33})
			dc.DrawLine(float64(i*colW), 6, float64(i*colW), float64(h)-6)
			dc.SetLineWidth(1)
			dc.Stroke()
		}

		dc.SetFontFace(labelFace)
		if isToday {
			dc.SetColor(accent)
		} else {
			dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xB3})
		}
		dc.DrawStringAnchored(day.label, x, yLabel, 0.5, 1.0)

		dc.SetFontFace(faFace)
		if isToday {
			dc.SetColor(accent)
		} else {
			dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xCC})
		}
		dc.DrawStringAnchored(day.icon, x, yIcon, 0.5, 1.0)

		dc.SetFontFace(tempFace)
		hiStr := fmt.Sprintf("%d%s", int(math.Round(day.hi)), tempSuffix)
		loStr := fmt.Sprintf("%d", int(math.Round(day.lo)))
		if isToday {
			dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF})
		} else {
			dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xE6})
		}
		dc.DrawStringAnchored(hiStr, x-6, yTemp, 1.0, 0.0)
		dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0x73})
		dc.DrawStringAnchored(loStr, x+4, yTemp, 0.0, 0.0)
	}

	// Close button — dark pill background + xmark glyph, always on top.
	closeFace, err := loadTTFFace(faSolidTTF, closeSize)
	if err != nil {
		return nil, fmt.Errorf("load close font: %w", err)
	}
	defer closeFace.Close()
	dc.SetColor(color.RGBA{R: 0x00, G: 0x00, B: 0x00, A: 0x80})
	dc.DrawCircle(closeX, closeY, 9)
	dc.Fill()
	dc.SetFontFace(closeFace)
	dc.SetColor(color.RGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF})
	dc.DrawStringAnchored(closeIcon, closeX, closeY, 0.5, 0.5)

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

func loadTTFFace(data []byte, size float64) (font.Face, error) {
	f, err := truetype.Parse(data)
	if err != nil {
		return nil, err
	}
	return truetype.NewFace(f, &truetype.Options{Size: size, DPI: 72, Hinting: font.HintingFull}), nil
}

func lerpColor(a, b color.RGBA, t float64) color.RGBA {
	lerp := func(x, y uint8) uint8 { return uint8(float64(x) + (float64(y)-float64(x))*t) }
	return color.RGBA{R: lerp(a.R, b.R), G: lerp(a.G, b.G), B: lerp(a.B, b.B), A: 0xFF}
}
