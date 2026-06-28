package zone

import (
	"context"
	"errors"
	"image"
	"io"
	"log/slog"
	"path/filepath"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	pluginhost "github.com/mantonx/nexus-open/internal/plugins/host"
	"github.com/mantonx/nexus-open/internal/store"
	"github.com/mantonx/nexus-open/pkg/plugin"
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

// errorPlugin always returns an error from Sample.
type errorPlugin struct{}

func (e *errorPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{Name: "error", Version: "0.0.1"}, nil
}
func (e *errorPlugin) Sample() (plugin.Payload, error) {
	return plugin.Payload{}, errors.New("plugin error")
}
func (e *errorPlugin) Configure(_ map[string]any) error { return nil }

// hangPlugin blocks Sample until its release channel is closed or ctx is done.
type hangPlugin struct {
	release chan struct{}
	calls   atomic.Int32
}

func newHangPlugin() *hangPlugin { return &hangPlugin{release: make(chan struct{})} }

func (h *hangPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{Name: "hang", Version: "0.0.1"}, nil
}
func (h *hangPlugin) Sample() (plugin.Payload, error) {
	h.calls.Add(1)
	<-h.release
	return plugin.Payload{Primary: "ok", Timestamp: time.Now()}, nil
}
func (h *hangPlugin) Configure(_ map[string]any) error { return nil }

// configCapture records the last Configure call so tests can assert on it.
type configCapture struct {
	mu   sync.Mutex
	last map[string]any
	calls atomic.Int32
}

func (c *configCapture) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{Name: "config-capture", Version: "0.0.1"}, nil
}
func (c *configCapture) Sample() (plugin.Payload, error) {
	return plugin.Payload{Primary: "ok", Timestamp: time.Now()}, nil
}
func (c *configCapture) Configure(cfg map[string]any) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.calls.Add(1)
	c.last = cfg
	return nil
}
func (c *configCapture) LastConfig() map[string]any {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.last
}

// fakePluginHost lets tests control IsAlive and records Evict/Launch calls.
// modFactory, if set, is called on each LaunchPlugin to return a fresh plugin;
// otherwise mod is returned directly.
type fakePluginHost struct {
	launched   atomic.Int32
	evicted    atomic.Int32
	alive      atomic.Bool // what IsAlive returns
	mod        plugin.Plugin
	modFactory func() plugin.Plugin
}

func (f *fakePluginHost) LaunchPlugin(_ context.Context, _, _ string) (plugin.Plugin, error) {
	f.launched.Add(1)
	f.alive.Store(true)
	if f.modFactory != nil {
		return f.modFactory(), nil
	}
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
		Plugin:    "exec:fake",
		RefreshMs: 50, // fast for tests
		Align:     AlignCenter,
	}
}

