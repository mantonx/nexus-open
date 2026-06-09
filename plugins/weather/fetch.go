package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strconv"
	"time"
)

const (
	openMeteoBaseURL   = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&wind_speed_unit=%s&latitude=%.4f&longitude=%.4f&current=temperature_2m,weather_code,wind_speed_10m,is_day"
	openMeteoDailyURL  = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&latitude=%.4f&longitude=%.4f&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=7"
	nominatimSearchURL = "https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1"
	defaultLat         = 40.7282
	defaultLon         = -74.0776
)

// WeatherData holds current conditions returned by the API.
type WeatherData struct {
	Location    string
	Temperature float64
	Description string
	Icon        string
	WeatherCode int
	Unit        string
	WindSpeed   float64
}

type coords struct {
	lat float64
	lon float64
}

func (m *WeatherPlugin) fetchWeather() (*WeatherData, error) {
	m.mu.RLock()
	location := m.location
	unit := m.unit
	m.mu.RUnlock()

	if location == "" {
		return nil, fmt.Errorf("no location configured")
	}

	lat, lon, err := m.getCityCoordinates(location)
	if err != nil {
		lat, lon = defaultLat, defaultLon
	}

	tempUnit, windUnit := "celsius", "kmh"
	if unit == "imperial" {
		tempUnit, windUnit = "fahrenheit", "mph"
	}

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(fmt.Sprintf(openMeteoBaseURL, tempUnit, windUnit, lat, lon))
	if err != nil {
		return nil, fmt.Errorf("fetch weather: %w", err)
	}
	defer func() { _ = resp.Body.Close() }()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status: %d", resp.StatusCode)
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
		return nil, fmt.Errorf("decode weather: %w", err)
	}

	return &WeatherData{
		Location:    location,
		Temperature: result.Current.Temperature,
		Description: weatherCodeToCondition(result.Current.WeatherCode),
		Icon:        weatherCodeToIcon(result.Current.WeatherCode, result.Current.IsDay == 1),
		WeatherCode: result.Current.WeatherCode,
		Unit:        unit,
		WindSpeed:   result.Current.WindSpeed,
	}, nil
}

func (m *WeatherPlugin) getCityCoordinates(location string) (float64, float64, error) {
	m.coordsMu.Lock()
	if cached, ok := m.coordsCache[location]; ok {
		m.coordsMu.Unlock()
		return cached.lat, cached.lon, nil
	}
	m.coordsMu.Unlock()

	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequest("GET", fmt.Sprintf(nominatimSearchURL, url.QueryEscape(location)), nil)
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
		return 0, 0, fmt.Errorf("nominatim status: %d", resp.StatusCode)
	}

	var results []struct {
		Lat string `json:"lat"`
		Lon string `json:"lon"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&results); err != nil {
		return 0, 0, fmt.Errorf("decode coordinates: %w", err)
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

	m.coordsMu.Lock()
	m.coordsCache[location] = coords{lat: lat, lon: lon}
	m.coordsMu.Unlock()

	return lat, lon, nil
}
