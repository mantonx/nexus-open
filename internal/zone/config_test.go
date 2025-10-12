package zone

import (
	"testing"
)

func TestConfigValidation(t *testing.T) {
	tests := []struct {
		name    string
		config  Config
		wantErr bool
	}{
		{
			name: "valid config",
			config: Config{
				Name:    "Test Layout",
				Version: "1.0",
				Theme:   DefaultTheme(),
				Pages: []Page{
					{
						Name: "Main",
						Zones: []ZoneConfig{
							{ID: "zone1", Width: 160, Module: "builtin:clock", RefreshMs: 1000},
							{ID: "zone2", Width: 160, Module: "builtin:clock", RefreshMs: 1000},
							{ID: "zone3", Width: 160, Module: "builtin:clock", RefreshMs: 1000},
							{ID: "zone4", Width: 160, Module: "builtin:clock", RefreshMs: 1000},
						},
					},
				},
			},
			wantErr: false,
		},
		{
			name: "missing name",
			config: Config{
				Pages: []Page{{Name: "Main", Zones: []ZoneConfig{{ID: "z1", Width: 640, Module: "test", RefreshMs: 1000}}}},
			},
			wantErr: true,
		},
		{
			name: "no pages",
			config: Config{
				Name:  "Test",
				Pages: []Page{},
			},
			wantErr: true,
		},
		{
			name: "zone widths don't sum to 640",
			config: Config{
				Name: "Test",
				Pages: []Page{
					{
						Name: "Main",
						Zones: []ZoneConfig{
							{ID: "zone1", Width: 200, Module: "test", RefreshMs: 1000},
							{ID: "zone2", Width: 200, Module: "test", RefreshMs: 1000},
						},
					},
				},
			},
			wantErr: true,
		},
		{
			name: "zone width too small",
			config: Config{
				Name: "Test",
				Pages: []Page{
					{
						Name: "Main",
						Zones: []ZoneConfig{
							{ID: "zone1", Width: 50, Module: "test", RefreshMs: 1000},
							{ID: "zone2", Width: 590, Module: "test", RefreshMs: 1000},
						},
					},
				},
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.config.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Config.Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestPageComputeOffsets(t *testing.T) {
	page := Page{
		Name: "Test",
		Zones: []ZoneConfig{
			{ID: "zone1", Width: 160, Module: "test", RefreshMs: 1000},
			{ID: "zone2", Width: 200, Module: "test", RefreshMs: 1000},
			{ID: "zone3", Width: 120, Module: "test", RefreshMs: 1000},
			{ID: "zone4", Width: 160, Module: "test", RefreshMs: 1000},
		},
	}

	page.ComputeOffsets()

	expectedOffsets := []int{0, 160, 360, 480}

	for i, zone := range page.Zones {
		if zone.X != expectedOffsets[i] {
			t.Errorf("Zone %d: got X=%d, want %d", i, zone.X, expectedOffsets[i])
		}
	}
}

func TestThemeColors(t *testing.T) {
	theme := DefaultTheme()

	// Test parsing valid colors
	bgColor := theme.GetBgColor()
	if bgColor.R != 0x10 || bgColor.G != 0x10 || bgColor.B != 0x10 {
		t.Errorf("Background color incorrect: got R=%d G=%d B=%d", bgColor.R, bgColor.G, bgColor.B)
	}

	accentColor := theme.GetAccentColor()
	if accentColor.R != 0x00 || accentColor.G != 0xC8 || accentColor.B != 0xFF {
		t.Errorf("Accent color incorrect: got R=%d G=%d B=%d", accentColor.R, accentColor.G, accentColor.B)
	}
}

func TestZoneConfigValidation(t *testing.T) {
	tests := []struct {
		name    string
		zone    ZoneConfig
		wantErr bool
	}{
		{
			name:    "valid zone",
			zone:    ZoneConfig{ID: "test", Width: 160, Module: "builtin:clock", RefreshMs: 1000},
			wantErr: false,
		},
		{
			name:    "missing ID",
			zone:    ZoneConfig{Width: 160, Module: "test", RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "width too small",
			zone:    ZoneConfig{ID: "test", Width: 50, Module: "test", RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "width too large",
			zone:    ZoneConfig{ID: "test", Width: 700, Module: "test", RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "missing module",
			zone:    ZoneConfig{ID: "test", Width: 160, RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "refresh too fast",
			zone:    ZoneConfig{ID: "test", Width: 160, Module: "test", RefreshMs: 50},
			wantErr: true,
		},
		{
			name:    "invalid alignment",
			zone:    ZoneConfig{ID: "test", Width: 160, Module: "test", RefreshMs: 1000, Align: "invalid"},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.zone.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("ZoneConfig.Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}
