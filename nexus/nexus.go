package nexus

import (
	"nexus-open/nexus/instruments"

	"github.com/google/gousb"
)

// Device-specific constants
const (
	vid = 0x1b1c // Corsair Vendor ID
	pid = 0x1b8e // iCUE Nexus Product ID
)

// Display settings
const (
	width             = 640 // Display width in pixels
	height            = 48  // Display height in pixels
	brightness        = 2   // Display brightness (0-2)
	screenRefreshRate = 24  // Refresh rate in Hz
)

// Configuration variables
var (
	unit     = "imperial"        // Temperature/wind speed unit
	location = "Jersey City, NJ" // Default location for weather data
)

// Device connection state
var (
	device    *gousb.Device    // Nexus USB device
	usbintf   *gousb.Interface // Nexus USB interface
	connected bool             // Connection status
)

func StartNexus() {
	// Initialize device connection
	InitializeDevice()

	// // Start monitoring channels
	tempChan := instruments.StartTempatureMonitor(&connected)
	networkChan := instruments.StartNetworkMonitor(&connected)
	weatherChan := instruments.StartWeatherMonitor(location, &unit, &connected)

	// Start display update loop
	StartDisplayUpdate(tempChan, networkChan, weatherChan)

	// Start touch input reading
	StartTouchMonitor()

	// Keep main thread running
	select {}
}
