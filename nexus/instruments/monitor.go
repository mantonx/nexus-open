package instruments

import (
	"log"
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

// StartWeatherMonitor initializes and returns a channel that streams weather information updates.
// It continuously monitors weather data in the background and sends updates through the returned channel
// when the system is connected. The monitoring is controlled by the connected parameter - when false,
// the monitoring loop continues but does not fetch or send new data.
//
// Parameters:
//   - connected: A pointer to a boolean that controls whether weather updates are active
//
// Returns:
//   - A channel of WeatherInfo pointers through which weather updates are sent
func StartWeatherMonitor(location string, unit *string, connected *bool) chan *WeatherInfo {
	weatherChan := make(chan *WeatherInfo)
	weatherInfo := GetWeatherData(location, unit)

	go func() {
		for {
			if !*connected {
				continue
			}

			weatherInfo = GetWeatherData(location, unit)
			weatherChan <- weatherInfo
			time.Sleep(weatherUpdateInterval)
		}
	}()

	return weatherChan
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
