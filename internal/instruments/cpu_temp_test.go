package instruments

import (
	"context"
	"log/slog"
	"os"
	"testing"
	"time"
)

func TestCPUTemp_Lifecycle(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewCPUTemp(logger, 100*time.Millisecond)

	if inst.Name() != "cpu_temperature" {
		t.Errorf("expected name 'cpu_temperature', got %s", inst.Name())
	}

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
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
	time.Sleep(200 * time.Millisecond)

	// Get current reading (might be 0 if system doesn't support temp reading)
	temp := inst.GetCurrent()
	if temp < 0 || temp > 150 {
		t.Errorf("unexpected temperature value: %.2f", temp)
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

func TestCPUTemp_UpdateInterval(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	// Test custom interval
	inst := NewCPUTemp(logger, 2*time.Second)
	if inst.UpdateInterval() != 2*time.Second {
		t.Errorf("expected interval 2s, got %v", inst.UpdateInterval())
	}

	// Test default interval
	inst = NewCPUTemp(logger, 0)
	if inst.UpdateInterval() != 5*time.Second {
		t.Errorf("expected default interval 5s, got %v", inst.UpdateInterval())
	}
}

func TestCPUTemp_ContextCancellation(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewCPUTemp(logger, 50*time.Millisecond)

	ctx, cancel := context.WithCancel(context.Background())

	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}

	// Wait a bit for it to start
	time.Sleep(100 * time.Millisecond)

	// Cancel context
	cancel()

	// Wait for goroutine to exit
	time.Sleep(100 * time.Millisecond)

	// Stop should succeed even after context cancellation
	if err := inst.Stop(); err != nil {
		t.Errorf("stop after context cancel failed: %v", err)
	}
}

func TestCPUTemp_ConcurrentAccess(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewCPUTemp(logger, 10*time.Millisecond)

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}
	defer inst.Stop()

	// Concurrent reads should not panic
	done := make(chan bool)
	for i := 0; i < 10; i++ {
		go func() {
			for j := 0; j < 100; j++ {
				_ = inst.GetCurrent()
			}
			done <- true
		}()
	}

	// Wait for all goroutines
	for i := 0; i < 10; i++ {
		<-done
	}
}
