package api

import (
	"encoding/json"
	"image/png"
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
			_ = s.swipeSim.UpdateLiveSwipe(progress, isLeft)
			time.Sleep(stepInterval)
		}
		// Finalize at release_at progress — matches real finger lifting mid-swipe.
		releaseProgress := float32(releaseStep) / float32(req.Steps)
		_ = s.swipeSim.FinalizeLiveSwipe(releaseProgress, req.Velocity, isLeft)
	}()

	s.respondSuccess(w, "Swipe simulation started", map[string]interface{}{
		"direction":   req.Direction,
		"duration_ms": req.DurationMs,
		"finalize_ms": req.FinalizeMs,
		"steps":       req.Steps,
		"velocity":    req.Velocity,
	})
}

// handleSwipeUpdate feeds a single live-swipe progress update.
// POST /api/debug/swipe/update  body: {"progress": 0.35, "is_left": true}
func (s *Server) handleSwipeUpdate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.swipeSim == nil {
		s.respondError(w, "Swipe simulator not available", http.StatusServiceUnavailable)
		return
	}
	var req struct {
		Progress float32 `json:"progress"`
		IsLeft   bool    `json:"is_left"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if err := s.swipeSim.UpdateLiveSwipe(req.Progress, req.IsLeft); err != nil {
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// handleSwipeFinalize finalises a live swipe (finger lifted).
// POST /api/debug/swipe/finalize  body: {"progress": 0.6, "velocity": 300, "is_left": true}
func (s *Server) handleSwipeFinalize(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.swipeSim == nil {
		s.respondError(w, "Swipe simulator not available", http.StatusServiceUnavailable)
		return
	}
	var req struct {
		Progress float32 `json:"progress"`
		Velocity float32 `json:"velocity"`
		IsLeft   bool    `json:"is_left"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if err := s.swipeSim.FinalizeLiveSwipe(req.Progress, req.Velocity, req.IsLeft); err != nil {
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// handleDebugTap simulates a hardware tap at a given X coordinate (0–639).
// POST /api/debug/tap  body: {"x": 320}
//
// Matches the hardware tap path exactly: dismiss detail overlay if showing,
// otherwise find the zone at x and execute its OnTap action.
func (s *Server) handleDebugTap(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.zoneTapper == nil {
		s.respondError(w, "Zone tapper not available", http.StatusServiceUnavailable)
		return
	}
	var req struct {
		X int `json:"x"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if req.X < 0 || req.X >= 640 {
		s.respondError(w, "x must be 0–639", http.StatusBadRequest)
		return
	}

	if s.zoneTapper.IsShowingDetail() {
		s.zoneTapper.ClearDetail()
		w.WriteHeader(http.StatusNoContent)
		return
	}

	if err := s.zoneTapper.HandleZoneTapAtX(req.X); err != nil {
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// handleDebugTapZone triggers OnTap directly for a zone by ID, bypassing the
// detail-dismiss logic. POST /api/debug/tap-zone  body: {"zone_id": "system.weather"}
func (s *Server) handleDebugTapZone(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.zoneTapper == nil {
		s.respondError(w, "Zone tapper not available", http.StatusServiceUnavailable)
		return
	}
	var req struct {
		ZoneID string `json:"zone_id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if req.ZoneID == "" {
		s.respondError(w, "zone_id is required", http.StatusBadRequest)
		return
	}
	if err := s.zoneTapper.HandleZoneTap(req.ZoneID); err != nil {
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// handleDebugRenderDetail renders the current detail overlay frame as a PNG.
// GET /api/debug/render-detail
//
// Returns 404 if no detail frame has been rendered yet (tap the weather zone first).
// Useful for visually verifying detail_overlay.go changes without needing the live app.
func (s *Server) handleDebugRenderDetail(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.detailProvider == nil {
		s.respondError(w, "Detail frame provider not available", http.StatusServiceUnavailable)
		return
	}
	frame := s.detailProvider.GetDetailFrame()
	if frame == nil {
		s.respondError(w, "No detail frame available — tap a zone with OnTap support first", http.StatusNotFound)
		return
	}
	w.Header().Set("Content-Type", "image/png")
	if err := png.Encode(w, frame); err != nil {
		s.logger.Error("render-detail: png encode failed", "error", err)
	}
}

// handleSwipeCancel cancels an in-progress live swipe.
// POST /api/debug/swipe/cancel
func (s *Server) handleSwipeCancel(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.swipeSim == nil {
		s.respondError(w, "Swipe simulator not available", http.StatusServiceUnavailable)
		return
	}
	if err := s.swipeSim.CancelLiveSwipe(); err != nil {
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
