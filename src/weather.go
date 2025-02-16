package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"
)

type WeatherInfo struct {
	Temperature float64
	Condition   string
	Forecast    string
}

func GetWeatherData() {
	city := "New York"
	lat, lon, err := GetCityCoordinates(city)
	if err != nil {
		fmt.Println(err)
		return
	}
	fmt.Print(lat, lon)
}

// GetCityCoordinates returns the latitude and longitude for a given city name
// using the OpenStreetMap Nominatim API
func GetCityCoordinates(city string) (float64, float64, error) {
	url := fmt.Sprintf("https://nominatim.openstreetmap.org/search?q=%s&format=json&limit=1", city)

	client := &http.Client{}
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return 0, 0, err
	}

	req.Header.Set("User-Agent", "Nexus Next Weather App")

	resp, err := client.Do(req)
	if err != nil {
		return 0, 0, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return 0, 0, err
	}

	var result []struct {
		Lat string `json:"lat"`
		Lon string `json:"lon"`
	}

	if err := json.Unmarshal(body, &result); err != nil {
		return 0, 0, err
	}

	if len(result) == 0 {
		return 0, 0, errors.New("city not found")
	}

	lat, err := strconv.ParseFloat(result[0].Lat, 64)
	if err != nil {
		return 0, 0, err
	}

	lon, err := strconv.ParseFloat(result[0].Lon, 64)
	if err != nil {
		return 0, 0, err
	}

	fmt.Printf("City: %s, Latitude: %f, Longitude: %f\n", city, lat, lon)

	return lat, lon, nil
}
