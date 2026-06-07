// Package device provides Nexus device implementation using direct libusb access.
package device

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"sync"
	"time"

	"github.com/mantonx/nexus-open/internal/touch"
)

// NexusDevice implements the Device interface via direct libusb transfers.
type NexusDevice struct {
	logger *slog.Logger
	config ConnectionConfig

	handle       *usbHandle
	mu           sync.RWMutex // guards connected, handle
	writeMu      sync.Mutex   // serialises concurrent frame + brightness writes
	connected    bool
	manufacturer string
	product      string

	touchReader *touch.HIDTouchReader

	reconnecting  bool
	stopReconnect chan struct{}
}

// NewNexusDevice creates a new Nexus device.
func NewNexusDevice(logger *slog.Logger, config ConnectionConfig) *NexusDevice {
	return &NexusDevice{
		logger:        logger,
		config:        config,
		stopReconnect: make(chan struct{}),
	}
}

// Connect establishes a USB connection to the device, retrying up to 3 times.
func (n *NexusDevice) Connect(ctx context.Context) error {
	const maxAttempts = 3
	const retryDelay = 2 * time.Second

	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		if attempt > 1 {
			n.logger.Info("retrying device connect", "attempt", attempt, "max", maxAttempts)
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(retryDelay):
			}
		}
		lastErr = n.connectOnce()
		if lastErr == nil {
			return nil
		}
		if isNotRetryable(lastErr) {
			return lastErr
		}
		n.logger.Warn("device connect attempt failed", "attempt", attempt, "error", lastErr)
	}
	return lastErr
}

func isNotRetryable(err error) bool {
	if err == nil {
		return false
	}
	msg := strings.ToLower(err.Error())
	return strings.Contains(msg, "not found") ||
		strings.Contains(msg, "permission") ||
		strings.Contains(msg, "access denied")
}

func (n *NexusDevice) connectOnce() error {
	n.mu.Lock()
	defer n.mu.Unlock()

	if n.connected && n.handle != nil {
		return nil
	}

	if n.handle != nil {
		n.handle.close()
		n.handle = nil
	}

	h, mfr, prod, err := usbOpen(uint16(n.config.VendorID), uint16(n.config.ProductID))
	if err != nil {
		return classifyOpenError(err)
	}

	n.handle = h
	n.connected = true
	n.manufacturer = mfr
	n.product = prod

	n.clearDisplay()

	n.touchReader = touch.NewHIDTouchReader(h, n.logger)

	n.logger.Info("USB device connected",
		"vendor_id", n.config.VendorID,
		"product_id", n.config.ProductID,
		"manufacturer", mfr,
		"product", prod)

	if !n.reconnecting {
		n.reconnecting = true
		n.stopReconnect = make(chan struct{}) // fresh channel each connect cycle
		go n.monitorConnection()
	}

	return nil
}

// Disconnect closes the USB device connection.
func (n *NexusDevice) Disconnect() error {
	n.mu.Lock()
	defer n.mu.Unlock()

	if n.reconnecting {
		close(n.stopReconnect)
		n.reconnecting = false
	}

	if n.handle != nil {
		n.handle.close()
		n.handle = nil
	}

	n.connected = false
	n.logger.Info("USB device disconnected")
	return nil
}

// IsConnected returns whether the device is currently connected.
func (n *NexusDevice) IsConnected() bool {
	n.mu.RLock()
	defer n.mu.RUnlock()
	return n.connected
}

// Health returns an error if the device is not connected.
func (n *NexusDevice) Health() error {
	n.mu.RLock()
	defer n.mu.RUnlock()
	if !n.connected {
		return ErrDeviceDisconnected
	}
	return nil
}

// SendFrame sends a rendered frame to the device display.
func (n *NexusDevice) SendFrame(ctx context.Context, data []byte) error {
	n.mu.RLock()
	connected, h := n.connected, n.handle
	n.mu.RUnlock()

	if !connected || h == nil {
		return ErrDeviceDisconnected
	}

	if len(data) != FrameSize {
		return fmt.Errorf("%w: expected %d bytes, got %d", ErrInvalidFrame, FrameSize, len(data))
	}

	const chunkSize = 1024
	const headerSize = 8
	const maxPayload = chunkSize - headerSize

	totalChunks := (len(data) + maxPayload - 1) / maxPayload

	n.writeMu.Lock()
	defer n.writeMu.Unlock()

	for chunkNum := 0; chunkNum < totalChunks; chunkNum++ {
		start := chunkNum * maxPayload
		end := start + maxPayload
		if end > len(data) {
			end = len(data)
		}
		payloadLen := end - start

		packet := make([]byte, chunkSize)
		packet[0] = 0x02
		packet[1] = 0x05
		packet[2] = 0x40
		if chunkNum == totalChunks-1 {
			packet[3] = 0x01
		}
		packet[4] = byte(chunkNum & 0xFF)
		packet[5] = byte((chunkNum >> 8) & 0xFF)
		packet[6] = byte(payloadLen & 0xFF)
		packet[7] = byte((payloadLen >> 8) & 0xFF)

		for i := 0; i < payloadLen; i += 4 {
			if start+i+3 < len(data) {
				packet[headerSize+i] = data[start+i+2]   // B
				packet[headerSize+i+1] = data[start+i+1] // G
				packet[headerSize+i+2] = data[start+i]   // R
				packet[headerSize+i+3] = data[start+i+3] // A
			}
		}

		if err := h.writeFrame(packet); err != nil {
			n.logger.Error("USB write failed", "chunk", chunkNum, "error", err)
			n.mu.Lock()
			n.connected = false
			n.mu.Unlock()
			return fmt.Errorf("failed to write chunk %d: %w", chunkNum, err)
		}
	}

	return nil
}

