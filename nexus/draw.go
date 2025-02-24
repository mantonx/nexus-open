/*
Package nexus provides functionality for drawing and managing visual elements on a display.

The package handles various drawing operations including time display, system temperatures,
network statistics, and weather information. It supports animated backgrounds, custom fonts,
and dynamic color management.

Key features:
  - Dynamic text color management with named colors and hex code support
  - Animated background support with GIF processing
  - Time display with configurable 12/24-hour format and blinking colon
  - System temperature display for CPU and GPU
  - Network statistics visualization with automatic unit conversion
  - Weather information display with configurable units (metric/imperial)
  - Custom font support with fallback to basic system font
  - Thread-safe color and time format management using atomic values

The package uses a combination of standard Go image packages and custom drawing routines
to create a flexible display system. It maintains thread safety through sync.Once and
atomic operations for shared resources.

Global variables:
  - d: Text drawing context
  - face: Current font face
  - background: Slice of background image frames for animation
  - getBackgroundOnce: Ensures single background loading
  - speedSymbol: Unit for wind speed display
  - degreeSymbol: Unit for temperature display
  - currentTextColor: Thread-safe storage for text color
  - currentTimeFormat: Thread-safe storage for time format

The package automatically initializes with white text color and 24-hour time format
by default. Background images should be properly sized GIF files that match the
display dimensions.
*/
package nexus

import (
	"bytes"
	"embed"
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/gif"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"nexus-open/nexus/instruments"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"
)

type ImageConfig struct {
	BackgroundImg string
	BgColor       string
}

//go:embed images/*
var images embed.FS

var (
	d                 *font.Drawer  // Text drawing context
	face              font.Face     // Font face
	background        []*image.RGBA // Background image frames
	getBackgroundOnce sync.Once     // Ensures background is loaded only once
	speedSymbol       string        // Unit for wind speed
	degreeSymbol      string        // Unit for temperature
	currentTextColor  atomic.Value  // stores color.RGBA
	currentTimeFormat atomic.Value  // stores string
)

// init initializes the default text color as white (RGBA: 255,255,255,255)
// and sets the default time format to "24h". This function is automatically
// called when the package is imported.
func init() {
	currentTextColor.Store(color.RGBA{R: 255, G: 255, B: 255, A: 255}) // Default text color: white
	currentTimeFormat.Store("12h")                                     // Default time format: 12-hour
}

// InitImageBuffer creates and returns a new byte slice to be used as an RGBA image buffer.
// The buffer size is calculated as width * height * 4, where 4 represents the RGBA channels
// (Red, Green, Blue, Alpha) per pixel. Each channel uses 1 byte.
//
// Parameters:
//   - width: The width of the image in pixels
//   - height: The height of the image in pixels
//
// Returns:
//   - []byte: A zeroed byte slice with size width * height * 4
func InitImageBuffer(width, height int) []byte {
	return make([]byte, width*height*4)
}

// CreateImageContext creates and returns a new RGBA image context with the specified configuration.
// It handles background image loading (including animated backgrounds), fallback solid colors,
// and text rendering setup.
//
// Parameters:
//   - config: ImageConfig containing background image and color settings
//   - customFace: Optional variadic parameter for custom font face. If not provided or nil,
//     defaults to basicfont.Face7x13
//
// The function performs the following operations:
//  1. Loads background image (if specified) using a singleton pattern
//  2. Creates fallback solid color background if image loading fails
//  3. Handles animated backgrounds by selecting appropriate frame based on current time
//  4. Sets up font face and text drawing context
//  5. Configures text color from atomic storage
//
// Returns:
//
//	*image.RGBA: New image context ready for drawing operations
func CreateImageContext(config ImageConfig, customFace ...font.Face) *image.RGBA {
	var err error

	getBackgroundOnce.Do(func() {
		background, err = convertBackgroundImage(config.BackgroundImg)
	})

	if err != nil {
		// Fallback to solid color if background image fails to load
		img := image.NewRGBA(image.Rect(0, 0, width, height))
		bgColor := parseColor(config.BgColor, color.RGBA{R: 0, G: 0, B: 0, A: 255})
		draw.Draw(img, img.Bounds(), &image.Uniform{bgColor}, image.Point{}, draw.Src)
	}

	// Use the first frame of the animated background
	img := image.NewRGBA(image.Rect(0, 0, width, height))

	if len(background) > 0 {
		// Convert to 24 Hz by dividing by 41.666667ms (1000/24)
		frameIndex := (time.Now().UnixNano() / 41666667) % int64(len(background))
		draw.Draw(img, img.Bounds(), background[int(frameIndex)], image.Point{}, draw.Src)
	}

	// Set up font and text drawing context
	if len(customFace) > 0 && customFace[0] != nil {
		face = customFace[0]
	} else {
		face = basicfont.Face7x13 // default font
	}

	face = LoadSystemFont("HackNerdFont-Regular.ttf")

	// Always use current text color from atomic storage
	textColor := currentTextColor.Load().(color.RGBA)

	d = &font.Drawer{
		Dst:  img,
		Src:  image.NewUniform(textColor),
		Face: face,
		Dot: fixed.Point26_6{
			X: fixed.I(width / 2),
			Y: fixed.I(height / 2),
		},
	}

	return img
}

