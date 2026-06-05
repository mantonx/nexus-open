package zone

import (
	"image"
	"math"
	"time"
)

// UpdateLiveSwipe updates the live swipe progress for interactive transitions.
// Called continuously while the user's finger is moving.
func (m *Manager) UpdateLiveSwipe(progress float32, isLeft bool) error {
	rawProgress := progress
	if progress < 0 {
		progress = 0
	} else if progress > 1 {
		progress = 1
	}

	m.transitionMu.RLock()
	transitionActive := m.transition.Active
	var transitionProgress float64
	if transitionActive && m.transition.Duration > 0 {
		elapsed := time.Since(m.transition.StartTime)
		transitionProgress = math.Min(1, float64(elapsed)/float64(m.transition.Duration))
	}
	m.transitionMu.RUnlock()

	m.liveSwipeMu.Lock()
	defer m.liveSwipeMu.Unlock()

	// Ignore stray packets within 250ms of a finalize. Fast flicks release the
	// finger while HID packets are still in-flight; without this guard the tail
	// packet restarts a new live swipe that immediately cancels.
	if !m.liveSwipeActive {
		if !m.lastSwipeFinalize.IsZero() && time.Since(m.lastSwipeFinalize) < 250*time.Millisecond {
			m.logger.Info("🚫 live swipe blocked — within cooldown",
				"since_ms", time.Since(m.lastSwipeFinalize).Milliseconds(),
				"direction", map[bool]string{true: "left", false: "right"}[isLeft])
			return nil
		}
		m.liveSwipeActive = true
		m.liveSwipeLeft = isLeft

		numPages := len(m.config.Pages)
		m.liveSwipeBoundary = (!isLeft && m.currentPage == 0) ||
			(isLeft && m.currentPage == numPages-1)
		if m.liveSwipeBoundary {
			m.logger.Debug("live swipe at boundary — rubber-band mode",
				"page", m.currentPage,
				"direction", map[bool]string{true: "left", false: "right"}[isLeft])
		}

		m.lastFrameMu.Lock()
		oldFrame := m.lastFrame
		m.lastFrameMu.Unlock()

		if oldFrame == nil {
			frame, err := m.RenderFrame()
			if err != nil {
				m.logger.Error("failed to render current frame", "error", err)
				return err
			}
			oldFrame = frame
		}

		targetPage := m.currentPage
		if isLeft {
			targetPage = (m.currentPage + 1) % len(m.config.Pages)
		} else {
			targetPage = (m.currentPage - 1 + len(m.config.Pages)) % len(m.config.Pages)
		}

		// Use cached target frame if available; otherwise render asynchronously.
		var targetFrame *image.RGBA
		m.pageCacheMu.RLock()
		cachedFrame, hasCached := m.pageCache[targetPage]
		m.pageCacheMu.RUnlock()

		var newFrame *image.RGBA
		previewActive := false
		previewThreshold := liveSwipePreviewThreshold

		if hasCached {
			targetFrame = cachedFrame
			newFrame = targetFrame
			previewActive = true
			m.logger.Debug("cached target frame ready for live swipe", "page", targetPage)
		} else {
			m.logger.Debug("cache cold — rendering target page async", "page", targetPage)
			newFrame = oldFrame // show current page until async render completes
			go func() {
				if rendered, err := m.renderPageFrame(targetPage); err == nil {
					m.pageCacheMu.Lock()
					m.pageCache[targetPage] = rendered
					m.pageCacheMu.Unlock()
					m.liveSwipeMu.Lock()
					m.liveSwipeTargetFrame = rendered
					m.liveSwipeMu.Unlock()
					m.transitionMu.Lock()
					if m.transition.Active && m.transition.IsManual() {
						m.transition.NewFrame = rendered
					}
					m.transitionMu.Unlock()
				}
			}()
		}

		m.liveSwipeTargetFrame = targetFrame
		m.liveSwipePreviewActive = previewActive

		direction := 1
		if !isLeft {
			direction = -1
		}
		transitionType := TransitionSlideLeft
		if !isLeft {
			transitionType = TransitionSlideRight
		}

		m.transitionMu.Lock()
		m.transition.StartManual(transitionType, oldFrame, newFrame, direction)
		m.transition.SetManualProgress(float64(progress))
		m.transitionMu.Unlock()

		m.logger.Debug("live swipe started",
			"progress", int(progress*100),
			"direction", map[bool]string{true: "left", false: "right"}[isLeft],
			"target_frame_ready", targetFrame != nil,
			"preview_active", previewActive,
			"preview_threshold_pct", int(previewThreshold*100))
	}

	// Rubber-band resistance at page boundaries: sqrt curve gives strong
	// resistance near zero and softens as drag extends.
	if m.liveSwipeBoundary {
		progress = float32(math.Sqrt(float64(progress)) * 0.25)
	}

	m.liveSwipeProgress = progress

	// Direction reversal mid-swipe: swap frames and flip transition type so
	// the visual direction matches the finger.
	if m.liveSwipeLeft != isLeft {
		m.liveSwipeLeft = isLeft
		m.liveSwipeTargetFrame = nil
		m.liveSwipePreviewActive = false

		m.transitionMu.Lock()
		if m.transition.Active && m.transition.IsManual() {
			m.transition.OldFrame, m.transition.NewFrame = m.transition.NewFrame, m.transition.OldFrame
			if isLeft {
				m.transition.Type = TransitionSlideLeft
				m.transition.Direction = 1
			} else {
				m.transition.Type = TransitionSlideRight
				m.transition.Direction = -1
			}
			m.transition.SetManualProgress(1 - float64(progress))
		}
		m.transitionMu.Unlock()

		m.logger.Debug("live swipe direction reversed",
			"new_direction", map[bool]string{true: "left", false: "right"}[isLeft],
			"progress_pct", int(progress*100))
	}

	previewLog := ""
	m.transitionMu.Lock()
	if m.transition.Active {
		m.transition.SetManualProgress(float64(progress))
		if m.transition.IsManual() {
			targetFrame := m.liveSwipeTargetFrame
			if targetFrame != nil {
				if !m.liveSwipePreviewActive && float64(progress) >= liveSwipePreviewThreshold {
					if m.transition.NewFrame != targetFrame {
						m.transition.NewFrame = targetFrame
					}
					m.liveSwipePreviewActive = true
					previewLog = "engaged"
				} else if m.liveSwipePreviewActive && float64(progress) < liveSwipePreviewThreshold {
					if m.transition.NewFrame != m.transition.OldFrame {
						m.transition.NewFrame = m.transition.OldFrame
					}
					m.liveSwipePreviewActive = false
					previewLog = "reverted"
				}
			}
		}
	}
	manualActive := false
	manualProgress := 0.0
	if m.transition.Active && m.transition.IsManual() {
		manualActive = true
		manualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.Unlock()

	if previewLog != "" {
		m.logger.Debug("swipe preview frame "+previewLog,
			"progress_pct", int(progress*100),
			"threshold_pct", int(liveSwipePreviewThreshold*100))
	}

	progressPct := int(transitionProgress * 100)
	if manualActive {
		progressPct = int(manualProgress * 100)
	}

	m.logger.Debug("swipe progress updated",
		"raw_progress", rawProgress,
		"clamped_progress", progress,
		"direction", map[bool]string{true: "left", false: "right"}[isLeft],
		"transition_active", transitionActive,
		"transition_progress_pct", progressPct,
		"transition_manual", manualActive)

	return nil
}

// FinalizeLiveSwipe commits an in-progress live swipe and smoothly completes
// the transition using spring physics seeded with the finger's release velocity.
func (m *Manager) FinalizeLiveSwipe(progress float32, velocity float32, isLeft bool) error {
	m.liveSwipeMu.Lock()

	if progress < 0 {
		progress = 0
	} else if progress > 1 {
		progress = 1
	}

	if !m.liveSwipeActive {
		m.logger.Info("⚠️ FinalizeLiveSwipe called but no active swipe",
			"direction", map[bool]string{true: "left", false: "right"}[isLeft],
			"progress_pct", int(progress*100))
		m.liveSwipeLeft = isLeft
		m.liveSwipeProgress = progress
		m.lastSwipeFinalize = time.Now()
		m.lastSwipeDirLeft = isLeft
		m.liveSwipeMu.Unlock()
		return nil
	}

	// Boundary swipes always cancel — never commit a wrap-around page change.
	if m.liveSwipeBoundary {
		m.liveSwipeBoundary = false
		m.liveSwipeMu.Unlock()
		return m.CancelLiveSwipe()
	}

	// Use the direction captured at swipe start to avoid jitter flipping pages.
	swipeLeft := m.liveSwipeLeft

	now := time.Now()
	var sinceLastFinalize time.Duration
	if !m.lastSwipeFinalize.IsZero() {
		sinceLastFinalize = now.Sub(m.lastSwipeFinalize)
	}
	m.lastSwipeFinalize = now
	m.lastSwipeDirLeft = swipeLeft

	m.transitionMu.RLock()
	transitionActive := m.transition.Active
	transitionType := m.transition.Type
	transitionDirection := m.transition.Direction
	transitionDuration := m.transition.Duration
	transitionStart := m.transition.StartTime
	manualActive := transitionActive && m.transition.IsManual()
	manualProgress := 0.0
	if manualActive {
		manualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	var transitionProgress float64
	if transitionDuration > 0 {
		transitionProgress = math.Min(1, float64(time.Since(transitionStart))/float64(transitionDuration))
	}

	m.liveSwipeProgress = progress
	currentProgress := progress
	m.logger.Debug("finalizing live swipe",
		"current_progress", int(currentProgress*100),
		"velocity_px_s", int(velocity),
		"direction", map[bool]string{true: "left", false: "right"}[swipeLeft],
		"since_last_finalize_ms", sinceLastFinalize.Milliseconds(),
		"transition_active", transitionActive,
		"transition_type", transitionType,
		"transition_direction", transitionDirection,
		"transition_progress_pct", int(transitionProgress*100),
		"transition_manual", manualActive,
		"transition_manual_progress_pct", int(manualProgress*100))

	var targetPage int
	if swipeLeft {
		targetPage = (m.currentPage + 1) % len(m.config.Pages)
	} else {
		targetPage = m.currentPage - 1
		if targetPage < 0 {
			targetPage = len(m.config.Pages) - 1
		}
	}

	oldPage := m.currentPage
	m.currentPage = targetPage

	m.logger.Info("✅ PAGE SWITCH (momentum)",
		"from", oldPage,
		"to", targetPage,
		"progress_pct", int(currentProgress*100),
		"velocity_px_s", int(velocity))

	m.transitionMu.Lock()
	if m.transition.Active {
		targetFrame := m.liveSwipeTargetFrame
		if targetFrame != nil && m.transition.IsManual() {
			if m.transition.NewFrame != targetFrame {
				m.transition.NewFrame = targetFrame
			}
			m.liveSwipePreviewActive = true
		}
		if m.transition.IsManual() {
			m.transition.FinalizeManual(velocity)
		} else {
			m.transition.Duration = 120 * time.Millisecond
			m.transition.StartTime = time.Now().Add(-time.Duration(float64(currentProgress) * float64(120*time.Millisecond)))
		}
	}
	m.transitionMu.Unlock()

	m.liveSwipeActive = false
	m.liveSwipeProgress = 0
	m.liveSwipeTargetFrame = nil
	m.liveSwipePreviewActive = false
	m.liveSwipeBoundary = false
	m.liveSwipeMu.Unlock()

	if err := m.initializePage(); err != nil {
		m.logger.Error("failed to initialize page after swipe", "error", err)
	}

	if m.onPageChange != nil {
		go func() {
			if err := m.onPageChange(targetPage); err != nil {
				m.logger.Error("page change callback failed", "error", err)
			}
		}()
	}
	go m.preRenderAdjacentPages()

	targetFrame := m.getCachedPageFrame(targetPage)
	if targetFrame == nil {
		m.logger.Debug("swipe finalize cache miss", "page", targetPage)
		frame, err := m.renderImmediateFrameForCurrentPage()
		if err != nil {
			m.logger.Error("failed to render target page for swipe completion", "page", targetPage, "error", err)
		} else {
			targetFrame = frame
			m.pageCacheMu.Lock()
			m.pageCache[targetPage] = frame
			m.pageCacheMu.Unlock()
		}
	} else {
		m.logger.Debug("swipe finalize using cached frame", "page", targetPage)
	}

	if targetFrame != nil {
		m.transitionMu.Lock()
		if m.transition.Active {
			m.transition.NewFrame = targetFrame
		}
		m.transitionMu.Unlock()
	}

	m.transitionMu.RLock()
	postManualActive := m.transition.Active && m.transition.IsManual()
	postManualProgress := 0.0
	if postManualActive {
		postManualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("finalize complete",
		"current_page", m.currentPage,
		"target_page", targetPage,
		"transition_manual_active", postManualActive,
		"transition_manual_progress_pct", int(postManualProgress*100))

	return nil
}

// CancelLiveSwipe cancels an in-progress live swipe and springs back to the current page.
func (m *Manager) CancelLiveSwipe() error {
	m.liveSwipeMu.Lock()
	defer m.liveSwipeMu.Unlock()

	if !m.liveSwipeActive {
		return nil
	}

	currentProgress := m.liveSwipeProgress
	swipeLeft := m.liveSwipeLeft
	m.transitionMu.RLock()
	transitionActive := m.transition.Active
	transitionType := m.transition.Type
	transitionDirection := m.transition.Direction
	manualActive := transitionActive && m.transition.IsManual()
	manualProgress := 0.0
	if manualActive {
		manualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("canceling live swipe",
		"progress_pct", int(currentProgress*100),
		"direction", map[bool]string{true: "left", false: "right"}[swipeLeft],
		"transition_active", transitionActive,
		"transition_type", transitionType,
		"transition_direction", transitionDirection,
		"transition_manual", manualActive,
		"transition_manual_progress_pct", int(manualProgress*100))

	m.transitionMu.Lock()
	if m.transition.Active {
		if m.transition.IsManual() {
			m.transition.AnimateManualTo(0, 0)
		} else {
			m.transition.Active = false
		}
	}
	m.transitionMu.Unlock()

	m.liveSwipeActive = false
	m.liveSwipeProgress = 0
	m.liveSwipeTargetFrame = nil
	m.liveSwipePreviewActive = false
	m.liveSwipeBoundary = false
	m.lastSwipeFinalize = time.Now()

	m.transitionMu.RLock()
	postManualActive := m.transition.Active && m.transition.IsManual()
	postManualProgress := 0.0
	if postManualActive {
		postManualProgress = m.transition.ManualProgress()
	}
	m.transitionMu.RUnlock()

	m.logger.Debug("cancel complete",
		"current_page", m.currentPage,
		"transition_manual_active", postManualActive,
		"transition_manual_progress_pct", int(postManualProgress*100))

	return nil
}
