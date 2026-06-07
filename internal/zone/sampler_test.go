package zone

import (
	"context"
	"image"
	"io"
	"log/slog"
	"sync/atomic"
	"testing"
	"time"

	pluginhost "github.com/mantonx/nexus-next/internal/plugins/host"
	"github.com/mantonx/nexus-next/pkg/plugin"
)

// ── fakes ─────────────────────────────────────────────────────────────────────

// fakePlugin counts Sample calls.
type fakePlugin struct {
	calls atomic.Int32
}

func (f *fakePlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{Name: "fake", Version: "0.0.1"}, nil
}

func (f *fakePlugin) Sample() (plugin.Payload, error) {
	f.calls.Add(1)
	return plugin.Payload{Primary: "ok", Timestamp: time.Now()}, nil
}

func (f *fakePlugin) Configure(_ map[string]any) error { return nil }

// fakePluginHost lets tests control IsAlive and records Evict/Launch calls.
type fakePluginHost struct {
	launched atomic.Int32
	evicted  atomic.Int32
	alive    atomic.Bool // what IsAlive returns
	mod      plugin.Plugin
}

func (f *fakePluginHost) LaunchPlugin(_ context.Context, _, _ string) (plugin.Plugin, error) {
	f.launched.Add(1)
	f.alive.Store(true)
	return f.mod, nil
}

func (f *fakePluginHost) IsAlive(_ string) bool { return f.alive.Load() }

func (f *fakePluginHost) Evict(_ string) { f.evicted.Add(1) }

// compile-time check: fakePluginHost satisfies the interface
var _ pluginhost.PluginHost = (*fakePluginHost)(nil)

func execZoneConfig(id string) ZoneConfig {
	return ZoneConfig{
		ID:        id,
		Width:     640,
		Plugin:    "exec:./plugins/fake/fake",
		RefreshMs: 50, // fast for tests
		Align:     AlignCenter,
	}
}

// ── IsAlive / Evict unit tests (no goroutines) ────────────────────────────────

func TestHost_IsAlive_NeverLaunched(t *testing.T) {
	h := &fakePluginHost{}
	if h.IsAlive("z") {
		t.Error("IsAlive should be false before any launch")
	}
}

func TestHost_Evict_RecordsCall(t *testing.T) {
	h := &fakePluginHost{}
	h.Evict("z")
	if h.evicted.Load() != 1 {
		t.Errorf("evicted = %d, want 1", h.evicted.Load())
	}
}

// ── crash-restart integration ─────────────────────────────────────────────────

func TestSampler_CrashRestart_EvictsAndRelaunches(t *testing.T) {
	mod := &fakePlugin{}
	host := &fakePluginHost{mod: mod}
	host.alive.Store(true) // starts alive

	cfg := &Config{
		Name: "test", Version: "1.0", Theme: DefaultTheme(),
		Pages: []Page{{Name: "P", Zones: []ZoneConfig{execZoneConfig("z1")}}},
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	manager := &Manager{
		logger:     logger,
		config:     cfg,
		zones:      make(map[string]*Zone),
		renderers:  make(map[string]*Renderer),
		payloads:   make(map[string]*plugin.Payload),
		transition: NewTransitionState(),
		pageCache:  make(map[int]*image.RGBA),
		ctx:        ctx,
		cancel:     cancel,
	}

	s := NewSampler(ctx, logger, manager, nil, "")
	s.pluginHost = host

	// Override backoff constants via the zone being exec: — the loop will check
	// IsAlive after each tick. Simulate crash by flipping alive to false.
	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Let one tick fire (zone is alive → no restart).
	time.Sleep(120 * time.Millisecond)
	evictedBefore := host.evicted.Load()

	// Simulate crash: mark the process as dead.
	host.alive.Store(false)

	// Wait long enough for the backoff (crashBackoffInit = 1s) plus a tick.
	// We shorten this by overriding the constants — but since they're package-level
	// consts we test with the real 1s value; use a generous deadline.
	deadline := time.After(3 * time.Second)
	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-deadline:
			t.Fatalf("timed out waiting for evict+relaunch (evicted=%d launched=%d)",
				host.evicted.Load(), host.launched.Load())
		case <-ticker.C:
			if host.evicted.Load() > evictedBefore && host.launched.Load() >= 2 {
				// Success: evicted the dead process, relaunched at least once.
				return
			}
		}
	}
}

func TestSampler_CrashRestart_StatusBecomesError(t *testing.T) {
	mod := &fakePlugin{}
	host := &fakePluginHost{mod: mod}
	host.alive.Store(true)

	cfg := &Config{
		Name: "test", Version: "1.0", Theme: DefaultTheme(),
		Pages: []Page{{Name: "P", Zones: []ZoneConfig{execZoneConfig("z2")}}},
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	manager := &Manager{
		logger:     logger,
		config:     cfg,
		zones:      make(map[string]*Zone),
		renderers:  make(map[string]*Renderer),
		payloads:   make(map[string]*plugin.Payload),
		transition: NewTransitionState(),
		pageCache:  make(map[int]*image.RGBA),
		ctx:        ctx,
		cancel:     cancel,
	}

	s := NewSampler(ctx, logger, manager, nil, "")
	s.pluginHost = host

	if err := s.startZoneSampling(execZoneConfig("z2")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Let it sample once successfully.
	time.Sleep(120 * time.Millisecond)

	// Crash it.
	host.alive.Store(false)

	// Zone status should transition to "error" while the backoff wait is running.
	deadline := time.After(3 * time.Second)
	tick := time.NewTicker(50 * time.Millisecond)
	defer tick.Stop()

	for {
		select {
		case <-deadline:
			t.Fatalf("timed out; zone status = %q", s.GetZoneStatus("z2").Status)
		case <-tick.C:
			if s.GetZoneStatus("z2").Status == "error" {
				return
			}
		}
	}
}
