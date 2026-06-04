# Natural Swipe Improvement Project Plan

## Overview
Improve swipe gesture behavior to feel natural and responsive, eliminating the "buggy" feeling reported by users. Current implementation uses a simple 30% distance threshold, which doesn't account for user intent signals like velocity and momentum.

## Current Issues
1. ❌ Hard 30% threshold feels arbitrary and unnatural
2. ❌ Fast flicks don't commit unless you drag far enough
3. ❌ Slow drags past threshold commit even if user is hesitant
4. ❌ No physics-based animation feedback
5. ❌ Snap-back feels jarring and instant

## Goals
- ✅ Make fast flicks commit with less distance (like iOS/Android)
- ✅ Make slow drags require more commitment distance
- ✅ Add physics-based spring animations for snap-back
- ✅ Momentum-based completion animations
- ✅ Predictable and tunable behavior

---

## Phase 1: Core Velocity-Based Heuristics

### 1.1 Add Velocity Tracking
**Files**: `internal/touch/event.go`, `internal/touch/reader.go`

**Changes**:
- [x] Add `Velocity float32` field to Event struct
- [ ] Calculate velocity (pixels/second) at swipe release
- [ ] Log velocity for debugging and tuning

**Acceptance Criteria**:
- Velocity is calculated as `abs(dx) / duration.Seconds()`
- Logged in "swipe complete" message
- Available in handleSwipeComplete

---

### 1.2 Create SwipeConfig
**Files**: `internal/touch/handler.go` (new file: `swipe_config.go`)

**Changes**:
- [ ] Create `SwipeConfig` struct with tunable parameters
- [ ] Add sensible defaults based on research
- [ ] Make config injectable into Handler

**Parameters**:
```go
type SwipeConfig struct {
    // Velocity thresholds (pixels/second)
    VelocityFastFlick   float32 // 800  - fast flick commits with less distance
    VelocityMedium      float32 // 400  - boundary for standard behavior

    // Distance thresholds (fraction of screen width 0.0-1.0)
    DistanceAutoCancel  float32 // 0.15 - below this always cancels
    DistanceAutoCommit  float32 // 0.50 - above this always commits
    DistanceStandard    float32 // 0.30 - standard threshold (medium velocity)
    DistanceFastFlick   float32 // 0.20 - with fast velocity
    DistanceSlowDrag    float32 // 0.40 - with slow velocity
}
```

**Defaults** (to start, will tune based on testing):
- VelocityFastFlick: 800 px/s
- VelocityMedium: 400 px/s
- DistanceAutoCancel: 0.15 (15%)
- DistanceAutoCommit: 0.50 (50%)
- DistanceStandard: 0.30 (30%)
- DistanceFastFlick: 0.20 (20%)
- DistanceSlowDrag: 0.40 (40%)

**Acceptance Criteria**:
- Config struct compiles
- Default values set
- Can be passed to Handler

---

### 1.3 Implement Multi-Heuristic Decision Algorithm
**Files**: `internal/touch/handler.go`

**Changes**:
- [ ] Replace simple threshold check with intelligent decision function
- [ ] Add `shouldCommitSwipe(progress, velocity, config) bool`
- [ ] Update handleSwipeComplete to use new logic
- [ ] Add detailed logging for debugging decisions

**Algorithm**:
```go
func shouldCommitSwipe(progress, velocity float32, cfg SwipeConfig) bool {
    // Auto-commit zone: dragged past halfway
    if progress >= cfg.DistanceAutoCommit {
        return true
    }

    // Auto-cancel zone: barely moved
    if progress < cfg.DistanceAutoCancel {
        return false
    }

    // Fast flick: high velocity with moderate distance
    if velocity >= cfg.VelocityFastFlick && progress >= cfg.DistanceFastFlick {
        return true
    }

    // Slow drag: requires more distance to show commitment
    if velocity < cfg.VelocityMedium {
        return progress >= cfg.DistanceSlowDrag
    }

    // Medium velocity: standard threshold
    return progress >= cfg.DistanceStandard
}
```

**Acceptance Criteria**:
- Algorithm compiles and integrates into handleSwipeComplete
- Logs show which heuristic triggered the decision
- Fast flicks commit with less distance
- Slow drags need more distance

---

## Phase 2: Physics-Based Animations

### 2.1 Velocity-Aware Completion Animation
**Files**: `internal/zone/manager.go`, `internal/touch/handler.go`

**Changes**:
- [ ] Pass velocity to NextPage/PrevPage (or via context)
- [ ] Calculate animation duration based on velocity
- [ ] Faster swipes = faster completion animation
- [ ] Slower swipes = standard animation

**Formula**:
```
baseDuration = 100ms
velocityFactor = clamp(velocity / 1000, 0.5, 2.0)
actualDuration = baseDuration / velocityFactor
// Fast swipe (1000 px/s): 50-100ms
// Slow swipe (200 px/s): 100-150ms
```

**Acceptance Criteria**:
- Fast flicks complete faster
- Animation feels like it carries momentum
- No jarring jumps

---

