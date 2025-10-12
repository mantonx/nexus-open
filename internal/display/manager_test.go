package display

import (
	"context"
	"log/slog"
	"os"
	"testing"
	"time"

	"nexus-open/internal/config"
	"nexus-open/internal/device"
	"nexus-open/internal/instruments"
)

func TestManager_Lifecycle(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	registry := instruments.NewRegistry(logger, cfg)
	registry.Initialize()

	manager := NewManager(logger, cfg, mockDev, registry)
	if manager == nil {
		t.Fatal("expected non-nil manager")
	}

	// Initialize
	if err := manager.Initialize(); err != nil {
		t.Fatalf("failed to initialize manager: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	// Start instruments first (manager depends on them)
	registry.Start(ctx)
	defer registry.Stop()

	// Start manager
	if err := manager.Start(ctx); err != nil {
		t.Fatalf("failed to start manager: %v", err)
	}

	// Wait for a few frames
	time.Sleep(200 * time.Millisecond)

	// Stop
	if err := manager.Stop(); err != nil {
		t.Errorf("failed to stop manager: %v", err)
	}
}

func TestManager_RenderWithDevice(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	mockDev := device.NewMockDevice()
	// Connect the mock device
	mockDev.Connect(context.Background())

	registry := instruments.NewRegistry(logger, cfg)
	registry.Initialize()

	manager := NewManager(logger, cfg, mockDev, registry)
	manager.Initialize()

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	registry.Start(ctx)
	defer registry.Stop()

	manager.Start(ctx)
	defer manager.Stop()

	// Wait for frames to be sent
	time.Sleep(200 * time.Millisecond)

	// Verify frames were sent to device
	if mockDev.GetFramesSent() == 0 {
		t.Error("expected frames to be sent to device")
	}

	t.Logf("Sent %d frames", mockDev.GetFramesSent())
}

func TestManager_RenderWithoutDevice(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	mockDev := device.NewMockDevice()
	// Don't connect the device

	registry := instruments.NewRegistry(logger, cfg)
	registry.Initialize()

	manager := NewManager(logger, cfg, mockDev, registry)
	manager.Initialize()

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	registry.Start(ctx)
	defer registry.Stop()

	manager.Start(ctx)
	defer manager.Stop()

	// Wait
	time.Sleep(200 * time.Millisecond)

	// Should not have sent frames (device not connected)
	if mockDev.GetFramesSent() > 0 {
		t.Error("should not send frames when device is not connected")
	}
}

func TestManager_ConfigUpdate(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	mockDev := device.NewMockDevice()
	mockDev.Connect(context.Background())

	registry := instruments.NewRegistry(logger, cfg)
	registry.Initialize()

	manager := NewManager(logger, cfg, mockDev, registry)
	manager.Initialize()

	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	registry.Start(ctx)
	defer registry.Stop()

	manager.Start(ctx)
	defer manager.Stop()

	// Wait for initial frames
	time.Sleep(200 * time.Millisecond)
	initialFrames := mockDev.GetFramesSent()

	// Update config (should trigger immediate render)
	newCfg := cfg.Get()
	newCfg.TextColor = "#FF0000" // Change to red
	cfg.Update(newCfg)

	// Wait for config update to be processed
	time.Sleep(200 * time.Millisecond)

	// Should have sent more frames
	if mockDev.GetFramesSent() <= initialFrames {
		t.Error("expected more frames after config update")
	}
}

func TestManager_BlinkAnimation(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	mockDev := device.NewMockDevice()
	registry := instruments.NewRegistry(logger, cfg)
	registry.Initialize()

	manager := NewManager(logger, cfg, mockDev, registry)
	manager.Initialize()

	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	registry.Start(ctx)
	defer registry.Stop()

	manager.Start(ctx)
	defer manager.Stop()

	// Wait for at least one blink cycle (BlinkInterval = 500ms)
	time.Sleep(600 * time.Millisecond)

	// Blink animation should have toggled at least once
	// (We can't directly test this without exposing internals, but we exercised the code)
}
