package zone

import (
	"context"
	"image"
	"io"
	"log/slog"
	"sync"
	"testing"
	"time"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

func newTestRenderManager(t *testing.T) *Manager {
	t.Helper()
	config := &Config{
		Name:    "test",
		Version: "1.0",
		Theme:   DefaultTheme(),
		Pages: []Page{
			{
				Name: "A",
				Zones: []ZoneConfig{
					{ID: "z1", Width: 320, Plugin: "builtin:test", RefreshMs: 200, Align: AlignCenter},
					{ID: "z2", Width: 320, Plugin: "builtin:test", RefreshMs: 200, Align: AlignCenter},
				},
			},
			{
				Name: "B",
				Zones: []ZoneConfig{
					{ID: "z3", Width: 640, Plugin: "builtin:test", RefreshMs: 200, Align: AlignCenter},
				},
			},
		},
	}
	if err := config.Validate(); err != nil {
		t.Fatalf("config: %v", err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	m := &Manager{
		logger:      logger,
		config:      config,
		currentPage: 0,
		zones:       make(map[string]*Zone),
		renderers:   make(map[string]*Renderer),
		payloads:    make(map[string]*plugin.Payload),
		transition:  NewTransitionState(),
		frameDirty:  true,
		frameBufs: [2]*image.RGBA{
			image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight)),
			image.NewRGBA(image.Rect(0, 0, DisplayWidth, DisplayHeight)),
		},
		pageCache:   make(map[int]*image.RGBA),
		choiceIndex: make(map[string]int),
		ctx:         ctx,
		cancel:      cancel,
	}
	if err := m.initializePage(); err != nil {
		t.Fatalf("initializePage: %v", err)
	}
	return m
}

func TestRenderFrameDirtyFlag(t *testing.T) {
	m := newTestRenderManager(t)

	frame1, err := m.RenderFrame()
	if err != nil {
		t.Fatalf("RenderFrame: %v", err)
	}
	if frame1 == nil {
		t.Fatal("expected non-nil frame")
	}

	// Second call with no updates must return the cached frame pointer.
	frame2, err := m.RenderFrame()
	if err != nil {
		t.Fatalf("RenderFrame (cached): %v", err)
	}
	if frame1 != frame2 {
		t.Fatal("expected same frame pointer on clean render (dirty flag not working)")
	}

	// After a payload update the dirty flag should fire a fresh render.
	m.payloadsMu.Lock()
	m.payloads["z1"] = &plugin.Payload{Primary: "new", Severity: plugin.SeverityOK, Timestamp: time.Now()}
	m.payloadsMu.Unlock()
	m.lastFrameMu.Lock()
	m.frameDirty = true
	m.lastFrameMu.Unlock()

	frame3, err := m.RenderFrame()
	if err != nil {
		t.Fatalf("RenderFrame (dirty): %v", err)
	}
	if frame3 == frame2 {
		t.Fatal("expected fresh frame after payload update")
	}
}

// TestPreRenderAdjacentPagesRace verifies that concurrent pre-rendering and
// live rendering do not race on shared renderer state. Run with -race.
func TestPreRenderAdjacentPagesRace(t *testing.T) {
	m := newTestRenderManager(t)

	// Seed a payload so rendering actually exercises the font path.
	m.payloadsMu.Lock()
	for id := range m.zones {
		m.payloads[id] = &plugin.Payload{Primary: "42%", Secondary: "test", Severity: plugin.SeverityOK, Timestamp: time.Now()}
	}
	m.payloadsMu.Unlock()

	var wg sync.WaitGroup

	// Concurrent live renders.
	wg.Add(1)
	go func() {
		defer wg.Done()
		for i := 0; i < 20; i++ {
			m.lastFrameMu.Lock()
			m.frameDirty = true
			m.lastFrameMu.Unlock()
			if _, err := m.RenderFrame(); err != nil {
				t.Errorf("RenderFrame: %v", err)
				return
			}
		}
	}()

	// Concurrent pre-renders of adjacent pages (the path that was racing).
	wg.Add(1)
	go func() {
		defer wg.Done()
		for i := 0; i < 20; i++ {
			m.preRenderAdjacentPages()
		}
	}()

	wg.Wait()
}

func TestUpdatePayloadSetsDirtyFlag(t *testing.T) {
	m := newTestRenderManager(t)

	// Render once to clear the dirty flag.
	if _, err := m.RenderFrame(); err != nil {
		t.Fatalf("RenderFrame: %v", err)
	}
	m.lastFrameMu.Lock()
	dirty := m.frameDirty
	m.lastFrameMu.Unlock()
	if dirty {
		t.Fatal("expected frameDirty=false after render")
	}

	if err := m.UpdatePayload("z1", plugin.Payload{
		Primary:   "99%",
		Severity:  plugin.SeverityWarn,
		Timestamp: time.Now(),
	}); err != nil {
		t.Fatalf("UpdatePayload: %v", err)
	}

	m.lastFrameMu.Lock()
	dirty = m.frameDirty
	m.lastFrameMu.Unlock()
	if !dirty {
		t.Fatal("expected frameDirty=true after UpdatePayload")
	}
}
