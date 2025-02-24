// Package main implements a screen management system for the iCUE Nexus device.
//
// The screen management system handles the following functionalities:
// - Real-time temperature monitoring (CPU and GPU)
// - Network statistics display
// - Weather information updates
// - Device connection management
// - Screen buffer management and rendering
//
// The main components are:
//   - UpdateScreen: Manages concurrent updates from multiple data sources
//   - updateDeviceScreen: Safely updates the device display with new state
//   - resetDevice: Handles device disconnection and cleanup
//   - CreateNexusScreen: Renders the display content
//   - setNexusImage: Handles low-level USB communication for screen updates
//
// The screen refresh rate is maintained at 24 Hz for optimal performance.
// Thread safety is ensured through mutex locks when accessing shared device resources.
//
// USB Protocol Details:
// The device communicates using a custom protocol with:
// - 1024*4 byte buffer size
// - 120 chunks of image data
// - RGBA color format (8 bits per channel)
// - Custom header format for each chunk
//
// Note: The device automatically handles disconnection events and will attempt
// to gracefully handle connection loss without throwing errors.

package nexus

import (
	"bufio"
	"fmt"
	"log"
	"nexus-open/nexus/instruments"
	"sync"
	"time"
)

type CreateScreenConfig struct {
	cputemp         float64
	gputemp         float64
	network         instruments.NetworkStats
	weather         *instruments.WeatherInfo
	timeFormat      string
	textColor       string
	backgroundColor string
}

var deviceMutex sync.Mutex

// StartDisplayUpdate initiates a goroutine that manages the display updates for system metrics.
// It receives data from three channels:
//   - tempChan: provides CPU and GPU temperature readings
//   - networkChan: provides network statistics
//   - weatherChan: provides weather information updates
//
// The function maintains an internal state that is updated whenever new data arrives from any
// of the input channels. The display is refreshed at a rate defined by screenRefreshRate (24Hz).
// If a display update fails, it logs the error and attempts to reset the display device.
//
// This function is non-blocking as it launches the update loop in a separate goroutine.
func StartDisplayUpdate(
	tempChan <-chan instruments.SystemTemperature,
	networkChan <-chan instruments.NetworkStats,
	weatherChan <-chan *instruments.WeatherInfo,
	configUpdate <-chan struct{},
	weatherUpdate chan<- struct{}, // Add weather update trigger
) {
	go func() {
		state := struct {
			cpu               float64
			gpu               float64
			network           instruments.NetworkStats
			weather           *instruments.WeatherInfo
			lastWeatherUpdate time.Time
		}{}

		refreshRate := time.NewTicker(time.Second / screenRefreshRate) // 24 Hz (~0.042s)

		defer refreshRate.Stop()

		for {
			select {
			case temps := <-tempChan:
				state.cpu, state.gpu = temps.CPU, temps.GPU // Fix: Change GPU to temps.GPU
			case network := <-networkChan:
				state.network = network
			case weather := <-weatherChan:
				if weather != nil {
					state.weather = weather
					state.lastWeatherUpdate = time.Now()
					if err := updateDisplay(&state); err != nil {
						log.Printf("Weather update display failed: %v", err)
					}
				}
			case <-configUpdate:
				// Update display settings immediately without blocking
				if cfg := GetConfig(); cfg != nil {
					SetTimeFormat(cfg.TimeFormat)
					SetTextColor(cfg.TextColor)
					// Trigger weather update
					select {
					case weatherUpdate <- struct{}{}:
					default:
					}
					// Force weather update if it's been more than 30 seconds
					if time.Since(state.lastWeatherUpdate) > 30*time.Second {
						if weather := instruments.GetWeatherData(cfg.Location, &cfg.Unit); weather != nil {
							state.weather = weather
							state.lastWeatherUpdate = time.Now()
						}
					}
					// Immediate display update
					if err := updateDisplay(&state); err != nil {
						log.Printf("Config update display failed: %v", err)
					}
				}
			case <-refreshRate.C:
				if err := updateDisplay(&state); err != nil {
					log.Printf("Screen update failed: %v", err)
					resetDevice()
				}
			}
		}
	}()
}

