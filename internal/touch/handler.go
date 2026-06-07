// Package touch provides zone-aware touch input handling and gesture recognition.
package touch

import (
	"context"
	"errors"
	"log/slog"
	"time"

	"github.com/mantonx/nexus-open/internal/zone"
)

// DeviceReader is an interface for reading touch events from a device.
// This allows the handler to work with any device implementation without
// importing the device package (breaking circular dependency).
type DeviceReader interface {
	IsConnected() bool
	ReadTouch(ctx context.Context) ([]Event, error)
}

// Common errors
var (
	ErrDeviceDisconnected = errors.New("device disconnected")
)

// Handler processes touch events and dispatches them to zone-aware actions.
type Handler struct {
	logger      *slog.Logger
	device      DeviceReader
	zoneManager *zone.Manager

	// Gesture state
	lastTouch time.Time

	// Configuration
	swipeEnabled bool
	tapEnabled   bool
	swipeConfig  SwipeConfig
}

// NewHandler creates a new touch handler.
func NewHandler(logger *slog.Logger, dev DeviceReader, zm *zone.Manager) *Handler {
	return &Handler{
		logger:       logger,
		device:       dev,
		zoneManager:  zm,
		swipeEnabled: true,
		tapEnabled:   true,
		swipeConfig:  DefaultSwipeConfig(),
	}
}

// Start begins processing touch events.
func (h *Handler) Start(ctx context.Context) error {
	h.logger.Info("touch handler started")

	go h.processLoop(ctx)

	return nil
}

// processLoop continuously reads and processes touch events.
// HID reads are blocking — we run them in a tight loop without a ticker so
// packets are processed as soon as they arrive rather than waiting for the
// next poll interval (previously up to 50ms stale).
//
// On disconnect, the loop waits for the device to reconnect using exponential
// backoff (1s→2s→4s…→30s) rather than exiting — touch resumes automatically
// when the device is plugged back in without requiring an app restart.
func (h *Handler) processLoop(ctx context.Context) {
	backoff := time.Second
	const maxBackoff = 30 * time.Second

	for {
		select {
		case <-ctx.Done():
			h.logger.Info("touch handler stopped")
			return
		default:
		}

		if err := h.processEvents(ctx); err != nil {
			if err == ErrDeviceDisconnected {
				h.logger.Warn("touch: device disconnected, waiting for reconnect",
					"retry_in", backoff)
				select {
				case <-ctx.Done():
					return
				case <-time.After(backoff):
				}
				if backoff < maxBackoff {
					backoff *= 2
					if backoff > maxBackoff {
						backoff = maxBackoff
					}
				}
				continue
			}
			h.logger.Debug("touch processing error", "error", err)
		} else {
			// Successful read — reset backoff so a reconnect starts fresh.
			backoff = time.Second
		}
	}
}

// processEvents reads and handles touch events from the device.
func (h *Handler) processEvents(ctx context.Context) error {
	if !h.device.IsConnected() {
		// Device absent — pause briefly to avoid spinning at 100% CPU.
		time.Sleep(50 * time.Millisecond)
		return nil
	}

	events, err := h.device.ReadTouch(ctx)
	if err != nil {
		// Any read error from a connected device means the device went away —
		// treat it as a disconnect so the reconnect backoff loop fires.
		return ErrDeviceDisconnected
	}

	for _, event := range events {
		h.handleEvent(event)
	}

	return nil
}

// handleEvent processes a single touch event.
func (h *Handler) handleEvent(event Event) {
	h.lastTouch = time.Now()

	// Handle live swipe tracking
	if event.SwipeActive {
		// Live swipe in progress - update transition progress
		h.handleLiveSwipe(event)
		return
	}

	// Handle completed gestures
	switch event.Button {
	case 0: // Tap gesture
		if h.tapEnabled {
			h.handleTap(event)
		}
	case 1: // Swipe left (completed)
		if h.swipeEnabled {
			h.handleSwipeComplete(event, true) // true = left
		}
	case 2: // Swipe right (completed)
		if h.swipeEnabled {
			h.handleSwipeComplete(event, false) // false = right
		}
	default:
		h.logger.Debug("unknown touch event button", "button", event.Button)
	}
}

