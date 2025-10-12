package display

import (
	"image"
	"image/color"

	"golang.org/x/image/font"
	"golang.org/x/image/math/fixed"
)

// Icon represents a renderable icon (either Unicode or sprite)
type Icon struct {
	Type     IconType
	Unicode  string     // Unicode character for text-based icons
	Sprite   *image.RGBA // Sprite image for bitmap icons
	Width    int
	Height   int
}

// IconType defines the type of icon rendering
type IconType int

const (
	IconTypeUnicode IconType = iota
	IconTypeSprite
)

// IconSet contains all available icons
type IconSet struct {
	// Weather icons
	WeatherSunny        Icon
	WeatherCloudy       Icon
	WeatherRainy        Icon
	WeatherSnowy        Icon
	WeatherThunderstorm Icon
	WeatherFoggy        Icon
	WeatherPartlyCloudy Icon

	// System icons
	SystemCPU           Icon
	SystemGPU           Icon
	SystemRAM           Icon
	SystemNetworkUp     Icon
	SystemNetworkDown   Icon
	SystemTemperature   Icon

	// Status icons
	StatusConnected     Icon
	StatusDisconnected  Icon
	StatusError         Icon
}

// NewIconSet creates a new set of icons using Font Awesome
func NewIconSet() *IconSet {
	return &IconSet{
		// Weather icons using Font Awesome Unicode
		WeatherSunny:        Icon{Type: IconTypeUnicode, Unicode: "\uf185", Width: 12, Height: 12}, // fa-sun
		WeatherCloudy:       Icon{Type: IconTypeUnicode, Unicode: "\uf0c2", Width: 12, Height: 12}, // fa-cloud
		WeatherRainy:        Icon{Type: IconTypeUnicode, Unicode: "\uf73d", Width: 12, Height: 12}, // fa-cloud-rain
		WeatherSnowy:        Icon{Type: IconTypeUnicode, Unicode: "\uf2dc", Width: 12, Height: 12}, // fa-snowflake
		WeatherThunderstorm: Icon{Type: IconTypeUnicode, Unicode: "\uf76c", Width: 12, Height: 12}, // fa-poo-storm
		WeatherFoggy:        Icon{Type: IconTypeUnicode, Unicode: "\uf75f", Width: 12, Height: 12}, // fa-smog
		WeatherPartlyCloudy: Icon{Type: IconTypeUnicode, Unicode: "\uf6c4", Width: 12, Height: 12}, // fa-cloud-sun

		// System icons using Font Awesome Unicode
		SystemCPU:           Icon{Type: IconTypeUnicode, Unicode: "\uf2db", Width: 10, Height: 10}, // fa-microchip
		SystemGPU:           Icon{Type: IconTypeUnicode, Unicode: "\uf26c", Width: 10, Height: 10}, // fa-tv (monitor/display)
		SystemRAM:           Icon{Type: IconTypeUnicode, Unicode: "\uf538", Width: 10, Height: 10}, // fa-memory
		SystemNetworkUp:     Icon{Type: IconTypeUnicode, Unicode: "\uf062", Width: 10, Height: 10}, // fa-arrow-up
		SystemNetworkDown:   Icon{Type: IconTypeUnicode, Unicode: "\uf063", Width: 10, Height: 10}, // fa-arrow-down
		SystemTemperature:   Icon{Type: IconTypeUnicode, Unicode: "\uf2c9", Width: 10, Height: 10}, // fa-thermometer-half

		// Status icons using Font Awesome Unicode
		StatusConnected:     Icon{Type: IconTypeUnicode, Unicode: "\uf00c", Width: 8, Height: 8},  // fa-check
		StatusDisconnected:  Icon{Type: IconTypeUnicode, Unicode: "\uf00d", Width: 8, Height: 8},  // fa-times
		StatusError:         Icon{Type: IconTypeUnicode, Unicode: "\uf071", Width: 8, Height: 8},  // fa-exclamation-triangle
	}
}