### 2.2 Spring-Damped Snap-Back Animation
**Files**: `internal/zone/manager.go`

**Changes**:
- [ ] Replace instant snap-back with spring animation
- [ ] Add configurable spring parameters (stiffness, damping)
- [ ] Implement spring physics in CancelLiveSwipe
- [ ] Duration based on how far user dragged

**Spring Parameters**:
```go
type SpringConfig struct {
    Stiffness float32 // 300 - how "tight" the spring is
    Damping   float32 // 0.7 - how much bounce (0=lots, 1=none)
}
```

**Acceptance Criteria**:
- Snap-back has smooth deceleration curve
- Feels like a gentle "pull back" not a snap
- Duration proportional to distance dragged (further = longer)

---

## Phase 3: Advanced Polish

### 3.1 Edge Case: First/Last Page Bounce
**Files**: `internal/zone/manager.go`, `internal/touch/handler.go`

**Changes**:
- [ ] Detect swipes at first/last page boundaries
- [ ] Show visual rubber-band effect (slight overscroll)
- [ ] Snap back with spring animation
- [ ] Different feel from cancellation (more resistance)

**Acceptance Criteria**:
- Swiping right on first page shows resistance
- Swiping left on last page shows resistance
- Visual feedback is clear but not jarring

---

### 3.2 Velocity Smoothing
**Files**: `internal/touch/reader.go`

**Changes**:
- [ ] Add moving average filter for velocity
- [ ] Track last N samples (3-5) of instantaneous velocity
- [ ] Use averaged velocity for decision making
- [ ] Prevents noise spikes from triggering false fast-flicks

**Acceptance Criteria**:
- Velocity is stable and not jittery
- Still responsive to real fast flicks
- No false positives from touch noise

---

### 3.3 Direction Change Handling
**Files**: `internal/touch/reader.go`

**Changes**:
- [ ] Detect when user changes swipe direction mid-gesture
- [ ] Reset velocity calculation on direction change
- [ ] Prevents accidental commits from back-and-forth motion

**Acceptance Criteria**:
- Swiping left then right cancels properly
- Doesn't accumulate velocity across direction changes

---

## Phase 4: Testing & Tuning

### 4.1 Device Testing
**Tasks**:
- [ ] Test fast flicks at various speeds
- [ ] Test slow drags at boundaries
- [ ] Test cancellations at various distances
- [ ] Test edge cases (first/last page)
- [ ] Get subjective feedback on "feel"

### 4.2 Parameter Tuning
**Tasks**:
- [ ] Adjust velocity thresholds based on real usage
- [ ] Adjust distance thresholds for best feel
- [ ] Tune spring parameters for snap-back
- [ ] Tune animation durations

### 4.3 Logging Analysis
**Tasks**:
- [ ] Collect logs of swipe events during testing
- [ ] Analyze velocity distribution
- [ ] Find edge cases where decisions feel wrong
- [ ] Iterate on thresholds

---

## Implementation Order

### Sprint 1: Foundation (1-2 days)
1. ✅ Add Velocity field to Event (done)
2. Calculate velocity in reader
3. Create SwipeConfig struct
4. Implement decision algorithm
5. Test basic velocity-based behavior

### Sprint 2: Polish (1-2 days)
6. Add velocity-aware completion animations
7. Implement spring snap-back
8. Add detailed logging
9. Initial device testing

### Sprint 3: Advanced (1 day)
10. Edge case handling (first/last page)
11. Velocity smoothing
12. Direction change detection

### Sprint 4: Tuning (ongoing)
13. Collect usage data
14. Tune parameters
15. Iterate based on feedback

---

## Success Metrics

### Qualitative
- ✅ Swipes feel natural and responsive
- ✅ Fast flicks work reliably
- ✅ Slow drags are predictable
- ✅ Cancellations feel smooth, not jarring
- ✅ No "buggy" feeling reported

### Quantitative
- ✅ <5% false commits (intended cancel, actually committed)
- ✅ <5% false cancels (intended commit, actually cancelled)
- ✅ Average decision time <10ms
- ✅ Animation frame rate stays at 30 FPS

---

## Risk Mitigation

### Risk: Over-tuned parameters feel worse
**Mitigation**: Keep old behavior as fallback, use feature flag

### Risk: Velocity calculation is noisy
**Mitigation**: Add smoothing, use multiple samples

### Risk: Different users have different preferences
**Mitigation**: Make config user-settable (future enhancement)

### Risk: Performance impact from complex calculations
**Mitigation**: Profile, optimize if needed, decision logic is O(1)

---

## Future Enhancements (Post-Launch)

- [ ] User-configurable swipe sensitivity
- [ ] Haptic feedback on swipe commit/cancel (if hardware supports)
- [ ] Multi-finger gestures (pinch, two-finger swipe)
- [ ] Position-aware swipes (swipe on specific zones)
- [ ] Gesture recording for debugging
- [ ] A/B testing framework for tuning

---

## References

- iOS Human Interface Guidelines: Gestures
- Android Material Design: Gestures
- Research paper: "One-Euro Filter" (already implemented for position)
- Flutter PageView physics implementation
