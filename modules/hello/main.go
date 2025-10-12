// hello is a simple example module that demonstrates the module interface
package main

import (
	"time"

	"github.com/hashicorp/go-plugin"

	"nexus-open/pkg/module"
)

// HelloModule is a simple example module
type HelloModule struct {
	counter int
}

// Describe returns module metadata
func (m *HelloModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Hello Module",
		Version:     "1.0.0",
		Author:      "Nexus Examples",
		Description: "Simple example module that says hello",
		Icon:        "hand-wave",
		RefreshMs:   2000,
	}, nil
}

// Sample returns a hello payload
func (m *HelloModule) Sample() (module.Payload, error) {
	m.counter++

	return module.Payload{
		Primary:   "Hello!",
		Secondary: "Example Module",
		Severity:  module.SeverityOK,
		Spark:     []float32{0.2, 0.4, 0.6, 0.8, 1.0, 0.8, 0.6, 0.4},
		TTL:       2 * time.Second,
		Icon:      "hand-wave",
		Timestamp: time.Now(),
	}, nil
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: module.Handshake,
		Plugins: map[string]plugin.Plugin{
			"module": &module.ModulePlugin{Impl: &HelloModule{}},
		},
	})
}
