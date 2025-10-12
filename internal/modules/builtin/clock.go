// Package builtin contains built-in modules compiled into the host
package builtin

import (
	"time"

	"nexus-open/pkg/module"
)

// ClockModule displays current time and date
type ClockModule struct{}

// NewClock creates a new clock module
func NewClock() *ClockModule {
	return &ClockModule{}
}

// Describe returns module metadata
func (m *ClockModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Clock",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays current time and date",
		Icon:        "clock",
		RefreshMs:   1000, // Update every second
	}, nil
}

// Sample returns current time payload
func (m *ClockModule) Sample() (module.Payload, error) {
	now := time.Now()

	return module.Payload{
		Primary:   now.Format("15:04"),          // 24-hour time
		Secondary: now.Format("Mon, Jan 02"),    // Day, Month Date
		Severity:  module.SeverityOK,
		TTL:       1 * time.Second,
		Icon:      "clock",
		Timestamp: now,
	}, nil
}
