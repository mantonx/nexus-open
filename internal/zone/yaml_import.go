package zone

import (
	"os"

	"gopkg.in/yaml.v3"
)

// legacyModuleConfig mirrors the old zone-configs.yaml structure.
type legacyModuleConfig struct {
	ModuleDefaults map[string]map[string]interface{} `yaml:"module_defaults,omitempty"`
	ZoneOverrides  map[string]map[string]interface{} `yaml:"zone_overrides,omitempty"`
}

// loadZoneYAML reads a legacy zone-configs.yaml file.
func loadZoneYAML(path string) (*legacyModuleConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var cfg legacyModuleConfig
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}
	if cfg.ZoneOverrides == nil {
		cfg.ZoneOverrides = make(map[string]map[string]interface{})
	}
	return &cfg, nil
}
