package zone

import (
	"image"
	"image/draw"
	"time"

	"github.com/fogleman/gg"
	"github.com/mantonx/nexus-open/internal/design"
)

const (
	rippleDuration  = 350 * time.Millisecond
	rippleMaxRadius = 26.0 // px at full expansion
	rippleRingWidth = 3.0
	rippleGlowSteps = 8.0
)

// TapRipple tracks a single in-flight tap ripple animation.
type TapRipple struct {
	active    bool
	startedAt time.Time
	cx        float64 // centre x in display coordinates
	cy        float64 // centre y — fixed to vertical midpoint of zone
}

// rippleHoldMin is the minimum time ShowDetail waits after a ripple starts,
// so at least a few frames of the ring are visible before the detail slides in.
const rippleHoldMin = 120 * time.Millisecond

// waitForRipple sleeps until the ripple has been on screen for rippleHoldMin.
// Returns immediately if no ripple is active.
func (m *Manager) waitForRipple() {
	m.rippleMu.Lock()
	if !m.ripple.active {
		m.rippleMu.Unlock()
		return
	}
	elapsed := time.Since(m.ripple.startedAt)
	m.rippleMu.Unlock()
	if remaining := rippleHoldMin - elapsed; remaining > 0 {
		time.Sleep(remaining)
	}
}

// StartTapRipple begins a ripple animation centred on the tapped x coordinate.
// Safe to call from any goroutine.
func (m *Manager) StartTapRipple(tapX int) {
	m.rippleMu.Lock()
	m.ripple = TapRipple{
		active:    true,
		startedAt: time.Now(),
		cx:        float64(tapX),
		cy:        float64(design.DisplayHeight) / 2,
	}
	m.rippleMu.Unlock()

	// Mark the frame dirty so the render loop picks up the animation immediately.
	m.lastFrameMu.Lock()
	m.frameDirty = true
	m.lastFrameMu.Unlock()
}

// compositeRipple blits the current ripple frame onto dst, if one is active.
// Returns true while the animation is still running (caller should keep dirty).
func (m *Manager) compositeRipple(dst *image.RGBA) bool {
	m.rippleMu.Lock()
	r := m.ripple
	m.rippleMu.Unlock()

	if !r.active {
		return false
	}

	elapsed := time.Since(r.startedAt)
	if elapsed >= rippleDuration {
		m.rippleMu.Lock()
		m.ripple.active = false
		m.rippleMu.Unlock()
		return false
	}

	t := float64(elapsed) / float64(rippleDuration) // 0→1 over lifetime

	// Ease out: fast expansion, slow fade.
	eased := 1 - (1-t)*(1-t)
	radius := rippleMaxRadius * eased

	// Opacity: full at t=0.15, fades to 0 at t=1.
	var alpha float64
	if t < 0.15 {
		alpha = t / 0.15
	} else {
		alpha = 1.0 - (t-0.15)/0.85
	}

	overlay := image.NewRGBA(dst.Bounds())
	ov := gg.NewContextForRGBA(overlay)

	ar := float64(design.Info.R) / 255
	ag := float64(design.Info.G) / 255
	ab := float64(design.Info.B) / 255

	// Outer glow — concentric stroked rings fading outward.
	for step := 0.0; step < rippleGlowSteps; step++ {
		gt := step / rippleGlowSteps
		ov.SetRGBA(ar, ag, ab, alpha*0.22*(1-gt))
		ov.SetLineWidth(1.5)
		ov.DrawCircle(r.cx, r.cy, radius+step*1.2)
		ov.Stroke()
	}

	// Main ring stroke.
	ov.SetRGBA(ar, ag, ab, alpha*0.85)
	ov.SetLineWidth(rippleRingWidth)
	ov.DrawCircle(r.cx, r.cy, radius)
	ov.Stroke()

	draw.Draw(dst, dst.Bounds(), overlay, image.Point{}, draw.Over)
	return true
}
