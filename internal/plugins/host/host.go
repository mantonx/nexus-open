// Package host manages the lifecycle of external plugins — plugins that run as
// separate processes and communicate with the host over net/rpc.
//
// This package handles
// the subprocess and RPC transport details for the exec: plugin type.
package host

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"sync"
	"time"

	"github.com/hashicorp/go-hclog"
	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// Host manages the lifecycle of external (subprocess) plugins.
type Host struct {
	logger  *slog.Logger
	clients map[string]*pluginClient
	mu      sync.RWMutex
}

// pluginClient represents a single running external plugin.
type pluginClient struct {
	client *goplugin.Client
	plugin plugin.Plugin
	path   string
}

// NewHost creates a new plugin host.
func NewHost(logger *slog.Logger) *Host {
	return &Host{
		logger:  logger,
		clients: make(map[string]*pluginClient),
	}
}

// LaunchPlugin starts an external plugin subprocess and returns the Plugin
// interface. If the plugin is already running, the existing instance is returned.
func (h *Host) LaunchPlugin(ctx context.Context, id, path string) (plugin.Plugin, error) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if existing, ok := h.clients[id]; ok {
		h.logger.Debug("reusing running plugin", "id", id, "path", path)
		return existing.plugin, nil
	}

	client := goplugin.NewClient(&goplugin.ClientConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{},
		},
		Cmd:              exec.Command(path),
		AllowedProtocols: []goplugin.Protocol{goplugin.ProtocolNetRPC},
		Logger: hclog.New(&hclog.LoggerOptions{
			Name:   "plugin",
			Output: os.Stderr,
			Level:  hclog.Error,
		}),
	})

	rpcClient, err := client.Client()
	if err != nil {
		client.Kill()
		return nil, fmt.Errorf("failed to connect to plugin: %w", err)
	}

	raw, err := rpcClient.Dispense("plugin")
	if err != nil {
		client.Kill()
		return nil, fmt.Errorf("failed to dispense plugin: %w", err)
	}

	mod, ok := raw.(plugin.Plugin)
	if !ok {
		client.Kill()
		return nil, fmt.Errorf("plugin does not implement the Plugin interface")
	}

	desc, err := mod.Describe()
	if err != nil {
		client.Kill()
		return nil, fmt.Errorf("plugin describe failed: %w", err)
	}

	h.clients[id] = &pluginClient{client: client, plugin: mod, path: path}
	h.logger.Info("plugin launched",
		"id", id,
		"name", desc.Name,
		"version", desc.Version,
		"path", path)

	return mod, nil
}

// GetPlugin returns a previously launched plugin, or an error if not running.
func (h *Host) GetPlugin(id string) (plugin.Plugin, error) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	c, ok := h.clients[id]
	if !ok {
		return nil, fmt.Errorf("plugin not running: %s", id)
	}
	return c.plugin, nil
}

// StopPlugin terminates a running external plugin.
func (h *Host) StopPlugin(id string) error {
	h.mu.Lock()
	defer h.mu.Unlock()

	c, ok := h.clients[id]
	if !ok {
		return fmt.Errorf("plugin not running: %s", id)
	}

	c.client.Kill()
	delete(h.clients, id)
	h.logger.Info("plugin stopped", "id", id, "path", c.path)
	return nil
}

// StopAll terminates all running external modules.
func (h *Host) StopAll() {
	h.mu.Lock()
	defer h.mu.Unlock()

	for id, c := range h.clients {
		c.client.Kill()
		h.logger.Debug("plugin stopped", "id", id)
	}
	h.clients = make(map[string]*pluginClient)
	h.logger.Info("all external plugins stopped")
}

// SampleWithTimeout calls Sample() on a running plugin, returning an error if
// it doesn't respond within timeout.
func (h *Host) SampleWithTimeout(id string, timeout time.Duration) (plugin.Payload, error) {
	mod, err := h.GetPlugin(id)
	if err != nil {
		return plugin.Payload{}, err
	}

	type result struct {
		payload plugin.Payload
		err     error
	}
	ch := make(chan result, 1)
	go func() {
		p, e := mod.Sample()
		ch <- result{p, e}
	}()

	select {
	case r := <-ch:
		return r.payload, r.err
	case <-time.After(timeout):
		return plugin.Payload{}, fmt.Errorf("plugin %s did not respond within %v", id, timeout)
	}
}
