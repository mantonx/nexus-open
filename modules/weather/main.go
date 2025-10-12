// weather is a module that monitors weather conditions
package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"sync"
	"time"

	"github.com/hashicorp/go-plugin"
	"gopkg.in/yaml.v3"

	"nexus-open/pkg/module"
)

const (
	openMeteoBaseURL   = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&wind_speed_unit=%s&latitude=%.4f&longitude=%.4f&current=temperature_2m,weather_code,wind_speed_10m,is_day"
	nominatimSearchURL = "https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1"
	defaultLat         = 40.7128  // New York, NY
	defaultLon         = -74.0060 // New York, NY
	cacheTimeout       = 5 * time.Minute
)

// Config represents the weather configuration
type Config struct {
	Location string `yaml:"location"`
	Unit     string `yaml:"unit"`
}

// WeatherModule monitors weather conditions
type WeatherModule struct {
	mu          sync.RWMutex
	lastUpdate  time.Time
	cachedData  *WeatherData
	location    string
	unit        string // "metric" or "imperial"
	configPath  string
	coordsCache map[string]coords
	coordsMu    sync.Mutex
}

type coords struct {
	lat float64
	lon float64
}

// WeatherData holds weather information
type WeatherData struct {
	Location    string
	Temperature float64
	Description string
	Icon        string
	WeatherCode int
	Unit        string
	WindSpeed   float64
}

// NewWeatherModule creates a new weather module
func NewWeatherModule() *WeatherModule {
	homeDir, _ := os.UserHomeDir()
	configPath := filepath.Join(homeDir, ".config", "nexus-open", "config.yaml")

	return &WeatherModule{
		configPath:  configPath,
		coordsCache: make(map[string]coords),
		unit:        "imperial", // default
	}
}

// Describe returns module metadata
func (m *WeatherModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Weather",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors weather conditions via Open-Meteo API",
		Icon:        "cloud",
		RefreshMs:   300000, // Update every 5 minutes
	}, nil
}

// Sample returns current weather data
func (m *WeatherModule) Sample() (module.Payload, error) {
	// Load config
	m.loadConfig()

	// Check cache
	m.mu.RLock()
	if m.cachedData != nil && time.Since(m.lastUpdate) < cacheTimeout {
		data := m.cachedData
		m.mu.RUnlock()
		return m.formatPayload(data), nil
	}
	m.mu.RUnlock()

	// Fetch fresh data
	data, err := m.fetchWeather()
	if err != nil {
		// Return cached data if available, even if stale
		m.mu.RLock()
		if m.cachedData != nil {
			data := m.cachedData
			m.mu.RUnlock()
			return m.formatPayload(data), nil
		}
		m.mu.RUnlock()

		return module.Payload{
			Primary:   "—",
			Secondary: "No Weather",
			Severity:  module.SeverityWarn,
			TTL:       5 * time.Minute,
			Timestamp: time.Now(),
		}, nil
	}

	// Cache the new data
	m.mu.Lock()
	m.cachedData = data
	m.lastUpdate = time.Now()
	m.mu.Unlock()

	return m.formatPayload(data), nil
}

// formatPayload converts WeatherData to module.Payload
func (m *WeatherModule) formatPayload(data *WeatherData) module.Payload {
	var tempStr string
	if data.Unit == "imperial" {
		tempStr = fmt.Sprintf("%.0f°F", data.Temperature)
	} else {
		tempStr = fmt.Sprintf("%.0f°C", data.Temperature)
	}

	return module.Payload{
		Primary:   tempStr,
		Secondary: fmt.Sprintf("%s %s", data.Location, data.Icon),
		Severity:  module.SeverityOK,
		TTL:       5 * time.Minute,
		Icon:      "cloud",
		Timestamp: time.Now(),
	}
}

// loadConfig loads weather configuration from config file
func (m *WeatherModule) loadConfig() {
	data, err := os.ReadFile(m.configPath)
	if err != nil {
		return
	}

	var config struct {
		Location string `yaml:"location"`
		Unit     string `yaml:"unit"`
	}

	if err := yaml.Unmarshal(data, &config); err != nil {
		return
	}

	m.mu.Lock()
	if config.Location != "" {
		m.location = config.Location
	}
	if config.Unit != "" {
		m.unit = config.Unit
	}
	m.mu.Unlock()
}

// fetchWeather fetches current weather data
func (m *WeatherModule) fetchWeather() (*WeatherData, error) {
	m.mu.RLock()
	location := m.location
	unit := m.unit
	m.mu.RUnlock()

	if location == "" {
		return nil, fmt.Errorf("no location configured")
	}

	// Get coordinates
	lat, lon, err := m.getCityCoordinates(location)
	if err != nil {
		lat, lon = defaultLat, defaultLon
	}

	// Fetch weather
	var tempUnit, windUnit string
	if unit == "imperial" {
		tempUnit = "fahrenheit"
		windUnit = "mph"
	} else {
		tempUnit = "celsius"
		windUnit = "kmh"
	}

	weatherURL := fmt.Sprintf(openMeteoBaseURL, tempUnit, windUnit, lat, lon)

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(weatherURL)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch weather: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	var result struct {
		Current struct {
			Temperature float64 `json:"temperature_2m"`
			WeatherCode int     `json:"weather_code"`
			WindSpeed   float64 `json:"wind_speed_10m"`
			IsDay       int     `json:"is_day"`
		} `json:"current"`
	}

	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("failed to decode weather data: %w", err)
	}

	icon := weatherCodeToIcon(result.Current.WeatherCode, result.Current.IsDay == 1)
	condition := weatherCodeToCondition(result.Current.WeatherCode)

	return &WeatherData{
		Location:    location,
		Temperature: result.Current.Temperature,
		Description: condition,
		Icon:        icon,
		WeatherCode: result.Current.WeatherCode,
		Unit:        unit,
		WindSpeed:   result.Current.WindSpeed,
	}, nil
}

