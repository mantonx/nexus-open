package config

import (
	"image/color"
	"path/filepath"
	"testing"
)

func newTestManager(t *testing.T) *Manager {
	t.Helper()
	dbPath := filepath.Join(t.TempDir(), "test.db")
	mgr, err := NewManagerFromPath(dbPath, nil)
	if err != nil {
		t.Fatalf("NewManagerFromPath() error = %v", err)
	}
	return mgr
}

func TestConfig_Validate(t *testing.T) {
	tests := []struct {
		name    string
		config  Config
		wantErr bool
		errType error
	}{
		{
			name:    "valid config",
			config:  Config{BackgroundColor: "#000000", TextColor: "#FFFFFF"},
			wantErr: false,
		},
		{
			name:    "invalid text color",
			config:  Config{BackgroundColor: "#000000", TextColor: "not-a-color"},
			wantErr: true,
			errType: ErrInvalidColor,
		},
		{
			name:    "invalid background color",
			config:  Config{BackgroundColor: "bad", TextColor: "#FFFFFF"},
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
	cfg := Config{TextColor: "#FF8040"}
	c, err := cfg.GetTextColor()
	if err != nil {
		t.Fatalf("GetTextColor() error = %v", err)
	}
	rgba, ok := c.(color.RGBA)
	if !ok {
		t.Fatalf("expected color.RGBA, got %T", c)
	}
	if rgba.R != 0xFF || rgba.G != 0x80 || rgba.B != 0x40 {
		t.Errorf("color = %v, want {R:255 G:128 B:64}", rgba)
	}
}

func TestManager_LoadCreateDefault(t *testing.T) {
	mgr := newTestManager(t)

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
	dbPath := filepath.Join(t.TempDir(), "test.db")
	mgr, err := NewManagerFromPath(dbPath, nil)
	if err != nil {
		t.Fatalf("NewManagerFromPath() error = %v", err)
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

	// Verify persistence — open a second manager on the same DB.
	mgr2, err := NewManagerFromPath(dbPath, nil)
	if err != nil {
		t.Fatalf("reload error = %v", err)
	}
	if got := mgr2.Get().BackgroundColor; got != newCfg.BackgroundColor {
		t.Errorf("after reload: BackgroundColor = %q, want %q", got, newCfg.BackgroundColor)
	}
}

func TestManager_UpdateInvalid(t *testing.T) {
	mgr := newTestManager(t)

	if err := mgr.Update(Config{BackgroundColor: "not-a-color", TextColor: "#FFFFFF"}); err == nil {
		t.Error("Update() should have failed with invalid config")
	}

	if mgr.Get().BackgroundColor == "not-a-color" {
		t.Error("config was updated despite validation error")
	}
}

func TestManager_Watch(t *testing.T) {
	mgr := newTestManager(t)

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
			t.Errorf("Watch received BackgroundColor = %q, want %q",
				received.BackgroundColor, newCfg.BackgroundColor)
		}
	default:
		t.Error("Watch channel did not receive update")
	}
}
