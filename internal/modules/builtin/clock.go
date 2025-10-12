// Package builtin contains built-in modules compiled into the host
package builtin

import (
	"time"

	"nexus-open/pkg/module"
)

// ClockModule displays current time and date with blinking colon
type ClockModule struct {
	showColon bool // Toggles every call for blinking effect
}

// NewClock creates a new clock module
func NewClock() *ClockModule {
	return &ClockModule{showColon: true}
}

// Describe returns module metadata
func (m *ClockModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Clock",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays current time and date with blinking colon",
		Icon:        "clock",
		RefreshMs:   500, // Update every 500ms for blink effect
	}, nil
}

// Sample returns current time payload
func (m *ClockModule) Sample() (module.Payload, error) {
	now := time.Now()

	// Toggle colon visibility for blinking effect
	m.showColon = !m.showColon

	var timeStr string
	if m.showColon {
		timeStr = now.Format("15:04") // "15:04" with colon
	} else {
		timeStr = now.Format("15 04") // "15 04" without colon (space instead)
	}

	return module.Payload{
		Primary:   timeStr,
		Secondary: now.Format("Mon, Jan 02"), // Day, Month Date
		Severity:  module.SeverityOK,
		TTL:       500 * time.Millisecond,
		Icon:      "clock",
		Timestamp: now,
	}, nil
}
