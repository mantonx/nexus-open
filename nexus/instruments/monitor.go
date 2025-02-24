package instruments

import (
	"log"
	"nexus-open/nexus/configuration"
	"sync/atomic"
	"time"
)

const (
	weatherUpdateInterval = 10 * time.Minute
	tempUpdateInterval    = 5 * time.Second
	networkUpdateInterval = 1 * time.Second
)

type SystemTemperature struct {
	CPU float64
	GPU float64
}

type NetworkStats struct {
	Sent     int
	Received int
}

// WeatherState holds current weather data and update status
type WeatherState struct {
	lastLocation string
	info         *WeatherInfo
	updating     atomic.Bool
}

// StartWeatherMonitor initializes and runs a weather monitoring service in the background.
// It periodically fetches weather data based on the location specified in the configuration.
//
// Parameters:
//   - getConfig: A function that returns the current NexusConfig. Must not be nil.
//   - connected: A pointer to a boolean indicating if the system is currently connected.
//
// Returns:
//   - A receive-only channel that provides WeatherInfo updates
//   - A send-only channel to request immediate weather updates
//
// The monitor runs in a goroutine and will:
//   - Update weather data periodically based on weatherUpdateInterval
//   - Update immediately when requested through the update channel
//   - Update when location changes in configuration
//   - Only update when system is connected
//   - Use atomic operations to prevent concurrent updates
func StartWeatherMonitor(
	getConfig func() *configuration.NexusConfig,
	connected *bool,
) (chan *WeatherInfo, chan<- struct{}) {
	if getConfig == nil {
		log.Fatal("Weather monitor: config getter function is required")
	}

	weatherChan := make(chan *WeatherInfo, 1)

	updateChan := make(chan struct{}, 1)

	state := &WeatherState{}

	go func() {
		ticker := time.NewTicker(weatherUpdateInterval)
		defer ticker.Stop()

		// Weather update handler
		updateWeather := func() {
			if !state.updating.CompareAndSwap(false, true) {
				return // Already updating
			}

			defer state.updating.Store(false)

			cfg := getConfig()

			if cfg == nil {
				log.Printf("Weather monitor: no config available")
				return
			}

			// Always update if location changed
			locationChanged := state.lastLocation != cfg.Location

			if locationChanged {
				log.Printf("Weather monitor: location changed from %q to %q",
					state.lastLocation, cfg.Location)
				state.lastLocation = cfg.Location
			}

			if cfg.Location == "" {
				return
			}

			info := GetWeatherData(cfg.Location, &cfg.Unit)

			if info != nil {
				state.info = info
				log.Printf("Weather updated for %s: %.1f%s",
					cfg.Location, info.Temperature,
					map[string]string{"metric": "°C", "imperial": "°F"}[cfg.Unit])
				select {
				case weatherChan <- info:
				default:
				}
			}
		}

		// Initial update
		updateWeather()

		// Periodic updates
		for {
			select {
			case <-ticker.C:
				if *connected {
					updateWeather()
				}
			case <-updateChan:
				// Immediate update when requested
				if *connected {
					log.Printf("Weather monitor: update requested")
					updateWeather()
				}
			}
		}
	}()

	return weatherChan, updateChan
}

// StartTempatureMonitor initializes and runs a temperature monitoring goroutine.
// It takes a pointer to a boolean indicating connection status and returns a channel
// that receives Temperature updates.
//
// The monitor continuously checks CPU and GPU temperatures when connected is true.
// If either temperature check fails, it logs the error and retries after 1 second.
// Successfully read temperatures are sent through the returned channel as Temperature structs.
//
// The monitoring runs in a separate goroutine and continues until the program terminates.
// Temperature updates are sent at intervals defined by tempUpdateInterval.
//
// Parameters:
//   - connected: *bool - Pointer to connection status flag
//
// Returns:
//   - chan Temperature - Channel through which temperature updates are sent
func StartTempatureMonitor(connected *bool) chan SystemTemperature {
	systemTempChan := make(chan SystemTemperature)

	go func() {
		for {
			if !*connected {
				continue
			}

			cpu, err := GetCPUTemp()
			if err != nil {
				log.Printf("Failed to get CPU temperature: %v", err)
				time.Sleep(tempUpdateInterval)
				continue
			}

			gpu, err := GetGPUTemp()
			if err != nil {
				log.Printf("Failed to get GPU temperature: %v", err)
				time.Sleep(tempUpdateInterval)
				continue
			}

			systemTempChan <- SystemTemperature{
				CPU: cpu,
				GPU: gpu,
			}
			time.Sleep(tempUpdateInterval)
		}
	}()

	return systemTempChan
}

// StartNetworkMonitor initializes and starts a network monitoring goroutine.
// It takes a pointer to a boolean that indicates connection status and returns
// a channel that streams NetworkStats.
//
// The monitor continuously checks network usage when connected is true,
// collecting sent and received bytes statistics. If network usage collection fails,
// the error is logged and the monitor continues operation.
//
// The monitoring runs at intervals defined by networkUpdateInterval.
// Network statistics are sent through the returned channel.
//
// Parameters:
//   - connected: *bool - Pointer to connection status flag
//
// Returns:
//   - chan NetworkStats - Channel streaming network statistics
func StartNetworkMonitor(connected *bool) chan NetworkStats {
	networkChan := make(chan NetworkStats)

	go func() {
		for {
			if !*connected {
				continue
			}
			sent, received, err := GetNetworkUsage()
			if err != nil {
				log.Printf("Failed to get network usage: %v", err)
				continue
			}
			networkChan <- NetworkStats{
				Sent:     sent,
				Received: received,
			}
			time.Sleep(networkUpdateInterval)
		}
	}()

	return networkChan
}
