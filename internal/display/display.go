// Package display provides display rendering and management for the Nexus device.
package display

import (
	"image"
	"image/color"
	"time"
)

// Renderer handles rendering display content.
type Renderer interface {
	// Render creates a new frame with the given state.
	Render(state State) (image.Image, error)

	// SetBackground updates the background image.
	SetBackground(img image.Image) error

	// SetColors updates text and background colors.
	SetColors(text, background color.Color)
}

// State holds the current display state for rendering.
type State struct {
	Time        time.Time
	TimeFormat  string // "12h" or "24h"
	Temperature TemperatureData
	Network     NetworkData
	Weather     *WeatherData
	Colors      ColorConfig
}

// TemperatureData holds system temperature information.
type TemperatureData struct {
	CPU float64 // CPU temperature in degrees
	GPU float64 // GPU temperature in degrees
}

// NetworkData holds network statistics.
type NetworkData struct {
	DownloadSpeed float64 // Download speed in bytes/sec
	UploadSpeed   float64 // Upload speed in bytes/sec
	TotalDown     uint64  // Total downloaded bytes
	TotalUp       uint64  // Total uploaded bytes
}

// WeatherData holds weather information.
type WeatherData struct {
	Location    string
	Temperature float64
	Description string
	Icon        string
	Unit        string // "metric" or "imperial"
	WindSpeed   float64
	Humidity    int
}

// ColorConfig holds display color configuration.
type ColorConfig struct {
	Text       color.Color
	Background color.Color
}

// Constants for display dimensions (matches device package).
const (
	Width  = 640
	Height = 48
)
