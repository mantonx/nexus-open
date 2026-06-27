// Package touch provides zone-aware touch input handling and gesture recognition.
package touch

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"sync/atomic"
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

// tapMaxMovePixels is the maximum finger drift (px) for a zone-targeting tap.
// Set to 100px (~15% of screen) to accommodate natural left-drift on lift.
// The swipe classifier catches anything that actually commits a page change,
// so raising this threshold does not risk accidental zone fires during swipes.
const tapMaxMovePixels = 100


// Handler processes touch events and dispatches them to zone-aware actions.
type Handler struct {
	logger      *slog.Logger
	device      DeviceReader
	zoneManager *zone.Manager

	// Gesture state
	lastTouch       time.Time
	detailInFlight  atomic.Bool // true while a handleDetailTap goroutine is running

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
	case 3: // Long press — dismiss detail overlay if showing
		if h.zoneManager.IsShowingDetail() {
			h.logger.Info("long press dismissed detail overlay")
			h.zoneManager.ClearDetail()
		}
	default:
		h.logger.Debug("unknown touch event button", "button", event.Button)
	}
}

// handleTap routes a tap to the zone at event.TapX and executes its OnTap action.
func (h *Handler) handleTap(event Event) {
	showingDetail := h.zoneManager.IsShowingDetail()
	inFlight := h.detailInFlight.Load()
	h.logger.Info("TAP_DIAG: handleTap entry",
		"x", event.TapX,
		"slide_dx", event.SlideX,
		"showing_detail", showingDetail,
		"detail_in_flight", inFlight)

	// While detail is shown, route the tap to the plugin (which may handle
	// prev/next controls). The plugin decides whether to keep the overlay open.
	if showingDetail {
		h.logger.Info("tap on detail overlay", "x", event.TapX, "dx", event.SlideX)
		h.zoneManager.StartTapRipple(event.TapX)
		h.zoneManager.HandleDetailTap(event.TapX, 24)
		return
	}

	// For zone targeting, require a stationary-enough touch (SlideX within tap
	// threshold) so sliding touches don't accidentally activate zones.
	if abs(event.SlideX) > tapMaxMovePixels {
		h.logger.Info("TAP_DIAG: sliding tap ignored for zone targeting", "dx", event.SlideX)
		return
	}

	tapX := event.TapX

	cfg := h.zoneManager.GetConfig()
	pageIdx := h.zoneManager.GetCurrentPage()
	if pageIdx >= len(cfg.Pages) {
		h.logger.Warn("TAP_DIAG: page index out of range", "page_idx", pageIdx, "pages", len(cfg.Pages))
		return
	}
	page := cfg.Pages[pageIdx]
	page.ComputeOffsets()

	h.logger.Info("TAP_DIAG: zone layout",
		"page", page.Name,
		"zones", func() []string {
			out := make([]string, len(page.Zones))
			for i, z := range page.Zones {
				out[i] = fmt.Sprintf("%s[x=%d w=%d]", z.ID, z.X, z.Width)
			}
			return out
		}())

	// Find which zone contains tapX.
	for _, z := range page.Zones {
		if tapX >= z.X && tapX < z.X+z.Width {
			h.logger.Info("tap on zone", "zone", z.ID, "x", tapX, "action", z.OnTap)
			h.executeTapAction(z, tapX)
			return
		}
	}
	h.logger.Info("TAP_DIAG: tap outside all zones", "x", tapX)
}

// executeTapAction runs the tap action for a zone, using the plugin's Tapper
// implementation as the authority when on_tap is not explicitly configured.
// The ripple is only started when the zone has an actionable tap response.
func (h *Handler) executeTapAction(z zone.ZoneConfig, tapX int) {
	switch h.zoneManager.EffectiveTapAction(z) {
	case zone.TapActionCycle:
		h.zoneManager.StartTapRipple(tapX)
		if err := h.zoneManager.CycleZonePlugin(z.ID); err != nil {
			h.logger.Warn("cycle zone plugin failed", "zone", z.ID, "error", err)
		}
	case zone.TapActionDetail:
		if h.detailInFlight.CompareAndSwap(false, true) {
			h.zoneManager.StartTapRipple(tapX)
			go func() {
				defer h.detailInFlight.Store(false)
				h.logger.Info("TAP_DIAG: calling HandleZoneTap", "zone", z.ID)
				start := time.Now()
				if err := h.zoneManager.HandleZoneTap(z.ID); err != nil {
					h.logger.Warn("TAP_DIAG: HandleZoneTap error",
						"zone", z.ID, "duration_ms", time.Since(start).Milliseconds(), "error", err)
				} else {
					h.logger.Info("TAP_DIAG: HandleZoneTap complete",
						"zone", z.ID, "duration_ms", time.Since(start).Milliseconds())
				}
			}()
		} else {
			h.logger.Info("TAP_DIAG: detail tap dropped — OnTap already in flight", "zone", z.ID)
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
