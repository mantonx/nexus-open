package api

import (
	"encoding/json"
	"net/http"
	"time"
)

// SwipeSimRequest describes a synthetic swipe to simulate.
type SwipeSimRequest struct {
	// Direction: "left" (forward) or "right" (backward). Default: "left".
	Direction string `json:"direction"`
	// DurationMs: total time for the drag phase in milliseconds. Default: 200.
	DurationMs int `json:"duration_ms"`
	// FinalizeMs: time for the snap-to-end animation after release. Default: 120.
	FinalizeMs int `json:"finalize_ms"`
	// Steps: number of incremental UpdateLiveSwipe calls during the drag. Default: 20.
	// More steps = smoother simulation of finger drag; fewer = choppier.
	Steps int `json:"steps"`
	// Velocity: finger velocity at release in pixels/second. Typical real swipes
	// range from ~120 px/s (slow drag) to ~500 px/s (fast flick). Default: 150.
	Velocity float32 `json:"velocity"`
	// ReleaseAt: progress (0–1) at which the finger is "lifted", triggering finalize.
	// Real swipes typically release at 0.5–0.8. Default: 0.7.
	ReleaseAt float32 `json:"release_at"`
}

// handleDebugSwipe drives a synthetic swipe through the live transition pipeline.
// POST /api/debug/swipe
//
// This exercises exactly the same code path as a real hardware touch swipe:
// UpdateLiveSwipe (drag phase) → FinalizeLiveSwipe (release + snap).
// Watch the result in the Flutter UI's preview tab or via the WS frame stream.
func (s *Server) handleDebugSwipe(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.swipeSim == nil {
		s.respondError(w, "Swipe simulator not available", http.StatusServiceUnavailable)
		return
	}

	var req SwipeSimRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	// Apply defaults
	if req.Direction == "" {
		req.Direction = "left"
	}
	if req.DurationMs <= 0 {
		req.DurationMs = 200
	}
	if req.FinalizeMs <= 0 {
		req.FinalizeMs = 120
	}
	if req.Steps <= 0 {
		req.Steps = 20
	}
	if req.Velocity <= 0 {
		req.Velocity = 150 // typical slow-drag velocity in px/s
	}
	if req.ReleaseAt <= 0 || req.ReleaseAt > 1 {
		req.ReleaseAt = 0.7
	}

	isLeft := req.Direction != "right"
	releaseStep := int(float32(req.Steps) * req.ReleaseAt)
	if releaseStep < 1 {
		releaseStep = 1
	}
	stepInterval := time.Duration(req.DurationMs) * time.Millisecond / time.Duration(req.Steps)

	// Run the swipe in a goroutine so the HTTP response returns immediately.
	go func() {
		// Drag phase: feed incremental progress up to release_at, then finalize.
		for i := 1; i <= releaseStep; i++ {
			progress := float32(i) / float32(req.Steps)
			s.swipeSim.UpdateLiveSwipe(progress, isLeft)
			time.Sleep(stepInterval)
		}
		// Finalize at release_at progress — matches real finger lifting mid-swipe.
		releaseProgress := float32(releaseStep) / float32(req.Steps)
		s.swipeSim.FinalizeLiveSwipe(releaseProgress, req.Velocity, isLeft)
	}()

	s.respondSuccess(w, "Swipe simulation started", map[string]interface{}{
		"direction":   req.Direction,
		"duration_ms": req.DurationMs,
		"finalize_ms": req.FinalizeMs,
		"steps":       req.Steps,
		"velocity":    req.Velocity,
	})
}
