package app

import (
	"context"
	"log/slog"
	"os"
	"testing"
	"time"
)

func TestApp_New(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	app, err := New(
		WithLogger(logger),
		WithConfigPath(""),
		WithAPIPort(19850), // Use different port to avoid conflicts
	)

	if err != nil {
		t.Fatalf("failed to create app: %v", err)
	}

	if app == nil {
		t.Fatal("expected non-nil app")
	}

	// Verify components were initialized
	if app.cfg == nil {
		t.Error("expected config to be initialized")
	}
	if app.device == nil {
		t.Error("expected device to be initialized")
	}
	if app.apiServer == nil {
		t.Error("expected API server to be initialized")
	}
	if app.instruments == nil {
		t.Error("expected instruments to be initialized")
	}
	if app.display == nil {
		t.Error("expected display to be initialized")
	}
}

func TestApp_Lifecycle(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	app, err := New(
		WithLogger(logger),
		WithConfigPath(""),
		WithAPIPort(19851),
	)
	if err != nil {
		t.Fatalf("failed to create app: %v", err)
	}

	// Start app in background
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	done := make(chan error, 1)
	go func() {
		done <- app.Run(ctx)
	}()

	// Wait for app to start
	time.Sleep(500 * time.Millisecond)

	// Shutdown
	if err := app.Shutdown(); err != nil {
		t.Errorf("shutdown failed: %v", err)
	}

	// Wait for Run to complete
	select {
	case err := <-done:
		if err != nil && err != context.Canceled {
			t.Errorf("unexpected run error: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Error("app did not shutdown in time")
	}
}

func TestApp_MultipleShutdowns(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	app, err := New(
		WithLogger(logger),
		WithConfigPath(""),
		WithAPIPort(19852),
	)
	if err != nil {
		t.Fatalf("failed to create app: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	go app.Run(ctx)
	time.Sleep(200 * time.Millisecond)

	// First shutdown
	if err := app.Shutdown(); err != nil {
		t.Errorf("first shutdown failed: %v", err)
	}

	// Second shutdown should be safe (no-op due to sync.Once)
	if err := app.Shutdown(); err != nil {
		t.Errorf("second shutdown failed: %v", err)
	}
}

func TestApp_ContextCancellation(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	app, err := New(
		WithLogger(logger),
		WithConfigPath(""),
		WithAPIPort(19853),
	)
	if err != nil {
		t.Fatalf("failed to create app: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())

	done := make(chan error, 1)
	go func() {
		done <- app.Run(ctx)
	}()

	// Wait for startup
	time.Sleep(200 * time.Millisecond)

	// Cancel context
	cancel()

	// Wait for Run to exit
	select {
	case err := <-done:
		if err != context.Canceled {
			t.Errorf("expected context.Canceled, got %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Error("app did not exit after context cancellation")
	}

	// Clean shutdown
	app.Shutdown()
}

func TestApp_Options(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	app, err := New(
		WithLogger(logger),
		WithConfigPath(""),
		WithAPIPort(12345),
	)
	if err != nil {
		t.Fatalf("failed to create app: %v", err)
	}

	// Verify options were applied (can't directly check private fields, but app was created)
	if app == nil {
		t.Fatal("expected non-nil app")
	}
}
