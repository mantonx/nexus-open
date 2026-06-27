package touch

import "testing"

func cfg() SwipeConfig { return DefaultSwipeConfig() }

func TestShouldCommit_AutoCommit(t *testing.T) {
	c := cfg()
	ok, reason := c.shouldCommitSwipe(c.DistanceAutoCommit, 0)
	if !ok {
		t.Errorf("at auto-commit threshold: expected commit, got cancel (%s)", reason)
	}
}

func TestShouldCommit_AutoCancel(t *testing.T) {
	c := cfg()
	// Just below DistanceAutoCancel — must always cancel regardless of velocity.
	ok, reason := c.shouldCommitSwipe(c.DistanceAutoCancel-0.001, 9999)
	if ok {
		t.Errorf("below auto-cancel threshold with high velocity: expected cancel, got commit (%s)", reason)
	}
}

func TestShouldCommit_StrongFlick(t *testing.T) {
	c := cfg()
	// Velocity above VelocityFlick, distance just above DistanceFlick.
	ok, reason := c.shouldCommitSwipe(c.DistanceFlick+0.001, c.VelocityFlick+1)
	if !ok {
		t.Errorf("strong flick: expected commit, got cancel (%s)", reason)
	}
}

func TestShouldCommit_FastFlick(t *testing.T) {
	c := cfg()
	ok, reason := c.shouldCommitSwipe(c.DistanceFastFlick+0.001, c.VelocityFastFlick+1)
	if !ok {
		t.Errorf("fast flick: expected commit, got cancel (%s)", reason)
	}
}

func TestShouldCommit_SlowDragAboveThreshold(t *testing.T) {
	c := cfg()
	ok, reason := c.shouldCommitSwipe(c.DistanceSlowDrag+0.001, c.VelocityMedium-1)
	if !ok {
		t.Errorf("slow drag above threshold: expected commit, got cancel (%s)", reason)
	}
}

func TestShouldCommit_SlowDragBelowThreshold(t *testing.T) {
	c := cfg()
	ok, reason := c.shouldCommitSwipe(c.DistanceSlowDrag-0.001, c.VelocityMedium-1)
	if ok {
		t.Errorf("slow drag below threshold: expected cancel, got commit (%s)", reason)
	}
}

func TestShouldCommit_StandardVelocityAbove(t *testing.T) {
	c := cfg()
	// Medium velocity, distance above DistanceStandard.
	ok, reason := c.shouldCommitSwipe(c.DistanceStandard+0.001, (c.VelocityMedium+c.VelocityFastFlick)/2)
	if !ok {
		t.Errorf("standard velocity above threshold: expected commit, got cancel (%s)", reason)
	}
}

func TestShouldCommit_StandardVelocityBelow(t *testing.T) {
	c := cfg()
	// Medium velocity, distance below DistanceStandard (but above auto-cancel).
	dist := (c.DistanceAutoCancel + c.DistanceStandard) / 2
	ok, reason := c.shouldCommitSwipe(dist, (c.VelocityMedium+c.VelocityFastFlick)/2)
	if ok {
		t.Errorf("standard velocity below threshold: expected cancel, got commit (%s)", reason)
	}
}

// TestShouldCommit_ReasonStrings validates that every branch returns a non-empty
// reason string — used in log output.
func TestShouldCommit_ReasonStrings(t *testing.T) {
	c := cfg()
	cases := []struct{ progress, velocity float32 }{
		{c.DistanceAutoCommit + 0.01, 0},
		{c.DistanceAutoCancel - 0.01, 0},
		{c.DistanceFlick + 0.01, c.VelocityFlick + 1},
		{c.DistanceFastFlick + 0.01, c.VelocityFastFlick + 1},
		{c.DistanceSlowDrag + 0.01, c.VelocityMedium - 1},
		{c.DistanceSlowDrag - 0.01, c.VelocityMedium - 1},
		{c.DistanceStandard + 0.01, (c.VelocityMedium + c.VelocityFastFlick) / 2},
		{c.DistanceStandard - 0.01, (c.VelocityMedium + c.VelocityFastFlick) / 2},
	}
	for _, tc := range cases {
		_, reason := c.shouldCommitSwipe(tc.progress, tc.velocity)
		if reason == "" {
			t.Errorf("shouldCommitSwipe(%.3f, %.0f) returned empty reason", tc.progress, tc.velocity)
		}
	}
}
