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
	Active      bool
	Type        TransitionType
	StartTime   time.Time
	Duration    time.Duration
	OldFrame    *image.RGBA // Previous page frame
	NewFrame    *image.RGBA // New page frame
	Direction   int         // 1 for forward, -1 for backward
}

// NewTransitionState creates a new transition state.
func NewTransitionState() *TransitionState {
	return &TransitionState{
		Active:   false,
		Duration: 300 * time.Millisecond, // Default 300ms transition
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
}

// GetProgress returns the transition progress (0.0 to 1.0).
func (ts *TransitionState) GetProgress() float64 {
	if !ts.Active {
		return 1.0
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

	switch ts.Type {
	case TransitionFade:
		return ts.renderFade(progress)
	case TransitionSlideLeft:
		return ts.renderSlide(progress, -1)
	case TransitionSlideRight:
		return ts.renderSlide(progress, 1)
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

// easeInOutCubic applies an easing function to progress for smoother transitions.
func easeInOutCubic(t float64) float64 {
	if t < 0.5 {
		return 4 * t * t * t
	}
	return 1 - (-2*t+2)*(-2*t+2)*(-2*t+2)/2
}
