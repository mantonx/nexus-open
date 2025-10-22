package config

import (
	"image/color"
	"os"
	"path/filepath"
	"testing"
)

func TestConfig_Validate(t *testing.T) {
	tests := []struct {
		name    string
		config  Config
		wantErr bool
		errType error
	}{
		{
			name: "valid config",
			config: Config{
				Location:        "New York, NY",
				TimeFormat:      "12h",
				Unit:            "imperial",
				BackgroundColor: "#000000",
				TextColor:       "#FFFFFF",
			},
			wantErr: false,
		},
		{
			name: "invalid time format",
			config: Config{
				Location:        "New York, NY",
				TimeFormat:      "invalid",
				Unit:            "imperial",
				BackgroundColor: "#000000",
				TextColor:       "#FFFFFF",
			},
			wantErr: true,
			errType: ErrInvalidTimeFormat,
		},
		{
			name: "invalid unit",
			config: Config{
				Location:        "New York, NY",
				TimeFormat:      "12h",
				Unit:            "invalid",
				BackgroundColor: "#000000",
				TextColor:       "#FFFFFF",
			},
			wantErr: true,
			errType: ErrInvalidUnit,
		},
		{
			name: "invalid text color",
			config: Config{
				Location:        "New York, NY",
				TimeFormat:      "12h",
				Unit:            "imperial",
				BackgroundColor: "#000000",
				TextColor:       "not-a-color",
			},
			wantErr: true,
			errType: ErrInvalidColor,
		},
		{
			name: "empty location",
			config: Config{
				Location:        "",
				TimeFormat:      "12h",
				Unit:            "imperial",
				BackgroundColor: "#000000",
				TextColor:       "#FFFFFF",
			},
			wantErr: true,
			errType: ErrInvalidLocation,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.config.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestConfig_GetTextColor(t *testing.T) {
	cfg := Config{
		TextColor: "#FF0000", // Red
	}

	col, err := cfg.GetTextColor()
	if err != nil {
		t.Fatalf("GetTextColor() error = %v", err)
	}

	expected := color.RGBA{R: 255, G: 0, B: 0, A: 255}
	if col != expected {
		t.Errorf("GetTextColor() = %v, want %v", col, expected)
	}
}

func TestConfig_GetBackgroundColor(t *testing.T) {
	cfg := Config{
		BackgroundColor: "#0000FF", // Blue
	}

	col, err := cfg.GetBackgroundColor()
	if err != nil {
		t.Fatalf("GetBackgroundColor() error = %v", err)
	}

	expected := color.RGBA{R: 0, G: 0, B: 255, A: 255}
	if col != expected {
		t.Errorf("GetBackgroundColor() = %v, want %v", col, expected)
	}
}

func TestIsValidHexColor(t *testing.T) {
	tests := []struct {
		input string
		valid bool
	}{
		{"#000000", true},
		{"#FFFFFF", true},
		{"#FF0000", true},
		{"#abc123", true},
		{"000000", false},   // Missing #
		{"#FFF", false},     // Too short
		{"#GGGGGG", false},  // Invalid hex
		{"#1234567", false}, // Too long
		{"", false},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := isValidHexColor(tt.input)
			if result != tt.valid {
				t.Errorf("isValidHexColor(%q) = %v, want %v", tt.input, result, tt.valid)
			}
		})
	}
}

func TestManager_LoadCreateDefault(t *testing.T) {
	// Create temp directory
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	// Create manager (should create default config)
	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	// Verify config was created
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		t.Error("Config file was not created")
	}

	// Verify default values
	cfg := mgr.Get()
	if cfg.Location != DefaultLocation {
		t.Errorf("Location = %q, want %q", cfg.Location, DefaultLocation)
	}
	if cfg.TimeFormat != DefaultTimeFormat {
		t.Errorf("TimeFormat = %q, want %q", cfg.TimeFormat, DefaultTimeFormat)
	}
	if cfg.Unit != DefaultUnit {
		t.Errorf("Unit = %q, want %q", cfg.Unit, DefaultUnit)
	}
}

func TestManager_Update(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	// Update configuration
	newCfg := Config{
		Location:        "San Francisco, CA",
		TimeFormat:      "24h",
		Unit:            "metric",
		BackgroundColor: "#111111",
		TextColor:       "#EEEEEE",
		BackgroundImage: "custom.png",
		ImagePaths:      []string{"img1.png", "img2.png"},
	}

	err = mgr.Update(newCfg)
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	// Verify update
	cfg := mgr.Get()
	if cfg.Location != newCfg.Location {
		t.Errorf("Location = %q, want %q", cfg.Location, newCfg.Location)
	}
	if cfg.TimeFormat != newCfg.TimeFormat {
		t.Errorf("TimeFormat = %q, want %q", cfg.TimeFormat, newCfg.TimeFormat)
	}
	if cfg.Unit != newCfg.Unit {
		t.Errorf("Unit = %q, want %q", cfg.Unit, newCfg.Unit)
	}

	// Create new manager to verify persistence
	mgr2, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	cfg2 := mgr2.Get()
	if cfg2.Location != newCfg.Location {
		t.Errorf("After reload: Location = %q, want %q", cfg2.Location, newCfg.Location)
	}
}

func TestManager_UpdateInvalid(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	// Try to update with invalid config
	invalidCfg := Config{
		Location:        "Test",
		TimeFormat:      "invalid", // Invalid!
		Unit:            "imperial",
		BackgroundColor: "#000000",
		TextColor:       "#FFFFFF",
	}

	err = mgr.Update(invalidCfg)
	if err == nil {
		t.Error("Update() should have failed with invalid config")
	}

	// Verify original config unchanged
	cfg := mgr.Get()
	if cfg.TimeFormat == "invalid" {
		t.Error("Config was updated despite validation error")
	}
}

func TestManager_Watch(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	// Register watcher
	ch := make(chan Config, 1)
	mgr.Watch(ch)

	// Update config
	newCfg := Config{
		Location:        "Boston, MA",
		TimeFormat:      "24h",
		Unit:            "metric",
		BackgroundColor: "#222222",
		TextColor:       "#DDDDDD",
		BackgroundImage: "test.png",
		ImagePaths:      []string{},
	}

	err = mgr.Update(newCfg)
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	// Verify watcher received update
	select {
	case received := <-ch:
		if received.Location != newCfg.Location {
			t.Errorf("Watcher received Location = %q, want %q", received.Location, newCfg.Location)
		}
	default:
		t.Error("Watcher did not receive config update")
	}
}