// SetTextColor updates the current text color used for drawing operations.
// It accepts a color string which can be in hex format (e.g. "#FF0000") or a named color.
// If an empty string is provided, the function returns without changing the current color.
// The color is parsed and stored in an atomic value for thread-safe access.
// If a drawer exists, its source color is updated to reflect the new text color.
// Default color is white (RGBA{255,255,255,255}) if parsing fails.
func SetTextColor(colorStr string) {
	if colorStr == "" {
		return // Don't change color if empty string
	}

	textColor := parseColor(colorStr, color.RGBA{R: 255, G: 255, B: 255, A: 255})
	currentTextColor.Store(textColor)

	// Update drawer if it exists
	if d != nil {
		d.Src = image.NewUniform(textColor)
	}
}

// SetTimeFormat sets the time format string used for time-related formatting operations.
// The format string must follow Go's time formatting conventions.
// This function is safe for concurrent use.
func SetTimeFormat(format string) {
	currentTimeFormat.Store(format)
}

// DrawTime draws the current time on the display with a blinking colon
// The time is right-aligned and positioned at the top of the screen
func DrawTime() {
	currentTime := time.Now()
	timeFormat := currentTimeFormat.Load().(string)
	var timeStr string

	if timeFormat == "12h" {
		timeStr = currentTime.Format("3:04 PM")
	} else {
		timeStr = currentTime.Format("15:04")
	}

	// Blinking colon effect at 1Hz
	if (currentTime.Unix() % 2) == 0 {
		timeStr = strings.Replace(timeStr, ":", " ", 1)
	}

	timeTextWidth := (&font.Drawer{Face: face}).MeasureString(timeStr)

	d.Dot = fixed.Point26_6{
		X: fixed.I(width) - timeTextWidth - fixed.I(10),
		Y: fixed.I(15),
	}

	d.DrawString(timeStr)
}

// DrawSystemTemperatures renders CPU and GPU temperatures with icons
// at the left side of the display. Each temperature is shown with a
// corresponding hardware icon and formatted to one decimal place.
func DrawSystemTemperatures(cpuTemp, gpuTemp float64) {
	// Draw CPU temperature with icon
	d.Dot = fixed.Point26_6{
		X: fixed.I(10),
		Y: fixed.I(15),
	}
	d.DrawString(fmt.Sprintf("\uf4bc %.1f °C", cpuTemp))

	// Draw GPU temperature with icon
	d.Dot = fixed.Point26_6{
		X: fixed.I(10),
		Y: fixed.I(40),
	}
	d.DrawString(fmt.Sprintf("\ueabe %.1f °C", gpuTemp))
}

// DrawNetworkStats renders network statistics on the display.
// It shows the network sent and received rates in a left-aligned format.
// The sent rate is displayed at y-coordinate 15,
// while the received rate is shown at y-coordinate 40.
// Both statistics are positioned at width/2 - 130 pixels from the left.
//
// Parameters:
//   - currentNetwork: instruments.NetworkStats containing the current sent/received bytes
func DrawNetworkStats(currentNetwork instruments.NetworkStats) {
	// Network sent text (left-aligned)
	sentText := formatNetworkRate("\uf093", int64(currentNetwork.Sent))

	d.Dot = fixed.Point26_6{
		X: fixed.I(width / 4),
		Y: fixed.I(15),
	}

	d.DrawString(sentText)

	// Network received text (left-aligned)
	recvText := formatNetworkRate("\uf019", int64(currentNetwork.Received))

	d.Dot = fixed.Point26_6{
		X: fixed.I(width / 4),
		Y: fixed.I(40),
	}

	d.DrawString(recvText)
}

// DrawWeather renders the current weather information on the screen.
// It displays temperature, weather condition, and wind speed in the top right corner
// using the configured measurement units and font settings.
// If weatherInfo is nil, the function returns without drawing anything.
//
// Parameters:
//   - weatherInfo: Pointer to WeatherInfo struct containing weather data to display
func DrawWeather(weatherInfo *instruments.WeatherInfo) {
	if weatherInfo == nil {
		return
	}

	setMeasurementUnits(unit)

	weatherText := fmt.Sprintf("%s %s %.1f%s %s %s", weatherInfo.Location, weatherInfo.Condition, weatherInfo.Temperature, degreeSymbol, weatherInfo.WindSpeed, speedSymbol)
	weatherTextWidth := (&font.Drawer{Face: face}).MeasureString(weatherText)

	d.Dot = fixed.Point26_6{
		X: fixed.I(width) - weatherTextWidth - fixed.I(10),
		Y: fixed.I(40),
	}

	d.DrawString(weatherText)
}

