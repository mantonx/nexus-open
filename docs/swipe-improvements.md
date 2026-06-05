# Swipe Pipeline Improvements

Tracked improvements identified after the June 2026 swipe tuning session.
Ordered by impact.

**Status as of 2026-06-05:** Items 1–4 are fully implemented. See notes below.

---

## High Impact

### 1. Velocity via trailing window ✅
**File:** `internal/touch/reader.go` — `velWindow` type

4-sample weighted average (`velWindow.velocity()`), newest sample weighted 4×.
Eliminates One-Euro directional asymmetry. Implemented.

### 2. Spring physics for the snap animation ✅
**File:** `internal/zone/transition.go` — `tickSpring()`

Semi-implicit Euler spring: stiffness=400, damping=36 (slightly under-damped).
Release velocity seeded from `FinalizeLiveSwipe`. Snap duration is emergent.
Implemented.

### 3. Direction change mid-swipe ✅
**File:** `internal/zone/manager.go` — `UpdateLiveSwipe()` lines ~1006–1030

On direction flip, swaps `OldFrame`/`NewFrame`, flips `TransitionType`, mirrors
progress around 0.5. Tested in `TestFinalizeLiveSwipeIgnoresDirectionJitter`.
Implemented.

### 4. Rubber-band at page boundaries ✅
**File:** `internal/zone/manager.go` — `UpdateLiveSwipe()` lines ~995–999

`liveSwipeBoundary` flag set on first packet. Progress scaled by `sqrt(p)*0.25`
for elastic resistance. Always cancelled on lift (never commits a boundary wrap).
Implemented.

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
