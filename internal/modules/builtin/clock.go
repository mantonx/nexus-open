// Package builtin contains built-in modules compiled into the host
package builtin

import (
	"time"

	"github.com/mantonx/nexus-next/pkg/module"
)

// ClockPlugin displays current time and date with blinking colon
type ClockPlugin struct {
	showColon bool
	format    ClockFormat
}

// ClockFormat describes the time format for the clock module.
type ClockFormat int

const (
	ClockFormat12Hour ClockFormat = iota
	ClockFormat24Hour
)

// NewClock creates a new clock module
func NewClock() *ClockPlugin {
	return NewClockWithFormat(ClockFormat12Hour)
}

// NewClockWithFormat creates a clock module with the requested format.
func NewClockWithFormat(format ClockFormat) *ClockPlugin {
	if format != ClockFormat24Hour {
		format = ClockFormat12Hour
	}
	return &ClockPlugin{showColon: true, format: format}
}

// NewClock24 returns a 24-hour clock module.
func NewClock24() *ClockPlugin {
	return NewClockWithFormat(ClockFormat24Hour)
}

// Describe returns module metadata
func (m *ClockPlugin) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Clock",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays current time and date with blinking colon",
		Icon:        "clock",
		RefreshMs:   1000, // Default 1s refresh to match typical digital clocks
	}, nil
}

// Sample returns current time payload
func (m *ClockPlugin) Sample() (module.Payload, error) {
	now := time.Now()

	// Toggle colon visibility for blinking effect
	m.showColon = !m.showColon

	timeStr := m.formatTime(now, m.showColon)

	return module.Payload{
		Primary:   timeStr,
		Secondary: now.Format("Mon, Jan 02"), // Day, Month Date
		Severity:  module.SeverityOK,
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

// OnConfigChanged implements module.PluginConfigNotifier interface.
// Clock module supports configuring the time format (12h or 24h).
func (m *ClockPlugin) OnConfigChanged(config map[string]interface{}) error {
	// Check for clock_format configuration
	if format, ok := config["clock_format"].(string); ok {
		switch format {
		case "24h", "24hour", "24":
			m.format = ClockFormat24Hour
		case "12h", "12hour", "12":
			m.format = ClockFormat12Hour
		}
	}
	return nil
}
