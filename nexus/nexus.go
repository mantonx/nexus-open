package nexus

import (
	"log"
	"nexus-open/nexus/configuration"
	"nexus-open/nexus/instruments"
	"sync"
	"time"

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

// Configuration state
var (
	config   *configuration.NexusConfig
	configMu sync.RWMutex
	updateCh = make(chan struct{}, 1) // Channel to signal config updates
)

// GetConfig returns the current configuration thread-safely
func GetConfig() *configuration.NexusConfig {
	configMu.RLock()
	defer configMu.RUnlock()
	return config
}

// watchConfig monitors for configuration changes
func watchConfig() {
	ticker := time.NewTicker(5 * time.Second)
	for range ticker.C {
		newConfig, err := configuration.LoadConfig("")
		if err != nil {
			log.Printf("Error loading config: %v", err)
			continue
		}

		configMu.Lock()
		if newConfig.Unit != config.Unit ||
			newConfig.Location != config.Location ||
			newConfig.TimeFormat != config.TimeFormat ||
			newConfig.TextColor != config.TextColor ||
			newConfig.BackgroundColor != config.BackgroundColor {
			config = newConfig
			// Update package variables
			unit = newConfig.Unit
			location = newConfig.Location
			select {
			case updateCh <- struct{}{}: // Signal update
			default:
			}
		}
		configMu.Unlock()
	}
}

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
	go watchConfig()

	// Initialize device connection
	InitializeDevice()

	// Start monitoring channels with config access
	tempChan := instruments.StartTempatureMonitor(&connected)
	networkChan := instruments.StartNetworkMonitor(&connected)
	weatherChan := instruments.StartWeatherMonitor(GetConfig, &connected)

	// Start display update loop with config update channel
	StartDisplayUpdate(tempChan, networkChan, weatherChan, updateCh)

	// Start touch input reading
	StartTouchMonitor()

	// Keep main thread running
	select {}
}
