// Package zone manages per-zone plugin configuration.
// ConfigManager delegates storage to the shared [store.DB], reading and
// writing zones.config_json as the single source of truth for per-zone
// plugin config (zone_plugin_config was removed in schema migration v5).
package zone

import (
	"log/slog"

	"github.com/mantonx/nexus-open/internal/store"
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

// Get returns the plugin config for a zone from zones.config_json.
// Returns nil when the zone has no stored config or does not exist.
func (m *ConfigManager) Get(zoneID, _ string) map[string]any {
	cfg, err := m.store.GetZonePluginConfig(zoneID)
	if err != nil {
		m.logger.Warn("zone config: read error", "zone_id", zoneID, "error", err)
		return nil
	}
	return cfg
}

// GetZoneOverride returns the stored config for zoneID.
func (m *ConfigManager) GetZoneOverride(zoneID string) map[string]any {
	return m.Get(zoneID, "")
}

// SetZoneOverride stores the plugin config for a specific zone.
func (m *ConfigManager) SetZoneOverride(zoneID string, cfg map[string]any) error {
	return m.store.SetZonePluginConfig(zoneID, cfg)
}

// DeleteZoneOverride clears the plugin config for a zone (resets to {}).
func (m *ConfigManager) DeleteZoneOverride(zoneID string) error {
	return m.store.SetZonePluginConfig(zoneID, map[string]any{})
}

// BroadcastConfigChange is a no-op: global config changes flow through
// settings.Manager and are delivered to plugins via the sampler.
func (m *ConfigManager) BroadcastConfigChange(_ map[string]any) {}

// BroadcastZoneConfigChange persists the config for a single zone.
func (m *ConfigManager) BroadcastZoneConfigChange(zoneID string, cfg map[string]any) error {
	return m.SetZoneOverride(zoneID, cfg)
}
