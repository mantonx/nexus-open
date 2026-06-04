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
				BackgroundColor: "#000000",
				TextColor:       "#FFFFFF",
			},
			wantErr: false,
		},
		{
			name: "invalid text color",
			config: Config{
				BackgroundColor: "#000000",
				TextColor:       "not-a-color",
			},
			wantErr: true,
			errType: ErrInvalidColor,
		},
		{
			name: "invalid background color",
			config: Config{
				BackgroundColor: "bad",
				TextColor:       "#FFFFFF",
			},
			wantErr: true,
			errType: ErrInvalidColor,
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
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	// Verify config file was created
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		t.Error("Config file was not created")
	}

	// Verify default values
	cfg := mgr.Get()
	if cfg.BackgroundColor != DefaultBackgroundColor {
		t.Errorf("BackgroundColor = %q, want %q", cfg.BackgroundColor, DefaultBackgroundColor)
	}
	if cfg.TextColor != DefaultTextColor {
		t.Errorf("TextColor = %q, want %q", cfg.TextColor, DefaultTextColor)
	}
	if cfg.Display.FontFamily != DefaultFontFamily {
		t.Errorf("Display.FontFamily = %q, want %q", cfg.Display.FontFamily, DefaultFontFamily)
	}
}

func TestManager_Update(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	newCfg := Config{
		BackgroundColor: "#111111",
		TextColor:       "#EEEEEE",
		BackgroundImage: "custom.png",
		ImagePaths:      []string{"img1.png", "img2.png"},
	}

	if err := mgr.Update(newCfg); err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	cfg := mgr.Get()
	if cfg.BackgroundColor != newCfg.BackgroundColor {
		t.Errorf("BackgroundColor = %q, want %q", cfg.BackgroundColor, newCfg.BackgroundColor)
	}
	if cfg.TextColor != newCfg.TextColor {
		t.Errorf("TextColor = %q, want %q", cfg.TextColor, newCfg.TextColor)
	}

	// Verify persistence
	mgr2, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() reload error = %v", err)
	}
	cfg2 := mgr2.Get()
	if cfg2.BackgroundColor != newCfg.BackgroundColor {
		t.Errorf("After reload: BackgroundColor = %q, want %q", cfg2.BackgroundColor, newCfg.BackgroundColor)
	}
}

func TestManager_UpdateInvalid(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.yaml")

	mgr, err := NewManager(configPath)
	if err != nil {
		t.Fatalf("NewManager() error = %v", err)
	}

	invalidCfg := Config{
		BackgroundColor: "not-a-color", // Invalid!
		TextColor:       "#FFFFFF",
	}

	if err := mgr.Update(invalidCfg); err == nil {
		t.Error("Update() should have failed with invalid config")
	}

	// Verify original config unchanged
	cfg := mgr.Get()
	if cfg.BackgroundColor == "not-a-color" {
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

	ch := make(chan Config, 1)
	mgr.Watch(ch)

	newCfg := Config{
		BackgroundColor: "#222222",
		TextColor:       "#DDDDDD",
		BackgroundImage: "test.png",
		ImagePaths:      []string{},
	}

	if err := mgr.Update(newCfg); err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	select {
	case received := <-ch:
		if received.BackgroundColor != newCfg.BackgroundColor {
			t.Errorf("Watcher received BackgroundColor = %q, want %q", received.BackgroundColor, newCfg.BackgroundColor)
		}
	default:
		t.Error("Watcher did not receive config update")
	}
}