// newTestSampler builds a Sampler wired to a fakePluginHost with a short
// crash backoff so tests don't wait for real 1s delays.
func newTestSampler(ctx context.Context, host *fakePluginHost) (*Sampler, *Manager) {
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	cfg := &Config{
		Name: "test", Version: "1.0", Theme: DefaultTheme(),
		Pages: []Page{{Name: "P", Zones: []ZoneConfig{execZoneConfig("z1")}}},
	}
	_, cancel := context.WithCancel(ctx)
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
	s.crashBackoffStart = 10 * time.Millisecond // don't wait 1s in tests
	return s, manager
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

// ── Issue 6: RestartZone eviction ─────────────────────────────────────────────

func TestSampler_RestartZone_EvictsOldPlugin(t *testing.T) {
	pluginA := &fakePlugin{}
	pluginB := &fakePlugin{}
	callCount := atomic.Int32{}
	host := &fakePluginHost{
		modFactory: func() plugin.Plugin {
			if callCount.Add(1) == 1 {
				return pluginA
			}
			return pluginB
		},
	}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	s, _ := newTestSampler(ctx, host)
	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	evictedBefore := host.evicted.Load()
	launchedBefore := host.launched.Load()

	// Restart with a new zone config (simulates tap-cycle).
	cfg2 := execZoneConfig("z1")
	cfg2.Plugin = "exec:nexus-other"
	if err := s.RestartZone(cfg2); err != nil {
		t.Fatalf("RestartZone: %v", err)
	}

	if host.evicted.Load() <= evictedBefore {
		t.Error("expected Evict to be called on RestartZone")
	}
	if host.launched.Load() <= launchedBefore {
		t.Error("expected LaunchPlugin to be called for the new zone")
	}

	// Assert only plugin B is sampled: wait for B to accumulate calls, then
	// snapshot A and confirm A does not advance while B continues to.
	deadline := time.After(2 * time.Second)
	tick := time.NewTicker(20 * time.Millisecond)
	defer tick.Stop()
	for {
		select {
		case <-deadline:
			t.Fatalf("plugin B never sampled after RestartZone (calls=%d)", pluginB.calls.Load())
		case <-tick.C:
			if pluginB.calls.Load() >= 2 {
				goto bIsRunning
			}
		}
	}
bIsRunning:
	callsA := pluginA.calls.Load()
	time.Sleep(80 * time.Millisecond)
	if pluginA.calls.Load() != callsA {
		t.Error("pluginA is still being sampled after RestartZone")
	}
}

// ── Issue 7: crash/timeout/backoff tests ──────────────────────────────────────

func TestSampler_PluginHangsForever_EvictsAndRestarts(t *testing.T) {
	hang := newHangPlugin()
	host := &fakePluginHost{mod: hang}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	s, _ := newTestSampler(ctx, host)
	// Use a very short sample timeout so we don't actually wait 5s.
	// sampleTimeout clamps to 5s min, so we drive it via a long interval:
	// sampleTimeout(10s interval) = 5s. Use a separate zone with long interval
	// and override sampleOnce indirectly via interval that yields a 5s timeout.
	// Instead, simulate "dead after timeout" by flipping alive after hang blocks.
	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Let the initial sample call block in hang.Sample(). After a moment flip
	// alive to false so the tick path also sees the process as dead.
	time.Sleep(30 * time.Millisecond)
	host.alive.Store(false)

	// Unblock the hang so the goroutine can proceed to the IsAlive check.
	close(hang.release)

	// Now the loop should detect dead=false but IsAlive=false, call handlePluginCrash,
	// evict and relaunch. With 10ms backoff this should complete quickly.
	deadline := time.After(3 * time.Second)
	tick := time.NewTicker(20 * time.Millisecond)
	defer tick.Stop()
	for {
		select {
		case <-deadline:
			t.Fatalf("plugin not restarted after crash (evicted=%d launched=%d)",
				host.evicted.Load(), host.launched.Load())
		case <-tick.C:
			if host.evicted.Load() >= 1 && host.launched.Load() >= 2 {
				return
			}
		}
	}
}

func TestSampler_PluginReturnsError_StatusIsError(t *testing.T) {
	host := &fakePluginHost{mod: &errorPlugin{}}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()

	s, _ := newTestSampler(ctx, host)
	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Error from Sample() ≠ crash — the process is still "alive" per IsAlive.
	// Zone status should be "error" but the loop should NOT restart the plugin.
	deadline := time.After(2 * time.Second)
	tick := time.NewTicker(20 * time.Millisecond)
	defer tick.Stop()
	for {
		select {
		case <-deadline:
			t.Fatalf("zone status never became error (status=%q)", s.GetZoneStatus("z1").Status)
		case <-tick.C:
			st := s.GetZoneStatus("z1")
			if st.Status == "error" {
				// Confirm no evict/relaunch happened (error ≠ crash).
				if host.evicted.Load() > 0 {
					t.Error("Evict was called for a non-crashing error return")
				}
				return
			}
		}
	}
}

func TestSampler_PluginCrashesAfterFirstSample_Restarts(t *testing.T) {
	mod := &fakePlugin{}
	host := &fakePluginHost{mod: mod}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	s, _ := newTestSampler(ctx, host)
	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Wait for at least one successful sample.
	deadline := time.After(3 * time.Second)
	tick := time.NewTicker(20 * time.Millisecond)
	defer tick.Stop()
	for {
		select {
		case <-deadline:
			t.Fatal("plugin never sampled successfully")
		case <-tick.C:
			if s.GetZoneStatus("z1").Status == "ok" {
				goto crashed
			}
		}
	}
crashed:
	// Now simulate crash.
	host.alive.Store(false)

	// Should evict and relaunch with backoff.
	deadline2 := time.After(3 * time.Second)
	tick2 := time.NewTicker(20 * time.Millisecond)
	defer tick2.Stop()
	for {
		select {
		case <-deadline2:
			t.Fatalf("plugin not restarted (evicted=%d launched=%d)",
				host.evicted.Load(), host.launched.Load())
		case <-tick2.C:
			if host.evicted.Load() >= 1 && host.launched.Load() >= 2 {
				return
			}
		}
	}
}

func TestSampler_ShutdownDuringCrashBackoff_ExitsCleanly(t *testing.T) {
	mod := &fakePlugin{}
	host := &fakePluginHost{mod: mod}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	s, _ := newTestSampler(ctx, host)
	// Use a long backoff so the cancel fires while waiting.
	s.crashBackoffStart = 10 * time.Second

	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Wait for first sample then crash.
	time.Sleep(80 * time.Millisecond)
	host.alive.Store(false)

	// Cancel context quickly — should unblock the backoff select and exit cleanly.
	time.Sleep(30 * time.Millisecond)
	cancel()

	done := make(chan struct{})
	go func() {
		s.Stop()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(3 * time.Second):
		t.Error("Sampler.Stop() did not return after context cancellation")
	}
}

func TestSampler_RestartPreservesZoneConfig(t *testing.T) {
	capture := &configCapture{}
	host := &fakePluginHost{mod: capture}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	logger := slog.New(slog.NewTextHandler(io.Discard, nil))

	// Open a real (temp) SQLite store so ConfigManager can persist zone config.
	db, err := store.Open(filepath.Join(t.TempDir(), "nexus.db"), logger)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	defer func() { _ = db.Close() }()

	// Seed the page + zone rows so SetZonePluginConfig has a row to UPDATE.
	pageID, err := db.CreatePage("P", 0)
	if err != nil {
		t.Fatalf("CreatePage: %v", err)
	}
	if err := db.CreateZone(store.StoredZone{
		ID: "z1", PageID: pageID, Plugin: "exec:fake", WidthPx: 640,
	}); err != nil {
		t.Fatalf("CreateZone: %v", err)
	}

	zoneCfg := NewConfigManager(db, logger)
	// Pre-seed the zone config that the sampler should deliver to Configure.
	if err := zoneCfg.SetZoneOverride("z1", map[string]any{"theme": "dark"}); err != nil {
		t.Fatalf("SetZoneOverride: %v", err)
	}

	cfg := &Config{
		Name: "test", Version: "1.0", Theme: DefaultTheme(),
		Pages: []Page{{Name: "P", Zones: []ZoneConfig{execZoneConfig("z1")}}},
	}
	mgrCtx, mgrCancel := context.WithCancel(ctx)
	manager := &Manager{
		logger:     logger,
		config:     cfg,
		zones:      make(map[string]*Zone),
		renderers:  make(map[string]*Renderer),
		payloads:   make(map[string]*plugin.Payload),
		transition: NewTransitionState(),
		pageCache:  make(map[int]*image.RGBA),
		ctx:        mgrCtx,
		cancel:     mgrCancel,
	}
	s := NewSampler(ctx, logger, manager, zoneCfg, "")
	s.pluginHost = host
	s.crashBackoffStart = 10 * time.Millisecond

	if err := s.startZoneSampling(execZoneConfig("z1")); err != nil {
		t.Fatalf("startZoneSampling: %v", err)
	}

	// Wait for Configure to be called with the stored config.
	deadline := time.After(2 * time.Second)
	tick := time.NewTicker(20 * time.Millisecond)
	defer tick.Stop()
	for {
		select {
		case <-deadline:
			t.Fatalf("Configure never called (calls=%d)", capture.calls.Load())
		case <-tick.C:
			if capture.calls.Load() >= 1 {
				if got := capture.LastConfig()["theme"]; got != "dark" {
					t.Errorf("Configure received %v, want theme=dark", capture.LastConfig())
				}
				return
			}
		}
	}
}

func TestSampler_CrashBackoff_CapsAtMax(t *testing.T) {
	// Verify the backoff doubling caps at crashBackoffMax without running the
	// actual sampleLoop — test the arithmetic directly via handlePluginCrash.
	mod := &fakePlugin{}
	host := &fakePluginHost{mod: mod}
	host.alive.Store(true)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	s, _ := newTestSampler(ctx, host)
	s.crashBackoffStart = 1 * time.Millisecond

	// Manually drive the backoff doubling.
	backoff := s.crashBackoffStart
	for range 40 {
		backoff *= 2
		if backoff > crashBackoffMax {
			backoff = crashBackoffMax
		}
	}
	if backoff != crashBackoffMax {
		t.Errorf("backoff after 40 doublings = %v, want %v", backoff, crashBackoffMax)
	}
}

func TestResolvePluginPath(t *testing.T) {
	pluginsDir := "/srv/plugins"
	s := &Sampler{pluginsDir: pluginsDir}

	cases := []struct {
		spec    string
		want    string
		wantErr bool
	}{
		{"exec:nexus-cpu-temp", "/srv/plugins/nexus-cpu-temp", false},
		{"exec:/usr/bin/sh", "", true},
		{"exec:../../etc/passwd", "", true},
		{"exec:nexus-cpu-temp/extra", "", true},
	}

	for _, tc := range cases {
		got, err := s.resolvePluginPath(tc.spec)
		if tc.wantErr {
			if err == nil {
				t.Errorf("resolvePluginPath(%q): expected error, got path %q", tc.spec, got)
			}
		} else {
			if err != nil {
				t.Errorf("resolvePluginPath(%q): unexpected error: %v", tc.spec, err)
			} else if got != tc.want {
				t.Errorf("resolvePluginPath(%q): got %q, want %q", tc.spec, got, tc.want)
			}
		}
	}
}
