// Package touch provides zone-aware touch input handling and gesture recognition.
package touch

import (
	"context"
	"log/slog"
	"time"

	"nexus-open/internal/device"
	"nexus-open/internal/zone"
)

// Handler processes touch events and dispatches them to zone-aware actions.
type Handler struct {
	logger      *slog.Logger
	device      device.Device
	zoneManager *zone.Manager

	// Gesture state
	lastTouch   time.Time
	touchActive bool

	// Configuration
	swipeEnabled bool
	tapEnabled   bool
}

// NewHandler creates a new touch handler.
func NewHandler(logger *slog.Logger, dev device.Device, zm *zone.Manager) *Handler {
	return &Handler{
		logger:       logger,
		device:       dev,
		zoneManager:  zm,
		swipeEnabled: true,
		tapEnabled:   true,
	}
}

// Start begins processing touch events.
func (h *Handler) Start(ctx context.Context) error {
	h.logger.Info("touch handler started")

	go h.processLoop(ctx)

	return nil
}

// processLoop continuously reads and processes touch events.
func (h *Handler) processLoop(ctx context.Context) {
	ticker := time.NewTicker(50 * time.Millisecond) // Poll at 20Hz
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			h.logger.Info("touch handler stopped")
			return
		case <-ticker.C:
			if err := h.processEvents(ctx); err != nil {
				if err == device.ErrDeviceDisconnected {
					h.logger.Warn("device disconnected, stopping touch processing")
					return
				}
				// Log but continue on other errors
				h.logger.Debug("touch processing error", "error", err)
			}
		}
	}
}

// processEvents reads and handles touch events from the device.
func (h *Handler) processEvents(ctx context.Context) error {
	if !h.device.IsConnected() {
		return nil // Silently skip if device not connected
	}

	events, err := h.device.ReadTouch(ctx)
	if err != nil {
		return err
	}

	for _, event := range events {
		h.handleEvent(event)
	}

	return nil
}

// handleEvent processes a single touch event.
func (h *Handler) handleEvent(event device.TouchEvent) {
	h.lastTouch = time.Now()

	switch event.Button {
	case 0: // Tap gesture
		if h.tapEnabled {
			h.handleTap()
		}
	case 1: // Swipe left
		if h.swipeEnabled {
			h.handleSwipeLeft()
		}
	case 2: // Swipe right
		if h.swipeEnabled {
			h.handleSwipeRight()
		}
	default:
		h.logger.Debug("unknown touch event button", "button", event.Button)
	}
}

// handleTap processes a tap gesture.
func (h *Handler) handleTap() {
	h.logger.Debug("tap gesture detected")

	// For now, taps cycle through zones or trigger zone-specific actions
	// In the future, this could be position-aware to detect which zone was tapped

	// Example: Cycle to next page on tap (simple behavior)
	// In production, you'd want to:
	// 1. Determine which zone was tapped based on X coordinate
	// 2. Execute that zone's TapAction (cycle modules, trigger action, etc.)

	// For now, just log it
	h.logger.Info("tap detected - zone-specific action would go here")
}

// handleSwipeLeft processes a left swipe gesture.
func (h *Handler) handleSwipeLeft() {
	h.logger.Debug("swipe left detected")

	// Swipe left = next page
	if err := h.zoneManager.NextPage(); err != nil {
		h.logger.Error("failed to switch to next page", "error", err)
		return
	}

	h.logger.Info("switched to next page",
		"page", h.zoneManager.GetCurrentPage(),
		"name", h.zoneManager.GetConfig().Pages[h.zoneManager.GetCurrentPage()].Name)
}

// handleSwipeRight processes a right swipe gesture.
func (h *Handler) handleSwipeRight() {
	h.logger.Debug("swipe right detected")

	// Swipe right = previous page
	if err := h.zoneManager.PrevPage(); err != nil {
		h.logger.Error("failed to switch to previous page", "error", err)
		return
	}

	h.logger.Info("switched to previous page",
		"page", h.zoneManager.GetCurrentPage(),
		"name", h.zoneManager.GetConfig().Pages[h.zoneManager.GetCurrentPage()].Name)
}

// SetSwipeEnabled enables or disables swipe gesture recognition.
func (h *Handler) SetSwipeEnabled(enabled bool) {
	h.swipeEnabled = enabled
	h.logger.Debug("swipe gestures", "enabled", enabled)
}

// SetTapEnabled enables or disables tap gesture recognition.
func (h *Handler) SetTapEnabled(enabled bool) {
	h.tapEnabled = enabled
	h.logger.Debug("tap gestures", "enabled", enabled)
}

// GetLastTouchTime returns the timestamp of the last touch event.
func (h *Handler) GetLastTouchTime() time.Time {
	return h.lastTouch
}
