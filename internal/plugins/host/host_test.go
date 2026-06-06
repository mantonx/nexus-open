package host

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

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
