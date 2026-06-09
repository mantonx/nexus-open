// hello is a simple example module that demonstrates the plugin interface.
// It has no configurable fields, but implements Configure to satisfy the
// Plugin interface and shows the schema pattern with an optional "greeting".
package main

import (
	"time"

	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

// HelloPlugin is a simple example plugin
type HelloPlugin struct {
	counter  int
	greeting string
}

// Describe returns plugin metadata
func (m *HelloPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Hello",
		Version:     "1.0.0",
		Author:      "Nexus Examples",
		Description: "Simple example plugin — useful as a starting point for custom plugins",
		Icon:        "hand-wave",
		RefreshMs:   2000,
		HasGraph:    true,
		Schema: plugin.ConfigSchema{
			Fields: []plugin.ConfigField{
				{
					Key:     "greeting",
					Label:   "Greeting",
					Type:    plugin.FieldTypeString,
					Default: "Hello!",
					Help:    "Text shown as the primary value",
				},
			},
		},
	}, nil
}

// Configure applies user-supplied config values.
func (m *HelloPlugin) Configure(cfg map[string]any) error {
	if v, ok := cfg["greeting"].(string); ok && v != "" {
		m.greeting = v
	}
	return nil
}

// Sample returns a hello payload
func (m *HelloPlugin) Sample() (plugin.Payload, error) {
	m.counter++
	primary := m.greeting
	if primary == "" {
		primary = "Hello!"
	}

	return plugin.Payload{
		Primary:   primary,
		Secondary: "Example Plugin",
		Severity:  plugin.SeverityOK,
		Spark:     []float32{0.2, 0.4, 0.6, 0.8, 1.0, 0.8, 0.6, 0.4},
		TTL:       2 * time.Second,
		Icon:      "hand-wave",
		Timestamp: time.Now(),
	}, nil
}

func main() {
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: &HelloPlugin{}},
		},
	})
}
