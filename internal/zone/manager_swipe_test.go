package zone

import (
	"context"
	"image"
	"io"
	"log/slog"
	"testing"
	"time"

	"nexus-open/pkg/module"
)

func newTestSwipeManager(t *testing.T) (*Manager, *image.RGBA) {
	t.Helper()

	config := &Config{
		Name:    "test-layout",
		Version: "1.0",
		Theme:   DefaultTheme(),
		Pages: []Page{
			{
				Name: "A",
				Zones: []ZoneConfig{
					{
						ID:        "zone-a",
						Width:     640,
						Module:    "builtin:test",
						RefreshMs: 200,
						Align:     AlignCenter,
					},
				},
			},
			{
				Name: "B",
				Zones: []ZoneConfig{
					{
						ID:        "zone-b",
						Width:     640,
						Module:    "builtin:test",
						RefreshMs: 200,
						Align:     AlignCenter,
					},
				},
			},
		},
	}

	if err := config.Validate(); err != nil {
		t.Fatalf("config validation failed: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)

	logger := slog.New(slog.NewTextHandler(io.Discard, nil))

	manager := &Manager{
		logger:      logger,
		config:      config,
		currentPage: 0,
		zones:       make(map[string]*Zone),
		renderers:   make(map[string]*Renderer),
		payloads:    make(map[string]*module.Payload),
		transition:  NewTransitionState(),
		pageCache:   make(map[int]*image.RGBA),
		ctx:         ctx,
		cancel:      cancel,
	}

	if err := manager.initializePage(); err != nil {
		t.Fatalf("initializePage: %v", err)
	}

	oldFrame, err := manager.RenderFrame()
	if err != nil {
		t.Fatalf("RenderFrame: %v", err)
	}
	if oldFrame == nil {
		t.Fatal("expected previous frame to be rendered")
	}

	return manager, oldFrame
}

func TestFinalizeLiveSwipeRefreshesTransitionFrame(t *testing.T) {
	manager, oldFrame := newTestSwipeManager(t)

	manager.liveSwipeMu.Lock()
	manager.liveSwipeActive = true
	manager.liveSwipeProgress = 0.4
	manager.liveSwipeLeft = true
	manager.liveSwipeMu.Unlock()

	manager.transitionMu.Lock()
	manager.transition.Start(TransitionSlideLeft, oldFrame, oldFrame, 1)
	manager.transitionMu.Unlock()

	// Ensure the cache is empty so FinalizeLiveSwipe must render a new frame.
	manager.pageCacheMu.Lock()
	manager.pageCache = make(map[int]*image.RGBA)
	manager.pageCacheMu.Unlock()

	if err := manager.FinalizeLiveSwipe(0.4, 250, true); err != nil {
		t.Fatalf("FinalizeLiveSwipe: %v", err)
	}

	if manager.currentPage != 1 {
		t.Fatalf("expected current page to be 1, got %d", manager.currentPage)
	}

	manager.transitionMu.RLock()
	newFrame := manager.transition.NewFrame
	active := manager.transition.Active
	duration := manager.transition.Duration
	manager.transitionMu.RUnlock()

	if !active {
		t.Fatalf("expected transition to remain active after finalization")
	}
	if duration < 60*time.Millisecond {
		t.Fatalf("expected transition duration to respect minimum, got %v", duration)
	}
	if newFrame == nil {
		t.Fatalf("expected transition new frame to be populated")
	}
	if newFrame == oldFrame {
		t.Fatalf("expected a distinct new frame when cache is missing")
	}

	manager.pageCacheMu.RLock()
	cachedFrame := manager.pageCache[1]
	manager.pageCacheMu.RUnlock()

	if cachedFrame == nil {
		t.Fatalf("expected page cache for target page to be populated")
	}
	if cachedFrame != newFrame {
		t.Fatalf("expected cached frame to match transition frame")
	}
}

func TestFinalizeLiveSwipeIgnoresDirectionJitter(t *testing.T) {
	manager, oldFrame := newTestSwipeManager(t)

	manager.liveSwipeMu.Lock()
	manager.liveSwipeActive = true
	manager.liveSwipeProgress = 0.55
	manager.liveSwipeLeft = true // gesture started to the left
	manager.liveSwipeMu.Unlock()

	manager.transitionMu.Lock()
	manager.transition.Start(TransitionSlideLeft, oldFrame, oldFrame, 1)
	manager.transitionMu.Unlock()

	// Simulate cache miss so we must render the target frame.
	manager.pageCacheMu.Lock()
	manager.pageCache = make(map[int]*image.RGBA)
	manager.pageCacheMu.Unlock()

	// Finalize swipe with contradictory direction hint (jitter).
	if err := manager.FinalizeLiveSwipe(0.55, 220, false); err != nil {
		t.Fatalf("FinalizeLiveSwipe: %v", err)
	}

	if manager.currentPage != 1 {
		t.Fatalf("expected to advance to next page despite jitter, got page %d", manager.currentPage)
	}

	manager.transitionMu.RLock()
	newFrame := manager.transition.NewFrame
	manager.transitionMu.RUnlock()

	if newFrame == nil {
		t.Fatal("expected transition new frame to be set after jitter handling")
	}
}