// updateDisplay updates the device's screen with system and weather information.
// It takes a pointer to a struct containing CPU temperature, GPU temperature,
// network statistics and weather information.
//
// The function ensures thread-safety by using deviceMutex when checking device connectivity.
// If the device is not connected or nil, the function returns early without error.
//
// The function creates a screen configuration with the provided state data and
// calls DrawScreen to update the physical display.
//
// Returns an error if the screen drawing operation fails, nil otherwise.
func updateDisplay(state *struct {
	cpu               float64
	gpu               float64
	network           instruments.NetworkStats
	weather           *instruments.WeatherInfo
	lastWeatherUpdate time.Time
}) error {
	deviceMutex.Lock()

	if !connected || device == nil {
		deviceMutex.Unlock()
		return nil
	}

	deviceMutex.Unlock()

	cfg := GetConfig()
	if cfg == nil {
		return nil
	}

	config := CreateScreenConfig{
		cputemp:         state.cpu,
		gputemp:         state.gpu,
		network:         state.network,
		weather:         state.weather,
		backgroundColor: cfg.BackgroundColor,
	}

	return drawDisplay(config)
}

// resetDevice safely closes and resets the current device connection.
// It acquires a device mutex lock to ensure thread-safe access,
// closes any existing device connection, and resets device state
// variables to their zero values. The mutex is automatically unlocked
// when the function returns.
func resetDevice() {
	deviceMutex.Lock()
	defer deviceMutex.Unlock()

	if device != nil {
		device.Close()
	}

	device = nil
	connected = false
}

// DrawScreen updates the display with various system information and weather data.
// It creates an image buffer, draws temperature information, network statistics,
// current time, and weather data onto the display using the provided configuration.
//
// Parameters:
//   - config: CreateScreenConfig containing system metrics and weather information
//
// Returns:
//   - error: nil if successful, error if display update fails
//
// If the display device is not initialized (nil), the function returns without error.
// On failed display updates, it marks the connection as disconnected and returns an error.
func drawDisplay(config CreateScreenConfig) error {
	if device == nil {
		return nil
	}

	// Get current config
	cfg := GetConfig()

	if cfg == nil {
		return fmt.Errorf("no configuration available")
	}

	// Create image with current background
	imageBuffer := InitImageBuffer(width, height)

	img := CreateImageContext(ImageConfig{
		BackgroundImg: "background.gif",
		BgColor:       cfg.BackgroundColor,
	})

	// Always update text settings before drawing
	SetTextColor(cfg.TextColor)
	SetTimeFormat(cfg.TimeFormat)

	// Draw all elements
	DrawSystemTemperatures(config.cputemp, config.gputemp)
	DrawNetworkStats(config.network)
	DrawTime()
	DrawWeather(config.weather)

	copy(imageBuffer, img.Pix)

	// Send to device
	if err := sendImageDataInChunks(imageBuffer); err != nil {
		connected = false
		return fmt.Errorf("failed to update display: %v", err)
	}

	return nil
}

func sendImageDataInChunks(imageData []byte) error {
	if !connected {
		fmt.Println("iCUE Nexus: not connected.")
		return nil
	}

	if len(imageData) != width*height*4 {
		return fmt.Errorf("incoming image data length mismatch")
	}

	// Get output endpoint from USB interface
	// libusb: endpoint 2 is not an OUT endpoint
	ep, err := usbintf.OutEndpoint(2)

	if err != nil {
		return fmt.Errorf("OutEndpoint(2): %v", err)
	}

	data := make([]byte, 1024*4) // 1024*4 byte buffer size
	data[0] = 2
	data[1] = 5
	data[2] = 31
	data[3] = 0
	data[4] = 0
	data[5] = 0
	data[6] = 248
	data[7] = 3

	writer := bufio.NewWriterSize(ep, 1024*4)

	// Split the image data into 120 chunks and send them sequentially
	for i := 0; i <= 120; i++ {
		data[4] = byte(i)
		if i != 120 {
			data[3] = 0
			data[6] = 248
		} else {
			data[3] = 1
			data[6] = 192
		}

		num2 := i * 254

		// Iterate through the image data and set the pixel values
		for num := 0; num < 255 && num2 < 30720; num++ {
			data[8+num*4] = imageData[num2*4+2]   // B
			data[8+num*4+1] = imageData[num2*4+1] // G
			data[8+num*4+2] = imageData[num2*4]   // R
			data[8+num*4+3] = 255                 // A
			num2++
		}

		// Write the data to the USB device using buffered writer
		_, err = writer.Write(data)

		// Check for errors during data transfer
		if err != nil {
			connected = false
			if err.Error() == "libusb: device was disconnected" {
				return nil // Device disconnection is expected, don't report as error
			}
			return fmt.Errorf("failed to write data: %v", err)
		}
	}

	// Flush the buffered writer to ensure all data is sent
	if err := writer.Flush(); err != nil {
		return fmt.Errorf("failed to flush data: %v", err)
	}

	return nil
}
