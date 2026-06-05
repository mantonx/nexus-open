# Swipe Pipeline Improvements

Tracked improvements identified after the June 2026 swipe tuning session.
Ordered by impact.

---

## High Impact

### 1. Velocity via trailing window
**File:** `internal/touch/reader.go:240-252`

Release velocity currently uses `max(instantaneous, average)` — a workaround for
the One-Euro filter's directional asymmetry (left flicks read ~2000 px/s, right
~400-600). The root fix is a proper 4-sample weighted average of the last few HID
packets before finger lift, weighted toward the most recent sample. This is how
iOS calculates release velocity and it would eliminate the asymmetry without the
max() hack.

### 2. Spring physics for the snap animation
**File:** `internal/zone/transition.go:108-140` (`AnimateManualTo`)

The finalize snap uses a fixed-duration `easeOutCubic` from current position to
1.0. A spring model (`velocity += (target - pos) * stiffness - velocity * damping;
pos += velocity * dt`) would carry the finger's actual momentum into the snap and
decelerate organically. This is the single biggest felt difference between our
transitions and iOS — content feels like it has mass.

### 3. Direction change mid-swipe
**File:** `internal/zone/manager.go:820` (`UpdateLiveSwipe`)

`liveSwipeLeft` locks to the direction at first touch and never updates. If the
user starts swiping right then reverses before lifting, we commit to the wrong
page. Should update direction from the current signed `dx` on each
`UpdateLiveSwipe` call (while keeping the original direction for the "started"
frame setup).

### 4. Rubber-band at page boundaries
**File:** `internal/zone/manager.go:840` (`UpdateLiveSwipe`)

Swiping right on page 0 (or left on the last page) currently wraps to the other
end silently. Should instead show elastic resistance: scale reported progress by a
decay factor (e.g. `resistance = progress * 0.3`) and always cancel on lift,
never committing a boundary-wrap swipe.

---

## Medium Impact

### 5. Cache miss stall on first swipe
**File:** `internal/zone/manager.go:848-865` (`UpdateLiveSwipe`)

If the page cache is cold when a swipe starts, `renderPageFrame()` is called
synchronously, blocking the live swipe goroutine for ~50-200ms. The display
freezes mid-drag. Should render the target frame asynchronously and show the
old frame until it's ready, then swap in the target frame once available.

### 6. Snap duration magic constants
**File:** `internal/zone/manager.go:1069-1095` (`FinalizeLiveSwipe`)

`minDuration`, `distanceStretch`, and the velocity multipliers (0.88, 0.94) are
empirically tuned with no physical model. Spring physics (item 2) would replace
this block entirely — duration becomes an emergent property of stiffness and
damping, not a hardcoded lookup.

### 7. One-Euro filter beta tuning
**File:** `internal/touch/reader.go:108-113`

`beta=0.007` was never tuned for this device — it's a generic default. A higher
beta (0.02-0.05) would make the filter track fast movements more aggressively,
reducing velocity undershoot on fast flicks. Should be validated against real
hardware samples at different swipe speeds.

---

## Low Impact / Correctness

### 8. Cooldown uses wall clock instead of gesture state
**File:** `internal/zone/manager.go:813` (`UpdateLiveSwipe`)

The 250ms post-finalize cooldown that blocks tail HID packets is timing-based.
The correct fix is tracking whether the hardware finger-down flag is still active
in the reader and propagating that as gesture state, so tail packets are blocked
by actual lift state rather than an arbitrary timer.

### 9. Validate `rawMax` against actual device
**File:** `internal/touch/reader.go:107`

`rawMax=1023` has a comment "1023 or 4095". If the device reports 12-bit
coordinates, the normalization scales everything wrong and all position/velocity
readings are off by ~4×. Should log the actual raw max value seen from the device
and verify against the HID descriptor.
