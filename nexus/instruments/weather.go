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
	Temperature float64
	Condition   string
	WindSpeed   string
}

const (
	openMeteoBaseURL   = "https://api.open-meteo.com/v1/forecast?temperature_unit=%s&wind_speed_unit=%s&latitude=%.4f&longitude=%.4f&current=temperature_2m,weather_code,wind_speed_10m,is_day"
	nominatimSearchURL = "https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1"
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
		log.Fatalf("Failed to get city coordinates: %v", err)
		return nil
	}

	weather, err := GetWeatherConditions(lat, lon)
	if err != nil {
		log.Fatalf("Failed to get weather forecast: %v", err)
		return nil
	}

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
func GetCityCoordinates(city string) (float64, float64, error) {
	baseURL := fmt.Sprintf(nominatimSearchURL, url.QueryEscape(city))

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
		WindSpeed:   fmt.Sprintf("%.1f", result.Current.WindSpeed),
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
	weatherCodes := map[int]struct{ day, night struct{ description string } }{
		0:  {day: struct{ description string }{description: "Sunny"}, night: struct{ description string }{description: "Clear"}},
		1:  {day: struct{ description string }{description: "Mainly Sunny"}, night: struct{ description string }{description: "Mainly Clear"}},
		2:  {day: struct{ description string }{description: "Partly Cloudy"}, night: struct{ description string }{description: "Partly Cloudy"}},
		3:  {day: struct{ description string }{description: "Cloudy"}, night: struct{ description string }{description: "Cloudy"}},
		45: {day: struct{ description string }{description: "Foggy"}, night: struct{ description string }{description: "Foggy"}},
		48: {day: struct{ description string }{description: "Rime Fog"}, night: struct{ description string }{description: "Rime Fog"}},
		51: {day: struct{ description string }{description: "Light Drizzle"}, night: struct{ description string }{description: "Light Drizzle"}},
		53: {day: struct{ description string }{description: "Drizzle"}, night: struct{ description string }{description: "Drizzle"}},
		55: {day: struct{ description string }{description: "Heavy Drizzle"}, night: struct{ description string }{description: "Heavy Drizzle"}},
		56: {day: struct{ description string }{description: "Light Freezing Drizzle"}, night: struct{ description string }{description: "Light Freezing Drizzle"}},
		57: {day: struct{ description string }{description: "Freezing Drizzle"}, night: struct{ description string }{description: "Freezing Drizzle"}},
		61: {day: struct{ description string }{description: "Light Rain"}, night: struct{ description string }{description: "Light Rain"}},
		63: {day: struct{ description string }{description: "Rain"}, night: struct{ description string }{description: "Rain"}},
		65: {day: struct{ description string }{description: "Heavy Rain"}, night: struct{ description string }{description: "Heavy Rain"}},
		66: {day: struct{ description string }{description: "Light Freezing Rain"}, night: struct{ description string }{description: "Light Freezing Rain"}},
		67: {day: struct{ description string }{description: "Freezing Rain"}, night: struct{ description string }{description: "Freezing Rain"}},
		71: {day: struct{ description string }{description: "Light Snow"}, night: struct{ description string }{description: "Light Snow"}},
		73: {day: struct{ description string }{description: "Snow"}, night: struct{ description string }{description: "Snow"}},
		75: {day: struct{ description string }{description: "Heavy Snow"}, night: struct{ description string }{description: "Heavy Snow"}},
		77: {day: struct{ description string }{description: "Snow Grains"}, night: struct{ description string }{description: "Snow Grains"}},
		80: {day: struct{ description string }{description: "Light Showers"}, night: struct{ description string }{description: "Light Showers"}},
		81: {day: struct{ description string }{description: "Showers"}, night: struct{ description string }{description: "Showers"}},
		82: {day: struct{ description string }{description: "Heavy Showers"}, night: struct{ description string }{description: "Heavy Showers"}},
		85: {day: struct{ description string }{description: "Light Snow Showers"}, night: struct{ description string }{description: "Light Snow Showers"}},
		86: {day: struct{ description string }{description: "Snow Showers"}, night: struct{ description string }{description: "Snow Showers"}},
		95: {day: struct{ description string }{description: "Thunderstorm"}, night: struct{ description string }{description: "Thunderstorm"}},
		96: {day: struct{ description string }{description: "Light Thunderstorms With Hail"}, night: struct{ description string }{description: "Light Thunderstorms With Hail"}},
		99: {day: struct{ description string }{description: "Thunderstorm With Hail"}, night: struct{ description string }{description: "Thunderstorm With Hail"}},
	}

	if weather, ok := weatherCodes[code]; ok {
		if isDay {
			return weather.day.description
		}
		return weather.night.description
	}
	return "Unknown"
}