// handleTap routes a tap to the zone at event.TapX and executes its OnTap action.
func (h *Handler) handleTap(event Event) {
	tapX := event.TapX

	cfg := h.zoneManager.GetConfig()
	pageIdx := h.zoneManager.GetCurrentPage()
	if pageIdx >= len(cfg.Pages) {
		return
	}
	page := cfg.Pages[pageIdx]
	page.ComputeOffsets()

	// Find which zone contains tapX.
	for _, z := range page.Zones {
		if tapX >= z.X && tapX < z.X+z.Width {
			h.logger.Info("tap on zone", "zone", z.ID, "x", tapX, "action", z.OnTap)
			h.executeTapAction(z)
			return
		}
	}
	h.logger.Debug("tap outside all zones", "x", tapX)
}

// executeTapAction runs the configured OnTap action for a zone.
func (h *Handler) executeTapAction(z zone.ZoneConfig) {
	switch z.OnTap {
	case zone.TapActionCycle:
		// Cycle the zone to its next plugin choice.
		if err := h.zoneManager.CycleZonePlugin(z.ID); err != nil {
			h.logger.Warn("cycle zone plugin failed", "zone", z.ID, "error", err)
		}
	case zone.TapActionNone, "":
		// No action configured — silently ignore.
	default:
		h.logger.Debug("unknown tap action", "zone", z.ID, "action", z.OnTap)
	}
}

// handleLiveSwipe processes live swipe progress events.
func (h *Handler) handleLiveSwipe(event Event) {
	isLeft := event.Button == 1

	h.logger.Debug("🪄 SWIPE LIVE",
		"progress_pct", int(event.SwipeProgress*100),
		"pixels", event.SwipePixels,
		"direction", map[bool]string{true: "left", false: "right"}[isLeft],
		"timestamp_ms", event.Timestamp.UnixMilli())

	// Update the zone manager with live swipe progress
	if err := h.zoneManager.UpdateLiveSwipe(event.SwipeProgress, isLeft); err != nil {
		h.logger.Debug("failed to update live swipe", "error", err)
		return
	}
}

// handleSwipeComplete processes a completed swipe gesture.
func (h *Handler) handleSwipeComplete(event Event, isLeft bool) {
	progress := event.SwipeProgress
	velocity := event.Velocity
	directionLabel := map[bool]string{true: "LEFT", false: "RIGHT"}[isLeft]

	h.logger.Info("🛬 SWIPE RELEASE",
		"direction", directionLabel,
		"progress_pct", int(progress*100),
		"velocity_px_s", int(velocity),
		"pixels", event.SwipePixels,
		"duration_ms", event.Duration.Milliseconds())

	// Use multi-heuristic decision algorithm to determine commit vs cancel
	shouldCommit, reason := h.swipeConfig.shouldCommitSwipe(progress, velocity)

	if shouldCommit {
		// Commit the swipe - finalize the live transition smoothly with momentum
		h.logger.Info("✅ SWIPE COMMIT",
			"direction", directionLabel,
			"progress", int(progress*100),
			"velocity_px_s", int(velocity),
			"pixels", event.SwipePixels,
			"reason", reason)

		// Finalize the live swipe with momentum-based duration
		// This handles the page change internally and smoothly completes the animation
		if err := h.zoneManager.FinalizeLiveSwipe(progress, velocity, isLeft); err != nil {
			h.logger.Error("❌ FinalizeLiveSwipe() failed", "error", err)
			return
		}
	} else {
		// Cancel the swipe - snap back to current page
		h.logger.Info("↩️ SWIPE CANCEL",
			"progress", int(progress*100),
			"velocity_px_s", int(velocity),
			"pixels", event.SwipePixels,
			"reason", reason)

		if err := h.zoneManager.CancelLiveSwipe(); err != nil {
			h.logger.Error("failed to cancel swipe", "error", err)
		}
	}
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
