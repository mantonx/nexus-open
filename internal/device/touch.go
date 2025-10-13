// Package device provides touch input handling for the Nexus device.
package device

import (
	"context"
	"fmt"
	"log/slog"
	"math"
	"time"

	"github.com/google/gousb"
)

// TouchGesture represents different types of touch gestures.
type TouchGesture int

const (
	GestureNone TouchGesture = iota
	GestureTap
	GestureSwipeLeft
	GestureSwipeRight
)

// TouchData represents raw touch input data.
type TouchData struct {
	X       int
	Y       int
	Pressed bool
	Time    time.Time
}

// TouchReader handles reading and processing touch input from the device.
type TouchReader struct {
	endpoint  *gousb.InEndpoint
	logger    *slog.Logger
	lastData  *TouchData
	startData *TouchData // Track where touch started
}

// NewTouchReader creates a new touch input reader.
func NewTouchReader(endpoint *gousb.InEndpoint, logger *slog.Logger) *TouchReader {
	return &TouchReader{
		endpoint: endpoint,
		logger:   logger,
	}
}

// Read reads touch input from the device and returns touch events.
// This is a non-blocking read with a short timeout.
func (t *TouchReader) Read(ctx context.Context) ([]TouchEvent, error) {
	if t.endpoint == nil {
		return nil, fmt.Errorf("touch endpoint not initialized")
	}

	// Read with short timeout for non-blocking behavior
	buffer := make([]byte, 64)
	n, err := t.endpoint.ReadContext(ctx, buffer)

	if err != nil {
		// Timeout is expected for non-blocking reads
		if err.Error() == "libusb: timeout [code -7]" || err.Error() == "context deadline exceeded" {
			return []TouchEvent{}, nil
		}
		// Device disconnected
		if err.Error() == "libusb: no device [code -4]" {
			return nil, ErrDeviceDisconnected
		}
		return nil, fmt.Errorf("failed to read touch data: %w", err)
	}

	if n < 8 {
		return []TouchEvent{}, nil // Not enough data
	}

	// Log what we're actually receiving
	if buffer[0] != 0 || buffer[1] != 0 || buffer[2] != 0 {
		t.logger.Info("RAW USB DATA",
			"bytes", n,
			"header", fmt.Sprintf("%02x %02x %02x", buffer[0], buffer[1], buffer[2]),
			"full", fmt.Sprintf("%02x %02x %02x %02x %02x %02x %02x %02x",
				buffer[0], buffer[1], buffer[2], buffer[3],
				buffer[4], buffer[5], buffer[6], buffer[7]))
	}

	// Validate protocol header: [0x01, 0x02, 0x21]
	if buffer[0] != 0x01 || buffer[1] != 0x02 || buffer[2] != 0x21 {
		// Not a touch packet, silent skip
		return []TouchEvent{}, nil
	}

	// Parse touch data
	touchState := buffer[5]
	x := int(buffer[6]) | (int(buffer[7]) << 8) // Little-endian
	pressed := touchState != 0

	// ALWAYS log when we get valid touch packets (for debugging)
	t.logger.Info("TOUCH PACKET",
		"pressed", pressed,
		"touchState", touchState,
		"x", x,
		"raw", fmt.Sprintf("%02x %02x %02x %02x %02x %02x %02x %02x",
			buffer[0], buffer[1], buffer[2], buffer[3],
			buffer[4], buffer[5], buffer[6], buffer[7]))

	currentData := &TouchData{
		X:       x,
		Y:       0, // Y coordinate not used in this device
		Pressed: pressed,
		Time:    time.Now(),
	}

	// Detect gestures
	events := []TouchEvent{}

	if t.lastData != nil {
		// Track touch start
		if currentData.Pressed && !t.lastData.Pressed {
			// Touch just started
			t.startData = currentData
			t.logger.Debug("touch started", "x", x)
		}

		// Detect gesture on release
		if !currentData.Pressed && t.lastData.Pressed && t.startData != nil {
			// Touch just released - check for gesture
			gesture := t.detectGesture(t.startData, currentData)
			if gesture != GestureNone {
				var button int
				switch gesture {
				case GestureTap:
					button = 0
				case GestureSwipeLeft:
					button = 1
				case GestureSwipeRight:
					button = 2
				}

				events = append(events, TouchEvent{
					Button:   button,
					Pressed:  true,
					Duration: currentData.Time.Sub(t.startData.Time),
				})

				t.logger.Debug("touch gesture detected",
					"gesture", gesture,
					"start_x", t.startData.X,
					"end_x", x,
					"dx", x-t.startData.X,
					"duration_ms", currentData.Time.Sub(t.startData.Time).Milliseconds())
			}
			t.startData = nil
		}
	}

	// Store current data for next comparison
	t.lastData = currentData

	return events, nil
}

// detectGesture detects touch gestures based on touch data.
func (t *TouchReader) detectGesture(last, current *TouchData) TouchGesture {
	// Only detect gestures on release
	if current.Pressed {
		return GestureNone
	}

	// Must have been pressed before
	if !last.Pressed {
		return GestureNone
	}

	dx := current.X - last.X
	duration := current.Time.Sub(last.Time)

	// Tap detection: minimal movement, quick duration
	if math.Abs(float64(dx)) < 50 && duration < 500*time.Millisecond {
		return GestureTap
	}

	// Swipe detection: significant horizontal movement
	if duration < 500*time.Millisecond {
		if dx < -100 { // Swipe left threshold
			return GestureSwipeLeft
		}
		if dx > 100 { // Swipe right threshold
			return GestureSwipeRight
		}
	}

	return GestureNone
}
