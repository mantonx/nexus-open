package main

// weatherCodeToIcon maps a WMO weather code to a FontAwesome Unicode glyph.
func weatherCodeToIcon(code int, isDay bool) string {
	icons := map[int]struct{ day, night string }{
		0:  {"", ""}, // sun / moon
		1:  {"", ""},
		2:  {"", ""}, // cloud-sun / cloud-moon
		3:  {"", ""}, // cloud
		45: {"", ""}, // smog
		48: {"", ""},
		51: {"", ""}, // cloud-rain
		53: {"", ""},
		55: {"", ""},
		56: {"", ""},
		57: {"", ""},
		61: {"", ""},
		63: {"", ""},
		65: {"", ""},
		66: {"", ""},
		67: {"", ""},
		71: {"", ""}, // snowflake
		73: {"", ""},
		75: {"", ""},
		77: {"", ""},
		80: {"", ""},
		81: {"", ""},
		82: {"", ""},
		85: {"", ""},
		86: {"", ""},
		95: {"", ""}, // thunderstorm
		96: {"", ""},
		99: {"", ""},
	}

	if w, ok := icons[code]; ok {
		if isDay {
			return w.day
		}
		return w.night
	}
	return "" // default: cloud
}

// weatherCodeToCondition maps a WMO weather code to a human-readable description.
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
	if c, ok := conditions[code]; ok {
		return c
	}
	return "Unknown"
}
