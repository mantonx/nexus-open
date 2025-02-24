package instruments

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"net/url"
	"strconv"
)

var tempUnit string
var windSpeedUnit string

type WeatherInfo struct {
	Location    string
	Temperature float64
	Condition   string
	WindSpeed   string
}

const (
	openMeteoBaseURL   = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&wind_speed_unit=%s&latitude=%.4f&longitude=%.4f&current=temperature_2m,weather_code,wind_speed_10m,is_day"
	nominatimSearchURL = "https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1"
	defaultLat         = 40.7128  // New York, NY
	defaultLon         = -74.0060 // New York, NY
)

func GetWeatherData(location string, unit *string) *WeatherInfo {
	// Validate and normalize temperature unit
	if *unit == "imperial" {
		tempUnit = "fahrenheit"
		windSpeedUnit = "mph"
	} else { // metric
		tempUnit = "celsius"
		windSpeedUnit = "kmh"
	}

	lat, lon, err := GetCityCoordinates(location)

	if err != nil {
		log.Printf("Failed to get city coordinates: %v, falling back to New York, NY", err)
		lat = defaultLat
		lon = defaultLat
	}

	weather, err := GetWeatherConditions(lat, lon)
	if err != nil {
		log.Fatalf("Failed to get weather forecast: %v", err)
		return nil
	}

	// Set the location in the weather info
	weather.Location = location

	return weather
}

// GetCityCoordinates takes a city name as input and returns its geographical coordinates (latitude and longitude)
// by querying the OpenStreetMap Nominatim API. The function performs HTTP GET request to fetch the location data.
//
// Parameters:
//   - city: string representing the name of the city to look up
//
// Returns:
//   - float64: latitude of the city
//   - float64: longitude of the city
//   - error: nil if successful, otherwise contains the error description
//     Possible errors include:
//   - HTTP request creation failure
//   - HTTP request execution failure
//   - Unexpected HTTP status code
//   - JSON decoding errors
//   - City not found in the database
//
// The function uses the Nominatim API which requires a User-Agent header and returns coordinates as strings
// that are converted to float64 values before being returned.
func GetCityCoordinates(location string) (float64, float64, error) {
	baseURL := fmt.Sprintf(nominatimSearchURL, url.QueryEscape(location))

	client := &http.Client{}
	req, err := http.NewRequestWithContext(context.Background(), "GET", baseURL, nil)

	if err != nil {
		return 0, 0, err
	}

	req.Header.Set("User-Agent", "Nexus Next/1.0")

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
		return 0, 0, fmt.Errorf("city not found")
	}

	// // Return the latitude and longitude as float64
	lat, _ := strconv.ParseFloat(results[0].Lat, 64)
	lon, _ := strconv.ParseFloat(results[0].Lon, 64)

	return lat, lon, nil
}

// GetWeatherConditions retrieves current weather information for the specified location.
//
// Parameters:
//   - lat: The latitude of the location (float64)
//   - lon: The longitude of the location (float64)
//   - tempUnit: The desired temperature unit ("celsius" or "fahrenheit")
//
// Returns:
//   - *WeatherInfo: A pointer to a WeatherInfo struct containing:
//   - Temperature: Current temperature in the specified unit
//   - Condition: Weather condition description
//   - WindSpeed: Wind speed formatted to one decimal place
//   - error: An error if the API request fails or response parsing fails
//
// The function uses the Open-Meteo API to fetch weather data including temperature,
// weather code, wind speed, and daylight status. It converts the weather code to
// a human-readable condition description internally.
func GetWeatherConditions(lat, lon float64) (*WeatherInfo, error) {
	baseURL := fmt.Sprintf(openMeteoBaseURL, tempUnit, windSpeedUnit, lat, lon)

	resp, err := http.Get(baseURL)

	if err != nil {
		return nil, err
	}

	defer resp.Body.Close()

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

	condition := weatherCodeToCondition(result.Current.WeatherCode, result.Current.IsDay == 1)

	return &WeatherInfo{
		Temperature: result.Current.Temperature,
		Condition:   condition,
		WindSpeed:   fmt.Sprintf("\ue31e %.1f", result.Current.WindSpeed),
	}, nil
}

// weatherCodeToCondition converts a numerical weather code and time of day into a human-readable weather condition string.
//
// The function takes two parameters:
//   - code: an integer representing the WMO (World Meteorological Organization) weather code
//   - isDay: a boolean indicating whether it's daytime (true) or nighttime (false)
//
// It returns a string description of the weather condition, with different descriptions for day and night
// for certain weather states (e.g., "Sunny" during day becomes "Clear" at night).
// If the provided weather code is not recognized, it returns "Unknown".
//
// Weather codes cover various conditions including:
//   - Clear/Sunny conditions (0-3)
//   - Fog conditions (45, 48)
//   - Precipitation (drizzle: 51-57, rain: 61-67, snow: 71-77)
//   - Showers (80-86)
//   - Thunderstorms (95-99)
func weatherCodeToCondition(code int, isDay bool) string {
	weatherCodes := map[int]struct{ day, night string }{
		0:  {day: "\ue30d", night: "\ue32b"}, // Clear sky
		1:  {day: "\ue302", night: "\ue37e"}, // Mainly clear
		2:  {day: "\ue312", night: "\ue379"}, // Partly cloudy
		3:  {day: "\ue33d", night: "\ue33d"}, // Cloudy
		45: {day: "\ue313", night: "\ue346"}, // Foggy
		48: {day: "\ue313", night: "\ue346"}, // Rime fog
		51: {day: "\ue308", night: "\ue325"}, // Light drizzle
		53: {day: "\ue308", night: "\ue325"}, // Drizzle
		55: {day: "\ue318", night: "\ue318"}, // Heavy drizzle
		56: {day: "\ue3aa", night: "\ue3ac"}, // Light freezing drizzle
		57: {day: "\ue3aa", night: "\ue3ac"}, // Freezing drizzle
		61: {day: "\ue308", night: "\ue325"}, // Light rain
		63: {day: "\ue318", night: "\ue318"}, // Rain
		65: {day: "\ue318", night: "\ue318"}, // Heavy rain
		66: {day: "\ue3aa", night: "\ue3ac"}, // Light freezing rain
		67: {day: "\ue3ad", night: "\ue3ad"}, // Freezing rain
		71: {day: "\ue31a", night: "\ue327"}, // Light snow
		73: {day: "\ue30a", night: "\ue30a"}, // Snow
		75: {day: "\ue30a", night: "\ue30a"}, // Heavy snow
		77: {day: "\ue30a", night: "\ue30a"}, // Snow grains
		80: {day: "\ue308", night: "\ue325"}, // Light showers
		81: {day: "\ue318", night: "\ue318"}, // Showers
		82: {day: "\ue318", night: "\ue318"}, // Heavy showers
		85: {day: "\ue31a", night: "\ue327"}, // Light snow showers
		86: {day: "\ue30a", night: "\ue30a"}, // Snow showers
		95: {day: "\ue30f", night: "\ue32a"}, // Thunderstorm
		96: {day: "\ue31d", night: "\ue31d"}, // Thunderstorm with hail
		99: {day: "\ue31d", night: "\ue31d"}, // Heavy thunderstorm with hail
	}

	if weather, ok := weatherCodes[code]; ok {
		if isDay {
			return weather.day
		}
		return weather.night
	}
	return "❓"
}
