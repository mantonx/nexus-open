// Package device provides Nexus device implementation using HID API.
package device

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/mantonx/nexus-next/internal/touch"

	"github.com/karalabe/hid"
)

// NexusDevice implements the Device interface using HID API
// This provides access to touch input, brightness, and animations
type NexusDevice struct {
	logger *slog.Logger
	config ConnectionConfig

	// HID device
	device      *hid.Device
	deviceInfo  *hid.DeviceInfo
	mu          sync.RWMutex
	connected   bool
	touchReader *touch.HIDTouchReader

	// Reconnection
	reconnecting  bool
	stopReconnect chan struct{}
}

// NewNexusDevice creates a new HID-based Nexus device
func NewNexusDevice(logger *slog.Logger, config ConnectionConfig) *NexusDevice {
	return &NexusDevice{
		logger:        logger,
		config:        config,
		stopReconnect: make(chan struct{}),
	}
}

// Connect establishes HID connection to the device
func (n *NexusDevice) Connect(ctx context.Context) error {
	n.mu.Lock()
	defer n.mu.Unlock()

	// Enumerate HID devices to find iCUE Nexus
	devices := hid.Enumerate(n.config.VendorID, n.config.ProductID)
	if len(devices) == 0 {
		return ErrDeviceNotFound
	}

	// Log all interfaces found
	n.logger.Info("HID interfaces found", "count", len(devices))
	for i, dev := range devices {
		n.logger.Info("HID interface",
			"index", i,
			"path", dev.Path,
			"interface", dev.Interface,
			"usage_page", fmt.Sprintf("0x%04x", dev.UsagePage),
			"usage", fmt.Sprintf("0x%04x", dev.Usage))
	}

	// Try to open each interface until one succeeds
	// This handles permission issues where some interfaces may not be accessible
	var device *hid.Device
	var lastErr error

	for i := range devices {
		n.logger.Debug("attempting to open HID interface", "index", i, "path", devices[i].Path)

		dev, err := devices[i].Open()
		if err != nil {
			n.logger.Debug("failed to open interface", "index", i, "error", err)
			lastErr = err
			continue
		}

		// Successfully opened
		device = dev
		n.deviceInfo = &devices[i]
		n.logger.Info("successfully opened HID interface",
			"index", i,
			"path", devices[i].Path,
			"interface", devices[i].Interface)
		break
	}

	if device == nil {
		if lastErr != nil {
			return fmt.Errorf("failed to open any HID interface (tried %d): %w", len(devices), lastErr)
		}
		return fmt.Errorf("failed to open any HID interface (tried %d)", len(devices))
	}

	n.device = device
	n.connected = true

	// Initialize touch reader
	n.touchReader = touch.NewHIDTouchReader(device, n.logger)

	n.logger.Info("HID device connected",
		"vendor_id", n.config.VendorID,
		"product_id", n.config.ProductID,
		"manufacturer", n.deviceInfo.Manufacturer,
		"product", n.deviceInfo.Product)

	// Start reconnection monitoring
	if !n.reconnecting {
		n.reconnecting = true
		go n.monitorConnection()
	}

	return nil
}

// Disconnect closes the HID device connection
func (n *NexusDevice) Disconnect() error {
	n.mu.Lock()
	defer n.mu.Unlock()

	// Stop reconnection monitoring
	if n.reconnecting {
		close(n.stopReconnect)
		n.reconnecting = false
	}

	if n.device != nil {
		if err := n.device.Close(); err != nil {
			n.logger.Warn("error closing HID device", "error", err)
		}
		n.device = nil
	}

	n.connected = false
	n.logger.Info("HID device disconnected")

	return nil
}

// IsConnected returns whether the device is connected
func (n *NexusDevice) IsConnected() bool {
	n.mu.RLock()
	defer n.mu.RUnlock()
	return n.connected
}

