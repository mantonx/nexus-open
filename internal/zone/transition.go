package zone

import (
	"image"
	"image/color"
	"image/draw"
	"math"
	"time"
)

// TransitionType defines the type of page transition effect.
type TransitionType int

const (
	TransitionNone TransitionType = iota
	TransitionFade
	TransitionSlideLeft
	TransitionSlideRight
)

// Spring constants for the finalize/cancel snap animation.
// stiffness controls how aggressively the snap pulls toward the target.
// damping prevents oscillation — at criticalDamping = 2*sqrt(stiffness) the
// spring settles without bouncing. We use slight under-damping for a crisp feel.
const (
	springStiffness = 400.0 // progress units/s² — snappy but not instant
	springDamping   = 36.0  // slightly under-damped: 2*sqrt(400)=40, we use 36
)

// TransitionState tracks the current state of a page transition.
type TransitionState struct {
	Active    bool
	Type      TransitionType
	StartTime time.Time
	Duration  time.Duration
	OldFrame  *image.RGBA // Previous page frame
	NewFrame  *image.RGBA // New page frame
	Direction int         // 1 for forward, -1 for backward

	// Manual (finger-driven) state
	manual         bool
	manualProgress float64

	// Spring animation state — used during finalize and cancel snaps.
	// Duration is emergent from stiffness/damping, not hardcoded.
	springActive    bool
	springTarget    float64  // 0 = cancel, 1 = commit
	springVelocity  float64  // progress units/second, seeded from finger release velocity
	springLastTick  time.Time
}

// NewTransitionState creates a new transition state.
func NewTransitionState() *TransitionState {
	return &TransitionState{
		Active:   false,
		Duration: 100 * time.Millisecond,
	}
}

// Start begins a transition from oldFrame to newFrame.
func (ts *TransitionState) Start(transType TransitionType, oldFrame, newFrame *image.RGBA, direction int) {
	ts.Active = true
	ts.Type = transType
	ts.StartTime = time.Now()
	ts.OldFrame = oldFrame
	ts.NewFrame = newFrame
	ts.Direction = direction
	ts.manual = false
	ts.manualProgress = 0
	ts.springActive = false
	ts.springVelocity = 0
}

// StartManual begins a transition driven by explicit progress updates.
func (ts *TransitionState) StartManual(transType TransitionType, oldFrame, newFrame *image.RGBA, direction int) {
	ts.Start(transType, oldFrame, newFrame, direction)
	ts.manual = true
	ts.manualProgress = 0
}

// SetManualProgress updates the progress for a finger-driven transition.
func (ts *TransitionState) SetManualProgress(progress float64) {
	if !ts.Active {
		return
	}
	if progress < 0 {
		progress = 0
	} else if progress > 1 {
		progress = 1
	}
	ts.manual = true
	ts.springActive = false
	ts.manualProgress = progress
	if progress >= 1 {
		ts.manual = false
		ts.Active = false
	}
}

// FinalizeManual starts a spring snap toward 1 (commit), seeded with the
// finger's release velocity converted to progress-units/second.
// releaseVelocityPxS is the finger speed in pixels/second from the touch reader.
func (ts *TransitionState) FinalizeManual(releaseVelocityPxS float32) {
	if !ts.Active {
		return
	}
	// Convert px/s to progress/s (progress = px / displayWidth).
	initialVel := float64(releaseVelocityPxS) / float64(DisplayWidth)
	ts.startSpring(1.0, initialVel)
}

// AnimateManualTo starts a spring snap toward target (0 = cancel, 1 = commit)
// with zero initial velocity. Used for cancel snaps where we don't have a
// meaningful release velocity.
func (ts *TransitionState) AnimateManualTo(target float64, _ time.Duration) {
	if !ts.Active {
		return
	}
	ts.startSpring(target, 0)
}

func (ts *TransitionState) startSpring(target, initialVelocity float64) {
	if target < 0 {
		target = 0
	} else if target > 1 {
		target = 1
	}
	ts.manual = true
	ts.springActive = true
	ts.springTarget = target
	ts.springVelocity = initialVelocity
	ts.springLastTick = time.Now()
}

// IsManual returns true if the transition is currently under manual control.
func (ts *TransitionState) IsManual() bool {
	return ts.manual
}

// ManualProgress returns the current manual progress (0-1).
func (ts *TransitionState) ManualProgress() float64 {
	if !ts.manual {
		return ts.GetProgress()
	}
	return ts.manualProgress
}

// GetProgress returns the transition progress (0.0 to 1.0) and advances the
// spring simulation if active.
func (ts *TransitionState) GetProgress() float64 {
	if !ts.Active {
		return 1.0
	}

	if ts.manual {
		if ts.springActive {
			ts.tickSpring()
		}

		if !ts.springActive {
			// Spring has settled — check if we're done.
			if ts.manualProgress >= 1 {
				ts.manual = false
				ts.Active = false
				ts.OldFrame = ts.NewFrame
				return 1
			}
			if ts.manualProgress <= 0 {
				ts.manual = false
				ts.Active = false
				ts.NewFrame = ts.OldFrame
				return 0
			}
		}

		return ts.manualProgress
	}

	// Timed (non-manual) transition.
	elapsed := time.Since(ts.StartTime)
	progress := float64(elapsed) / float64(ts.Duration)
	if progress >= 1.0 {
		ts.Active = false
		return 1.0
	}
	return progress
}

