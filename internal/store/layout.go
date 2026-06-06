package store

import (
	"encoding/json"
	"fmt"
)

// ── Layout types ──────────────────────────────────────────────────────────────

// StoredPage is a page row from the DB.
type StoredPage struct {
	ID   int64  `json:"id"`
	Name string `json:"name"`
	Ord  int    `json:"ord"`
}

// StoredZone is a zone row from the DB.
type StoredZone struct {
	ID          string                 `json:"id"`
	PageID      int64                  `json:"page_id"`
	Ord         int                    `json:"ord"`
	WidthPx     int                    `json:"width_px"`
	Plugin      string                 `json:"plugin"`
	RefreshMs   int                    `json:"refresh_ms"`
	Align       string                 `json:"align"`
	ConfigJSON  map[string]interface{} `json:"config"`
	ThemeJSON   map[string]interface{} `json:"theme_override,omitempty"`
}

// ── Pages ─────────────────────────────────────────────────────────────────────

// GetPages returns all pages ordered by ord.
func (s *DB) GetPages() ([]StoredPage, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(`SELECT id, name, ord FROM pages ORDER BY ord`)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()

	var pages []StoredPage
	for rows.Next() {
		var p StoredPage
		if err := rows.Scan(&p.ID, &p.Name, &p.Ord); err != nil {
			return nil, err
		}
		pages = append(pages, p)
	}
	return pages, rows.Err()
}

// CreatePage inserts a new page and returns its ID.
func (s *DB) CreatePage(name string, ord int) (int64, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	res, err := s.db.Exec(`INSERT INTO pages(name, ord) VALUES(?, ?)`, name, ord)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

// UpdatePage updates a page's name and/or order.
func (s *DB) UpdatePage(id int64, name string, ord int) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`UPDATE pages SET name = ?, ord = ? WHERE id = ?`, name, ord, id)
	return err
}

// DeletePage deletes a page (cascades to zones).
func (s *DB) DeletePage(id int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`DELETE FROM pages WHERE id = ?`, id)
	return err
}

