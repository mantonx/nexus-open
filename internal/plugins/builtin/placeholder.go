package builtin

import (
	"time"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// PlaceholderPlugin displays a placeholder message (for loading/errors)
type PlaceholderPlugin struct {
	message string
}

// NewPlaceholder creates a new placeholder plugin
func NewPlaceholder(message string) *PlaceholderPlugin {
	if message == "" {
		message = "Loading..."
	}
	return &PlaceholderPlugin{message: message}
}

// Describe returns plugin metadata
func (m *PlaceholderPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Placeholder",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays placeholder text",
		Icon:        "circle",
		RefreshMs:   5000,
	}, nil
}

// Sample returns placeholder payload
func (m *PlaceholderPlugin) Sample() (plugin.Payload, error) {
	return plugin.Payload{
		Primary:   "—",
		Secondary: m.message,
		Severity:  plugin.SeverityOK,
		TTL:       5 * time.Second,
		Timestamp: time.Now(),
	}, nil
}

// Configure implements plugin.Plugin. Placeholder has no configurable fields.
func (m *PlaceholderPlugin) Configure(_ map[string]any) error { return nil }
