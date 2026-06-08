package zone

import (
	"encoding/json"
	"fmt"

	"gopkg.in/yaml.v3"

	"github.com/mantonx/nexus-open/internal/store"
)

// LayoutImporter can atomically replace the full layout in the database.
type LayoutImporter interface {
	ImportLayout(pages []store.StoredPage, zonesByPage map[int64][]store.StoredZone) error
}

// SaveConfigToDB persists cfg to the database as a full atomic replace using
// ImportLayout. Page IDs are assigned sequentially from 1; callers should
// reload from DB after commit so the manager has the canonical IDs.
func SaveConfigToDB(db LayoutImporter, cfg *Config) error {
	var pages []store.StoredPage
	zonesByPage := make(map[int64][]store.StoredZone)

	for i, p := range cfg.Pages {
		pageID := int64(i + 1)
		pages = append(pages, store.StoredPage{ID: pageID, Name: p.Name, Ord: i})

		for j, z := range p.Zones {
			sz := store.StoredZone{
				ID:         z.ID,
				PageID:     pageID,
				Ord:        j,
				WidthPx:    z.Width,
				Plugin:     z.Plugin,
				RefreshMs:  z.RefreshMs,
				Align:      string(z.Align),
				OnTap:      string(z.OnTap),
				Choices:    z.Choices,
				ConfigJSON: z.PluginConfig,
			}
			if z.ThemeOverride != nil {
				raw, err := json.Marshal(z.ThemeOverride)
				if err != nil {
					return fmt.Errorf("zone: marshal theme for zone %q: %w", z.ID, err)
				}
				var m map[string]any
				if err := json.Unmarshal(raw, &m); err != nil {
					return fmt.Errorf("zone: unmarshal theme for zone %q: %w", z.ID, err)
				}
				sz.ThemeJSON = m
			}
			zonesByPage[pageID] = append(zonesByPage[pageID], sz)
		}
	}

	return db.ImportLayout(pages, zonesByPage)
}

// LoadConfigFromDB reconstructs a Config from the layout stored in the
// database. Pages are returned in ord order; zones within each page are
// returned in ord order.
//
// The global theme is always DefaultTheme() because theme is not persisted
// per-layout in the DB — per-zone ThemeOverrides are stored in theme_json and
// are applied to individual zones.
//
// Returns an error (and the caller should fall back to YAML) when the DB
// contains no pages.
func LoadConfigFromDB(db *store.DB) (*Config, error) {
	pages, err := db.GetPages()
	if err != nil {
		return nil, fmt.Errorf("zone: load pages from db: %w", err)
	}
	if len(pages) == 0 {
		return nil, fmt.Errorf("zone: no pages in db")
	}

	cfg := &Config{
		Name:    "User Layout",
		Version: "1.0",
		Theme:   DefaultTheme(),
		Nav: NavConfig{
			SwipeEnabled: true,
		},
	}

	for _, p := range pages {
		zones, err := db.GetZonesForPage(p.ID)
		if err != nil {
			return nil, fmt.Errorf("zone: load zones for page %d from db: %w", p.ID, err)
		}

		page := Page{Name: p.Name}
		for _, z := range zones {
			zc := ZoneConfig{
				ID:           z.ID,
				Width:        z.WidthPx,
				Plugin:       z.Plugin,
				RefreshMs:    z.RefreshMs,
				Align:        Alignment(z.Align),
				OnTap:        TapAction(z.OnTap),
				Choices:      z.Choices,
				PluginConfig: z.ConfigJSON,
			}

			// Unmarshal per-zone ThemeOverride from theme_json if present.
			if len(z.ThemeJSON) > 0 {
				raw, err := json.Marshal(z.ThemeJSON)
				if err != nil {
					return nil, fmt.Errorf("zone: marshal theme_json for zone %q: %w", z.ID, err)
				}
				var t Theme
				if err := json.Unmarshal(raw, &t); err != nil {
					return nil, fmt.Errorf("zone: unmarshal theme_json for zone %q: %w", z.ID, err)
				}
				// Only attach the override if at least one field is set.
				if t.Accent != "" || t.Bg != "" || t.Fg != "" || t.Muted != "" {
					zc.ThemeOverride = &t
				}
			}

			page.Zones = append(page.Zones, zc)
		}
		cfg.Pages = append(cfg.Pages, page)
	}

	return cfg, nil
}

// ExportConfigToYAML serialises cfg to YAML bytes using the struct's yaml
// tags. The result can be written directly to a .yaml file or returned as an
// HTTP download.
func ExportConfigToYAML(cfg *Config) ([]byte, error) {
	data, err := yaml.Marshal(cfg)
	if err != nil {
		return nil, fmt.Errorf("zone: marshal config to yaml: %w", err)
	}
	return data, nil
}
