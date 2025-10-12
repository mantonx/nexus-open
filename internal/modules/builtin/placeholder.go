package builtin

import (
	"time"

	"nexus-open/pkg/module"
)

// PlaceholderModule displays a placeholder message (for loading/errors)
type PlaceholderModule struct {
	message string
}

// NewPlaceholder creates a new placeholder module
func NewPlaceholder(message string) *PlaceholderModule {
	if message == "" {
		message = "Loading..."
	}
	return &PlaceholderModule{message: message}
}

// Describe returns module metadata
func (m *PlaceholderModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Placeholder",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays placeholder text",
		Icon:        "circle",
		RefreshMs:   5000,
	}, nil
}

// Sample returns placeholder payload
func (m *PlaceholderModule) Sample() (module.Payload, error) {
	return module.Payload{
		Primary:   "—",
		Secondary: m.message,
		Severity:  module.SeverityOK,
		TTL:       5 * time.Second,
		Timestamp: time.Now(),
	}, nil
}
