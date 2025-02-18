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
	screenRefreshRate = 60                // Screen refresh rate in Hz
)

var (
	device    *gousb.Device // Pointer to the Nexus device
	connected bool          // Flag to indicate if the Nexus device is connected
)

func main() {
	// Initialize device connection
	InitializeDevice()

	// Start monitoring channels
	tempChan := StartTempatureMonitor(&connected)
	networkChan := StartNetworkMonitor(&connected)
	weatherChan := StartWeatherMonitor(&connected)

	// Start screen update loop
	go UpdateScreen(tempChan, networkChan, weatherChan)

	// Keep main thread running
	select {}
}
