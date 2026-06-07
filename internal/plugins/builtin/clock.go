// Package builtin contains built-in modules compiled into the host
package builtin

import (
	"time"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// ClockPlugin displays current time and date with blinking colon
type ClockPlugin struct {
	showColon bool
	format    ClockFormat
}

// ClockFormat describes the time format for the clock plugin.
type ClockFormat int

const (
	ClockFormat12Hour ClockFormat = iota
	ClockFormat24Hour
)

// NewClock creates a new clock plugin
func NewClock() *ClockPlugin {
	return NewClockWithFormat(ClockFormat12Hour)
}

// NewClockWithFormat creates a clock plugin with the requested format.
func NewClockWithFormat(format ClockFormat) *ClockPlugin {
	if format != ClockFormat24Hour {
		format = ClockFormat12Hour
	}
	return &ClockPlugin{showColon: true, format: format}
}

// NewClock24 returns a 24-hour clock plugin.
func NewClock24() *ClockPlugin {
	return NewClockWithFormat(ClockFormat24Hour)
}

// Describe returns plugin metadata
func (m *ClockPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Clock",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays current time and date with blinking colon",
		Icon:        "clock",
		RefreshMs:   1000,
		Schema: plugin.ConfigSchema{
			Fields: []plugin.ConfigField{
				{
					Key:     "clock_format",
					Label:   "Format",
					Type:    plugin.FieldTypeEnum,
					Default: "12h",
					Options: []plugin.FieldOption{
						{Value: "12h", Label: "12-hour"},
						{Value: "24h", Label: "24-hour"},
					},
				},
			},
		},
	}, nil
}

// Sample returns current time payload
func (m *ClockPlugin) Sample() (plugin.Payload, error) {
	now := time.Now()

	// Toggle colon visibility for blinking effect
	m.showColon = !m.showColon

	timeStr := m.formatTime(now, m.showColon)

	return plugin.Payload{
		Primary:   timeStr,
		Secondary: now.Format("Mon, Jan 02"), // Day, Month Date
		Severity:  plugin.SeverityOK,
		TTL:       2 * time.Second, // Allow comfortable slack vs refresh to avoid stale flashes
		Icon:      "clock",
		Timestamp: now,
	}, nil
}

func (m *ClockPlugin) formatTime(t time.Time, showColon bool) string {
	switch m.format {
	case ClockFormat24Hour:
		if showColon {
			return t.Format("15:04")
		}
		return t.Format("15 04")
	default:
		if showColon {
			return t.Format("3:04 PM")
		}
		return t.Format("3 04 PM")
	}
}

// Configure applies zone-level plugin configuration.
func (m *ClockPlugin) Configure(cfg map[string]any) error {
	if format, ok := cfg["clock_format"].(string); ok {
		switch format {
		case "24h", "24hour", "24":
			m.format = ClockFormat24Hour
		case "12h", "12hour", "12":
			m.format = ClockFormat12Hour
		}
	}
	return nil
}
