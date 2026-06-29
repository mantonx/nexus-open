package host

import (
	"context"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"
)

// pluginBinary returns the path to a real plugin binary for integration tests,
// skipping the test if it hasn't been built yet.
func pluginBinary(t *testing.T) string {
	t.Helper()
	// Resolve repo root relative to this file's location.
	_, thisFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Skip("runtime.Caller failed — skipping integration test")
	}
	// internal/plugins/host/ → repo root is three levels up.
	repoRoot := filepath.Join(filepath.Dir(thisFile), "..", "..", "..")
	candidates := []string{
		filepath.Join(repoRoot, "plugins", "cpu-temp", "nexus-cpu-temp"),
		filepath.Join(repoRoot, "bin", "plugins", "nexus-cpu-temp"),
	}
	for _, p := range candidates {
		if info, err := os.Stat(p); err == nil && info.Mode()&0o111 != 0 {
			return p
		}
	}
	t.Skip("nexus-cpu-temp binary not built — run 'make build-plugins' to enable these tests")
	return ""
}

// ── helpers ───────────────────────────────────────────────────────────────────

func discardLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

// ── IsAlive ───────────────────────────────────────────────────────────────────

func TestIsAlive_UnknownID(t *testing.T) {
	h := NewHost(nil)
	if h.IsAlive("no-such-plugin") {
		t.Error("IsAlive should return false for an ID that was never launched")
	}
}

// ── Evict ─────────────────────────────────────────────────────────────────────

func TestEvict_MissingID(t *testing.T) {
	h := NewHost(nil)
	// Should not panic on an ID that was never registered.
	h.Evict("ghost")
}

// ── LaunchPlugin binary validation ────────────────────────────────────────────

func TestLaunchPlugin_MissingBinary(t *testing.T) {
	h := NewHost(nil)
	_, err := h.LaunchPlugin(context.Background(), "z", "/no/such/binary")
	if err == nil {
		t.Fatal("expected error for missing binary, got nil")
	}
}

