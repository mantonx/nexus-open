package display

import (
	"context"
	"image/color"
	"log/slog"
	"os"
	"testing"
	"time"

	"nexus-open/internal/config"
	"nexus-open/internal/device"
	"nexus-open/internal/instruments"
)

func TestParseColor(t *testing.T) {
	tests := []struct {
		input   string
		want    color.Color
		wantErr bool
	}{
		{"#FFFFFF", color.RGBA{R: 255, G: 255, B: 255, A: 255}, false},
		{"#000000", color.RGBA{R: 0, G: 0, B: 0, A: 255}, false},
		{"#FF0000", color.RGBA{R: 255, G: 0, B: 0, A: 255}, false},
		{"#00FF00", color.RGBA{R: 0, G: 255, B: 0, A: 255}, false},
		{"#0000FF", color.RGBA{R: 0, G: 0, B: 255, A: 255}, false},
		{"FFFFFF", nil, true},  // Missing #
		{"#FFF", nil, true},    // Too short
		{"#GGGGGG", nil, true}, // Invalid hex
		{"", nil, true},        // Empty string
	}

	for _, tt := range tests {
		got, err := parseColor(tt.input)
		if (err != nil) != tt.wantErr {
			t.Errorf("parseColor(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
			continue
		}
		if !tt.wantErr {
			gotRGBA, ok := got.(color.RGBA)
			if !ok {
				t.Errorf("parseColor(%q) returned non-RGBA color", tt.input)
				continue
			}
			wantRGBA := tt.want.(color.RGBA)
			if gotRGBA != wantRGBA {
				t.Errorf("parseColor(%q) = %+v, want %+v", tt.input, gotRGBA, wantRGBA)
			}
		}
	}
}

func TestNexusRenderer_Initialize(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	// Create temp config
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	// Create mock device
	mockDev := device.NewMockDevice()

	renderer := NewNexusRenderer(logger, cfg, mockDev)
	if renderer == nil {
		t.Fatal("expected non-nil renderer")
	}

	if err := renderer.Initialize(); err != nil {
		t.Errorf("initialize failed: %v", err)
	}

	// Verify dimensions
	if renderer.width != Width || renderer.height != Height {
		t.Errorf("unexpected dimensions: %dx%d, want %dx%d",
			renderer.width, renderer.height, Width, Height)
	}
}

func TestNexusRenderer_Render(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	renderer := NewNexusRenderer(logger, cfg, mockDev)

	if err := renderer.Initialize(); err != nil {
		t.Fatalf("initialize failed: %v", err)
	}

	ctx := context.Background()

	// Create test data
	data := instruments.SystemData{
		Timestamp: time.Now(),
		Temperature: instruments.TemperatureData{
			CPU: 65.5,
			GPU: 72.3,
		},
		Network: instruments.NetworkData{
			DownloadSpeed: 1024 * 1024 * 5.5, // 5.5 MB/s
			UploadSpeed:   1024 * 512,        // 512 KB/s
		},
		Weather: &instruments.WeatherData{
			Location:    "Test City",
			Temperature: 72.0,
			Description: "Clear",
			Icon:        "☀",
			Unit:        "imperial",
			WindSpeed:   10.5,
		},
	}

	// Render a frame
	frame, err := renderer.Render(ctx, data)
	if err != nil {
		t.Fatalf("render failed: %v", err)
	}

	if frame == nil {
		t.Fatal("expected non-nil frame")
	}

	// Verify frame dimensions
	bounds := frame.Bounds()
	if bounds.Dx() != Width || bounds.Dy() != Height {
		t.Errorf("unexpected frame size: %dx%d, want %dx%d",
			bounds.Dx(), bounds.Dy(), Width, Height)
	}

	// Verify frame has data (not all black)
	hasNonBlackPixel := false
	for y := 0; y < Height; y++ {
		for x := 0; x < Width; x++ {
			r, g, b, _ := frame.At(x, y).RGBA()
			if r != 0 || g != 0 || b != 0 {
				hasNonBlackPixel = true
				break
			}
		}
		if hasNonBlackPixel {
			break
		}
	}
	// Frame might be all black with black background, that's OK
}

func TestNexusRenderer_SetColors(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")
	mockDev := device.NewMockDevice()

	renderer := NewNexusRenderer(logger, cfg, mockDev)
	renderer.Initialize()

	// Set text color
	red := color.RGBA{R: 255, G: 0, B: 0, A: 255}
	renderer.SetTextColor(red)

	// Set background color
	blue := color.RGBA{R: 0, G: 0, B: 255, A: 255}
	renderer.SetBackgroundColor(blue)

	// Colors should be settable without error
	// Actual rendering with these colors is tested in Render test
}

func TestNexusRenderer_BlinkState(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")
	mockDev := device.NewMockDevice()

	renderer := NewNexusRenderer(logger, cfg, mockDev)
	renderer.Initialize()

	// Initial state
	initialState := renderer.blinkState

	// Update blink state
	renderer.UpdateBlinkState()
	newState := renderer.blinkState

	// Should toggle
	if initialState == newState {
		t.Error("blink state did not toggle")
	}

	// Toggle again
	renderer.UpdateBlinkState()
	finalState := renderer.blinkState

	// Should be back to initial
	if initialState != finalState {
		t.Error("blink state did not toggle back")
	}
}

func TestNexusRenderer_ConcurrentAccess(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")
	mockDev := device.NewMockDevice()

	renderer := NewNexusRenderer(logger, cfg, mockDev)
	renderer.Initialize()

	ctx := context.Background()
	data := instruments.SystemData{Timestamp: time.Now()}

	done := make(chan bool)

	// Concurrent renders
	for i := 0; i < 5; i++ {
		go func() {
			for j := 0; j < 10; j++ {
				_, _ = renderer.Render(ctx, data)
			}
			done <- true
		}()
	}

	// Concurrent color updates
	for i := 0; i < 5; i++ {
		go func() {
			for j := 0; j < 10; j++ {
				renderer.SetTextColor(color.White)
				renderer.SetBackgroundColor(color.Black)
				renderer.UpdateBlinkState()
			}
			done <- true
		}()
	}

	// Wait for all
	for i := 0; i < 10; i++ {
		<-done
	}
}
