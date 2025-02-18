package main

import (
	"log"
	"sync"
	"time"

	"github.com/google/gousb"
)

const (
	vid               = 0x1b1c            // Vendor ID for Corsair
	pid               = 0x1b8e            // Product ID for iCUE Nexus
	width             = 640               // Width of the Nexus display
	height            = 48                // Height of the Nexus display
	brightness        = 2                 // Brightness level (0-2)
	location          = "Jersey City, NJ" // Location for weather data
	screenRefreshRate = 60                // Screen refresh rate in Hz
)

var (
	device    *gousb.Device // Pointer to the Nexus device
	connected bool          // Flag to indicate if the Nexus device is connected
	deviceMu  sync.Mutex    // Mutex to protect device access
)

type TouchEvent struct {
	X       int
	Y       int
	Pressed bool
}

func main() {
	// Initialize device connection
	InitializeDevice()

	// Start monitoring channels
	tempChan := StartTempatureMonitor(&connected)
	networkChan := StartNetworkMonitor(&connected)
	weatherChan := StartWeatherMonitor(&connected)

	// Start screen update loop
	go UpdateScreen(tempChan, networkChan, weatherChan)

	// // Start touch input handler
	// touchChan := HandleTouchInput(device)

	// // Start touch event handler
	// go HandleTouchEvent(touchChan)

	// Keep main thread running
	select {}
}

func HandleTouchInput(dev *gousb.Device) chan TouchEvent {
	touchChan := make(chan TouchEvent)

	go func() {
		buf := make([]byte, 512)
		for {
			if dev == nil {
				time.Sleep(time.Second)
				continue
			}

			// Get config using the passed device parameter
			deviceMu.Lock()
			cfg, err := dev.Config(1)
			if err != nil {
				if device != nil {
					device.Close()
					device = nil
				}
				connected = false
				deviceMu.Unlock()
				log.Printf("Failed to get config: %v", err)
				time.Sleep(time.Second)
				continue
			}
			deviceMu.Unlock()

			// Get interface
			intf, err := cfg.Interface(0, 0)
			if err != nil {
				cfg.Close()
				log.Printf("Failed to get interface: %v", err)
				time.Sleep(100 * time.Millisecond)
				continue
			}

			// Get endpoint
			ep, err := intf.InEndpoint(1)
			if err != nil {
				intf.Close()
				cfg.Close()
				log.Printf("Failed to get endpoint: %v", err)
				time.Sleep(100 * time.Millisecond)
				continue
			}

			// Read data
			n, err := ep.Read(buf)
			if err != nil {
				intf.Close()
				cfg.Close()

				deviceMu.Lock()
				if device != nil {
					device.Close()
					device = nil
				}
				connected = false
				deviceMu.Unlock()

				log.Printf("Failed to read: %v", err)
				time.Sleep(100 * time.Millisecond)
				continue
			}

			if n > 0 && buf[0] == 1 {
				x := int(buf[1]) | int(buf[2])<<8
				y := int(buf[3]) | int(buf[4])<<8
				touchChan <- TouchEvent{X: x, Y: y, Pressed: true}
			}

			// Clean up resources
			intf.Close()
			cfg.Close()

			time.Sleep(10 * time.Millisecond)
		}
	}()

	return touchChan
}

func HandleTouchEvent(touchChan chan TouchEvent) {
	for event := range touchChan {
		if event.Pressed {
			println("Display touched at coordinates:", event.X, event.Y)
		}
	}
}
