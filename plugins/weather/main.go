package main

import (
	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

func main() {
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: NewWeatherPlugin()},
		},
	})
}
