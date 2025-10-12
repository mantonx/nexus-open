// Package device provides HID feature report support for the Nexus device via libusb.
// This includes brightness control and firmware queries.
// Uses USB control transfers instead of a separate HID library to avoid device conflicts.
package device

import (
	"errors"
	"fmt"
	"log/slog"

	"github.com/google/gousb"
)

// HIDFeatureDevice provides HID feature report capabilities via libusb control transfers.
// This allows controlling brightness, animations, and querying firmware information
// without needing a separate HID library that would conflict with bulk transfers.
type HIDFeatureDevice struct {
	device *gousb.Device
	logger *slog.Logger
}

// NewHIDFeatureDevice creates a new HID feature device controller.
// The device parameter should be the already-opened gousb device.
func NewHIDFeatureDevice(device *gousb.Device, logger *slog.Logger) *HIDFeatureDevice {
	return &HIDFeatureDevice{
		device: device,
		logger: logger,
	}
}

// sendFeatureReport sends a HID feature report using USB control transfer.
// This is equivalent to SendFeatureReport in HID libraries.
func (h *HIDFeatureDevice) sendFeatureReport(data []byte) error {
	if h.device == nil {
		return ErrDeviceDisconnected
	}

	// HID Set_Report request
	// bmRequestType: 0x21 (Host to Device, Class, Interface)
	// bRequest: 0x09 (SET_REPORT)
	// wValue: 0x0300 | reportID (0x03xx for Feature Report)
	// wIndex: interface number (0)
	// data: report data
	requestType := uint8(0x21)                // Host to Device, Class, Interface
	request := uint8(0x09)                    // SET_REPORT
	value := uint16(0x0300) | uint16(data[0]) // Feature report + report ID
	index := uint16(0)                        // Interface 0

	_, err := h.device.Control(requestType, request, value, index, data)
	if err != nil {
		return fmt.Errorf("failed to send feature report: %w", err)
	}

	return nil
}

// getFeatureReport receives a HID feature report using USB control transfer.
// This is equivalent to GetFeatureReport in HID libraries.
func (h *HIDFeatureDevice) getFeatureReport(reportID byte, length int) ([]byte, error) {
	if h.device == nil {
		return nil, ErrDeviceDisconnected
	}

	// HID Get_Report request
	// bmRequestType: 0xA1 (Device to Host, Class, Interface)
	// bRequest: 0x01 (GET_REPORT)
	// wValue: 0x0300 | reportID (0x03xx for Feature Report)
	// wIndex: interface number (0)
	requestType := uint8(0xA1)                 // Device to Host, Class, Interface
	request := uint8(0x01)                     // GET_REPORT
	value := uint16(0x0300) | uint16(reportID) // Feature report + report ID
	index := uint16(0)                         // Interface 0

	data := make([]byte, length)
	data[0] = reportID

	n, err := h.device.Control(requestType, request, value, index, data)
	if err != nil {
		return nil, fmt.Errorf("failed to get feature report: %w", err)
	}

	return data[:n], nil
}

// SetBrightness sets the display brightness (0-100).
// Uses the brightness mapping discovered from reverse engineering:
// 0-100 maps to PWM values [0, 4, 12, 16, 64]
func (h *HIDFeatureDevice) SetBrightness(brightness int) error {
	if brightness < 0 || brightness > 100 {
		return errors.New("brightness must be between 0 and 100")
	}

	// Map brightness 0-100 to PWM values
	// Based on companion-module-icue-nexus implementation
	brightnessMap := []byte{0, 4, 12, 16, 64}
	level := brightness / 25 // Map to 0-4 range
	if level > 4 {
		level = 4
	}

	// Create feature report (32 bytes)
	// Format from reverse engineering:
	// [3, 1, brightness_value, 1, 120, 0, 192, 3, zeros...]
	data := make([]byte, 32)
	data[0] = 3 // Report ID
	data[1] = 1 // Brightness command
	data[2] = brightnessMap[level]
	data[3] = 1
	data[4] = 120
	data[5] = 0
	data[6] = 192
	data[7] = 3
	// Remaining bytes are zeros

	if err := h.sendFeatureReport(data); err != nil {
		return fmt.Errorf("failed to set brightness: %w", err)
	}

	h.logger.Debug("brightness set",
		"brightness", brightness,
		"pwm_value", brightnessMap[level])

	return nil
}

// GetFirmwareVersion queries the device firmware version.
// Returns the firmware version string from the device.
func (h *HIDFeatureDevice) GetFirmwareVersion() (string, error) {
	// Request feature report 5 (64 bytes)
	// Firmware string starts at byte 6, null-terminated
	data, err := h.getFeatureReport(5, 64)
	if err != nil {
		return "", fmt.Errorf("failed to get firmware version: %w", err)
	}

	if len(data) < 7 {
		return "", errors.New("firmware report too short")
	}

	// Find null terminator starting at byte 6
	end := 6
	for end < len(data) && data[end] != 0 {
		end++
	}

	version := string(data[6:end])
	h.logger.Debug("firmware version retrieved", "version", version)

	return version, nil
}
