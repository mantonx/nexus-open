// Package instruments provides data collection from various system sources.
package instruments

import (
	"context"
	"time"
)

// Instrument represents a data source that can be started and stopped.
// Each instrument collects specific system data (temperature, network, weather, etc.)
type Instrument interface {
	// Name returns the instrument's unique identifier
	Name() string

	// Start begins collecting data and returns a channel for updates.
	// The instrument should respect context cancellation.
	Start(ctx context.Context) error

	// Stop gracefully stops the instrument
	Stop() error

	// UpdateInterval returns how often this instrument updates
	UpdateInterval() time.Duration
}

// Data represents the collected instrument data at a specific time
type Data struct {
	Timestamp time.Time
	Values    map[string]interface{}
}

// SystemData holds all collected instrument data for display
type SystemData struct {
	Timestamp   time.Time
	Temperature TemperatureData
	Network     NetworkData
	Weather     *WeatherData
	CPULoad     float64
}

// TemperatureData holds system temperature information
type TemperatureData struct {
	CPU float64 // CPU temperature in Celsius
	GPU float64 // GPU temperature in Celsius
}

// NetworkData holds network statistics
type NetworkData struct {
	DownloadSpeed float64 // Download speed in bytes/sec
	UploadSpeed   float64 // Upload speed in bytes/sec
	TotalDown     uint64  // Total downloaded bytes
	TotalUp       uint64  // Total uploaded bytes
}

// WeatherData holds weather information
type WeatherData struct {
	Location    string
	Temperature float64 // Temperature in configured unit
	Description string
	Icon        string
	WeatherCode int    // Open-Meteo weather code for icon lookup
	Unit        string // "metric" or "imperial"
	WindSpeed   float64
	Humidity    int
}
