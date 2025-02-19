package main

import (
	"bufio"
	"fmt"
	"log"
	"sync"
	"time"
)

type CreateScreenConfig struct {
	cputemp float64
	gputemp float64
	network NetworkStats
	weather *WeatherInfo
}

var deviceMutex sync.Mutex

func UpdateScreen(tempChan <-chan Temperature, networkChan <-chan NetworkStats, weatherChan <-chan *WeatherInfo) {
	go func() {
		state := struct {
			cpu     float64
			gpu     float64
			network NetworkStats
			weather *WeatherInfo
		}{}

		refreshRate := time.NewTicker(time.Second / screenRefreshRate) // 24 Hz (~0.042s)

		defer refreshRate.Stop()

		for {
			select {
			case temps := <-tempChan:
				state.cpu, state.gpu = temps.CPU, temps.GPU
			case network := <-networkChan:
				state.network = network
			case weather := <-weatherChan:
				state.weather = weather
			case <-refreshRate.C:
				if err := updateDeviceScreen(&state); err != nil {
					log.Printf("Screen update failed: %v", err)
					resetDevice()
				}
			}
		}
	}()
}

func updateDeviceScreen(state *struct {
	cpu     float64
	gpu     float64
	network NetworkStats
	weather *WeatherInfo
}) error {
	deviceMutex.Lock()

	if !connected || device == nil {
		deviceMutex.Unlock()
		return nil
	}

	deviceMutex.Unlock()

	config := CreateScreenConfig{
		cputemp: state.cpu,
		gputemp: state.gpu,
		network: state.network,
		weather: state.weather,
	}

	return CreateNexusScreen(config)
}

func resetDevice() {
	deviceMutex.Lock()
	defer deviceMutex.Unlock()

	if device != nil {
		device.Close()
	}

	device = nil
	connected = false
}

func CreateNexusScreen(config CreateScreenConfig) error {
	if device == nil {
		return nil
	}

	// Prepare and draw image
	imageBuffer := InitImageBuffer(width, height)
	img := CreateImageContext(ImageConfig{BackgroundImg: "/home/fictional/Development/nexus-next/src/background.gif", BgColor: "black"})
	SetTextColor("yellow")

	DrawTemperatures(config.cputemp, config.gputemp)
	DrawNetworkStats(config.network)
	DrawTime()
	DrawWeather(config.weather)

	copy(imageBuffer, img.Pix)

	// Update display
	if err := setNexusImage(imageBuffer); err != nil {
		connected = false
		return fmt.Errorf("failed to update display: %v", err)
	}

	return nil
}

func setNexusImage(imageData []byte) error {
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

	data := make([]byte, 1024*4) // Increased buffer size to accommodate header + data
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
