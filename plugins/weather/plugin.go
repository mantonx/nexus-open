package main

import (
	"fmt"
	"strings"
	"sync"
	"time"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

const sampleCacheTTL = 5 * time.Minute

// WeatherPlugin monitors weather conditions.
type WeatherPlugin struct {
	mu          sync.RWMutex
	lastUpdate  time.Time
	cachedData  *WeatherData
	location    string
	unit        string // "metric" or "imperial"
	coordsCache map[string]coords
	coordsMu    sync.Mutex
}

func NewWeatherPlugin() *WeatherPlugin {
	return &WeatherPlugin{
		coordsCache: make(map[string]coords),
		location:    "Jersey City, NJ",
		unit:        "imperial",
	}
}

func (m *WeatherPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Weather",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors weather conditions via Open-Meteo API",
		Icon:        "cloud",
		RefreshMs:   300000,
		Schema: plugin.ConfigSchema{
			Fields: []plugin.ConfigField{
				{
					Key: "unit", Label: "Units", Type: plugin.FieldTypeEnum, Default: "imperial",
					Options: []plugin.FieldOption{{Value: "imperial", Label: "°F"}, {Value: "metric", Label: "°C"}},
				},
				{
					Key: "location", Label: "Location", Type: plugin.FieldTypeLocation, Default: "Jersey City, NJ",
				},
			},
		},
	}, nil
}

func (m *WeatherPlugin) Sample() (plugin.Payload, error) {
	m.mu.RLock()
	if m.cachedData != nil && time.Since(m.lastUpdate) < sampleCacheTTL {
		data := m.cachedData
		m.mu.RUnlock()
		return m.formatPayload(data), nil
	}
	m.mu.RUnlock()

	data, err := m.fetchWeather()
	if err != nil {
		m.mu.RLock()
		if m.cachedData != nil {
			data := m.cachedData
			m.mu.RUnlock()
			p := m.formatPayload(data)
			p.TTL = sampleCacheTTL
			return p, nil
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

	m.mu.Lock()
	m.cachedData = data
	m.lastUpdate = time.Now()
	m.mu.Unlock()

	return m.formatPayload(data), nil
}

func (m *WeatherPlugin) Configure(cfg map[string]any) error {
	m.mu.Lock()
	oldLocation := m.location
	oldUnit := m.unit
	if location, ok := cfg["location"].(string); ok && location != "" {
		m.location = location
	}
	if unit, ok := cfg["unit"].(string); ok && unit != "" {
		m.unit = unit
	}
	newLocation := m.location
	newUnit := m.unit
	m.mu.Unlock()

	if newLocation != oldLocation || newUnit != oldUnit {
		data, err := m.fetchWeather()
		if err != nil {
			m.mu.Lock()
			m.cachedData = nil
			m.lastUpdate = time.Time{}
			m.mu.Unlock()
			return nil
		}
		m.mu.Lock()
		m.cachedData = data
		m.lastUpdate = time.Now()
		m.mu.Unlock()
	}
	return nil
}

func (m *WeatherPlugin) formatPayload(data *WeatherData) plugin.Payload {
	var tempVal, tempUnit string
	if data.Unit == "imperial" {
		tempVal = fmt.Sprintf("%.0f", data.Temperature)
		tempUnit = "°F"
	} else {
		tempVal = fmt.Sprintf("%.0f", data.Temperature)
		tempUnit = "°C"
	}

	loc := data.Location
	if i := strings.Index(loc, ","); i > 0 {
		loc = strings.TrimSpace(loc[:i])
	}

	return plugin.Payload{
		Primary:   tempVal + tempUnit,
		Value:     tempVal,
		ValueUnit: tempUnit,
		Secondary: loc,
		Severity:  plugin.SeverityOK,
		Icon:      data.Icon,
		Timestamp: time.Now(),
	}
}
