package builtin

import (
	"time"

	"github.com/mantonx/nexus-next/pkg/module"
)

// PlaceholderPlugin displays a placeholder message (for loading/errors)
type PlaceholderPlugin struct {
	message string
}

// NewPlaceholder creates a new placeholder module
func NewPlaceholder(message string) *PlaceholderPlugin {
	if message == "" {
		message = "Loading..."
	}
	return &PlaceholderPlugin{message: message}
}

// Describe returns module metadata
func (m *PlaceholderPlugin) Describe() (module.Descriptor, error) {
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
func (m *PlaceholderPlugin) Sample() (module.Payload, error) {
	return module.Payload{
		Primary:   "—",
		Secondary: m.message,
		Severity:  module.SeverityOK,
		TTL:       5 * time.Second,
		Timestamp: time.Now(),
	}, nil
}

// OnConfigChanged implements module.PluginConfigNotifier interface.
// Placeholder module doesn't use configuration, so this is a no-op.
func (m *PlaceholderPlugin) OnConfigChanged(config map[string]interface{}) error {
	// Placeholder module doesn't need configuration
	return nil
}