// getCityCoordinates gets coordinates for a city name
func (m *WeatherModule) getCityCoordinates(location string) (float64, float64, error) {
	// Check cache first
	m.coordsMu.Lock()
	if cached, ok := m.coordsCache[location]; ok {
		m.coordsMu.Unlock()
		return cached.lat, cached.lon, nil
	}
	m.coordsMu.Unlock()

	// Fetch from API
	searchURL := fmt.Sprintf(nominatimSearchURL, url.QueryEscape(location))

	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequest("GET", searchURL, nil)
	if err != nil {
		return 0, 0, err
	}
	req.Header.Set("User-Agent", "Nexus-Open/2.0")

	resp, err := client.Do(req)
	if err != nil {
		return 0, 0, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return 0, 0, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	var results []struct {
		Lat string `json:"lat"`
		Lon string `json:"lon"`
	}

	if err := json.NewDecoder(resp.Body).Decode(&results); err != nil {
		return 0, 0, fmt.Errorf("failed to decode JSON: %w", err)
	}

	if len(results) == 0 {
		return 0, 0, fmt.Errorf("location not found")
	}

	lat, err := strconv.ParseFloat(results[0].Lat, 64)
	if err != nil {
		return 0, 0, fmt.Errorf("invalid latitude: %w", err)
	}

	lon, err := strconv.ParseFloat(results[0].Lon, 64)
	if err != nil {
		return 0, 0, fmt.Errorf("invalid longitude: %w", err)
	}

	// Cache the coordinates
	m.coordsMu.Lock()
	m.coordsCache[location] = coords{lat: lat, lon: lon}
	m.coordsMu.Unlock()

	return lat, lon, nil
}

// weatherCodeToIcon converts WMO weather code to icon character
func weatherCodeToIcon(code int, isDay bool) string {
	weatherIcons := map[int]struct{ day, night string }{
		0:  {day: "☀", night: "☾"},
		1:  {day: "☀", night: "☾"},
		2:  {day: "⛅", night: "☁"},
		3:  {day: "☁", night: "☁"},
		45: {day: "🌫", night: "🌫"},
		48: {day: "🌫", night: "🌫"},
		51: {day: "🌦", night: "🌧"},
		53: {day: "🌦", night: "🌧"},
		55: {day: "🌧", night: "🌧"},
		56: {day: "🌧", night: "🌧"},
		57: {day: "🌧", night: "🌧"},
		61: {day: "🌦", night: "🌧"},
		63: {day: "🌧", night: "🌧"},
		65: {day: "🌧", night: "🌧"},
		66: {day: "🌧", night: "🌧"},
		67: {day: "🌧", night: "🌧"},
		71: {day: "🌨", night: "🌨"},
		73: {day: "❄", night: "❄"},
		75: {day: "❄", night: "❄"},
		77: {day: "❄", night: "❄"},
		80: {day: "🌦", night: "🌧"},
		81: {day: "🌧", night: "🌧"},
		82: {day: "🌧", night: "🌧"},
		85: {day: "🌨", night: "🌨"},
		86: {day: "❄", night: "❄"},
		95: {day: "⛈", night: "⛈"},
		96: {day: "⛈", night: "⛈"},
		99: {day: "⛈", night: "⛈"},
	}

	if weather, ok := weatherIcons[code]; ok {
		if isDay {
			return weather.day
		}
		return weather.night
	}
	return "❓"
}

// weatherCodeToCondition converts WMO weather code to text description
func weatherCodeToCondition(code int) string {
	conditions := map[int]string{
		0:  "Clear",
		1:  "Mainly Clear",
		2:  "Partly Cloudy",
		3:  "Cloudy",
		45: "Foggy",
		48: "Rime Fog",
		51: "Light Drizzle",
		53: "Drizzle",
		55: "Heavy Drizzle",
		56: "Light Freezing Drizzle",
		57: "Freezing Drizzle",
		61: "Light Rain",
		63: "Rain",
		65: "Heavy Rain",
		66: "Light Freezing Rain",
		67: "Freezing Rain",
		71: "Light Snow",
		73: "Snow",
		75: "Heavy Snow",
		77: "Snow Grains",
		80: "Light Showers",
		81: "Showers",
		82: "Heavy Showers",
		85: "Light Snow Showers",
		86: "Snow Showers",
		95: "Thunderstorm",
		96: "Thunderstorm with Hail",
		99: "Heavy Thunderstorm with Hail",
	}

	if cond, ok := conditions[code]; ok {
		return cond
	}
	return "Unknown"
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: module.Handshake,
		Plugins: map[string]plugin.Plugin{
			"module": &module.ModulePlugin{Impl: NewWeatherModule()},
		},
	})
}
