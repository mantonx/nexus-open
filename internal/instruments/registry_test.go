package instruments

import (
	"context"
	"log/slog"
	"os"
	"testing"
	"time"

	"nexus-open/internal/settings"
)

func TestRegistry_Lifecycle(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	registry := NewRegistry(logger, cfg)
	if registry == nil {
		t.Fatal("expected non-nil registry")
	}

	// Initialize
	if err := registry.Initialize(); err != nil {
		t.Fatalf("failed to initialize registry: %v", err)
	}

	// Verify instruments were registered
	if len(registry.instruments) == 0 {
		t.Error("expected instruments to be registered")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	// Start
	if err := registry.Start(ctx); err != nil {
		t.Fatalf("failed to start registry: %v", err)
	}

	// Should not be able to start twice
	if err := registry.Start(ctx); err == nil {
		t.Error("expected error when starting already started registry")
	}

	// Wait for data collection
	time.Sleep(1200 * time.Millisecond)

	// Get data
	data := registry.GetData()
	if data.Timestamp.IsZero() {
		t.Error("expected non-zero timestamp")
	}

	// Stop
	if err := registry.Stop(); err != nil {
		t.Errorf("failed to stop registry: %v", err)
	}

	// Should be safe to stop twice
	if err := registry.Stop(); err != nil {
		t.Errorf("stopping twice should not error: %v", err)
	}
}

func TestRegistry_GetData(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	registry := NewRegistry(logger, cfg)
	registry.Initialize()

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	registry.Start(ctx)
	defer registry.Stop()

	// Wait for at least one aggregation
	time.Sleep(1200 * time.Millisecond)

	data := registry.GetData()

	// Verify data structure
	if data.Timestamp.IsZero() {
		t.Error("expected non-zero timestamp")
	}

	// CPU/GPU temps might be 0 on some systems, that's OK
	// Network data might be 0 on idle system, that's OK
	// Weather might be nil if location not set, that's OK

	// Just verify the data structure is accessible
	_ = data.Temperature.CPU
	_ = data.Temperature.GPU
	_ = data.Network.DownloadSpeed
}

func TestRegistry_ConfigWatch(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	registry := NewRegistry(logger, cfg)
	registry.Initialize()

	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()

	registry.Start(ctx)
	defer registry.Stop()

	// Wait for startup
	time.Sleep(100 * time.Millisecond)

	// Update config
	newCfg := cfg.Get()
	newCfg.Location = "San Francisco, CA"
	newCfg.Unit = "metric"
	cfg.Update(newCfg)

	// Wait for config watcher to process
	time.Sleep(200 * time.Millisecond)

	// Verify weather instrument was updated (we can't easily test this without exposing internals)
	// But at least we exercised the config watch path
}

func TestRegistry_InstrumentCount(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")

	registry := NewRegistry(logger, cfg)
	registry.Initialize()

	// Should have 4 instruments: CPU temp, GPU temp, Network, Weather
	expectedCount := 4
	if len(registry.instruments) != expectedCount {
		t.Errorf("expected %d instruments, got %d", expectedCount, len(registry.instruments))
	}

	// Verify each instrument exists
	names := []string{"cpu_temperature", "gpu_temperature", "network", "weather"}
	for _, name := range names {
		if _, exists := registry.instruments[name]; !exists {
			t.Errorf("expected instrument %s to be registered", name)
		}
	}
}
