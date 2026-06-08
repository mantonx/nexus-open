package store

import (
	"context"
	"encoding/json"
	"fmt"

	dbgen "github.com/mantonx/nexus-open/internal/store/db"
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
	ID         string         `json:"id"`
	PageID     int64          `json:"page_id"`
	Ord        int            `json:"ord"`
	WidthPx    int            `json:"width_px"`
	Plugin     string         `json:"plugin"`
	RefreshMs  int            `json:"refresh_ms"`
	Align      string         `json:"align"`
	OnTap      string         `json:"on_tap,omitempty"`
	Choices    []string       `json:"choices,omitempty"`
	ConfigJSON map[string]any `json:"config"`
	ThemeJSON  map[string]any `json:"theme_override,omitempty"`
}

// ── Pages ─────────────────────────────────────────────────────────────────────

// GetPages returns all pages ordered by ord.
func (s *DB) GetPages() ([]StoredPage, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.q.ListPages(context.Background())
	if err != nil {
		return nil, err
	}
	pages := make([]StoredPage, len(rows))
	for i, r := range rows {
		pages[i] = StoredPage{ID: r.ID, Name: r.Name, Ord: int(r.Ord)}
	}
	return pages, nil
}

// CreatePage inserts a new page and returns its ID.
func (s *DB) CreatePage(name string, ord int) (int64, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.InsertPage(context.Background(), dbgen.InsertPageParams{
		Name: name,
		Ord:  int64(ord),
	})
}

// UpdatePage updates a page's name and/or order.
func (s *DB) UpdatePage(id int64, name string, ord int) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.UpdatePage(context.Background(), dbgen.UpdatePageParams{
		ID:   id,
		Name: name,
		Ord:  int64(ord),
	})
}

// DeletePage deletes a page (cascades to zones).
func (s *DB) DeletePage(id int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.DeletePage(context.Background(), id)
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

	qtx := s.q.WithTx(tx)
	for i, id := range order {
		if err := qtx.UpdatePageOrd(context.Background(), dbgen.UpdatePageOrdParams{
			Ord: int64(i),
			ID:  id,
		}); err != nil {
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

	rows, err := s.q.ListZonesForPage(context.Background(), pageID)
	if err != nil {
		return nil, err
	}
	zones := make([]StoredZone, 0, len(rows))
	for _, r := range rows {
		z := StoredZone{
			ID:        r.ID,
			PageID:    r.PageID,
			Ord:       int(r.Ord),
			WidthPx:   int(r.WidthPx),
			Plugin:    r.Plugin,
			RefreshMs: int(r.RefreshMs),
			Align:     r.Align,
			OnTap:     r.OnTap,
		}
		json.Unmarshal([]byte(r.ConfigJson), &z.ConfigJSON)  //nolint:errcheck
		json.Unmarshal([]byte(r.ThemeJson), &z.ThemeJSON)    //nolint:errcheck
		json.Unmarshal([]byte(r.ChoicesJson), &z.Choices)    //nolint:errcheck
		zones = append(zones, z)
	}
	return zones, nil
}

// GetZonePageID returns the page_id for a zone. Returns 0 if not found.
func (s *DB) GetZonePageID(zoneID string) (int64, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	return s.q.GetZonePageID(context.Background(), zoneID)
}

// GetZonePluginConfig returns the plugin config for a single zone.
// Returns nil if the zone has no stored config.
func (s *DB) GetZonePluginConfig(zoneID string) (map[string]any, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	raw, err := s.q.GetZoneConfigJSON(context.Background(), zoneID)
	if err != nil {
		return nil, err
	}
	if raw == "" || raw == "{}" || raw == "null" {
		return nil, nil
	}
	var m map[string]any
	if err := json.Unmarshal([]byte(raw), &m); err != nil {
		return nil, err
	}
	return m, nil
}

// SetZonePluginConfig updates config_json for a zone.
func (s *DB) SetZonePluginConfig(zoneID string, cfg map[string]any) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	raw, err := json.Marshal(cfg)
	if err != nil {
		return err
	}
	return s.q.UpdateZoneConfigJSON(context.Background(), dbgen.UpdateZoneConfigJSONParams{
		ID:         zoneID,
		ConfigJson: string(raw),
	})
}

// CreateZone inserts a new zone.
func (s *DB) CreateZone(z StoredZone) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	cfgRaw, _ := json.Marshal(z.ConfigJSON)
	themeRaw, _ := json.Marshal(z.ThemeJSON)
	choicesRaw, _ := json.Marshal(z.Choices)

	return s.q.InsertZone(context.Background(), dbgen.InsertZoneParams{
		ID:          z.ID,
		PageID:      z.PageID,
		Ord:         int64(z.Ord),
		WidthPx:     int64(z.WidthPx),
		Plugin:      z.Plugin,
		RefreshMs:   int64(z.RefreshMs),
		Align:       z.Align,
		OnTap:       z.OnTap,
		ChoicesJson: string(choicesRaw),
		ConfigJson:  string(cfgRaw),
		ThemeJson:   string(themeRaw),
	})
}

