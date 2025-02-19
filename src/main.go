package main

import (
	"github.com/google/gousb"
)

const (
	vid               = 0x1b1c            // Vendor ID for Corsair
	pid               = 0x1b8e            // Product ID for iCUE Nexus
	width             = 640               // Width of the Nexus display
	height            = 48                // Height of the Nexus display
	brightness        = 2                 // Brightness level (0-2)
	location          = "Jersey City, NJ" // Location for weather data
	screenRefreshRate = 24                // Screen refresh rate in Hz
)

var (
	device    *gousb.Device    // USB device for the iCUE Nexus
	usbintf   *gousb.Interface // USB interface for the iCUE Nexus
	connected bool             // Flag to indicate if the Nexus device is connected
)

type TouchEvent struct {
	X       int
	Y       int
	Pressed bool
}

func main() {
	// Initialize device connection
	InitializeDevice()

	// // Start monitoring channels
	tempChan := StartTempatureMonitor()
	networkChan := StartNetworkMonitor()
	weatherChan := StartWeatherMonitor()

	// // Start screen update loop
	StartDisplayUpdate(tempChan, networkChan, weatherChan)

	// Start touch input reading
	StartTouchMonitor()

	// Keep main thread running
	select {}
}
