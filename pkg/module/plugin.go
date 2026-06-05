package module

// This file implements the RPC transport layer for exec: plugins — plugins
// that run as external subprocesses. It uses hashicorp/go-plugin for
// subprocess management and net/rpc for communication.
//
// Plugin authors call plugin.Serve() in their main(). The host side
// (internal/plugin/host) handles launching and communicating with it.

import (
	"encoding/gob"
	"net/rpc"

	"github.com/hashicorp/go-plugin"
)

// Handshake ensures the host and plugin binary are compatible.
var Handshake = plugin.HandshakeConfig{
	ProtocolVersion:  1,
	MagicCookieKey:   "NEXUS_EXEC_MODULE",
	MagicCookieValue: "nexus-open-v2",
}

// ExecPlugin is the go-plugin bridge for exec: plugins.
type ExecPlugin struct {
	Impl Plugin
}

func (p *ExecPlugin) Server(*plugin.MuxBroker) (interface{}, error) {
	return &pluginRPC{Impl: p.Impl}, nil
}

func (ExecPlugin) Client(b *plugin.MuxBroker, c *rpc.Client) (interface{}, error) {
	return &rpcClient{client: c}, nil
}

// rpcClient is the host-side stub that calls the plugin over net/rpc.
type rpcClient struct {
	client *rpc.Client
}

func (c *rpcClient) Describe() (Descriptor, error) {
	var resp Descriptor
	err := c.client.Call("Plugin.Describe", new(interface{}), &resp)
	return resp, err
}

func (c *rpcClient) Sample() (Payload, error) {
	var resp Payload
	err := c.client.Call("Plugin.Sample", new(interface{}), &resp)
	return resp, err
}

func (c *rpcClient) OnConfigChanged(config map[string]interface{}) error {
	var resp interface{}
	return c.client.Call("Plugin.OnConfigChanged", config, &resp)
}

// pluginRPC is the plugin-side handler that serves requests from the host.
type pluginRPC struct {
	Impl Plugin
}

func (s *pluginRPC) Describe(args interface{}, resp *Descriptor) error {
	desc, err := s.Impl.Describe()
	*resp = desc
	return err
}

func (s *pluginRPC) Sample(args interface{}, resp *Payload) error {
	payload, err := s.Impl.Sample()
	*resp = payload
	return err
}

func (s *pluginRPC) OnConfigChanged(config map[string]interface{}, resp *interface{}) error {
	if notifier, ok := SupportsPluginConfig(s.Impl); ok {
		return notifier.OnConfigChanged(config)
	}
	return nil
}

func init() {
	gob.Register(Descriptor{})
	gob.Register(Payload{})
	gob.Register(map[string]interface{}{})
}
