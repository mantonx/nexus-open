package zone

import (
	"image"
	"image/color"
	"image/draw"
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

// TransitionState tracks the current state of a page transition.
type TransitionState struct {
	Active          bool
	Type            TransitionType
	StartTime       time.Time
	Duration        time.Duration
	OldFrame        *image.RGBA // Previous page frame
	NewFrame        *image.RGBA // New page frame
	Direction       int         // 1 for forward, -1 for backward
	manual          bool
	manualProgress  float64
	manualAnimating bool
	manualFrom      float64
	manualTo        float64
	manualDuration  time.Duration
	manualStart     time.Time
}

// NewTransitionState creates a new transition state.
func NewTransitionState() *TransitionState {
	return &TransitionState{
		Active:   false,
		Duration: 100 * time.Millisecond, // Very snappy 100ms transitions
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
	ts.manualAnimating = false
}

// StartManual begins a transition that will be driven by explicit progress updates.
func (ts *TransitionState) StartManual(transType TransitionType, oldFrame, newFrame *image.RGBA, direction int) {
	ts.Start(transType, oldFrame, newFrame, direction)
	ts.manual = true
	ts.manualProgress = 0
	ts.manualAnimating = false
}

// SetManualProgress updates the progress for a manually controlled transition.
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
	ts.manualAnimating = false
	ts.manualProgress = progress
	if progress >= 1 {
		ts.manual = false
		ts.Active = false
	}
}

// FinalizeManual disables manual control and maps the current progress onto a timed transition.
func (ts *TransitionState) FinalizeManual(duration time.Duration) {
	if !ts.Active {
		return
	}
	ts.AnimateManualTo(1, duration)
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

// AnimateManualTo animates manual progress towards a target over the specified duration.
func (ts *TransitionState) AnimateManualTo(target float64, duration time.Duration) {
	if !ts.Active {
		return
	}
	if target < 0 {
		target = 0
	} else if target > 1 {
		target = 1
	}

	ts.manual = true
	ts.manualAnimating = false

	if duration <= 0 {
		ts.manualProgress = target
		if target <= 0 || target >= 1 {
			ts.manual = false
			ts.Active = false
			if target >= 1 {
				ts.OldFrame = ts.NewFrame
			} else {
				ts.NewFrame = ts.OldFrame
			}
		}
		return
	}

	ts.manualAnimating = true
	ts.manualFrom = ts.manualProgress
	ts.manualTo = target
	ts.manualDuration = duration
	ts.manualStart = time.Now()
}

// GetProgress returns the transition progress (0.0 to 1.0).
func (ts *TransitionState) GetProgress() float64 {
	if !ts.Active {
		return 1.0
	}

	if ts.manual {
		if ts.manualAnimating {
			if ts.manualDuration <= 0 {
				ts.manualProgress = ts.manualTo
				ts.manualAnimating = false
			} else {
				frac := float64(time.Since(ts.manualStart)) / float64(ts.manualDuration)
				if frac >= 1 {
					ts.manualProgress = ts.manualTo
					ts.manualAnimating = false
				} else {
					// easeOutCubic for the finalize snap: starts fast (continuing
				// finger momentum) and decelerates smoothly to a stop.
				eased := easeOutCubic(frac)
				ts.manualProgress = ts.manualFrom + (ts.manualTo-ts.manualFrom)*eased
				}
			}
		}

		if !ts.manualAnimating {
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

	elapsed := time.Since(ts.StartTime)
	progress := float64(elapsed) / float64(ts.Duration)

	if progress >= 1.0 {
		ts.Active = false
		return 1.0
	}

	return progress
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
	// Manual transitions: GetProgress returns the raw finger position (drag)
	// or an already-eased value (finalize snap via AnimateManualTo). Either
	// way, no additional easing is applied here — doing so would double-ease
	// the finalize snap and make it feel front-loaded and jerky.
	//
	// Timed transitions (non-manual page switches): apply easeInOutCubic for
	// a natural acceleration/deceleration arc.
	var easedProgress float64
	if ts.manual {
		// Manual / finger-driven: GetProgress() already applies easing inside
		// AnimateManualTo (finalize snap) or returns raw finger position (drag).
		// Don't double-ease here.
		easedProgress = progress
	} else {
		// Timed page switch (non-manual): apply easeInOutCubic for a natural
		// acceleration/deceleration arc.
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

	// Blend old and new frames
	for y := 0; y < DisplayHeight; y++ {
		for x := 0; x < DisplayWidth; x++ {
			oldColor := ts.OldFrame.RGBAAt(x, y)
			newColor := ts.NewFrame.RGBAAt(x, y)

			// Linear interpolation
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

	// Calculate offsets based on direction
	offset := int(float64(DisplayWidth) * progress * float64(direction))

	// Draw old frame sliding out
	oldRect := image.Rect(offset, 0, DisplayWidth+offset, DisplayHeight)
	if oldRect.Min.X < DisplayWidth && oldRect.Max.X > 0 {
		// Clip to visible bounds
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

	// Draw new frame sliding in
	newRect := image.Rect(offset-DisplayWidth*direction, 0, DisplayWidth+offset-DisplayWidth*direction, DisplayHeight)
	if newRect.Min.X < DisplayWidth && newRect.Max.X > 0 {
		// Clip to visible bounds
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

// easeOutCubic applies an ease-out easing function for snappy transitions.
// Starts fast and decelerates towards the end.
func easeOutCubic(t float64) float64 {
	return 1 - (1-t)*(1-t)*(1-t)
}

// easeInOutCubic applies an easing function to progress for smoother transitions.
func easeInOutCubic(t float64) float64 {
	if t < 0.5 {
		return 4 * t * t * t
	}
	return 1 - (-2*t+2)*(-2*t+2)*(-2*t+2)/2
}