func setMeasurementUnits(unit string) {
	if unit == "metric" {
		degreeSymbol = "°C"
		speedSymbol = "km/h"
	} else if unit == "imperial" {
		degreeSymbol = "°F"
		speedSymbol = "mph"
	} else {
		degreeSymbol = "K"
		speedSymbol = "m/s"
	}
}

// colorMap returns a map of predefined color names to their corresponding RGBA values.
// The map includes basic colors (black, white, red, green, blue) and additional colors
// like yellow, cyan, magenta, purple, orange, pink, gray, brown, teal, and silver.
// All colors are defined with full opacity (A: 255).
func colorMap() map[string]color.RGBA {
	return map[string]color.RGBA{
		"black":   {R: 0, G: 0, B: 0, A: 255},
		"red":     {R: 255, G: 0, B: 0, A: 255},
		"green":   {R: 0, G: 255, B: 0, A: 255},
		"blue":    {R: 0, G: 0, B: 255, A: 255},
		"white":   {R: 255, G: 255, B: 255, A: 255},
		"yellow":  {R: 255, G: 255, B: 0, A: 255},
		"cyan":    {R: 0, G: 255, B: 255, A: 255},
		"magenta": {R: 255, G: 0, B: 255, A: 255},
		"purple":  {R: 128, G: 0, B: 128, A: 255},
		"orange":  {R: 255, G: 165, B: 0, A: 255},
		"pink":    {R: 255, G: 192, B: 203, A: 255},
		"gray":    {R: 128, G: 128, B: 128, A: 255},
		"brown":   {R: 165, G: 42, B: 42, A: 255},
		"teal":    {R: 0, G: 128, B: 128, A: 255},
		"silver":  {R: 192, G: 192, B: 192, A: 255},
	}
}

// parseColor converts a color string to color.RGBA. It accepts either a hex color string
// in the format "#RRGGBB" or a named color string. If the input string is not a valid color
// format, it returns the provided default color.
//
// Parameters:
//   - colorStr: A string representing the color in either hex format ("#RRGGBB") or as a named color
//   - defaultColor: The fallback color.RGBA to use if parsing fails
//
// Returns:
//   - color.RGBA: The parsed color, or defaultColor if parsing fails
func parseColor(colorStr string, defaultColor color.RGBA) color.RGBA {
	// Check if hex color
	if len(colorStr) == 7 && colorStr[0] == '#' {
		var r, g, b uint8
		if _, err := fmt.Sscanf(colorStr[1:], "%02x%02x%02x", &r, &g, &b); err == nil {
			return color.RGBA{R: r, G: g, B: b, A: 255}
		}
	}

	// Check named color
	if color, exists := colorMap()[colorStr]; exists {
		return color
	}

	return defaultColor
}

// formatNetworkRate formats network bandwidth rates with appropriate units.
// It takes a label string and a rate in Kbps (kilobits per second) as input.
// For rates above 1000 Kbps, it converts to Mbps (megabits per second) with one decimal place.
// For rates below or equal to 1000 Kbps, it keeps the original Kbps unit.
// Returns a formatted string combining the label and the rate with proper units.
func formatNetworkRate(label string, rate int64) string {
	if rate > 1000 {
		return fmt.Sprintf("%s %.1f Mbps", label, float64(rate)/1024)
	}
	return fmt.Sprintf("%s %d Kbps", label, rate)
}

// convertBackgroundImage takes a path to an image file and converts it into a slice of RGBA images.
// For GIF files, it returns all frames as separate RGBA images.
// For JPEG and PNG files, it returns a single RGBA image in a slice.
//
// Parameters:
//   - imgPath: string representing the path to the image file
//
// Returns:
//   - []*image.RGBA: a slice of RGBA images (multiple frames for GIFs, single frame for JPEG/PNG)
//   - error: nil if successful, otherwise an error describing what went wrong
func convertBackgroundImage(fileName string) ([]*image.RGBA, error) {
	// Get the embedded image file
	imgFile, err := images.ReadFile("images/" + fileName)

	if err != nil {
		return nil, fmt.Errorf("failed to read embedded image: %v", err)
	}

	// For GIF images, handle multiple frames
	if strings.HasSuffix(strings.ToLower(fileName), ".gif") {
		gifImg, err := gif.DecodeAll(bytes.NewReader(imgFile))
		if err != nil {
			return nil, fmt.Errorf("failed to decode GIF: %v", err)
		}

		frames := make([]*image.RGBA, len(gifImg.Image))
		for i, img := range gifImg.Image {
			bounds := img.Bounds()
			rgba := image.NewRGBA(bounds)
			draw.Draw(rgba, bounds, img, bounds.Min, draw.Src)
			frames[i] = rgba
		}
		return frames, nil
	}

	// For JPEG and PNG, handle single frame
	img, _, err := image.Decode(bytes.NewReader(imgFile))
	if err != nil {
		return nil, fmt.Errorf("failed to decode image: %v", err)
	}

	bounds := img.Bounds()
	rgba := image.NewRGBA(bounds)
	draw.Draw(rgba, bounds, img, bounds.Min, draw.Src)
	return []*image.RGBA{rgba}, nil
}
