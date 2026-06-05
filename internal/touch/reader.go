// Package touch provides HID touch input reading with velocity-aware smoothing (One-Euro filter).
package touch

import (
	"context"
	"fmt"
	"log/slog"
	"math"
	"time"

	"github.com/karalabe/hid"
)

// ===== One-Euro filter for velocity-aware smoothing =====

type oneEuro struct {
	minCutoff   float64 // e.g., 1.0
	beta        float64 // e.g., 0.007
	dCutoff     float64 // e.g., 1.0
	prevX       float64
	prevDx      float64
	initialized bool
}

func (f *oneEuro) alpha(cutoff, dt float64) float64 {
	if cutoff <= 0 {
		return 1 // no smoothing
	}
	tau := 1.0 / (2.0 * math.Pi * cutoff)
	return 1.0 / (1.0 + tau/dt)
}

func (f *oneEuro) filter(x float64, dt float64) float64 {
	if !f.initialized {
		f.prevX = x
		f.prevDx = 0
		f.initialized = true
		return x
	}
	dx := (x - f.prevX) / dt
	aD := f.alpha(f.dCutoff, dt)
	dxHat := aD*dx + (1-aD)*f.prevDx

	cutoff := f.minCutoff + f.beta*math.Abs(dxHat)
	a := f.alpha(cutoff, dt)
	xHat := a*x + (1-a)*f.prevX

	f.prevX, f.prevDx = xHat, dxHat
	return xHat
}

// ===== Utilities =====

func clamp01(v float64) float64 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func bFromDx(dx int) int {
	if dx < 0 {
		return 1 // swipe left
	}
	return 2 // swipe right
}

// ===== Velocity trailing window =====

// velSample is one position+time snapshot stored during a swipe.
type velSample struct {
	x float64
	t time.Time
}

// velWindow holds the last velWindowSize samples for release-velocity estimation.
// On lift we compute a weighted average over the window, weighting recent samples
// more heavily — the same approach used by UIKit on iOS.
const velWindowSize = 4

type velWindow struct {
	samples [velWindowSize]velSample
	count   int // total samples written (wraps at velWindowSize)
}

func (w *velWindow) reset() { w.count = 0 }

func (w *velWindow) push(x float64, t time.Time) {
	w.samples[w.count%velWindowSize] = velSample{x, t}
	w.count++
}

// velocity returns a weighted release velocity in pixels/second.
// Weights are [1, 2, 3, 4] oldest-to-newest so the terminal motion dominates.
// Returns 0 if fewer than 2 samples are available.
func (w *velWindow) velocity() float64 {
	n := w.count
	if n > velWindowSize {
		n = velWindowSize
	}
	if n < 2 {
		return 0
	}

	// Samples are stored in a ring; oldest is at index (count % size) when full.
	start := 0
	if w.count > velWindowSize {
		start = w.count % velWindowSize
	}

	var weightedSum, weightTotal float64
	for i := 0; i < n-1; i++ {
		a := w.samples[(start+i)%velWindowSize]
		b := w.samples[(start+i+1)%velWindowSize]
		dt := b.t.Sub(a.t).Seconds()
		if dt <= 0 {
			continue
		}
		// Weight increases linearly toward the most recent pair.
		weight := float64(i + 1)
		weightedSum += (b.x - a.x) / dt * weight
		weightTotal += weight
	}
	if weightTotal == 0 {
		return 0
	}
	return math.Abs(weightedSum / weightTotal)
}

// ===== HIDTouchReader: HID packet parsing + gesture engine =====

type HIDTouchReader struct {
	device *hid.Device
	logger *slog.Logger

	// Coordinate normalization
	screenWidth int // 640 for Nexus
	rawMin      int // 0
	rawMax      int // 1023 or 4095

	// One-Euro filter for smoothing
	euro oneEuro

	// Velocity trailing window — last N position samples before lift
	vel velWindow

	// Gesture state
	lastT       time.Time
	downPrev    bool
	pressT      time.Time
	startX      int
	swipeActive bool
	maxRawSeen  int // tracks highest raw X seen — logged on first swipe to validate rawMax
}

// NewHIDTouchReader creates a new HID touch reader with velocity-aware smoothing
func NewHIDTouchReader(device *hid.Device, logger *slog.Logger) *HIDTouchReader {
	return &HIDTouchReader{
		device:      device,
		logger:      logger,
		screenWidth: 640,
		rawMin:      0,
		rawMax:      486, // observed device max — HID reports 10-bit coords in 0-486 range
		euro: oneEuro{
			minCutoff: 1.0,
			beta:      0.007,
			dCutoff:   1.0,
		},
	}
}

// normalize converts raw touch coordinate to screen pixels
func (t *HIDTouchReader) normalize(raw int) float64 {
	if raw < t.rawMin {
		raw = t.rawMin
	}
	if raw > t.rawMax {
		raw = t.rawMax
	}
	return (float64(raw-t.rawMin) / float64(t.rawMax-t.rawMin)) * float64(t.screenWidth-1)
}

// px converts a fraction to pixels
func (t *HIDTouchReader) px(frac float64) int {
	return int(math.Round(frac * float64(t.screenWidth)))
}