// tickSpring advances the spring simulation by the elapsed time since the last tick.
// Uses semi-implicit Euler integration — stable for the stiffness values we use.
func (ts *TransitionState) tickSpring() {
	now := time.Now()
	dt := now.Sub(ts.springLastTick).Seconds()
	ts.springLastTick = now

	// Clamp dt so a long pause (e.g. debugger) doesn't catapult the spring.
	if dt > 0.05 {
		dt = 0.05
	}

	displacement := ts.manualProgress - ts.springTarget
	// Spring force: F = -stiffness*x - damping*v
	acceleration := -springStiffness*displacement - springDamping*ts.springVelocity
	ts.springVelocity += acceleration * dt
	ts.manualProgress += ts.springVelocity * dt

	// Clamp to valid range.
	if ts.manualProgress < 0 {
		ts.manualProgress = 0
	} else if ts.manualProgress > 1 {
		ts.manualProgress = 1
	}

	// Settle check: spring is done when both displacement and velocity are tiny.
	settled := math.Abs(ts.manualProgress-ts.springTarget) < 0.002 &&
		math.Abs(ts.springVelocity) < 0.01
	if settled {
		ts.manualProgress = ts.springTarget
		ts.springVelocity = 0
		ts.springActive = false
	}
}

// IsComplete returns whether the transition is finished.
func (ts *TransitionState) IsComplete() bool {
	return !ts.Active || ts.GetProgress() >= 1.0
}

// Render renders the current transition frame.
func (ts *TransitionState) Render() *image.RGBA {
	if !ts.Active || ts.OldFrame == nil || ts.NewFrame == nil {
		if ts.NewFrame != nil {
			return ts.NewFrame
		}
		return image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	}

	progress := ts.GetProgress()
	// Manual transitions: GetProgress already advances the spring or returns the
	// raw finger position — no additional easing applied here.
	// Timed transitions: apply easeInOutCubic.
	var easedProgress float64
	if ts.manual {
		easedProgress = progress
	} else {
		easedProgress = easeInOutCubic(progress)
	}

	switch ts.Type {
	case TransitionFade:
		return ts.renderFade(easedProgress)
	case TransitionSlideLeft:
		return ts.renderSlide(easedProgress, -1)
	case TransitionSlideRight:
		return ts.renderSlide(easedProgress, 1)
	default:
		return ts.NewFrame
	}
}

// renderFade creates a fade transition between two frames.
func (ts *TransitionState) renderFade(progress float64) *image.RGBA {
	result := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	for y := 0; y < DisplayHeight; y++ {
		for x := 0; x < DisplayWidth; x++ {
			oldColor := ts.OldFrame.RGBAAt(x, y)
			newColor := ts.NewFrame.RGBAAt(x, y)
			r := uint8(float64(oldColor.R)*(1-progress) + float64(newColor.R)*progress)
			g := uint8(float64(oldColor.G)*(1-progress) + float64(newColor.G)*progress)
			b := uint8(float64(oldColor.B)*(1-progress) + float64(newColor.B)*progress)
			a := uint8(float64(oldColor.A)*(1-progress) + float64(newColor.A)*progress)
			result.SetRGBA(x, y, color.RGBA{R: r, G: g, B: b, A: a})
		}
	}
	return result
}

// renderSlide creates a slide transition between two frames.
// direction: -1 for left, 1 for right
func (ts *TransitionState) renderSlide(progress float64, direction int) *image.RGBA {
	result := image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight))
	offset := int(float64(DisplayWidth) * progress * float64(direction))

	// Draw old frame sliding out.
	oldRect := image.Rect(offset, 0, DisplayWidth+offset, DisplayHeight)
	if oldRect.Min.X < DisplayWidth && oldRect.Max.X > 0 {
		srcOffsetX := 0
		if oldRect.Min.X < 0 {
			srcOffsetX = -oldRect.Min.X
			oldRect.Min.X = 0
		}
		if oldRect.Max.X > DisplayWidth {
			oldRect.Max.X = DisplayWidth
		}
		draw.Draw(result, oldRect, ts.OldFrame, image.Point{X: srcOffsetX, Y: 0}, draw.Src)
	}

	// Draw new frame sliding in.
	newRect := image.Rect(offset-DisplayWidth*direction, 0, DisplayWidth+offset-DisplayWidth*direction, DisplayHeight)
	if newRect.Min.X < DisplayWidth && newRect.Max.X > 0 {
		srcOffsetX := 0
		if newRect.Min.X < 0 {
			srcOffsetX = -newRect.Min.X
			newRect.Min.X = 0
		}
		if newRect.Max.X > DisplayWidth {
			newRect.Max.X = DisplayWidth
		}
		draw.Draw(result, newRect, ts.NewFrame, image.Point{X: srcOffsetX, Y: 0}, draw.Src)
	}

	return result
}

// easeInOutCubic applies an easing function for timed (non-manual) transitions.
func easeInOutCubic(t float64) float64 {
	if t < 0.5 {
		return 4 * t * t * t
	}
	return 1 - (-2*t+2)*(-2*t+2)*(-2*t+2)/2
}
