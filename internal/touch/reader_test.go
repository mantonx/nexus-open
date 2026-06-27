package touch

import (
	"context"
	"io"
	"log/slog"
	"math"
	"testing"
	"time"
)

// discardLogger returns a logger that discards all output, suitable for tests.
func discardLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

// ===== oneEuro filter =====

func TestOneEuroPassThrough_FirstSample(t *testing.T) {
	f := oneEuro{minCutoff: 1.0, beta: 0.007, dCutoff: 1.0}
	// First call must return the input exactly (no history yet).
	out := f.filter(100.0, 0.01)
	if out != 100.0 {
		t.Errorf("first sample: got %v, want 100.0", out)
	}
}

func TestOneEuroSmooths_FastMovement(t *testing.T) {
	// At high speed (large dx) the filter should track closely but not exactly.
	f := oneEuro{minCutoff: 1.0, beta: 0.007, dCutoff: 1.0}
	dt := 0.01 // 100 Hz

	// Feed a step-function: 0→100 and confirm the output converges toward 100.
	f.filter(0, dt)
	out := f.filter(100, dt)
	if out <= 0 || out >= 100 {
		t.Errorf("expected smoothed value between 0 and 100, got %v", out)
	}

	// After many samples of a constant value the output should converge.
	for i := 0; i < 500; i++ {
		out = f.filter(100, dt)
	}
	if math.Abs(out-100) > 0.5 {
		t.Errorf("filter did not converge: got %v, want ~100", out)
	}
}

func TestOneEuroAlpha_ZeroCutoff(t *testing.T) {
	f := oneEuro{}
	// cutoff ≤ 0 must return 1 (no smoothing) without panicking.
	a := f.alpha(0, 0.01)
	if a != 1.0 {
		t.Errorf("alpha(0, dt): got %v, want 1.0", a)
	}
}

// ===== velWindow =====

func TestVelWindow_TooFewSamples(t *testing.T) {
	var w velWindow
	if v := w.velocity(); v != 0 {
		t.Errorf("empty window: got %v, want 0", v)
	}

	now := time.Now()
	w.push(0, now)
	if v := w.velocity(); v != 0 {
		t.Errorf("single sample: got %v, want 0", v)
	}
}

func TestVelWindow_SteadyVelocity(t *testing.T) {
	// Push 4 samples at 100 px/s: x advances 10 px every 100 ms.
	var w velWindow
	base := time.Now()
	for i := 0; i < velWindowSize; i++ {
		w.push(float64(i*10), base.Add(time.Duration(i)*100*time.Millisecond))
	}
	v := w.velocity()
	// Each interval: 10 px / 0.1 s = 100 px/s. Weighted average of equal segments = 100.
	if math.Abs(v-100) > 5 {
		t.Errorf("steady velocity: got %v, want ~100 px/s", v)
	}
}

func TestVelWindow_RingWrapsCorrectly(t *testing.T) {
	// Push more than velWindowSize samples and confirm velocity reflects only
	// the most recent window (not stale samples from before the wrap).
	var w velWindow
	base := time.Now()

	// First batch: slow (10 px/s)
	for i := 0; i < velWindowSize; i++ {
		w.push(float64(i), base.Add(time.Duration(i)*100*time.Millisecond))
	}

	// Second batch: fast (1000 px/s) — should dominate after wrap.
	fastBase := base.Add(time.Duration(velWindowSize) * 100 * time.Millisecond)
	for i := 0; i < velWindowSize; i++ {
		w.push(float64(velWindowSize)+float64(i*100), fastBase.Add(time.Duration(i)*100*time.Millisecond))
	}

	v := w.velocity()
	if v < 500 {
		t.Errorf("after wrap, velocity should reflect fast samples; got %v", v)
	}
}

// ===== normalize and coordinate helpers =====

func TestNormalize_Clamps(t *testing.T) {
	r := NewHIDTouchReader(nil, discardLogger())

	// Below rawMin → should clamp to 0 screen pixels.
	if got := r.normalize(-1); got != 0 {
		t.Errorf("normalize(-1): got %v, want 0", got)
	}

	// Above rawMax → should clamp to screenWidth-1.
	want := float64(r.screenWidth - 1)
	if got := r.normalize(r.rawMax + 100); got != want {
		t.Errorf("normalize(rawMax+100): got %v, want %v", got, want)
	}
}

