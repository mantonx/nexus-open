// Package main provides functionality for monitoring and processing touch events from a USB device.
//
// The package implements a touch monitor system that:
// - Continuously reads touch input data from a USB device
// - Parses raw touch data into structured touch events
// - Handles device disconnection and reconnection gracefully
// - Provides touch event information through a channel-based interface
//
// The touch event monitoring system operates asynchronously using goroutines and channels,
// allowing for non-blocking touch event processing. It includes automatic retry mechanisms
// for handling device disconnections and reconnections.
//
// TouchEvent represents a single touch interaction with coordinates (X,Y) and press state.
// The system filters duplicate events to prevent event flooding and provides detailed error
// handling for various USB device states.
//
// Example usage:
//
//	eventChan := StartTouchMonitor()
//	for event := range eventChan {
//	    // Process touch events
//	    fmt.Printf("Touch at (%d,%d), pressed: %v\n", event.X, event.Y, event.Pressed)
//	}
//
// The package relies on the github.com/google/gousb library for USB device communication.
package main

import (
	"fmt"
	"time"

	"github.com/google/gousb"
)

func StartTouchMonitor() <-chan TouchEvent {
	events := make(chan TouchEvent)

	go func() {
		for {
			if err := readTouchInput(device); err != nil {
				connected = false
				time.Sleep(time.Second) // Wait before retrying
				if !connected {
					continue
				}
			}
		}
	}()

	return events
}

// readTouchInput handles USB touch input events from the specified USB device.
// It opens the device's input endpoint and processes incoming touch events.
// The function takes ownership of device lifecycle and ensures proper cleanup.
//
// Parameters:
//   - device: Pointer to an initialized gousb.Device to read touch input from
//
// Returns:
//   - error: Returns nil on successful processing, or an error if:
//   - The device is not initialized
//   - Failed to get input endpoint
//   - Error occurred during touch event processing
func readTouchInput(device *gousb.Device) error {
	if device == nil {
		return fmt.Errorf("device not initialized")
	}

	defer usbintf.Close() // Close USB interface on function exit

	// Get input endpoint
	in, err := usbintf.InEndpoint(1) // Input endpoint is 1

	if err != nil {
		return fmt.Errorf("failed to get input endpoint: %v", err)
	}

	return processTouchEvents(in)
}

// processTouchEvents continuously reads touch data from a USB endpoint and processes it into touch events.
// It reads raw touch data in bytes, parses it into TouchEvent structs, and prints changes in touch state.
// The function filters duplicate events by comparing with the last processed event.
// If the device is disconnected, it sets the global connected flag to false and returns an error.
//
// Parameters:
//   - in: Pointer to a gousb.InEndpoint for reading USB touch data
//
// Returns:
//   - error: Returns an error if the device is disconnected or if other USB read errors occur
//
// The function runs in an infinite loop until an error occurs or the device is disconnected.
func processTouchEvents(in *gousb.InEndpoint) error {
	touchData := make([]byte, 1024)
	var lastEvent *TouchEvent

	for {
		_, err := in.Read(touchData)
		if err != nil {
			if err.Error() == "libusb: no device [code -4]" {
				connected = false
				return fmt.Errorf("device disconnected")
			}
			time.Sleep(100 * time.Millisecond)
			continue
		}

		if evt := parseTouchEvent(touchData); evt != nil {
			if lastEvent == nil || *evt != *lastEvent {
				fmt.Printf("Touch event: x=%d, y=%d, pressed=%v\n", evt.X, evt.Y, evt.Pressed)
				lastEvent = evt
			}
		}
	}
}

// parseTouchEvent processes raw touch input data and converts it to a TouchEvent structure.
// The function expects a byte slice containing touch event data in a specific format:
// - First 3 bytes must be [1,2,33] as a header signature
// - Bytes 5-6: X coordinate (high byte, low byte)
// - Bytes 7-8: Y coordinate (high byte, low byte)
// - Byte 2: Press state (33 indicates pressed)
//
// Returns nil if the data format is invalid (incorrect header signature).
// Returns a populated TouchEvent struct if parsing succeeds.
func parseTouchEvent(data []byte) *TouchEvent {
	if data[0] != 1 || data[1] != 2 || data[2] != 33 {
		return nil
	}

	return &TouchEvent{
		X:       int(data[5])*256 + int(data[6]),
		Y:       int(data[7])*256 + int(data[8]),
		Pressed: data[2] == 33, // 33 indicates pressed
	}
}
