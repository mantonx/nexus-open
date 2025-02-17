package main

import (
	"log"
	"time"

	"github.com/google/gousb"
)

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

// RetryConnectNexus monitors and maintains connection to an iCUE Nexus device.
// It runs as a goroutine that periodically checks the connection status and attempts
// to reconnect if the connection is lost.
//
// The function implements an exponential backoff strategy for reconnection attempts:
// - Checks connection every 5 seconds
// - On connection loss, attempts up to 10 reconnections with increasing delays
// - Monitors device health by checking interface validity
//
// The reconnection process will continue indefinitely until a successful connection
// is established or the program terminates.
//
// Note: This function is non-blocking as it launches a goroutine to handle the
// connection monitoring in the background.
func RetryConnectNexus() {
	// Start device monitoring the connection
	go func() {
		const (
			reconnectInterval = 5 * time.Second // Reconnect interval (5 seconds)
			maxRetries        = 10              // Maximum number of reconnection attempts (with exponential backoff)
		)

		ticker := time.NewTicker(reconnectInterval)
		defer ticker.Stop()

		reconnect := func() bool {
			for i := 0; i < maxRetries; i++ {
				if newDevice := ConnectNexus(); newDevice != nil {
					if device != nil {
						device.Close() // Ensure old device is properly closed
					}
					device = newDevice
					connected = true
					log.Println("iCUE Nexus: Successfully reconnected")
					return true
				}

				if i < maxRetries-1 {
					backoff := time.Duration(1<<uint(i)) * time.Second
					log.Printf("iCUE Nexus: Reconnection attempt %d failed, waiting %v", i+1, backoff)
					time.Sleep(backoff)
				}
			}
			return false
		}

		for range ticker.C {
			if !connected {
				if !reconnect() {
					log.Println("iCUE Nexus: Failed all reconnection attempts")
				}
				continue
			}

			// Check device health
			if device == nil {
				log.Println("iCUE Nexus: Device handle is not available")
				connected = false
				continue
			}

			// Check if device is still responding
			intf, done, err := device.DefaultInterface()

			if err != nil {
				log.Printf("iCUE Nexus: Connection lost - %v", err)
				connected = false
				device.Close()
				continue
			}

			if intf == nil {
				log.Println("iCUE Nexus: Invalid interface detected")
				connected = false
				device.Close()
				done()
				continue
			}

			done() // Close interface properly
		}
	}()
}
