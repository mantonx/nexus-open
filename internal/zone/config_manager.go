// Package zone manages per-zone plugin configuration.
// ConfigManager now delegates storage to the shared [store.DB].
package zone

import (
	"log/slog"

	"github.com/mantonx/nexus-next/internal/store"
)

// ConfigManager manages zone-specific plugin configurations.
// It is a thin adapter between the zone package and the store layer.
type ConfigManager struct {
	store  *store.DB
	logger *slog.Logger
}

// NewConfigManager creates a ConfigManager backed by the given store.
func NewConfigManager(s *store.DB, logger *slog.Logger) *ConfigManager {
	if logger == nil {
		logger = slog.Default()
	}
	return &ConfigManager{store: s, logger: logger}
}

// Get returns the effective config for a zone.
// Resolution order: zone override → nil (plugin defaults removed; use zone IDs).
func (m *ConfigManager) Get(zoneID, _ string) map[string]interface{} {
	cfg, err := m.store.GetZoneConfig(zoneID)
	if err != nil {
		m.logger.Warn("zone config: read error", "zone_id", zoneID, "error", err)
		return nil
	}
	return cfg
}

// GetPluginDefault returns the default config for a plugin path.
// Stored under the key "plugin:<pluginPath>" for namespace separation.
// NOTE: Breaking change — DB keys were previously "module:<path>"; existing records are not migrated.
func (m *ConfigManager) GetPluginDefault(pluginPath string) map[string]interface{} {
	return m.Get("plugin:"+pluginPath, "")
}

// GetZoneOverride returns the stored config for zoneID (same as Get now).
func (m *ConfigManager) GetZoneOverride(zoneID string) map[string]interface{} {
	return m.Get(zoneID, "")
}

// SetPluginDefault stores default config for a plugin path.
// Stored under the key "plugin:<pluginPath>" for namespace separation.
// NOTE: Breaking change — DB keys were previously "module:<path>"; existing records are not migrated.
func (m *ConfigManager) SetPluginDefault(pluginPath string, cfg map[string]interface{}) error {
	return m.store.SetZoneConfig("plugin:"+pluginPath, cfg)
}

// SetZoneOverride stores the config for a specific zone.
func (m *ConfigManager) SetZoneOverride(zoneID string, cfg map[string]interface{}) error {
	return m.store.SetZoneConfig(zoneID, cfg)
}

// DeleteZoneOverride removes the stored config for zoneID.
func (m *ConfigManager) DeleteZoneOverride(zoneID string) error {
	return m.store.DeleteZoneConfig(zoneID)
}

// BroadcastConfigChange notifies all zones of a global config change
// (location, unit, time format). Implements zone.ZoneConfigNotifier.
func (m *ConfigManager) BroadcastConfigChange(cfg map[string]interface{}) {
	// Global config changes are handled at the sampler level via OnConfigChanged.
	// The store doesn't need to persist these — they flow from settings.Manager.
}

// BroadcastZoneConfigChange updates and persists the config for a single zone.
func (m *ConfigManager) BroadcastZoneConfigChange(zoneID string, cfg map[string]interface{}) error {
	return m.SetZoneOverride(zoneID, cfg)
}

// ImportFromYAML imports zone configs from a legacy zone-configs.yaml file.
// Called once on first run.
func (m *ConfigManager) ImportFromYAML(path string) error {
	legacy, err := loadZoneYAML(path)
	if err != nil {
		return nil // file doesn't exist — fresh install
	}

	m.logger.Info("zone config: importing legacy zone-configs.yaml", "path", path)

	for zoneID, cfg := range legacy.ZoneOverrides {
		if err := m.store.SetZoneConfig(zoneID, cfg); err != nil {
			m.logger.Warn("zone config: import error", "zone_id", zoneID, "error", err)
		}
	}
	return nil
}
