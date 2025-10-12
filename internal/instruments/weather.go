package instruments

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"net/url"
	"strconv"
	"sync"
	"time"
)

const (
	openMeteoBaseURL   = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&wind_speed_unit=%s&latitude=%.4f&longitude=%.4f&current=temperature_2m,weather_code,wind_speed_10m,is_day"
	nominatimSearchURL = "https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1"
	defaultLat         = 40.7128  // New York, NY
	defaultLon         = -74.0060 // New York, NY
)

// Weather monitors weather conditions for a configured location
type Weather struct {
	logger       *slog.Logger
	interval     time.Duration
	dataChan     chan *WeatherData
	mu           sync.RWMutex
	current      *WeatherData
	lastLocation string
	unit         string // "metric" or "imperial"
	ctx          context.Context
	cancel       context.CancelFunc
	wg           sync.WaitGroup
	updating     bool
}

// NewWeather creates a new weather instrument
func NewWeather(logger *slog.Logger, interval time.Duration) *Weather {
	if interval == 0 {
		interval = 10 * time.Minute
	}
	return &Weather{
		logger:   logger,
		interval: interval,
		dataChan: make(chan *WeatherData, 1),
		unit:     "imperial", // default
	}
}

func (w *Weather) Name() string {
	return "weather"
}

func (w *Weather) UpdateInterval() time.Duration {
	return w.interval
}

func (w *Weather) Start(ctx context.Context) error {
	w.mu.Lock()
	if w.cancel != nil {
		w.mu.Unlock()
		return fmt.Errorf("instrument already started")
	}
	w.ctx, w.cancel = context.WithCancel(ctx)
	w.mu.Unlock()

	w.wg.Add(1)
	go w.run()

	w.logger.Debug("weather monitor started", "interval", w.interval)
	return nil
}

func (w *Weather) Stop() error {
	w.mu.Lock()
	if w.cancel == nil {
		w.mu.Unlock()
		return nil
	}
	w.cancel()
	w.mu.Unlock()

	w.wg.Wait()
	w.logger.Debug("weather monitor stopped")
	return nil
}

// GetCurrent returns the most recent weather data
func (w *Weather) GetCurrent() *WeatherData {
	w.mu.RLock()
	defer w.mu.RUnlock()
	return w.current
}

// SetLocation updates the location and triggers an immediate update
func (w *Weather) SetLocation(location string) {
	w.mu.Lock()
	if w.lastLocation != location {
		w.logger.Info("weather location changed", "from", w.lastLocation, "to", location)
		w.lastLocation = location
		w.mu.Unlock()
		w.updateNow()
	} else {
		w.mu.Unlock()
	}
}

// SetUnit updates the unit system ("metric" or "imperial")
func (w *Weather) SetUnit(unit string) {
	w.mu.Lock()
	if w.unit != unit {
		w.logger.Info("weather unit changed", "from", w.unit, "to", unit)
		w.unit = unit
		w.mu.Unlock()
		w.updateNow()
	} else {
		w.mu.Unlock()
	}
}

func (w *Weather) run() {
	defer w.wg.Done()

	ticker := time.NewTicker(w.interval)
	defer ticker.Stop()

	// Initial update
	w.updateNow()

	for {
		select {
		case <-w.ctx.Done():
			return
		case <-ticker.C:
			w.updateNow()
		}
	}
}

func (w *Weather) updateNow() {
	w.mu.Lock()
	if w.updating {
		w.mu.Unlock()
		return
	}
	w.updating = true
	location := w.lastLocation
	unit := w.unit
	w.mu.Unlock()

	defer func() {
		w.mu.Lock()
		w.updating = false
		w.mu.Unlock()
	}()

	if location == "" {
		w.logger.Debug("weather update skipped: no location configured")
		return
	}

	data, err := w.fetchWeather(location, unit)
	if err != nil {
		w.logger.Warn("failed to fetch weather", "location", location, "error", err)
		return
	}

	w.mu.Lock()
	w.current = data
	w.mu.Unlock()

	select {
	case w.dataChan <- data:
	default:
	}

	w.logger.Info("weather updated",
		"location", location,
		"temperature", data.Temperature,
		"unit", unit)
}

func (w *Weather) fetchWeather(location, unit string) (*WeatherData, error) {
	// Get coordinates
	lat, lon, err := w.getCityCoordinates(location)
	if err != nil {
		w.logger.Warn("failed to get coordinates, using defaults", "location", location, "error", err)
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

	req, err := http.NewRequestWithContext(w.ctx, "GET", weatherURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
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
		Humidity:    0, // Not provided by Open-Meteo free tier
	}, nil
}

func (w *Weather) getCityCoordinates(location string) (float64, float64, error) {
	searchURL := fmt.Sprintf(nominatimSearchURL, url.QueryEscape(location))

	req, err := http.NewRequestWithContext(w.ctx, "GET", searchURL, nil)
	if err != nil {
		return 0, 0, err
	}

	req.Header.Set("User-Agent", "Nexus-Open/1.0")

	client := &http.Client{Timeout: 10 * time.Second}
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

	return lat, lon, nil
}

// weatherCodeToIcon converts WMO weather code to icon character
func weatherCodeToIcon(code int, isDay bool) string {
	// Using standard Unicode symbols that work with common fonts
	weatherIcons := map[int]struct{ day, night string }{
		0:  {day: "☀", night: "☾"}, // Clear sky
		1:  {day: "☀", night: "☾"}, // Mainly clear
		2:  {day: "⛅", night: "☁"}, // Partly cloudy
		3:  {day: "☁", night: "☁"}, // Cloudy
		45: {day: "🌫", night: "🌫"}, // Foggy
		48: {day: "🌫", night: "🌫"}, // Rime fog
		51: {day: "🌦", night: "🌧"}, // Light drizzle
		53: {day: "🌦", night: "🌧"}, // Drizzle
		55: {day: "🌧", night: "🌧"}, // Heavy drizzle
		56: {day: "🌧", night: "🌧"}, // Light freezing drizzle
		57: {day: "🌧", night: "🌧"}, // Freezing drizzle
		61: {day: "🌦", night: "🌧"}, // Light rain
		63: {day: "🌧", night: "🌧"}, // Rain
		65: {day: "🌧", night: "🌧"}, // Heavy rain
		66: {day: "🌧", night: "🌧"}, // Light freezing rain
		67: {day: "🌧", night: "🌧"}, // Freezing rain
		71: {day: "🌨", night: "🌨"}, // Light snow
		73: {day: "❄", night: "❄"}, // Snow
		75: {day: "❄", night: "❄"}, // Heavy snow
		77: {day: "❄", night: "❄"}, // Snow grains
		80: {day: "🌦", night: "🌧"}, // Light showers
		81: {day: "🌧", night: "🌧"}, // Showers
		82: {day: "🌧", night: "🌧"}, // Heavy showers
		85: {day: "🌨", night: "🌨"}, // Light snow showers
		86: {day: "❄", night: "❄"}, // Snow showers
		95: {day: "⛈", night: "⛈"}, // Thunderstorm
		96: {day: "⛈", night: "⛈"}, // Thunderstorm with hail
		99: {day: "⛈", night: "⛈"}, // Heavy thunderstorm with hail
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