// SendFrame sends a frame to the device using HID writes
func (n *NexusDevice) SendFrame(ctx context.Context, data []byte) error {
	n.mu.RLock()
	if !n.connected || n.device == nil {
		n.mu.RUnlock()
		return ErrDeviceDisconnected
	}
	device := n.device
	n.mu.RUnlock()

	// Validate frame size (640x48x4 = 122,880 bytes RGBA)
	if len(data) != FrameSize {
		return fmt.Errorf("%w: expected %d bytes, got %d", ErrInvalidFrame, FrameSize, len(data))
	}

	// Send frame in chunks using HID protocol
	// Using NexusTool's protocol: 0x40 with variable payload length
	const chunkSize = 1024
	const headerSize = 8
	const maxPayload = chunkSize - headerSize

	totalChunks := (len(data) + maxPayload - 1) / maxPayload

	for chunkNum := 0; chunkNum < totalChunks; chunkNum++ {
		start := chunkNum * maxPayload
		end := start + maxPayload
		if end > len(data) {
			end = len(data)
		}

		payloadLen := end - start
		isLast := chunkNum == totalChunks-1

		// Build chunk packet
		packet := make([]byte, chunkSize)
		packet[0] = 0x02 // Endpoint 2
		packet[1] = 0x05 // Command: Send Image
		packet[2] = 0x40 // Protocol variant (NexusTool uses this)
		if isLast {
			packet[3] = 0x01 // Last chunk flag
		} else {
			packet[3] = 0x00
		}
		packet[4] = byte(chunkNum & 0xFF)        // Chunk number low
		packet[5] = byte((chunkNum >> 8) & 0xFF) // Chunk number high
		packet[6] = byte(payloadLen & 0xFF)      // Payload length low
		packet[7] = byte((payloadLen >> 8) & 0xFF) // Payload length high

		// Copy payload (convert RGBA to BGRA for device)
		for i := 0; i < payloadLen; i += 4 {
			if start+i+3 < len(data) {
				// Swap R and B for BGR format
				packet[headerSize+i] = data[start+i+2]     // B
				packet[headerSize+i+1] = data[start+i+1]   // G
				packet[headerSize+i+2] = data[start+i]     // R
				packet[headerSize+i+3] = data[start+i+3]   // A
			}
		}

		// Write via HID
		_, err := device.Write(packet)
		if err != nil {
			n.logger.Error("HID write failed", "chunk", chunkNum, "error", err)
			return fmt.Errorf("failed to write chunk %d: %w", chunkNum, err)
		}
	}

	return nil
}

// ReadTouch reads touch events from the HID device
func (n *NexusDevice) ReadTouch(ctx context.Context) ([]touch.Event, error) {
	n.mu.RLock()
	if !n.connected || n.touchReader == nil {
		n.mu.RUnlock()
		return []touch.Event{}, ErrDeviceDisconnected
	}
	reader := n.touchReader
	n.mu.RUnlock()

	return reader.Read(ctx)
}

// Health checks the device connection health
func (n *NexusDevice) Health() error {
	n.mu.RLock()
	defer n.mu.RUnlock()

	if !n.connected {
		return ErrDeviceDisconnected
	}

	return nil
}

// SetBrightness sets the display brightness (0-100)
func (n *NexusDevice) SetBrightness(brightness int) error {
	n.mu.RLock()
	if !n.connected || n.device == nil {
		n.mu.RUnlock()
		return ErrDeviceDisconnected
	}
	device := n.device
	n.mu.RUnlock()

	if brightness < 0 || brightness > 100 {
		return fmt.Errorf("brightness must be 0-100, got %d", brightness)
	}

	// Use simple NexusTool protocol: [3, 1, brightness]
	report := []byte{3, 1, byte(brightness)}

	_, err := device.Write(report) // HID uses Write for feature reports too
	if err != nil {
		return fmt.Errorf("failed to set brightness: %w", err)
	}

	n.logger.Debug("brightness set", "value", brightness)
	return nil
}

// GetFirmwareVersion queries the device firmware version
func (n *NexusDevice) GetFirmwareVersion() (string, error) {
	n.mu.RLock()
	if !n.connected || n.device == nil {
		n.mu.RUnlock()
		return "", ErrDeviceDisconnected
	}
	device := n.device
	n.mu.RUnlock()

	// Read Feature Report 5 (64 bytes)
	report := make([]byte, 64)
	report[0] = 5 // Report ID

	bytesRead, err := device.Read(report)
	if err != nil {
		return "", fmt.Errorf("failed to read feature report: %w", err)
	}

	// Firmware string starts at byte 6, null-terminated
	if bytesRead < 7 {
		return "", fmt.Errorf("feature report too short: %d bytes", bytesRead)
	}

	// Find null terminator
	end := 6
	for end < bytesRead && report[end] != 0 {
		end++
	}

	firmware := string(report[6:end])
	n.logger.Debug("firmware version", "version", firmware)

	return firmware, nil
}

// monitorConnection monitors for device disconnection and attempts reconnection
func (n *NexusDevice) monitorConnection() {
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-n.stopReconnect:
			return
		case <-ticker.C:
			if err := n.Health(); err != nil {
				n.logger.Warn("device health check failed, attempting reconnect", "error", err)

				// Attempt reconnection
				ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
				if err := n.Connect(ctx); err != nil {
					n.logger.Error("reconnection failed", "error", err)
				} else {
					n.logger.Info("device reconnected successfully")
				}
				cancel()
			}
		}
	}
}
