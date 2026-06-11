package plugin

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
	ProtocolVersion:  2,
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
	client      *rpc.Client
	tapProbed   bool
	tapSupported bool
}

// SupportsTap returns true if the remote plugin implements Tapper.
// The result is probed once via RPC and cached for all subsequent calls.
func (c *rpcClient) SupportsTap() bool {
	if c.tapProbed {
		return c.tapSupported
	}
	var supported bool
	err := c.client.Call("Plugin.SupportsTap", new(any), &supported)
	c.tapProbed = true
	c.tapSupported = err == nil && supported
	return c.tapSupported
}

func (c *rpcClient) Describe() (Descriptor, error) {
	var resp Descriptor
	err := c.client.Call("Plugin.Describe", new(any), &resp)
	return resp, err
}

func (c *rpcClient) Sample() (Payload, error) {
	var resp Payload
	err := c.client.Call("Plugin.Sample", new(any), &resp)
	return resp, err
}

func (c *rpcClient) Configure(cfg map[string]any) error {
	var resp any
	return c.client.Call("Plugin.Configure", cfg, &resp)
}

// OnTap implements Tapper over RPC. Returns ErrNotTapper when the plugin's
// Impl does not implement Tapper — same semantic as a failed type assertion.
func (c *rpcClient) OnTap() (DetailPayload, error) {
	var resp DetailPayload
	err := c.client.Call("Plugin.OnTap", new(any), &resp)
	return resp, err
}

// pluginRPC is the plugin-side handler that serves requests from the host.
type pluginRPC struct {
	Impl Plugin
}

func (s *pluginRPC) Describe(args any, resp *Descriptor) error {
	desc, err := s.Impl.Describe()
	*resp = desc
	return err
}

func (s *pluginRPC) Sample(args any, resp *Payload) error {
	payload, err := s.Impl.Sample()
	*resp = payload
	return err
}

func (s *pluginRPC) Configure(cfg map[string]any, resp *any) error {
	return s.Impl.Configure(cfg)
}

func (s *pluginRPC) SupportsTap(args any, resp *bool) error {
	_, ok := s.Impl.(Tapper)
	*resp = ok
	return nil
}

func (s *pluginRPC) OnTap(args any, resp *DetailPayload) error {
	tapper, ok := s.Impl.(Tapper)
	if !ok {
		return ErrNotTapper
	}
	detail, err := tapper.OnTap()
	*resp = detail
	return err
}

func init() {
	gob.Register(Descriptor{})
	gob.Register(ConfigSchema{})
	gob.Register(ConfigField{})
	gob.Register(FieldOption{})
	gob.Register(Payload{})
	gob.Register(DetailPayload{})
	gob.Register(map[string]any{})
	gob.Register([]any{})
}
