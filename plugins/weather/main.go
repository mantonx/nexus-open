// weather is a module that monitors weather conditions
package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"time"

	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

const (
	openMeteoBaseURL   = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&wind_speed_unit=%s&latitude=%.4f&longitude=%.4f&current=temperature_2m,weather_code,wind_speed_10m,is_day"
	nominatimSearchURL = "https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1"
	defaultLat         = 40.7282  // Jersey City, NJ
	defaultLon         = -74.0776 // Jersey City, NJ
	cacheTimeout       = 5 * time.Minute
)

// WeatherPlugin monitors weather conditions
type WeatherPlugin struct {
	mu          sync.RWMutex
	lastUpdate  time.Time
	cachedData  *WeatherData
	location    string
	unit        string // "metric" or "imperial"
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

// NewWeatherPlugin creates a new weather module
func NewWeatherPlugin() *WeatherPlugin {
	wm := &WeatherPlugin{
		coordsCache: make(map[string]coords),
		location:    "Jersey City, NJ",
		unit:        "imperial", // default (°F)
	}
	return wm
}

// Describe returns module metadata
func (m *WeatherPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Weather",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors weather conditions via Open-Meteo API",
		Icon:        "cloud",
		RefreshMs:   300000, // Update every 5 minutes
	}, nil
}

// Sample returns current weather data
func (m *WeatherPlugin) Sample() (plugin.Payload, error) {
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

		return plugin.Payload{
			Primary:   "—",
			Secondary: "No Weather",
			Severity:  plugin.SeverityWarn,
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

// OnConfigChanged implements plugin.ConfigNotifier interface.
// This method is called by the host when configuration changes via the API,
// allowing the weather module to react instantly without file watching.
func (m *WeatherPlugin) OnConfigChanged(config map[string]interface{}) error {
	m.mu.Lock()
	oldLocation := m.location
	oldUnit := m.unit

	// Extract location from config
	if location, ok := config["location"].(string); ok && location != "" {
		m.location = location
	}

	// Extract unit from config
	if unit, ok := config["unit"].(string); ok && unit != "" {
		m.unit = unit
	}

	newLocation := m.location
	newUnit := m.unit
	m.mu.Unlock()

	// If location or unit changed, fetch fresh data immediately
	if newLocation != oldLocation || newUnit != oldUnit {
		fmt.Printf("weather: config updated - location=%q unit=%q (fetching immediately)\n",
			newLocation, newUnit)

		// Fetch weather data synchronously (fetchWeather doesn't hold locks)
		data, err := m.fetchWeather()
		if err != nil {
			fmt.Printf("weather: failed to fetch after config change: %v\n", err)
			// Clear cache anyway so next Sample() will retry
			m.mu.Lock()
			m.cachedData = nil
			m.lastUpdate = time.Time{}
			m.mu.Unlock()
			return nil
		}

		// Update cache with new data
		m.mu.Lock()
		m.cachedData = data
		m.lastUpdate = time.Now()
		m.mu.Unlock()

		fmt.Printf("weather: immediately updated to %s, %s%.1f\n",
			data.Location,
			map[bool]string{true: "°F", false: "°C"}[data.Unit == "imperial"],
			data.Temperature)
	}

	return nil
}

// formatPayload converts WeatherData to plugin.Payload
func (m *WeatherPlugin) formatPayload(data *WeatherData) plugin.Payload {
	var tempStr string
	if data.Unit == "imperial" {
		tempStr = fmt.Sprintf("%.0f°F", data.Temperature)
	} else {
		tempStr = fmt.Sprintf("%.0f°C", data.Temperature)
	}

	// Strip state/country suffix (", NJ" / ", US") — city name alone fits better.
	loc := data.Location
	if i := strings.Index(loc, ","); i > 0 {
		loc = strings.TrimSpace(loc[:i])
	}
	secondary := loc

	fmt.Printf("weather payload primary=%q secondary=%q icon=%q\n",
		tempStr, secondary, data.Icon)

	return plugin.Payload{
		Primary:   tempStr,
		Secondary: secondary,
		Severity:  plugin.SeverityOK,
		TTL:       5 * time.Minute,
		Icon:      data.Icon,
		Timestamp: time.Now(),
	}
}

// fetchWeather fetches current weather data
func (m *WeatherPlugin) fetchWeather() (*WeatherData, error) {
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
	defer func() { _ = resp.Body.Close() }()

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
func (m *WeatherPlugin) getCityCoordinates(location string) (float64, float64, error) {
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
	defer func() { _ = resp.Body.Close() }()

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
		0:  {day: "\uf185", night: "\uf186"}, // sun / moon
		1:  {day: "\uf185", night: "\uf186"},
		2:  {day: "\uf6c4", night: "\uf6c3"}, // cloud-sun / cloud-moon
		3:  {day: "\uf0c2", night: "\uf0c2"}, // cloud
		45: {day: "\uf75f", night: "\uf75f"}, // smog
		48: {day: "\uf75f", night: "\uf75f"},
		51: {day: "\uf73d", night: "\uf73d"}, // cloud-rain
		53: {day: "\uf73d", night: "\uf73d"},
		55: {day: "\uf73d", night: "\uf73d"},
		56: {day: "\uf73d", night: "\uf73d"},
		57: {day: "\uf73d", night: "\uf73d"},
		61: {day: "\uf73d", night: "\uf73d"},
		63: {day: "\uf73d", night: "\uf73d"},
		65: {day: "\uf73d", night: "\uf73d"},
		66: {day: "\uf73d", night: "\uf73d"},
		67: {day: "\uf73d", night: "\uf73d"},
		71: {day: "\uf2dc", night: "\uf2dc"}, // snowflake
		73: {day: "\uf2dc", night: "\uf2dc"},
		75: {day: "\uf2dc", night: "\uf2dc"},
		77: {day: "\uf2dc", night: "\uf2dc"},
		80: {day: "\uf73d", night: "\uf73d"},
		81: {day: "\uf73d", night: "\uf73d"},
		82: {day: "\uf73d", night: "\uf73d"},
		85: {day: "\uf2dc", night: "\uf2dc"},
		86: {day: "\uf2dc", night: "\uf2dc"},
		95: {day: "\uf76c", night: "\uf76c"}, // thunderstorm
		96: {day: "\uf76c", night: "\uf76c"},
		99: {day: "\uf76c", night: "\uf76c"},
	}

	if weather, ok := weatherIcons[code]; ok {
		if isDay {
			return weather.day
		}
		return weather.night
	}
	return "\uf0c2" // default cloud
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
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: NewWeatherPlugin()},
		},
	})
}
