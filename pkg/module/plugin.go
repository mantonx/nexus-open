package module

import (
	"encoding/gob"
	"net/rpc"

	"github.com/hashicorp/go-plugin"
)

// Handshake is used to verify plugin compatibility
var Handshake = plugin.HandshakeConfig{
	ProtocolVersion:  1,
	MagicCookieKey:   "NEXUS_MODULE_PLUGIN",
	MagicCookieValue: "nexus-open-v2",
}

// ModulePlugin is the implementation of plugin.Plugin for go-plugin
type ModulePlugin struct {
	Impl Module
}

// Server returns the RPC server for this plugin
func (p *ModulePlugin) Server(*plugin.MuxBroker) (interface{}, error) {
	return &RPCServer{Impl: p.Impl}, nil
}

// Client returns the RPC client for this plugin
func (ModulePlugin) Client(b *plugin.MuxBroker, c *rpc.Client) (interface{}, error) {
	return &RPCClient{client: c}, nil
}

// RPCClient is the client-side implementation that calls the plugin
type RPCClient struct {
	client *rpc.Client
}

// Describe calls the remote Describe method
func (c *RPCClient) Describe() (Descriptor, error) {
	var resp Descriptor
	err := c.client.Call("Plugin.Describe", new(interface{}), &resp)
	return resp, err
}

// Sample calls the remote Sample method
func (c *RPCClient) Sample() (Payload, error) {
	var resp Payload
	err := c.client.Call("Plugin.Sample", new(interface{}), &resp)
	return resp, err
}

// RPCServer is the server-side implementation that serves the plugin
type RPCServer struct {
	Impl Module
}

// Describe implements the Describe RPC
func (s *RPCServer) Describe(args interface{}, resp *Descriptor) error {
	desc, err := s.Impl.Describe()
	*resp = desc
	return err
}

// Sample implements the Sample RPC
func (s *RPCServer) Sample(args interface{}, resp *Payload) error {
	payload, err := s.Impl.Sample()
	*resp = payload
	return err
}

func init() {
	// Register types for gob encoding
	gob.Register(Descriptor{})
	gob.Register(Payload{})
}
