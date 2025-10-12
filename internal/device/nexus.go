package device

import (
	"bufio"
	"context"
	"errors"
	"log/slog"
	"sync"
	"time"

	"github.com/google/gousb"
)

// NexusDevice implements Device interface for Corsair iCUE Nexus.
type NexusDevice struct {
	mu     sync.RWMutex
	logger *slog.Logger

	config ConnectionConfig

	// USB resources
	ctx       *gousb.Context
	device    *gousb.Device
	intf      *gousb.Interface
	connected bool

	// HID feature support
	hidFeatures *HIDFeatureDevice

	// Touch input support
	touchReader *TouchReader

	// Reconnection state
	reconnecting bool
	stopReconnect chan struct{}
}

// NewNexusDevice creates a new Nexus device instance.
func NewNexusDevice(logger *slog.Logger, config ConnectionConfig) *NexusDevice {
	if logger == nil {
		logger = slog.Default()
	}

	// Set defaults if not provided
	if config.VendorID == 0 {
		config.VendorID = 0x1b1c // Corsair
	}
	if config.ProductID == 0 {
		config.ProductID = 0x1b8e // iCUE Nexus
	}
	if config.ReconnectRetries == 0 {
		config.ReconnectRetries = 10
	}
	if config.ReconnectDelay == 0 {
		config.ReconnectDelay = 5 * time.Second
	}

	return &NexusDevice{
		logger:        logger,
		config:        config,
		stopReconnect: make(chan struct{}),
	}
}

// Connect establishes a connection to the Nexus device.
func (n *NexusDevice) Connect(ctx context.Context) error {
	n.mu.Lock()
	defer n.mu.Unlock()

	if n.connected {
		return nil // Already connected
	}

	// Initialize USB context if needed
	if n.ctx == nil {
		n.ctx = gousb.NewContext()
	}

	// Find and open device
	devices, err := n.ctx.OpenDevices(func(desc *gousb.DeviceDesc) bool {
		return desc.Vendor == gousb.ID(n.config.VendorID) &&
			desc.Product == gousb.ID(n.config.ProductID)
	})
	if err != nil {
		return NewDeviceError("open_devices", err)
	}

	if len(devices) == 0 {
		return NewDeviceError("find_device", ErrDeviceNotFound)
	}

	// Use first matching device
	n.device = devices[0]

	// Close any extra devices
	for i := 1; i < len(devices); i++ {
		devices[i].Close()
	}

	// Set auto detach kernel driver
	if err := n.device.SetAutoDetach(true); err != nil {
		n.device.Close()
		n.device = nil
		return NewDeviceError("auto_detach", err)
	}

	// Get device configuration
	config, err := n.device.Config(1)
	if err != nil {
		n.device.Close()
		n.device = nil
		return NewDeviceError("get_config", ErrConfigFailed)
	}

	// Claim interface
	intf, err := config.Interface(0, 0)
	if err != nil {
		n.device.Close()
		n.device = nil
		return NewDeviceError("claim_interface", ErrInterfaceFailed)
	}

	n.intf = intf
	n.connected = true

	// Create HID features controller using the same USB device
	// This uses USB control transfers, so no separate connection needed
	n.hidFeatures = NewHIDFeatureDevice(n.device, n.logger)

	// Initialize touch reader with interrupt endpoint 1 (IN)
	if touchEp, err := intf.InEndpoint(1); err == nil {
		n.touchReader = NewTouchReader(touchEp, n.logger)
		n.logger.Debug("touch input initialized", "endpoint", 1)
	} else {
		n.logger.Warn("touch endpoint not available", "error", err)
	}

	n.logger.Info("device connected",
		"vendor_id", n.config.VendorID,
		"product_id", n.config.ProductID,
	)

	// Start reconnection monitoring
	if !n.reconnecting {
		n.reconnecting = true
		go n.monitorConnection()
	}

	return nil
}

// Disconnect closes the device connection.
func (n *NexusDevice) Disconnect() error {
	n.mu.Lock()
	defer n.mu.Unlock()

	// Stop reconnection monitoring
	if n.reconnecting {
		close(n.stopReconnect)
		n.reconnecting = false
	}

	return n.disconnect()
}

// disconnect closes resources without locking (internal use).
func (n *NexusDevice) disconnect() error {
	if !n.connected {
		return nil
	}

	// HID features share the same device, so no separate close needed
	n.hidFeatures = nil

	if n.intf != nil {
		n.intf.Close()
		n.intf = nil
	}

	if n.device != nil {
		n.device.Close()
		n.device = nil
	}

	n.connected = false

	n.logger.Info("device disconnected")
	return nil
}

// IsConnected returns the current connection status.
func (n *NexusDevice) IsConnected() bool {
	n.mu.RLock()
	defer n.mu.RUnlock()
	return n.connected
}