// UpdateZone updates a zone's mutable fields.
func (s *DB) UpdateZone(z StoredZone) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	cfgRaw, _ := json.Marshal(z.ConfigJSON)
	themeRaw, _ := json.Marshal(z.ThemeJSON)
	choicesRaw, _ := json.Marshal(z.Choices)

	result, err := s.q.UpdateZone(context.Background(), dbgen.UpdateZoneParams{
		ID:          z.ID,
		PageID:      z.PageID,
		Ord:         int64(z.Ord),
		WidthPx:     int64(z.WidthPx),
		Plugin:      z.Plugin,
		RefreshMs:   int64(z.RefreshMs),
		Align:       z.Align,
		OnTap:       z.OnTap,
		ChoicesJson: string(choicesRaw),
		ConfigJson:  string(cfgRaw),
		ThemeJson:   string(themeRaw),
	})
	if err != nil {
		return err
	}
	if n, _ := result.RowsAffected(); n == 0 {
		return fmt.Errorf("zone %q not found", z.ID)
	}
	return nil
}

// DeleteZone removes a zone by ID.
func (s *DB) DeleteZone(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.DeleteZone(context.Background(), id)
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

	qtx := s.q.WithTx(tx)
	for i, id := range order {
		if err := qtx.UpdateZoneOrd(context.Background(), dbgen.UpdateZoneOrdParams{
			Ord:    int64(i),
			ID:     id,
			PageID: pageID,
		}); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// HasLayout reports whether any pages exist in the DB.
func (s *DB) HasLayout() (bool, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	count, err := s.q.CountPages(context.Background())
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

	qtx := s.q.WithTx(tx)
	if err := qtx.DeleteAllPages(context.Background()); err != nil {
		return err
	}
	for _, p := range pages {
		if err := qtx.InsertPageWithID(context.Background(), dbgen.InsertPageWithIDParams{
			ID:   p.ID,
			Name: p.Name,
			Ord:  int64(p.Ord),
		}); err != nil {
			return err
		}
		for _, z := range zonesByPage[p.ID] {
			cfgRaw, _ := json.Marshal(z.ConfigJSON)
			themeRaw, _ := json.Marshal(z.ThemeJSON)
			choicesRaw, _ := json.Marshal(z.Choices)
			if err := qtx.InsertZone(context.Background(), dbgen.InsertZoneParams{
				ID:          z.ID,
				PageID:      z.PageID,
				Ord:         int64(z.Ord),
				WidthPx:     int64(z.WidthPx),
				Plugin:      z.Plugin,
				RefreshMs:   int64(z.RefreshMs),
				Align:       z.Align,
				OnTap:       z.OnTap,
				ChoicesJson: string(choicesRaw),
				ConfigJson:  string(cfgRaw),
				ThemeJson:   string(themeRaw),
			}); err != nil {
				return err
			}
		}
	}
	return tx.Commit()
}

// SetZonePlugin updates the plugin field for a single zone.
// Used by CycleZonePlugin to persist user-initiated plugin changes.
func (s *DB) SetZonePlugin(zoneID, pluginID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.UpdateZonePlugin(context.Background(), dbgen.UpdateZonePluginParams{
		ID:     zoneID,
		Plugin: pluginID,
	})
}

// ── Payload cache ─────────────────────────────────────────────────────────────

// SavePayloadCache persists a serialised DetailPayload for a zone.
// fetchedAt is the Unix timestamp of when the payload was fetched.
func (s *DB) SavePayloadCache(zoneID, pluginID string, payload []byte, fetchedAt int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.UpsertPayloadCache(context.Background(), dbgen.UpsertPayloadCacheParams{
		ZoneID:    zoneID,
		PluginID:  pluginID,
		Payload:   string(payload),
		FetchedAt: fetchedAt,
	})
}

// LoadPayloadCache returns the cached payload bytes and fetch timestamp for a
// zone, or (nil, 0, nil) if no cache entry exists.
func (s *DB) LoadPayloadCache(zoneID string) (payload []byte, fetchedAt int64, err error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	row, err := s.q.GetPayloadCache(context.Background(), zoneID)
	if err != nil {
		// sql.ErrNoRows means no cache — not an error the caller needs to handle.
		return nil, 0, nil
	}
	return []byte(row.Payload), row.FetchedAt, nil
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