// Read reads and processes touch events from HID input reports
func (t *HIDTouchReader) Read(ctx context.Context) ([]Event, error) {
	if t.device == nil {
		return nil, fmt.Errorf("HID device not initialized")
	}

	// Read HID report (blocking)
	buffer := make([]byte, 64)
	bytesRead, err := t.device.Read(buffer)
	if err != nil {
		t.logger.Debug("HID read error", "error", err)
		return []Event{}, fmt.Errorf("HID read: %w", err)
	}

	if bytesRead < 8 {
		return []Event{}, nil // Not enough data
	}

	// Validate touch protocol header: [0x01, 0x02, 0x21]
	if buffer[0] != 0x01 || buffer[1] != 0x02 || buffer[2] != 0x21 {
		// Not a touch packet - skip silently
		return []Event{}, nil
	}

	// Parse touch data
	touchState := buffer[5]
	rawX := int(buffer[6]) | (int(buffer[7]) << 8) // Little-endian
	down := touchState != 0

	if rawX > t.maxRawSeen {
		t.maxRawSeen = rawX
	}

	now := time.Now()

	// Calculate dt for smoothing
	if t.lastT.IsZero() {
		t.lastT = now
	}
	dt := now.Sub(t.lastT).Seconds()
	if dt <= 0 {
		dt = 1.0 / 500.0 // Assume 500Hz if timestamps are too close
	}
	t.lastT = now

	// Apply One-Euro filter for smooth coordinates
	x := t.normalize(rawX)
	xs := t.euro.filter(x, dt)
	xi := int(math.Round(xs))

	// Gesture detection thresholds
	const (
		tapMaxMoveFrac = 0.05 // 5% of screen width for tap
		tapMaxDur      = 1 * time.Second
	)

	// Adaptive swipe threshold based on velocity (snappier at higher speeds)
	speed := math.Abs(t.euro.prevDx)                      // pixels/sec after normalization
	swipeStartFrac := 0.09 - math.Min(0.04, speed/1200.0) // base range: [0.05..0.09]
	if swipeStartFrac < 0.07 {
		swipeStartFrac = 0.07 // keep short taps from triggering swipes
	}

	events := []Event{}

	// State machine for gesture detection
	if down && !t.downPrev {
		// Touch press
		t.startX = xi
		t.pressT = now
		t.swipeActive = false
		t.downPrev = true
		t.vel.reset()
		t.vel.push(float64(xi), now)
		t.logger.Debug("touch press", "x", xi, "raw", rawX)
		return events, nil
	}

	if down && t.downPrev {
		// Touch held - check for swipe
		dx := xi - t.startX
		if !t.swipeActive && abs(dx) >= t.px(swipeStartFrac) {
			t.swipeActive = true
			t.logger.Debug("swipe activated", "dx", dx, "threshold", t.px(swipeStartFrac))
		}

		if t.swipeActive {
			t.vel.push(float64(xi), now)
			// Emit live swipe progress
			prog := clamp01(float64(abs(dx)) / float64(t.screenWidth))
			events = append(events, Event{
				Button:        bFromDx(dx),
				Pressed:       true,
				SwipeProgress: float32(prog),
				SwipeActive:   true,
				SwipePixels:   dx,
				Timestamp:     now,
			})
			t.logger.Debug("live swipe", "dx", dx, "progress", int(prog*100))
		}
		return events, nil
	}

	if !down && t.downPrev {
		// Touch release
		t.downPrev = false
		dx := xi - t.startX
		dur := now.Sub(t.pressT)

		if t.swipeActive {
			prog := clamp01(float64(abs(dx)) / float64(t.screenWidth))
			// Release velocity from the trailing window: weighted average of the
			// last velWindowSize position samples, newest weighted most. This
			// captures terminal finger speed regardless of swipe direction and
			// eliminates the One-Euro filter's directional asymmetry.
			velocity := float32(t.vel.velocity())

			events = append(events, Event{
				Button:        bFromDx(dx),
				Pressed:       false,
				Duration:      dur,
				SwipeProgress: float32(prog),
				SwipeActive:   false,
				Velocity:      velocity,
				SwipePixels:   dx,
				Timestamp:     now,
			})
			t.logger.Info("swipe complete",
				"dx", dx,
				"progress", int(prog*100),
				"duration_ms", dur.Milliseconds(),
				"velocity_px_s", int(velocity),
				"max_raw_x_seen", t.maxRawSeen,
				"configured_raw_max", t.rawMax)
		} else if abs(dx) < t.px(tapMaxMoveFrac) && dur <= tapMaxDur {
			// Tap — xi is the smoothed display-pixel X position (0–screenWidth-1).
			events = append(events, Event{
				Button:    0,
				Pressed:   false,
				Duration:  dur,
				Timestamp: now,
				TapX:      xi,
			})
			t.logger.Info("tap detected", "x", xi, "duration_ms", dur.Milliseconds())
		} else {
			t.logger.Debug("gesture canceled", "dx", dx, "duration_ms", dur.Milliseconds())
		}
		t.swipeActive = false
		return events, nil
	}

	return events, nil
}
