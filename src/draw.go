package main

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"time"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"
)

var (
	d    *font.Drawer // Text drawing context
	face font.Face    // Font face
)

func InitImageBuffer(width, height int) []byte {
	return make([]byte, width*height*4)
}

func CreateImageContext(bgColorStr string, customFace ...font.Face) *image.RGBA {
	// Create background with specified color
	img := image.NewRGBA(image.Rect(0, 0, width, height))
	bgColor := parseColor(bgColorStr, color.RGBA{R: 0, G: 0, B: 0, A: 255})
	draw.Draw(img, img.Bounds(), &image.Uniform{bgColor}, image.Point{}, draw.Src)

	// Set up font and text drawing context
	if len(customFace) > 0 && customFace[0] != nil {
		face = customFace[0]
	} else {
		face = basicfont.Face7x13 // default font
	}

	face = LoadSystemFont("Hack-Regular.ttf")

	d = &font.Drawer{
		Dst:  img,
		Src:  image.NewUniform(color.RGBA{R: 255, G: 255, B: 255, A: 255}),
		Face: face,
		Dot: fixed.Point26_6{
			X: fixed.I(width / 2),
			Y: fixed.I(height / 2),
		},
	}

	return img
}

// SetTextColor sets the drawing color for text using either a named color or hex color code
func SetTextColor(colorStr string) {
	textColor := parseColor(colorStr, color.RGBA{R: 255, G: 255, B: 255, A: 255})
	d.Src = image.NewUniform(textColor)
}

// DrawTime draws the current time on the display with a blinking colon
// The time is right-aligned and positioned at the top of the screen
func DrawTime() {
	currentTime := time.Now()
	timeStr := currentTime.Format("3:04 PM")

	// Blinking colon effect at 60Hz (every 500ms)
	if (currentTime.UnixNano()/100000000)%2 == 0 && len(timeStr) >= 3 {
		if idx := len(timeStr) - 6; idx >= 0 {
			timeStr = timeStr[:idx] + " " + timeStr[idx+1:]
		}
	}

	timeTextWidth := (&font.Drawer{Face: face}).MeasureString(timeStr)

	d.Dot = fixed.Point26_6{
		X: fixed.I(width) - timeTextWidth - fixed.I(10),
		Y: fixed.I(15),
	}

	d.DrawString(timeStr)
}

func DrawTemperatures(cpuTemp, gpuTemp float64) {
	d.Dot = fixed.Point26_6{
		X: fixed.I(10),
		Y: fixed.I(15),
	}
	d.DrawString(fmt.Sprintf("CPU: %.1f C", cpuTemp))

	// GPU temperature text (left-aligned, bottom)
	d.Dot = fixed.Point26_6{
		X: fixed.I(10),
		Y: fixed.I(40),
	}
	d.DrawString(fmt.Sprintf("GPU: %.1f C", gpuTemp))
}

func DrawNetworkStats(currentNetwork NetworkStats) {
	// Network sent text (left-aligned)
	sentText := formatNetworkRate("Sent", int64(currentNetwork.Sent))
	d.Dot = fixed.Point26_6{
		X: fixed.I(width/2 - 90),
		Y: fixed.I(15),
	}
	d.DrawString(sentText)

	// Network received text (left-aligned)
	recvText := formatNetworkRate("Recv", int64(currentNetwork.Received))
	d.Dot = fixed.Point26_6{
		X: fixed.I(width/2 - 90),
		Y: fixed.I(40),
	}
	d.DrawString(recvText)
}

func DrawWeather(weatherInfo *WeatherInfo) {
	weatherText := fmt.Sprintf("Weather: %.1f F, %s, %s mph", weatherInfo.Temperature, weatherInfo.Condition, weatherInfo.WindSpeed)
	weatherTextWidth := (&font.Drawer{Face: face}).MeasureString(weatherText)

	d.Dot = fixed.Point26_6{
		X: fixed.I(width) - weatherTextWidth - fixed.I(10),
		Y: fixed.I(40),
	}

	d.DrawString(weatherText)
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
		return fmt.Sprintf("%s: %.1f Mbps", label, float64(rate)/1024)
	}
	return fmt.Sprintf("%s: %d Kbps", label, rate)
}