// ReorderPages sets ord for each page ID in a single transaction.
func (s *DB) ReorderPages(order []int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback() //nolint:errcheck

	for i, id := range order {
		if _, err := tx.Exec(`UPDATE pages SET ord = ? WHERE id = ?`, i, id); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// ── Zones ─────────────────────────────────────────────────────────────────────

// GetZonesForPage returns all zones for a page ordered by ord.
func (s *DB) GetZonesForPage(pageID int64) ([]StoredZone, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(
		`SELECT id, page_id, ord, width_px, module, refresh_ms, align, config_json, theme_json
		 FROM zones WHERE page_id = ? ORDER BY ord`,
		pageID,
	)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()

	var zones []StoredZone
	for rows.Next() {
		var z StoredZone
		var cfgRaw, themeRaw string
		if err := rows.Scan(&z.ID, &z.PageID, &z.Ord, &z.WidthPx, &z.Plugin,
			&z.RefreshMs, &z.Align, &cfgRaw, &themeRaw); err != nil {
			return nil, err
		}
		json.Unmarshal([]byte(cfgRaw), &z.ConfigJSON)   //nolint:errcheck
		json.Unmarshal([]byte(themeRaw), &z.ThemeJSON)  //nolint:errcheck
		zones = append(zones, z)
	}
	return zones, rows.Err()
}

// CreateZone inserts a new zone and returns any error.
// If ConfigJSON is non-empty it is also written to zone_plugin_config so the
// sampler picks it up without a separate POST /api/zones/:id/config call.
func (s *DB) CreateZone(z StoredZone) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	cfgRaw, _ := json.Marshal(z.ConfigJSON)
	themeRaw, _ := json.Marshal(z.ThemeJSON)

	_, err := s.db.Exec(
		`INSERT INTO zones(id, page_id, ord, width_px, module, refresh_ms, align, config_json, theme_json)
		 VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		z.ID, z.PageID, z.Ord, z.WidthPx, z.Plugin, z.RefreshMs, z.Align,
		string(cfgRaw), string(themeRaw),
	)
	if err != nil {
		return err
	}
	if len(z.ConfigJSON) > 0 {
		_, err = s.db.Exec(
			`INSERT INTO zone_plugin_config(zone_id, config_json) VALUES(?, ?)
			 ON CONFLICT(zone_id) DO UPDATE SET config_json = excluded.config_json`,
			z.ID, string(cfgRaw),
		)
	}
	return err
}

// UpdateZone updates a zone's mutable fields.
// If ConfigJSON is non-empty it is also synced to zone_plugin_config.
func (s *DB) UpdateZone(z StoredZone) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	cfgRaw, _ := json.Marshal(z.ConfigJSON)
	themeRaw, _ := json.Marshal(z.ThemeJSON)

	res, err := s.db.Exec(
		`UPDATE zones
		 SET page_id=?, ord=?, width_px=?, module=?, refresh_ms=?, align=?,
		     config_json=?, theme_json=?
		 WHERE id=?`,
		z.PageID, z.Ord, z.WidthPx, z.Plugin, z.RefreshMs, z.Align,
		string(cfgRaw), string(themeRaw), z.ID,
	)
	if err != nil {
		return err
	}
	if n, _ := res.RowsAffected(); n == 0 {
		return fmt.Errorf("zone %q not found", z.ID)
	}
	if len(z.ConfigJSON) > 0 {
		_, err = s.db.Exec(
			`INSERT INTO zone_plugin_config(zone_id, config_json) VALUES(?, ?)
			 ON CONFLICT(zone_id) DO UPDATE SET config_json = excluded.config_json`,
			z.ID, string(cfgRaw),
		)
	}
	return err
}

// DeleteZone removes a zone by ID.
func (s *DB) DeleteZone(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`DELETE FROM zones WHERE id = ?`, id)
	return err
}

// ReorderZones sets ord for each zone ID within a page.
func (s *DB) ReorderZones(pageID int64, order []string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback() //nolint:errcheck

	for i, id := range order {
		if _, err := tx.Exec(
			`UPDATE zones SET ord = ? WHERE id = ? AND page_id = ?`, i, id, pageID,
		); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// HasLayout reports whether any pages exist in the DB.
func (s *DB) HasLayout() (bool, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var count int
	err := s.db.QueryRow(`SELECT COUNT(*) FROM pages`).Scan(&count)
	return count > 0, err
}

// GetFullLayout returns all pages with their zones.
func (s *DB) GetFullLayout() ([]StoredPage, map[int64][]StoredZone, error) {
	pages, err := s.GetPages()
	if err != nil {
		return nil, nil, err
	}

	zoneMap := make(map[int64][]StoredZone, len(pages))
	for _, p := range pages {
		zones, err := s.GetZonesForPage(p.ID)
		if err != nil {
			return nil, nil, err
		}
		zoneMap[p.ID] = zones
	}
	return pages, zoneMap, nil
}

// ImportLayout writes a full layout in a single transaction, replacing any
// existing pages and zones. Used on first run to seed from YAML.
func (s *DB) ImportLayout(pages []StoredPage, zonesByPage map[int64][]StoredZone) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback() //nolint:errcheck

	if _, err := tx.Exec(`DELETE FROM pages`); err != nil {
		return err
	}

	for _, p := range pages {
		if _, err := tx.Exec(
			`INSERT INTO pages(id, name, ord) VALUES(?, ?, ?)`,
			p.ID, p.Name, p.Ord,
		); err != nil {
			return err
		}
		for _, z := range zonesByPage[p.ID] {
			cfgRaw, _ := json.Marshal(z.ConfigJSON)
			themeRaw, _ := json.Marshal(z.ThemeJSON)
			if _, err := tx.Exec(
				`INSERT INTO zones(id, page_id, ord, width_px, module, refresh_ms, align, config_json, theme_json)
				 VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)`,
				z.ID, z.PageID, z.Ord, z.WidthPx, z.Plugin, z.RefreshMs, z.Align,
				string(cfgRaw), string(themeRaw),
			); err != nil {
				return err
			}
			// Keep zone_plugin_config in sync with config_json.
			if len(z.ConfigJSON) > 0 {
				if _, err := tx.Exec(
					`INSERT INTO zone_plugin_config(zone_id, config_json) VALUES(?, ?)
					 ON CONFLICT(zone_id) DO UPDATE SET config_json = excluded.config_json`,
					z.ID, string(cfgRaw),
				); err != nil {
					return err
				}
			}
		}
	}

	return tx.Commit()
}

// ValidateZoneWidths checks that zones for a page sum to exactly 640px.
func ValidateZoneWidths(zones []StoredZone) error {
	total := 0
	for _, z := range zones {
		total += z.WidthPx
	}
	if total != 640 {
		return fmt.Errorf("zones sum to %dpx, must equal 640px", total)
	}
	return nil
}

