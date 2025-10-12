package instruments

import (
	"context"
	"log/slog"
	"os"
	"testing"
	"time"
)

func TestNetwork_Lifecycle(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewNetwork(logger, 500*time.Millisecond)

	if inst.Name() != "network" {
		t.Errorf("expected name 'network', got %s", inst.Name())
	}

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	// Start the instrument
	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}

	// Should not be able to start twice
	if err := inst.Start(ctx); err == nil {
		t.Error("expected error when starting already started instrument")
	}

	// Wait for at least one reading
	time.Sleep(600 * time.Millisecond)

	// Get current reading
	stats := inst.GetCurrent()
	// Network stats should be non-negative
	if stats.DownloadSpeed < 0 || stats.UploadSpeed < 0 {
		t.Errorf("unexpected negative network speed")
	}

	// Stop the instrument
	if err := inst.Stop(); err != nil {
		t.Errorf("failed to stop instrument: %v", err)
	}

	// Should be safe to stop twice
	if err := inst.Stop(); err != nil {
		t.Errorf("stopping twice should not error: %v", err)
	}
}

func TestNetwork_UpdateInterval(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	// Test custom interval
	inst := NewNetwork(logger, 2*time.Second)
	if inst.UpdateInterval() != 2*time.Second {
		t.Errorf("expected interval 2s, got %v", inst.UpdateInterval())
	}

	// Test default interval
	inst = NewNetwork(logger, 0)
	if inst.UpdateInterval() != 1*time.Second {
		t.Errorf("expected default interval 1s, got %v", inst.UpdateInterval())
	}
}

func TestNetwork_ContextCancellation(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewNetwork(logger, 200*time.Millisecond)

	ctx, cancel := context.WithCancel(context.Background())

	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}

	// Wait a bit
	time.Sleep(300 * time.Millisecond)

	// Cancel context
	cancel()

	// Wait for goroutine to exit
	time.Sleep(200 * time.Millisecond)

	// Stop should succeed
	if err := inst.Stop(); err != nil {
		t.Errorf("stop after context cancel failed: %v", err)
	}
}

func TestNetwork_DataStructure(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewNetwork(logger, 100*time.Millisecond)

	// Initial state should be zero values
	stats := inst.GetCurrent()
	if stats.DownloadSpeed != 0 || stats.UploadSpeed != 0 {
		t.Error("initial network stats should be zero")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}
	defer inst.Stop()

	// Wait for a reading
	time.Sleep(200 * time.Millisecond)

	// After starting, should have valid data structure
	stats = inst.GetCurrent()
	// Just verify the struct is readable (values might be 0 on idle system)
	_ = stats.DownloadSpeed
	_ = stats.UploadSpeed
	_ = stats.TotalDown
	_ = stats.TotalUp
}
