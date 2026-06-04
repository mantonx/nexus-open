// Package plugin manages external module plugins using go-plugin RPC
package plugin

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"sync"
	"time"

	"github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-next/pkg/module"
)

// Host manages plugin lifecycle (launch, communication, cleanup)
type Host struct {
	logger  *slog.Logger
	clients map[string]*pluginClient
	mu      sync.RWMutex
}

// pluginClient represents a single plugin instance
type pluginClient struct {
	client *plugin.Client
	module module.Module
	path   string
}

// NewHost creates a new plugin host
func NewHost(logger *slog.Logger) *Host {
	return &Host{
		logger:  logger,
		clients: make(map[string]*pluginClient),
	}
}

// LaunchModule launches an external module plugin
func (h *Host) LaunchModule(ctx context.Context, id, path string) (module.Module, error) {
	h.mu.Lock()
	defer h.mu.Unlock()

	// Check if already launched
	if existing, ok := h.clients[id]; ok {
		h.logger.Debug("module already launched", "id", id, "path", path)
		return existing.module, nil
	}

	// Configure plugin client
	client := plugin.NewClient(&plugin.ClientConfig{
		HandshakeConfig: module.Handshake,
		Plugins: map[string]plugin.Plugin{
			"module": &module.ModulePlugin{},
		},
		Cmd:              exec.Command(path),
		AllowedProtocols: []plugin.Protocol{plugin.ProtocolNetRPC},
		Logger:           hclog.New(&hclog.LoggerOptions{
			Name:   "plugin",
			Output: os.Stderr,
			Level:  hclog.Error,
		}),
	})

	// Connect to plugin
	rpcClient, err := client.Client()
	if err != nil {
		client.Kill()
		return nil, fmt.Errorf("failed to connect to plugin: %w", err)
	}

	// Get module interface
	raw, err := rpcClient.Dispense("module")
	if err != nil {
		client.Kill()
		return nil, fmt.Errorf("failed to dispense module: %w", err)
	}

	mod, ok := raw.(module.Module)
	if !ok {
		client.Kill()
		return nil, fmt.Errorf("plugin does not implement Module interface")
	}

	// Verify module responds
	desc, err := mod.Describe()
	if err != nil {
		client.Kill()
		return nil, fmt.Errorf("module describe failed: %w", err)
	}

	// Store client
	h.clients[id] = &pluginClient{
		client: client,
		module: mod,
		path:   path,
	}

	h.logger.Info("module launched",
		"id", id,
		"name", desc.Name,
		"version", desc.Version,
		"path", path)

	return mod, nil
}

// GetModule returns a previously launched module
func (h *Host) GetModule(id string) (module.Module, error) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	client, ok := h.clients[id]
	if !ok {
		return nil, fmt.Errorf("module not found: %s", id)
	}

	return client.module, nil
}

// KillModule terminates a plugin
func (h *Host) KillModule(id string) error {
	h.mu.Lock()
	defer h.mu.Unlock()

	client, ok := h.clients[id]
	if !ok {
		return fmt.Errorf("module not found: %s", id)
	}

	client.client.Kill()
	delete(h.clients, id)

	h.logger.Info("module killed", "id", id, "path", client.path)

	return nil
}

// KillAll terminates all plugins
func (h *Host) KillAll() {
	h.mu.Lock()
	defer h.mu.Unlock()

	for id, client := range h.clients {
		client.client.Kill()
		h.logger.Debug("module killed", "id", id)
	}

	h.clients = make(map[string]*pluginClient)
	h.logger.Info("all modules killed")
}

// SampleWithTimeout samples a module with timeout and retry logic
func (h *Host) SampleWithTimeout(id string, timeout time.Duration) (module.Payload, error) {
	mod, err := h.GetModule(id)
	if err != nil {
		return module.Payload{}, err
	}

	// Create timeout channel
	done := make(chan struct{})
	var payload module.Payload
	var sampleErr error

	go func() {
		payload, sampleErr = mod.Sample()
		close(done)
	}()

	select {
	case <-done:
		return payload, sampleErr
	case <-time.After(timeout):
		return module.Payload{}, fmt.Errorf("module sample timeout after %v", timeout)
	}
}