// SendFrame sends a frame of image data to the device.
func (n *NexusDevice) SendFrame(ctx context.Context, data []byte) error {
	n.mu.RLock()
	defer n.mu.RUnlock()

	if !n.connected || n.device == nil {
		return ErrDeviceDisconnected
	}

	if len(data) != FrameSize {
		return ErrInvalidFrame
	}

	// This will be implemented with the actual USB protocol
	// For now, placeholder to maintain interface
	return n.sendImageDataInChunks(data)
}

// sendImageDataInChunks sends frame data in chunks via USB.
func (n *NexusDevice) sendImageDataInChunks(imageData []byte) error {
	if n.intf == nil {
		return ErrDeviceDisconnected
	}

	// Get output endpoint
	ep, err := n.intf.OutEndpoint(2)
	if err != nil {
		return NewDeviceError("get_endpoint", err)
	}

	buffer := make([]byte, ChunkSize)
	buffer[0] = 2
	buffer[1] = 5
	buffer[2] = 31
	buffer[5] = 0
	buffer[7] = 3

	// Use buffered writer like the old code
	writer := bufio.NewWriterSize(ep, ChunkSize)

	// Send data in chunks
	for i := 0; i <= NumChunks-1; i++ {
		buffer[4] = byte(i)

		if i != NumChunks-1 {
			buffer[3] = 0
			buffer[6] = 248
		} else {
			buffer[3] = 1
			buffer[6] = 192
		}

		offset := i * 254

		// Copy pixel data (BGR format with alpha)
		for j := 0; j < 255 && offset < DisplayWidth*DisplayHeight; j++ {
			pixelIdx := offset * 4
			// Bounds check to prevent reading past the imageData array
			if pixelIdx+3 >= len(imageData) {
				break
			}
			buffer[8+j*4] = imageData[pixelIdx+2]   // B
			buffer[8+j*4+1] = imageData[pixelIdx+1] // G
			buffer[8+j*4+2] = imageData[pixelIdx]   // R
			buffer[8+j*4+3] = 255                   // A
			offset++
		}

		// Write chunk using buffered writer
		_, err := writer.Write(buffer)
		if err != nil {
			n.mu.Lock()
			n.connected = false
			n.mu.Unlock()
			return NewDeviceError("write", ErrSendFailed)
		}
	}

	// Flush buffered data
	if err := writer.Flush(); err != nil {
		return NewDeviceError("flush", err)
	}

	return nil
}

// ReadTouch reads touch events from the device.
func (n *NexusDevice) ReadTouch(ctx context.Context) ([]TouchEvent, error) {
	n.mu.RLock()
	defer n.mu.RUnlock()

	if !n.connected {
		return nil, ErrDeviceDisconnected
	}

	if n.touchReader == nil {
		return []TouchEvent{}, nil // Touch not available
	}

	// Create context with short timeout for non-blocking reads
	touchCtx, cancel := context.WithTimeout(ctx, 50*time.Millisecond)
	defer cancel()

	return n.touchReader.Read(touchCtx)
}

// Health checks if the device connection is healthy.
func (n *NexusDevice) Health() error {
	n.mu.RLock()
	defer n.mu.RUnlock()

	if n.device == nil {
		return NewDeviceError("health_check", ErrDeviceDisconnected)
	}

	if n.intf == nil {
		return NewDeviceError("health_check", ErrInterfaceFailed)
	}

	return nil
}

// monitorConnection monitors connection and attempts reconnection.
func (n *NexusDevice) monitorConnection() {
	ticker := time.NewTicker(n.config.ReconnectDelay)
	defer ticker.Stop()

	for {
		select {
		case <-n.stopReconnect:
			return
		case <-ticker.C:
			n.mu.RLock()
			connected := n.connected
			n.mu.RUnlock()

			if !connected {
				n.attemptReconnect()
			} else {
				// Check health
				if err := n.Health(); err != nil {
					n.logger.Warn("device health check failed", "error", err)
					n.mu.Lock()
					n.disconnect()
					n.mu.Unlock()
				}
			}
		}
	}
}

// attemptReconnect tries to reconnect to the device.
func (n *NexusDevice) attemptReconnect() {
	ctx := context.Background()

	for i := 0; i < n.config.ReconnectRetries; i++ {
		n.logger.Info("attempting reconnection",
			"attempt", i+1,
			"max_retries", n.config.ReconnectRetries,
		)

		if err := n.Connect(ctx); err == nil {
			n.logger.Info("reconnection successful")
			return
		}

		if i < n.config.ReconnectRetries-1 {
			backoff := time.Duration(1<<uint(i)) * time.Second
			time.Sleep(backoff)
		}
	}

	n.logger.Error("reconnection failed after all attempts")
}

// SetBrightness sets the display brightness (0-100).
// Requires HID feature support to be available.
func (n *NexusDevice) SetBrightness(brightness int) error {
	if n.hidFeatures == nil {
		return errors.New("HID features not available")
	}
	return n.hidFeatures.SetBrightness(brightness)
}

// GetFirmwareVersion queries and returns the device firmware version.
func (n *NexusDevice) GetFirmwareVersion() (string, error) {
	if n.hidFeatures == nil {
		return "", errors.New("HID features not available")
	}
	return n.hidFeatures.GetFirmwareVersion()
}