func TestLaunchPlugin_NotExecutable(t *testing.T) {
	// Write a file that exists but has no execute bit.
	dir := t.TempDir()
	path := filepath.Join(dir, "plugin")
	if err := os.WriteFile(path, []byte("not a binary"), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	h := NewHost(nil)
	_, err := h.LaunchPlugin(context.Background(), "z", path)
	if err == nil {
		t.Fatal("expected error for non-executable binary, got nil")
	}
}

func TestLaunchPlugin_CancelledContext(t *testing.T) {
	// An already-cancelled context should be rejected before the subprocess starts.
	// Write a dummy executable so binary validation passes.
	dir := t.TempDir()
	path := filepath.Join(dir, "plugin")
	if err := os.WriteFile(path, []byte("#!/bin/sh\nsleep 60"), 0o755); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	h := NewHost(nil)
	_, err := h.LaunchPlugin(ctx, "z", path)
	if err == nil {
		t.Fatal("expected error for cancelled context, got nil")
	}
}

// ── integration tests (require a real plugin binary) ─────────────────────────

func TestGetPlugin_NotRunning(t *testing.T) {
	h := NewHost(discardLogger())
	_, err := h.GetPlugin("never-launched")
	if err == nil {
		t.Fatal("expected error for plugin that was never launched")
	}
}

func TestStopPlugin_NotRunning(t *testing.T) {
	h := NewHost(discardLogger())
	err := h.StopPlugin("ghost")
	if err == nil {
		t.Fatal("expected error stopping a plugin that was never launched")
	}
}

func TestStopAll_EmptyHostIsNoop(t *testing.T) {
	h := NewHost(discardLogger())
	h.StopAll() // must not panic or block
}

func TestLaunchGetStop_RealPlugin(t *testing.T) {
	bin := pluginBinary(t)
	h := NewHost(discardLogger())

	mod, err := h.LaunchPlugin(context.Background(), "cpu", bin)
	if err != nil {
		t.Fatalf("LaunchPlugin: %v", err)
	}
	if mod == nil {
		t.Fatal("LaunchPlugin returned nil plugin")
	}

	// GetPlugin should return the same instance.
	got, err := h.GetPlugin("cpu")
	if err != nil {
		t.Fatalf("GetPlugin: %v", err)
	}
	if got != mod {
		t.Error("GetPlugin returned a different instance than LaunchPlugin")
	}

	// IsAlive should be true while the subprocess is running.
	if !h.IsAlive("cpu") {
		t.Error("IsAlive should be true for a running plugin")
	}

	// LaunchPlugin again with the same ID returns the existing instance (no new process).
	mod2, err := h.LaunchPlugin(context.Background(), "cpu", bin)
	if err != nil {
		t.Fatalf("second LaunchPlugin: %v", err)
	}
	if mod2 != mod {
		t.Error("second LaunchPlugin should reuse the existing plugin instance")
	}

	if err := h.StopPlugin("cpu"); err != nil {
		t.Fatalf("StopPlugin: %v", err)
	}

	// After stop, GetPlugin should fail and IsAlive should be false.
	if _, err := h.GetPlugin("cpu"); err == nil {
		t.Error("GetPlugin should fail after StopPlugin")
	}
	if h.IsAlive("cpu") {
		t.Error("IsAlive should be false after StopPlugin")
	}
}

func TestStopAll_TerminatesRunningPlugins(t *testing.T) {
	bin := pluginBinary(t)
	h := NewHost(discardLogger())

	for _, id := range []string{"cpu-a", "cpu-b"} {
		if _, err := h.LaunchPlugin(context.Background(), id, bin); err != nil {
			t.Fatalf("LaunchPlugin %s: %v", id, err)
		}
	}

	h.StopAll()

	for _, id := range []string{"cpu-a", "cpu-b"} {
		if h.IsAlive(id) {
			t.Errorf("plugin %s still alive after StopAll", id)
		}
		if _, err := h.GetPlugin(id); err == nil {
			t.Errorf("GetPlugin(%s) should fail after StopAll", id)
		}
	}
}

func TestSampleWithTimeout_ReturnsPayload(t *testing.T) {
	bin := pluginBinary(t)
	h := NewHost(discardLogger())
	defer h.StopAll()

	if _, err := h.LaunchPlugin(context.Background(), "cpu", bin); err != nil {
		t.Fatalf("LaunchPlugin: %v", err)
	}

	payload, err := h.SampleWithTimeout("cpu", 5*time.Second)
	if err != nil {
		t.Fatalf("SampleWithTimeout: %v", err)
	}
	if payload.Primary == "" {
		t.Error("expected non-empty Primary in payload")
	}
}

func TestSampleWithTimeout_UnknownPlugin(t *testing.T) {
	h := NewHost(discardLogger())
	_, err := h.SampleWithTimeout("no-such-plugin", time.Second)
	if err == nil {
		t.Fatal("expected error for unregistered plugin ID")
	}
}

func TestSampleWithTimeout_Expires(t *testing.T) {
	// Build a plugin-shaped executable that hangs forever in Sample.
	dir := t.TempDir()
	bin := filepath.Join(dir, "hang-plugin")
	// A shell script that satisfies the RPC handshake but hangs in Sample would
	// require a full go-plugin harness. Instead, use a binary that never exits
	// so the RPC connect itself times out, triggering the launchTimeout path.
	// We test the SampleWithTimeout timeout by launching a real plugin and then
	// using a zero-duration timeout, which should always expire before Sample returns.
	realBin := pluginBinary(t)
	_ = bin

	h := NewHost(discardLogger())
	defer h.StopAll()

	if _, err := h.LaunchPlugin(context.Background(), "cpu", realBin); err != nil {
		t.Fatalf("LaunchPlugin: %v", err)
	}

	// 1ns timeout — expires before the RPC round-trip completes.
	_, err := h.SampleWithTimeout("cpu", time.Nanosecond)
	if err == nil {
		t.Fatal("expected timeout error with 1ns deadline, got nil")
	}
}

func TestDescribePlugin_RealPlugin(t *testing.T) {
	bin := pluginBinary(t)
	desc, err := DescribePlugin(bin)
	if err != nil {
		t.Fatalf("DescribePlugin: %v", err)
	}
	if desc.Name == "" {
		t.Error("expected non-empty Name in descriptor")
	}
}

func TestDescribePlugin_MissingBinary(t *testing.T) {
	_, err := DescribePlugin("/no/such/binary")
	if err == nil {
		t.Fatal("expected error for missing binary")
	}
}
