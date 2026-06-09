package zone

import (
	"fmt"
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
							{ID: "zone1", Width: 160, Plugin: "builtin:clock", RefreshMs: 1000},
							{ID: "zone2", Width: 160, Plugin: "builtin:clock", RefreshMs: 1000},
							{ID: "zone3", Width: 160, Plugin: "builtin:clock", RefreshMs: 1000},
							{ID: "zone4", Width: 160, Plugin: "builtin:clock", RefreshMs: 1000},
						},
					},
				},
			},
			wantErr: false,
		},
		{
			name: "missing name",
			config: Config{
				Pages: []Page{{Name: "Main", Zones: []ZoneConfig{{ID: "z1", Width: 640, Plugin: "test", RefreshMs: 1000}}}},
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
							{ID: "zone1", Width: 200, Plugin: "test", RefreshMs: 1000},
							{ID: "zone2", Width: 200, Plugin: "test", RefreshMs: 1000},
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
							{ID: "zone1", Width: 50, Plugin: "test", RefreshMs: 1000},
							{ID: "zone2", Width: 590, Plugin: "test", RefreshMs: 1000},
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
			{ID: "zone1", Width: 160, Plugin: "test", RefreshMs: 1000},
			{ID: "zone2", Width: 200, Plugin: "test", RefreshMs: 1000},
			{ID: "zone3", Width: 120, Plugin: "test", RefreshMs: 1000},
			{ID: "zone4", Width: 160, Plugin: "test", RefreshMs: 1000},
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

	// Background and accent must match design tokens.
	bgColor := theme.GetBgColor()
	if bgColor.R != 0x05 || bgColor.G != 0x05 || bgColor.B != 0x05 {
		t.Errorf("Background color incorrect: got R=%d G=%d B=%d", bgColor.R, bgColor.G, bgColor.B)
	}

	accentColor := theme.GetAccentColor()
	if accentColor.R != 0x5A || accentColor.G != 0xA0 || accentColor.B != 0xE0 {
		t.Errorf("Accent color incorrect: got R=%d G=%d B=%d", accentColor.R, accentColor.G, accentColor.B)
	}
}

func TestPage_MaxZonesEnforced(t *testing.T) {
	zones := make([]ZoneConfig, MaxZonesPerPage+1)
	for i := range zones {
		zones[i] = ZoneConfig{ID: fmt.Sprintf("z%d", i), Width: 80, Plugin: "builtin:clock", RefreshMs: 1000}
	}
	// Force widths to sum to 640 for the over-cap test.
	zones[0].Width = 640 - 80*MaxZonesPerPage
	p := Page{Name: "Test", Zones: zones}
	if err := p.Validate(); err == nil {
		t.Error("expected error for page with more than MaxZonesPerPage zones")
	}
}

func TestPage_RedistributeWidths(t *testing.T) {
	tests := []struct {
		name    string
		n       int
		total   int
		floor   int
		want    []int
		wantErr bool
	}{
		{name: "1 zone",  n: 1, total: 640, floor: 80, want: []int{640}},
		{name: "2 zones", n: 2, total: 640, floor: 80, want: []int{320, 320}},
		{name: "3 zones", n: 3, total: 640, floor: 80, want: []int{214, 213, 213}},
		{name: "6 zones", n: 6, total: 640, floor: 80, want: []int{107, 107, 107, 107, 106, 106}},
		{name: "floor collision", n: 9, total: 640, floor: 80, wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := Page{Name: "p"}
			for i := range tt.n {
				p.Zones = append(p.Zones, ZoneConfig{ID: fmt.Sprintf("z%d", i)})
			}
			err := p.RedistributeWidths(tt.total, tt.floor)
			if (err != nil) != tt.wantErr {
				t.Fatalf("RedistributeWidths() error = %v, wantErr %v", err, tt.wantErr)
			}
			if tt.wantErr {
				return
			}
			sum := 0
			for i, z := range p.Zones {
				sum += z.Width
				if z.Width != tt.want[i] {
					t.Errorf("zone[%d] width = %d, want %d", i, z.Width, tt.want[i])
				}
			}
			if sum != tt.total {
				t.Errorf("total width = %d, want %d", sum, tt.total)
			}
		})
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
			zone:    ZoneConfig{ID: "test", Width: 160, Plugin: "builtin:clock", RefreshMs: 1000},
			wantErr: false,
		},
		{
			name:    "missing ID",
			zone:    ZoneConfig{Width: 160, Plugin: "test", RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "width too small",
			zone:    ZoneConfig{ID: "test", Width: 50, Plugin: "test", RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "width too large",
			zone:    ZoneConfig{ID: "test", Width: 700, Plugin: "test", RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "missing plugin",
			zone:    ZoneConfig{ID: "test", Width: 160, RefreshMs: 1000},
			wantErr: true,
		},
		{
			name:    "refresh too fast",
			zone:    ZoneConfig{ID: "test", Width: 160, Plugin: "test", RefreshMs: 50},
			wantErr: true,
		},
		{
			name:    "invalid alignment",
			zone:    ZoneConfig{ID: "test", Width: 160, Plugin: "test", RefreshMs: 1000, Align: "invalid"},
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
