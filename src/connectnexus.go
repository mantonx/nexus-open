package main

import (
	"log"
	"time"

	"github.com/google/gousb"
)

func InitializeDevice() {
	device = ConnectNexus()
	if device != nil {
		connected = true
		log.Println("iCUE Nexus: Connected")
	}
	RetryConnectNexus()
}

// ConnectNexus initializes a USB connection to the iCUE Nexus device.
// It creates a new USB context, searches for devices matching the specified vendor and product IDs,
// and establishes a connection with the first matching device found.
//
// The function performs the following steps:
// 1. Creates a new USB context
// 2. Searches for devices matching VID/PID
// 3. Sets auto detach for kernel driver
// 4. Configures the device
//
// Returns:
//   - *gousb.Device: A pointer to the connected USB device
//
// The function will log.Fatal in the following cases:
//   - No matching devices found
//   - Failed to open devices
//   - Failed to set auto detach
//   - Failed to get device configuration
var usbContext *gousb.Context

func ConnectNexus() *gousb.Device {
	if usbContext == nil {
		usbContext = gousb.NewContext()
	}

	devices, err := usbContext.OpenDevices(func(desc *gousb.DeviceDesc) bool {
		return desc.Vendor == gousb.ID(vid) && desc.Product == gousb.ID(pid)
	})

	if err != nil {
		log.Fatalf("Failed to open devices: %v", err)
	}

	if len(devices) == 0 {
		return nil
	}

	device = devices[0]

	if err := device.SetAutoDetach(true); err != nil {
		log.Fatalf("Failed to set auto detach: %v", err)
	}

	if _, err := device.Config(1); err != nil {
		log.Fatalf("Failed to get config: %v", err)
	}

	// Store config in a package-level variable or let it be garbage collected
	// when the device is closed
	return device
}

// RetryConnectNexus initiates a concurrent monitoring of the Nexus connection.
// It launches the monitorConnection function as a goroutine, which handles
// connection retries and maintenance in the background.
func RetryConnectNexus() {
	go monitorConnection()
}

// monitorConnection continuously monitors the connection status and device health.
// It attempts to reconnect if the connection is lost, with a fixed interval of 5 seconds
// between attempts and a maximum of 10 retries. It also performs periodic health checks
// on the connected device, closing the connection if the device becomes unhealthy.
// The function runs indefinitely until the program terminates.
func monitorConnection() {
	const (
		reconnectInterval = 5 * time.Second
		maxRetries        = 10
	)

	ticker := time.NewTicker(reconnectInterval)
	defer ticker.Stop()

	for range ticker.C {
		if !connected {
			attemptReconnection(maxRetries)
			continue
		}

		if !checkDeviceHealth() {
			connected = false
			if device != nil {
				device.Close()
			}
		}
	}
}

// attemptReconnection tries to re-establish connection with the Nexus device using exponential backoff.
// It attempts to connect up to maxRetries times. On successful connection, it closes any existing
// device connection before establishing the new one. Between retry attempts, it waits with exponential
// backoff starting at 1 second and doubling each time.
//
// Parameters:
//   - maxRetries: maximum number of reconnection attempts before giving up
func attemptReconnection(maxRetries int) {
	for i := 0; i < maxRetries; i++ {
		if newDevice := ConnectNexus(); newDevice != nil {
			if device != nil {
				device.Close()
			}
			device = newDevice
			connected = true
			log.Println("iCUE Nexus: Successfully reconnected")
			return
		}

		if i < maxRetries-1 {
			backoff := time.Duration(1<<uint(i)) * time.Second
			log.Printf("iCUE Nexus: Reconnection attempt %d failed, waiting %v", i+1, backoff)
			time.Sleep(backoff)
		}
	}
	log.Println("iCUE Nexus: Failed all reconnection attempts")
}

// checkDeviceHealth verifies the health status of the iCUE Nexus device connection.
// It checks if:
// - Device handle exists and is valid
// - Default interface is accessible
// - Interface is properly initialized
//
// Returns:
//   - true if device is healthy and ready for communication
//   - false if any health check fails, logging the specific failure reason
func checkDeviceHealth() bool {
	if device == nil {
		log.Println("iCUE Nexus: Device handle is not available")
		return false
	}

	intf, done, err := device.DefaultInterface()
	defer done()

	if err != nil {
		log.Printf("iCUE Nexus: Connection lost - %v", err)
		return false
	}

	if intf == nil {
		log.Println("iCUE Nexus: Invalid interface detected")
		return false
	}

	return true
}
