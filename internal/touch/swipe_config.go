package touch

// SwipeConfig contains tunable parameters for intelligent swipe gesture detection.
// These parameters control the multi-heuristic decision making that determines
// whether a swipe should commit to a page change or cancel back to the current page.
type SwipeConfig struct {
	// Velocity thresholds (pixels/second)
	// These determine how swipe velocity affects the distance threshold

	// VelocityFastFlick is the velocity threshold for "fast flick" behavior.
	// Swipes faster than this can commit with less distance traveled.
	// Default: 800 px/s (based on typical flick gestures)
	VelocityFastFlick float32

	// VelocityMedium is the boundary between medium and slow velocity.
	// Below this threshold, swipes are considered "slow drags" requiring more distance.
	// Default: 400 px/s
	VelocityMedium float32

	// Distance thresholds (fraction of screen width, 0.0 to 1.0)
	// These determine how far the user must swipe for various behaviors

	// DistanceAutoCancel is the minimum threshold below which swipes always cancel,
	// regardless of velocity. This prevents accidental triggers.
	// Default: 0.15 (15% of screen width ≈ 96px on 640px screen)
	DistanceAutoCancel float32

	// DistanceAutoCommit is the threshold above which swipes always commit,
	// regardless of velocity. Dragging past halfway shows clear intent.
	// Default: 0.50 (50% of screen width ≈ 320px on 640px screen)
	DistanceAutoCommit float32

	// DistanceStandard is the threshold for medium-velocity swipes.
	// This is the "normal" swipe distance for typical gestures.
	// Default: 0.30 (30% of screen width ≈ 192px on 640px screen)
	DistanceStandard float32

	// DistanceFastFlick is the reduced threshold for high-velocity swipes.
	// Fast flicks feel natural with less distance traveled.
	// Default: 0.20 (20% of screen width ≈ 128px on 640px screen)
	DistanceFastFlick float32

	// DistanceSlowDrag is the increased threshold for low-velocity swipes.
	// Slow drags require more distance to show commitment and prevent accidents.
	// Default: 0.40 (40% of screen width ≈ 256px on 640px screen)
	DistanceSlowDrag float32
}

// DefaultSwipeConfig returns a SwipeConfig with sensible defaults based on
// iOS/Android gesture behavior research and tuned for 640px Nexus screen.
// Note: Velocity thresholds are calibrated for the smaller screen size - actual
// swipe velocities on this device range from ~150-400 px/s for typical gestures.
func DefaultSwipeConfig() SwipeConfig {
	return SwipeConfig{
		// Velocity thresholds (tuned for 640px screen)
		// Velocity thresholds calibrated to corrected rawMax=486 px/s readings.
		// Typical flicks now read 500–5000 px/s; slow drags ~50–300 px/s.
		VelocityFastFlick: 800,  // Fast flick — commit with less distance above this
		VelocityMedium:    300,  // Boundary for slow vs medium

		// Distance thresholds — now correct relative to true screen width.
		DistanceAutoCancel: 0.05, // Always cancel below 5% (~32px) — clear non-intent
		DistanceAutoCommit: 0.40, // Always commit above 40% — clear intent
		DistanceStandard:   0.15, // Normal threshold (~96px)
		DistanceFastFlick:  0.07, // Fast flick threshold (~45px) — short flick commits
		DistanceSlowDrag:   0.20, // Slow drag threshold (~128px)
	}
}

// shouldCommitSwipe determines whether a swipe gesture should commit to a page change
// or cancel back to the current page, based on multiple heuristics.
//
// The decision algorithm considers:
// 1. Distance traveled (how far the user swiped)
// 2. Velocity (how fast the user swiped)
// 3. Auto-commit/cancel zones (clear intent zones)
//
// Returns true if the swipe should commit (change pages), false if it should cancel.
func (c *SwipeConfig) shouldCommitSwipe(progress, velocity float32) (bool, string) {
	// Auto-commit zone: dragged past halfway shows clear intent
	if progress >= c.DistanceAutoCommit {
		return true, "auto-commit (>50%)"
	}

	// Auto-cancel zone: barely moved, likely accidental or changed mind
	if progress < c.DistanceAutoCancel {
		return false, "auto-cancel (<15%)"
	}

	// Fast flick: high velocity with moderate distance
	// This matches iOS/Android behavior where quick flicks commit easily
	if velocity >= c.VelocityFastFlick && progress >= c.DistanceFastFlick {
		return true, "fast-flick"
	}

	// Slow drag: requires more distance to show commitment
	// Prevents accidental commits from slow, hesitant drags
	if velocity < c.VelocityMedium {
		result := progress >= c.DistanceSlowDrag
		if result {
			return true, "slow-drag-commit"
		}
		return false, "slow-drag-cancel"
	}

	// Medium velocity: standard threshold
	// Normal swipe behavior for typical gesture speed
	result := progress >= c.DistanceStandard
	if result {
		return true, "standard-commit"
	}
	return false, "standard-cancel"
}