// IconRenderer handles rendering icons to the display
type IconRenderer struct {
	iconSet     *IconSet
	font        font.Face // For system icons
	weatherFont font.Face // For larger weather icons
	color       color.Color
}

// NewIconRenderer creates a new icon renderer
func NewIconRenderer(iconSet *IconSet, f font.Face, weatherF font.Face, c color.Color) *IconRenderer {
	return &IconRenderer{
		iconSet:     iconSet,
		font:        f,
		weatherFont: weatherF,
		color:       c,
	}
}

// DrawIcon renders an icon at the specified position using the system icon font
func (ir *IconRenderer) DrawIcon(canvas *image.RGBA, icon Icon, x, y int) {
	switch icon.Type {
	case IconTypeUnicode:
		ir.drawUnicodeIconWithFont(canvas, icon.Unicode, x, y, ir.font)
	case IconTypeSprite:
		ir.drawSpriteIcon(canvas, icon.Sprite, x, y)
	}
}

// DrawWeatherIcon renders a weather icon at the specified position using the larger weather font
func (ir *IconRenderer) DrawWeatherIcon(canvas *image.RGBA, icon Icon, x, y int) {
	switch icon.Type {
	case IconTypeUnicode:
		ir.drawUnicodeIconWithFont(canvas, icon.Unicode, x, y, ir.weatherFont)
	case IconTypeSprite:
		ir.drawSpriteIcon(canvas, icon.Sprite, x, y)
	}
}

// drawUnicodeIconWithFont renders a Unicode character as an icon with a specific font
func (ir *IconRenderer) drawUnicodeIconWithFont(canvas *image.RGBA, unicode string, x, y int, f font.Face) {
	point := fixed.Point26_6{
		X: fixed.Int26_6(x * 64),
		Y: fixed.Int26_6(y * 64),
	}

	drawer := &font.Drawer{
		Dst:  canvas,
		Src:  &image.Uniform{ir.color},
		Face: f,
		Dot:  point,
	}

	drawer.DrawString(unicode)
}

// drawSpriteIcon renders a sprite image as an icon
func (ir *IconRenderer) drawSpriteIcon(canvas *image.RGBA, sprite *image.RGBA, x, y int) {
	if sprite == nil {
		return
	}

	// Draw sprite at position
	bounds := sprite.Bounds()
	for dy := 0; dy < bounds.Dy(); dy++ {
		for dx := 0; dx < bounds.Dx(); dx++ {
			srcX := bounds.Min.X + dx
			srcY := bounds.Min.Y + dy
			dstX := x + dx
			dstY := y + dy

			if dstX >= 0 && dstX < canvas.Bounds().Dx() && dstY >= 0 && dstY < canvas.Bounds().Dy() {
				canvas.Set(dstX, dstY, sprite.At(srcX, srcY))
			}
		}
	}
}

// GetWeatherIcon returns the appropriate weather icon based on weather code
// Weather codes from Open-Meteo API
func (is *IconSet) GetWeatherIcon(weatherCode int) Icon {
	switch {
	case weatherCode == 0, weatherCode == 1:
		return is.WeatherSunny
	case weatherCode == 2:
		return is.WeatherPartlyCloudy
	case weatherCode == 3:
		return is.WeatherCloudy
	case weatherCode >= 45 && weatherCode <= 48:
		return is.WeatherFoggy
	case weatherCode >= 51 && weatherCode <= 67:
		return is.WeatherRainy
	case weatherCode >= 71 && weatherCode <= 77, weatherCode == 85, weatherCode == 86:
		return is.WeatherSnowy
	case weatherCode == 95, weatherCode == 96, weatherCode == 99:
		return is.WeatherThunderstorm
	default:
		return is.WeatherCloudy // Default to cloudy
	}
}

// UpdateFont updates the font face used for Unicode icons
func (ir *IconRenderer) UpdateFont(f font.Face) {
	ir.font = f
}

// UpdateColor updates the color used for icons
func (ir *IconRenderer) UpdateColor(c color.Color) {
	ir.color = c
}
