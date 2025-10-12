package display

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"nexus-open/internal/config"
	"nexus-open/internal/device"
	"nexus-open/internal/instruments"
)

const (
	// FrameRate is the target display refresh rate (24 Hz)
	FrameRate = 24
	// FrameDuration is the time between frames
	FrameDuration = time.Second / FrameRate
	// BlinkInterval for the time colon (twice per second)
	BlinkInterval = 500 * time.Millisecond
)

// Manager orchestrates the display rendering loop
type Manager struct {
	logger     *slog.Logger
	cfg        *config.Manager
	device     device.Device
	instruments *instruments.Registry
	renderer   *NexusRenderer

	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// NewManager creates a new display manager
func NewManager(
	logger *slog.Logger,
	cfg *config.Manager,
	dev device.Device,
	registry *instruments.Registry,
) *Manager {
	return &Manager{
		logger:      logger,
		cfg:         cfg,
		device:      dev,
		instruments: registry,
	}
}

// Initialize prepares the display manager
func (m *Manager) Initialize() error {
	m.renderer = NewNexusRenderer(m.logger, m.cfg, m.device)
	if err := m.renderer.Initialize(); err != nil {
		return fmt.Errorf("failed to initialize renderer: %w", err)
	}

	m.logger.Info("display manager initialized")
	return nil
}

// Start begins the display update loop
func (m *Manager) Start(ctx context.Context) error {
	m.ctx, m.cancel = context.WithCancel(ctx)

	// Start main render loop
	m.wg.Add(1)
	go m.renderLoop()

	// Start blink animation
	m.wg.Add(1)
	go m.blinkLoop()

	// Watch for config changes
	m.wg.Add(1)
	go m.watchConfig()

	m.logger.Info("display manager started", "fps", FrameRate)
	return nil
}

// Stop gracefully stops the display manager
func (m *Manager) Stop() error {
	if m.cancel != nil {
		m.cancel()
	}
	m.wg.Wait()

	m.logger.Info("display manager stopped")
	return nil
}

// renderLoop is the main rendering loop that runs at the target frame rate
func (m *Manager) renderLoop() {
	defer m.wg.Done()

	ticker := time.NewTicker(FrameDuration)
	defer ticker.Stop()

	frameCount := 0
	startTime := time.Now()

	for {
		select {
		case <-m.ctx.Done():
			return
		case <-ticker.C:
			if err := m.renderFrame(); err != nil {
				m.logger.Error("frame render failed", "error", err)
			}

			frameCount++
			// Log FPS every 5 seconds
			if frameCount%120 == 0 {
				elapsed := time.Since(startTime)
				actualFPS := float64(frameCount) / elapsed.Seconds()
				m.logger.Debug("display stats",
					"frames", frameCount,
					"fps", fmt.Sprintf("%.1f", actualFPS))
			}
		}
	}
}

// renderFrame renders a single frame and sends it to the device
func (m *Manager) renderFrame() error {
	// Check if device is connected
	if !m.device.IsConnected() {
		return nil // Skip if not connected
	}

	// Get current instrument data
	data := m.instruments.GetData()

	// Render the frame
	frame, err := m.renderer.Render(m.ctx, data)
	if err != nil {
		return fmt.Errorf("render failed: %w", err)
	}

	// Convert to byte buffer
	frameData := frame.Pix

	// Send to device
	if err := m.device.SendFrame(m.ctx, frameData); err != nil {
		// Device might have disconnected
		m.logger.Warn("failed to send frame", "error", err)
		return err
	}

	return nil
}

// blinkLoop handles the blinking colon animation
func (m *Manager) blinkLoop() {
	defer m.wg.Done()

	ticker := time.NewTicker(BlinkInterval)
	defer ticker.Stop()

	for {
		select {
		case <-m.ctx.Done():
			return
		case <-ticker.C:
			m.renderer.UpdateBlinkState()
		}
	}
}

// watchConfig monitors configuration changes and updates the renderer
func (m *Manager) watchConfig() {
	defer m.wg.Done()

	// Create channel for config updates
	ch := make(chan config.Config, 1)
	m.cfg.Watch(ch)

	for {
		select {
		case <-m.ctx.Done():
			return
		case cfg := <-ch:
			m.logger.Debug("config changed, updating display")

			// Update colors
			if textColor, err := parseColor(cfg.TextColor); err == nil {
				m.renderer.SetTextColor(textColor)
			}

			if bgColor, err := parseColor(cfg.BackgroundColor); err == nil {
				m.renderer.SetBackgroundColor(bgColor)
			}

			// Force immediate frame update
			if err := m.renderFrame(); err != nil {
				m.logger.Warn("config update render failed", "error", err)
			}
		}
	}
}