func TestNormalize_Midpoint(t *testing.T) {
	r := NewHIDTouchReader(nil, discardLogger())
	mid := r.normalize(r.rawMax / 2)
	// Should be close to screenWidth/2.
	want := float64(r.screenWidth) / 2
	if math.Abs(mid-want) > 2 {
		t.Errorf("normalize(mid): got %v, want ~%v", mid, want)
	}
}

// ===== HID packet parsing via fake device =====

// fakeDevice feeds a sequence of pre-built 512-byte touch packets.
type fakeDevice struct {
	packets [][]byte
	idx     int
}

func (f *fakeDevice) ReadTouch(buf []byte, _ uint) (int, error) {
	if f.idx >= len(f.packets) {
		// Stall forever (simulate idle) by blocking — not needed here, tests
		// use context cancellation. Just return 0 bytes to terminate.
		return 0, nil
	}
	n := copy(buf, f.packets[f.idx])
	f.idx++
	return n, nil
}

// touchPacket builds a minimal valid touch report.
// touchState 1 = down, 0 = up. rawX is little-endian in bytes 6-7.
func touchPacket(touchState byte, rawX int) []byte {
	p := make([]byte, 512)
	p[0] = 0x01
	p[1] = 0x02
	p[2] = 0x21
	p[5] = touchState
	p[6] = byte(rawX & 0xFF)
	p[7] = byte((rawX >> 8) & 0xFF)
	return p
}

func TestRead_TapEmitted(t *testing.T) {
	// Press + release within longPressDur → should produce a tap event.
	fd := &fakeDevice{packets: [][]byte{
		touchPacket(1, 100), // press
		touchPacket(0, 0),   // release (device zeroes coords on lift)
	}}

	r := NewHIDTouchReader(fd, discardLogger())
	ctx := context.Background()

	// First Read: press — should return no events.
	events, err := r.Read(ctx)
	if err != nil {
		t.Fatalf("Read (press): %v", err)
	}
	if len(events) != 0 {
		t.Errorf("press: expected 0 events, got %d", len(events))
	}

	// Second Read: release — should return a tap event.
	events, err = r.Read(ctx)
	if err != nil {
		t.Fatalf("Read (release): %v", err)
	}
	if len(events) != 1 {
		t.Fatalf("release: expected 1 event, got %d", len(events))
	}
	e := events[0]
	if e.Button != 0 {
		t.Errorf("expected tap (Button 0), got Button %d", e.Button)
	}
	if e.Pressed {
		t.Errorf("expected Pressed=false for release event")
	}
}

func TestRead_ShortPacketIgnored(t *testing.T) {
	fd := &fakeDevice{packets: [][]byte{
		{0x01, 0x02, 0x21}, // only 3 bytes — below the 8-byte minimum
	}}
	r := NewHIDTouchReader(fd, discardLogger())
	events, err := r.Read(context.Background())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("short packet: expected 0 events, got %d", len(events))
	}
}

func TestRead_NonTouchPacketIgnored(t *testing.T) {
	bad := make([]byte, 16)
	bad[0] = 0xFF // wrong header
	fd := &fakeDevice{packets: [][]byte{bad}}
	r := NewHIDTouchReader(fd, discardLogger())
	events, err := r.Read(context.Background())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("bad header: expected 0 events, got %d", len(events))
	}
}

func TestRead_RawMaxAutocalibrates(t *testing.T) {
	// Send a coord larger than the initial rawMax (486) and confirm it's updated.
	fd := &fakeDevice{packets: [][]byte{
		touchPacket(1, 600), // rawX=600 > rawMax=486
	}}
	r := NewHIDTouchReader(fd, discardLogger())
	r.Read(context.Background()) //nolint — ignore events, checking side effect
	if r.rawMax != 600 {
		t.Errorf("rawMax should have auto-calibrated to 600, got %d", r.rawMax)
	}
}
