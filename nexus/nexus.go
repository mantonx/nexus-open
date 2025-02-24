package nexus

import (
	"log"
	"nexus-open/nexus/configuration"
	"nexus-open/nexus/instruments"
	"sync"

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
	configRefreshRate = 1   // Configuration refresh rate in seconds
)

// Configuration variables
var (
	unit     = "imperial" // Temperature/wind speed unit (imperial/metric)
	location string       // User's location (city, country
)

// Device connection state
var (
	device    *gousb.Device    // Nexus USB device
	usbintf   *gousb.Interface // Nexus USB interface
	connected bool             // Connection status
)

// Configuration state
var (
	config          *configuration.NexusConfig
	configMu        sync.RWMutex
	updateCh        = make(chan struct{}, 1) // Channel to signal config updates
	weatherUpdateCh chan<- struct{}          // Channel to trigger weather updates
)

func StartNexus() {
	var err error
	// Load initial configuration
	config, err = configuration.LoadConfig("")
	if err != nil {
		log.Printf("Error loading initial config: %v", err)
		return
	}

	// Set initial settings
	SetTimeFormat(config.TimeFormat)
	SetTextColor(config.TextColor)

	// Start configuration watcher
	go WatchConfig()

	// Initialize device connection
	InitializeDevice()

	// Start monitoring channels with proper type declarations
	tempChan := instruments.StartTempatureMonitor(&connected)
	networkChan := instruments.StartNetworkMonitor(&connected)
	weatherChan, weatherTrigger := instruments.StartWeatherMonitor(GetConfig, &connected)

	// Store weather update channel globally
	weatherUpdateCh = weatherTrigger

	// Convert channels to proper types
	tempChanRead := (<-chan instruments.SystemTemperature)(tempChan)
	networkChanRead := (<-chan instruments.NetworkStats)(networkChan)
	weatherChanRead := (<-chan *instruments.WeatherInfo)(weatherChan)

	// Start display update loop with all required channels
	StartDisplayUpdate(
		tempChanRead,
		networkChanRead,
		weatherChanRead,
		updateCh,
		weatherTrigger,
	)

	// Start touch input reading
	StartTouchMonitor()

	// Start API server
	SetupAPI()

	// Keep main thread running
	select {}
}
