// hello is a simple example module that demonstrates the module interface
package main

import (
	"time"

	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// HelloPlugin is a simple example plugin
type HelloPlugin struct {
	counter int
}

// Describe returns module metadata
func (m *HelloPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Hello Module",
		Version:     "1.0.0",
		Author:      "Nexus Examples",
		Description: "Simple example plugin that says hello",
		Icon:        "hand-wave",
		RefreshMs:   2000,
	}, nil
}

// Sample returns a hello payload
func (m *HelloPlugin) Sample() (plugin.Payload, error) {
	m.counter++

	return plugin.Payload{
		Primary:   "Hello!",
		Secondary: "Example Module",
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
