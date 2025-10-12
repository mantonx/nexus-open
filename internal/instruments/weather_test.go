package instruments

import (
	"context"
	"log/slog"
	"os"
	"testing"
	"time"
)

func TestWeather_Lifecycle(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewWeather(logger, 1*time.Minute)

	if inst.Name() != "weather" {
		t.Errorf("expected name 'weather', got %s", inst.Name())
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

	// Stop the instrument
	if err := inst.Stop(); err != nil {
		t.Errorf("failed to stop instrument: %v", err)
	}

	// Should be safe to stop twice
	if err := inst.Stop(); err != nil {
		t.Errorf("stopping twice should not error: %v", err)
	}
}

func TestWeather_UpdateInterval(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	// Test custom interval
	inst := NewWeather(logger, 5*time.Minute)
	if inst.UpdateInterval() != 5*time.Minute {
		t.Errorf("expected interval 5m, got %v", inst.UpdateInterval())
	}

	// Test default interval
	inst = NewWeather(logger, 0)
	if inst.UpdateInterval() != 10*time.Minute {
		t.Errorf("expected default interval 10m, got %v", inst.UpdateInterval())
	}
}

func TestWeather_SetLocation(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewWeather(logger, 1*time.Hour)

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}
	defer inst.Stop()

	// Set location
	inst.SetLocation("New York, NY")

	// Note: We don't test actual weather fetching in unit tests
	// as that requires network access and is slow
}

func TestWeather_SetUnit(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewWeather(logger, 1*time.Hour)

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	if err := inst.Start(ctx); err != nil {
		t.Fatalf("failed to start instrument: %v", err)
	}
	defer inst.Stop()

	// Set unit
	inst.SetUnit("metric")
	inst.SetUnit("imperial")

	// Setting same unit twice should not cause issues
	inst.SetUnit("imperial")
}

func TestWeather_InitialState(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	inst := NewWeather(logger, 1*time.Hour)

	// Before starting, should return nil
	if data := inst.GetCurrent(); data != nil {
		t.Error("expected nil weather data before starting")
	}
}

func TestWeatherCodeToIcon(t *testing.T) {
	tests := []struct {
		code  int
		isDay bool
		want  string
	}{
		{0, true, "\ue30d"},   // Clear sky day
		{0, false, "\ue32b"},  // Clear sky night
		{3, true, "\ue33d"},   // Cloudy
		{61, true, "\ue308"},  // Light rain day
		{95, false, "\ue32a"}, // Thunderstorm night
		{999, true, "❓"},     // Unknown code
	}

	for _, tt := range tests {
		got := weatherCodeToIcon(tt.code, tt.isDay)
		if got != tt.want {
			t.Errorf("weatherCodeToIcon(%d, %v) = %s, want %s", tt.code, tt.isDay, got, tt.want)
		}
	}
}

func TestWeatherCodeToCondition(t *testing.T) {
	tests := []struct {
		code int
		want string
	}{
		{0, "Clear"},
		{1, "Mainly Clear"},
		{2, "Partly Cloudy"},
		{45, "Foggy"},
		{61, "Light Rain"},
		{95, "Thunderstorm"},
		{999, "Unknown"},
	}

	for _, tt := range tests {
		got := weatherCodeToCondition(tt.code)
		if got != tt.want {
			t.Errorf("weatherCodeToCondition(%d) = %s, want %s", tt.code, got, tt.want)
		}
	}
}
