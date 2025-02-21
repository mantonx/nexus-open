package instruments

import (
	"log"
	"nexus-open/nexus/configuration"
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

// StartWeatherMonitor now takes a config getter function
func StartWeatherMonitor(
	getConfig func() *configuration.NexusConfig,
	connected *bool,
) chan *WeatherInfo {
	weatherChan := make(chan *WeatherInfo)

	go func() {
		for {
			if !*connected {
				time.Sleep(time.Second)
				continue
			}

			config := getConfig()
			weatherInfo := GetWeatherData(config.Location, &config.Unit)
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
