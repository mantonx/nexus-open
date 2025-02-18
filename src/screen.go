package main

import (
	"fmt"
	"log"
	"time"

	"github.com/google/gousb"
)

func UpdateScreen(tempChan <-chan Temperature, networkChan <-chan NetworkStats, weatherChan <-chan *WeatherInfo) {
	var (
		currentCPU     float64
		currentGPU     float64
		currentNetwork NetworkStats
		currentWeather *WeatherInfo
	)

	ticker := time.NewTicker(time.Second)

	defer ticker.Stop()

	// Loop to update screen
	// If there is new data available, update the screen
	// Otherwise, keep the screen as is
	// This loop runs every second
	for range ticker.C {
		select {
		case temps := <-tempChan:
			currentCPU, currentGPU = temps.CPU, temps.GPU
		case network := <-networkChan:
			currentNetwork = network
		case weather := <-weatherChan:
			currentWeather = weather
		default:
			if connected {
				// Update screen at ~60Hz
				time.Sleep(time.Second / screenRefreshRate)
				CreateNexusScreen(device, currentCPU, currentGPU, currentNetwork, currentWeather)
			}
		}
	}
}

// CreateNexusScreen updates the Nexus display device with system statistics and weather information
//
// It takes the following parameters:
// - device: Pointer to a USB device interface
// - cputemp: Current CPU temperature in degrees
// - gputemp: Current GPU temperature in degrees
// - currentNetwork: Network statistics including upload/download rates
// - weatherInfo: Pointer to weather information structure
//
// The function creates an image buffer, draws various information components including:
// - CPU and GPU temperatures
// - Network statistics
// - Current time
// - Weather information
//
// If the device is nil, the function returns without doing anything.
// If there are any errors during device communication, it logs the error,
// marks the device as disconnected and closes the connection.
func CreateNexusScreen(device *gousb.Device, cputemp, gputemp float64, currentNetwork NetworkStats, weatherInfo *WeatherInfo) {
	if device == nil {
		return
	}

	// Create and prepare image
	imageBuffer := InitImageBuffer(width, height)
	img := CreateImageContext("teal")
	SetTextColor("yellow")

	// Draw all components
	DrawTemperatures(cputemp, gputemp)
	DrawNetworkStats(currentNetwork)
	DrawTime()
	DrawWeather(weatherInfo)

	// Copy image data to buffer
	copy(imageBuffer, img.Pix)

	// Configure and update device
	if err := device.SetAutoDetach(true); err != nil {
		log.Printf("Warning: SetAutoDetach failed: %v", err)
	}

	if err := setNexusImage(device, imageBuffer); err != nil {
		log.Printf("Error: Failed to update display: %v", err)
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
			done()         // Close the interface
			device.Close() // Close the device
			connected = false
			return fmt.Errorf("failed to write data: %v", err)
		}
	}

	return nil
}
