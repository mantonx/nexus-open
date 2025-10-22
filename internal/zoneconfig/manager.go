// Package zoneconfig manages per-zone module configuration.
// Supports hybrid approach: module defaults + optional zone overrides.
package zoneconfig

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"gopkg.in/yaml.v3"
)

// Config represents the zone configuration file structure.
type Config struct {
	// ModuleDefaults are shared configs for all zones using a module
	ModuleDefaults map[string]map[string]interface{} `yaml:"module_defaults,omitempty"`
	// ZoneOverrides are per-zone configs that override module defaults
	ZoneOverrides map[string]map[string]interface{} `yaml:"zone_overrides,omitempty"`
}

// Manager manages zone-specific module configurations.
type Manager struct {
	path   string
	config *Config
	mu     sync.RWMutex
}

// NewManager creates a new zone config manager.
func NewManager(path string) (*Manager, error) {
	if path == "" {
		// Use default path
		configDir, err := os.UserConfigDir()
		if err != nil {
			return nil, fmt.Errorf("failed to get config dir: %w", err)
		}
		path = filepath.Join(configDir, "nexus-open", "zone-configs.yaml")
	}

	m := &Manager{
		path: path,
		config: &Config{
			ModuleDefaults: make(map[string]map[string]interface{}),
			ZoneOverrides:  make(map[string]map[string]interface{}),
		},
	}

	// Load existing config if file exists
	if _, err := os.Stat(path); err == nil {
		if err := m.load(); err != nil {
			return nil, fmt.Errorf("failed to load zone config: %w", err)
		}
	}

	return m, nil
}

// Get returns the effective config for a zone.
// Resolution order: zone override → module default → nil
func (m *Manager) Get(zoneID, modulePath string) map[string]interface{} {
	m.mu.RLock()
	defer m.mu.RUnlock()

	// 1. Check zone override
	if override, exists := m.config.ZoneOverrides[zoneID]; exists && len(override) > 0 {
		return copyMap(override)
	}

	// 2. Check module default
	if defaults, exists := m.config.ModuleDefaults[modulePath]; exists && len(defaults) > 0 {
		return copyMap(defaults)
	}

	// 3. No config
	return nil
}

// GetModuleDefault returns the default config for a module.
func (m *Manager) GetModuleDefault(modulePath string) map[string]interface{} {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if defaults, exists := m.config.ModuleDefaults[modulePath]; exists {
		return copyMap(defaults)
	}
	return nil
}

// GetZoneOverride returns the zone-specific override (if any).
func (m *Manager) GetZoneOverride(zoneID string) map[string]interface{} {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if override, exists := m.config.ZoneOverrides[zoneID]; exists {
		return copyMap(override)
	}
	return nil
}

// SetModuleDefault sets the default config for a module.
func (m *Manager) SetModuleDefault(modulePath string, config map[string]interface{}) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.config.ModuleDefaults == nil {
		m.config.ModuleDefaults = make(map[string]map[string]interface{})
	}

	m.config.ModuleDefaults[modulePath] = copyMap(config)
	return m.save()
}

// SetZoneOverride sets a zone-specific config override.
func (m *Manager) SetZoneOverride(zoneID string, config map[string]interface{}) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.config.ZoneOverrides == nil {
		m.config.ZoneOverrides = make(map[string]map[string]interface{})
	}

	m.config.ZoneOverrides[zoneID] = copyMap(config)
	return m.save()
}

// DeleteZoneOverride removes a zone-specific override.
func (m *Manager) DeleteZoneOverride(zoneID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	delete(m.config.ZoneOverrides, zoneID)
	return m.save()
}

// load reads the config from disk.
func (m *Manager) load() error {
	data, err := os.ReadFile(m.path)
	if err != nil {
		return fmt.Errorf("failed to read config file: %w", err)
	}

	var config Config
	if err := yaml.Unmarshal(data, &config); err != nil {
		return fmt.Errorf("failed to parse config: %w", err)
	}

	// Initialize maps if nil
	if config.ModuleDefaults == nil {
		config.ModuleDefaults = make(map[string]map[string]interface{})
	}
	if config.ZoneOverrides == nil {
		config.ZoneOverrides = make(map[string]map[string]interface{})
	}

	m.config = &config
	return nil
}

// save writes the config to disk (caller must hold lock).
func (m *Manager) save() error {
	// Ensure directory exists
	if err := os.MkdirAll(filepath.Dir(m.path), 0755); err != nil {
		return fmt.Errorf("failed to create config dir: %w", err)
	}

	data, err := yaml.Marshal(m.config)
	if err != nil {
		return fmt.Errorf("failed to marshal config: %w", err)
	}

	if err := os.WriteFile(m.path, data, 0644); err != nil {
		return fmt.Errorf("failed to write config file: %w", err)
	}

	return nil
}

// copyMap creates a deep copy of a map to prevent external modification.
func copyMap(src map[string]interface{}) map[string]interface{} {
	if src == nil {
		return nil
	}

	dst := make(map[string]interface{}, len(src))
	for k, v := range src {
		dst[k] = v
	}
	return dst
}