// ReadTouch reads touch events from the device.
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

// SetBrightness sets the display brightness (0–100).
func (n *NexusDevice) SetBrightness(brightness int) error {
	if brightness < 0 || brightness > 100 {
		return fmt.Errorf("brightness must be 0-100, got %d", brightness)
	}

	n.mu.RLock()
	connected, h := n.connected, n.handle
	n.mu.RUnlock()

	if !connected || h == nil {
		return ErrDeviceDisconnected
	}

	n.writeMu.Lock()
	defer n.writeMu.Unlock()

	if err := h.setBrightness(brightness); err != nil {
		return fmt.Errorf("failed to set brightness: %w", err)
	}

	n.logger.Debug("brightness set", "value", brightness)
	return nil
}

// GetFirmwareVersion is not yet implemented via raw libusb.
func (n *NexusDevice) GetFirmwareVersion() (string, error) {
	return "", fmt.Errorf("GetFirmwareVersion not implemented")
}

// DeviceInfo holds static information read from the USB device at connect time.
type DeviceInfo struct {
	Manufacturer string
	Product      string
	VendorID     uint16
	ProductID    uint16
}

// GetDeviceInfo returns the manufacturer, product, and USB IDs read at connect time.
func (n *NexusDevice) GetDeviceInfo() DeviceInfo {
	n.mu.RLock()
	defer n.mu.RUnlock()
	return DeviceInfo{
		Manufacturer: n.manufacturer,
		Product:      n.product,
		VendorID:     uint16(n.config.VendorID),
		ProductID:    uint16(n.config.ProductID),
	}
}

func (n *NexusDevice) clearDisplay() {
	if n.handle == nil {
		return
	}
	const (
		chunkSize  = 1024
		headerSize = 8
		maxPayload = chunkSize - headerSize
	)
	blank := make([]byte, FrameSize)
	totalChunks := (len(blank) + maxPayload - 1) / maxPayload
	for chunkNum := 0; chunkNum < totalChunks; chunkNum++ {
		start := chunkNum * maxPayload
		end := start + maxPayload
		if end > len(blank) {
			end = len(blank)
		}
		payloadLen := end - start
		packet := make([]byte, chunkSize)
		packet[0] = 0x02
		packet[1] = 0x05
		packet[2] = 0x40
		if chunkNum == totalChunks-1 {
			packet[3] = 0x01
		}
		packet[4] = byte(chunkNum & 0xFF)
		packet[5] = byte((chunkNum >> 8) & 0xFF)
		packet[6] = byte(payloadLen & 0xFF)
		packet[7] = byte((payloadLen >> 8) & 0xFF)
		if err := n.handle.writeFrame(packet); err != nil {
			n.logger.Warn("clearDisplay write failed", "chunk", chunkNum, "error", err)
			return
		}
	}
}

func (n *NexusDevice) monitorConnection() {
	const (
		pollInterval = 1 * time.Second
		maxBackoff   = 30 * time.Second
		// Pause between closing a failed handle and attempting to reopen.
		// Gives the device firmware time to reset its USB state before we
		// claim the interface again — prevents the rapid open/close storm
		// that can leave the device in a state requiring a physical replug.
		settleDelay = 2 * time.Second
	)

	backoff := pollInterval

	for {
		select {
		case <-n.stopReconnect:
			return
		case <-time.After(backoff):
		}

		if n.Health() == nil {
			backoff = pollInterval
			continue
		}

		n.logger.Warn("device disconnected, attempting reconnect", "retry_in", backoff)

		// Close the stale handle and let the device settle before reopening.
		n.mu.Lock()
		if n.handle != nil {
			n.handle.close()
			n.handle = nil
		}
		n.mu.Unlock()

		select {
		case <-n.stopReconnect:
			return
		case <-time.After(settleDelay):
		}

		// Single attempt — monitorConnection's backoff handles the retry cadence.
		// Using Connect's internal 3-attempt loop here would cause up to 3 rapid
		// opens per monitorConnection cycle, defeating the settle delay.
		if reconnErr := n.connectOnce(); reconnErr != nil {
			n.logger.Debug("reconnect attempt failed, will retry",
				"error", reconnErr,
				"next_retry_in", min(backoff*2, maxBackoff))
			backoff = min(backoff*2, maxBackoff)
		} else {
			n.logger.Info("device reconnected successfully")
			backoff = pollInterval
		}
	}
}

func min(a, b time.Duration) time.Duration {
	if a < b {
		return a
	}
	return b
}
