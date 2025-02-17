package main

import (
	"fmt"
	"log"
	"time"

	"github.com/google/gousb"
)

const (
	vid        = 0x1b1c            // Vendor ID for Corsair
	pid        = 0x1b8e            // Product ID for iCUE Nexus
	width      = 640               // Width of the Nexus display
	height     = 48                // Height of the Nexus display
	brightness = 2                 // Brightness level (0-2)
	location   = "Jersey City, NJ" // Location for weather data
)

var (
	device    *gousb.Device // Pointer to the Nexus device
	connected bool          // Flag to indicate if the Nexus device is connected
)

func main() {
	// Initial connection attempt
	device = ConnectNexus()

	if device != nil {
		connected = true
		fmt.Println("iCUE Nexus: Connected")
	}

	// Retry connecting to the Nexus device every 5 seconds
	RetryConnectNexus()

	// Monitor CPU and GPU temperatures
	tempChan := StartTempatureMonitor(&connected)

	// Monitor network statistics
	networkChan := StartNetworkMonitor(&connected)

	// Monitor weather information
	weatherChan := StartWeatherMonitor(&connected)

	// Start screen update goroutine
	// Pause the main thread and update the screen with the latest data
	// if the device is disconnected, the screen will not be updated
	go func() {
		var currentCPU, currentGPU float64
		var currentNetwork NetworkStats
		var currentWeather *WeatherInfo

		for {
			select {
			case temps := <-tempChan:
				currentCPU = temps.CPU
				currentGPU = temps.GPU
			case network := <-networkChan:
				currentNetwork = network
			case weather := <-weatherChan:
				currentWeather = weather
			default:
				if connected {
					createNexusScreen(device, currentCPU, currentGPU, currentNetwork, currentWeather)
				}
			}
			time.Sleep(1 * time.Second)
		}
	}()

	// Keep main thread running
	select {}
}

// createNexusScreen creates and displays an information screen on a Corsair Nexus device.
// It shows CPU temperature, GPU temperature, current time, and weather information.
//
// The screen layout is as follows:
// - Top left: CPU temperature in Celsius
// - Bottom left: GPU temperature in Celsius
// - Top right: Current time (blinking colon)
// - Bottom right: Weather information (temperature in Fahrenheit and condition)
//
// All text is displayed in red color on a black background.
//
// Parameters:
//   - device: Pointer to a gousb.Device representing the Nexus display device
//   - weatherInfo: Pointer to WeatherInfo struct containing current weather data
//
// The function will fatal log if it encounters errors while:
// - Getting CPU temperature
// - Getting GPU temperature
// - Setting the image on the Nexus device
func createNexusScreen(device *gousb.Device, cputemp float64, gputemp float64, currentNetwork NetworkStats, weatherInfo *WeatherInfo) {
	// Create a buffer to hold the image data
	imageBuffer := InitImageBuffer(width, height)

	// Create black background
	img := CreateImageContext("teal")

	// Set text color to blue
	SetTextColor("yellow")

	// Draw CPU and GPU temperatures
	DrawTemperatures(cputemp, gputemp)

	// Draw network statistics
	DrawNetworkStats(currentNetwork)

	// Draw current time
	DrawTime()

	// Draw weather information
	DrawWeather(weatherInfo)

	// Copy image data to buffer
	copy(imageBuffer, img.Pix)

	// Set auto detach for the device
	// This allows the device to be claimed by the kernel driver when the program exits
	if err := device.SetAutoDetach(true); err != nil {
		log.Printf("Warning: failed to set auto detach: %v", err)
	}

	// Set the image on the Corsair Nexus device
	// This function sends the image data to the device for displayS
	if err := setNexusImage(device, imageBuffer); err != nil {
		log.Printf("Failed to set Nexus image: %v", err)
		connected = false
		device.Close()
	}
}

// setNexusImage writes image data to a Corsair iCUE Nexus device.
// It takes a USB device pointer and raw RGBA image data as input.
// The image data must match the device's width*height*4 bytes resolution.
// The function splits the image into 120 chunks and sends them sequentially
// through USB bulk transfer with specific header information.
//
// Parameters:
//   - device: Pointer to a gousb.Device representing the Corsair Nexus USB device
//   - imageData: Byte slice containing raw RGBA image data (width*height*4 bytes)
//
// Returns:
//   - error: nil if successful, error object with description if failed
//
// The function will return early with nil if device is not connected.
// Errors can occur during interface acquisition, endpoint setup, or data transfer.
func setNexusImage(device *gousb.Device, imageData []byte) error {
	if !connected {
		fmt.Println("iCUE Nexus: not connected.")
		return nil
	}

	if len(imageData) != width*height*4 {
		return fmt.Errorf("incoming image data length mismatch")
	}

	// Get device interface and endpoint
	intf, done, err := device.DefaultInterface()

	if err != nil {
		return fmt.Errorf("DefaultInterface(): %v", err)
	}

	defer done()

	ep, err := intf.OutEndpoint(2)

	if err != nil {
		return fmt.Errorf("OutEndpoint(2): %v", err)
	}

	data := make([]byte, 1024*4) // Increased buffer size to accommodate header + data
	data[0] = 2
	data[1] = 5
	data[2] = 31
	data[3] = 0
	data[4] = 0
	data[5] = 0
	data[6] = 248
	data[7] = 3

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

		// Write the data to the USB device
		_, err = ep.Write(data)

		if err != nil {
			return fmt.Errorf("failed to write data: %v", err)
		}
	}

	return nil
}
