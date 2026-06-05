package app

import (
	"log/slog"

	"github.com/mantonx/nexus-next/internal/store"
	"github.com/mantonx/nexus-next/internal/zone"
)

// importLayoutFromYAML seeds the layout DB tables from the in-memory zone.Config
// that was loaded from the YAML layout file at startup. Called once on first run.
func importLayoutFromYAML(s *store.DB, cfg *zone.Config, logger *slog.Logger) error {
	if cfg == nil || len(cfg.Pages) == 0 {
		return nil
	}

	pages := make([]store.StoredPage, len(cfg.Pages))
	zoneMap := make(map[int64][]store.StoredZone, len(cfg.Pages))

	for i, p := range cfg.Pages {
		pageID := int64(i + 1) // stable IDs: 1-indexed
		pages[i] = store.StoredPage{ID: pageID, Name: p.Name, Ord: i}

		for j, z := range p.Zones {
			sz := store.StoredZone{
				ID:        z.ID,
				PageID:    pageID,
				Ord:       j,
				WidthPx:   z.Width,
				Plugin:    z.Plugin,
				RefreshMs: z.RefreshMs,
				Align:     string(z.Align),
			}
			if z.ThemeOverride != nil {
				sz.ThemeJSON = map[string]interface{}{
					"accent": z.ThemeOverride.Accent,
					"bg":     z.ThemeOverride.Bg,
					"fg":     z.ThemeOverride.Fg,
				}
			}
			zoneMap[pageID] = append(zoneMap[pageID], sz)
		}
	}

	if err := s.ImportLayout(pages, zoneMap); err != nil {
		return err
	}

	logger.Info("layout seeded from YAML", "pages", len(pages))
	return nil
}
