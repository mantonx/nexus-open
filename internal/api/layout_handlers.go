package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"

	"github.com/mantonx/nexus-next/internal/store"
	"github.com/mantonx/nexus-next/internal/zone"
)

// handleGetLayout returns the full layout: all pages with their zones.
// GET /api/layout
func (s *Server) handleGetLayout(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	pages, zoneMap, err := s.layoutStore.GetFullLayout()
	if err != nil {
		s.respondError(w, "Failed to load layout: "+err.Error(), http.StatusInternalServerError)
		return
	}

	type zoneOut struct {
		ID        string                 `json:"id"`
		PageID    int64                  `json:"page_id"`
		Ord       int                    `json:"ord"`
		WidthPx   int                    `json:"width_px"`
		Plugin    string                 `json:"plugin"`
		RefreshMs int                    `json:"refresh_ms"`
		Align     string                 `json:"align"`
		Config    map[string]interface{} `json:"config,omitempty"`
		Theme     map[string]interface{} `json:"theme_override,omitempty"`
	}
	type pageOut struct {
		ID    int64     `json:"id"`
		Name  string    `json:"name"`
		Ord   int       `json:"ord"`
		Zones []zoneOut `json:"zones"`
	}

	out := make([]pageOut, 0, len(pages))
	for _, p := range pages {
		zones := zoneMap[p.ID]
		zos := make([]zoneOut, 0, len(zones))
		for _, z := range zones {
			zos = append(zos, zoneOut{
				ID:        z.ID,
				PageID:    z.PageID,
				Ord:       z.Ord,
				WidthPx:   z.WidthPx,
				Plugin:    z.Plugin,
				RefreshMs: z.RefreshMs,
				Align:     z.Align,
				Config:    z.ConfigJSON,
				Theme:     z.ThemeJSON,
			})
		}
		out = append(out, pageOut{ID: p.ID, Name: p.Name, Ord: p.Ord, Zones: zos})
	}

	s.respondJSON(w, out, http.StatusOK)
}

// handleLayoutPages handles page collection operations.
// POST /api/layout/pages  — create a page
func (s *Server) handleLayoutPages(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	var req struct {
		Name string `json:"name"`
		Ord  int    `json:"ord"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if req.Name == "" {
		s.respondError(w, "name is required", http.StatusBadRequest)
		return
	}

	id, err := s.layoutStore.CreatePage(req.Name, req.Ord)
	if err != nil {
		s.respondError(w, "Failed to create page: "+err.Error(), http.StatusInternalServerError)
		return
	}

	s.triggerLayoutReload()
	s.respondSuccess(w, "Page created", map[string]any{"id": id, "name": req.Name, "ord": req.Ord})
}

// handleReorderPages sets the display order for all pages.
// POST /api/layout/pages/reorder  body: {"order": [3, 1, 2]}
func (s *Server) handleReorderPages(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	var req struct {
		Order []int64 `json:"order"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	if err := s.layoutStore.ReorderPages(req.Order); err != nil {
		s.respondError(w, "Failed to reorder pages: "+err.Error(), http.StatusInternalServerError)
		return
	}

	s.triggerLayoutReload()
	s.respondSuccess(w, "Pages reordered", nil)
}

// handleLayoutPage handles operations on a specific page.
// PUT /api/layout/pages/:id  — rename / reorder a page
// DELETE /api/layout/pages/:id  — delete a page
func (s *Server) handleLayoutPage(w http.ResponseWriter, r *http.Request) {
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	idStr := strings.TrimPrefix(r.URL.Path, "/api/layout/pages/")
	id, err := strconv.ParseInt(idStr, 10, 64)
	if err != nil || id == 0 {
		s.respondError(w, "Invalid page ID", http.StatusBadRequest)
		return
	}

	switch r.Method {
	case http.MethodPut:
		var req struct {
			Name string `json:"name"`
			Ord  int    `json:"ord"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
			return
		}
		if err := s.layoutStore.UpdatePage(id, req.Name, req.Ord); err != nil {
			s.respondError(w, "Failed to update page: "+err.Error(), http.StatusInternalServerError)
			return
		}
		s.triggerLayoutReload()
		s.respondSuccess(w, "Page updated", nil)

	case http.MethodDelete:
		if err := s.layoutStore.DeletePage(id); err != nil {
			s.respondError(w, "Failed to delete page: "+err.Error(), http.StatusInternalServerError)
			return
		}
		s.triggerLayoutReload()
		s.respondSuccess(w, "Page deleted", nil)

	default:
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// handleCreateZone creates a zone in a page.
// POST /api/layout/zones
func (s *Server) handleCreateZone(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	var z store.StoredZone
	if err := json.NewDecoder(r.Body).Decode(&z); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if z.ID == "" || z.PageID == 0 || z.WidthPx < 80 {
		s.respondError(w, "id, page_id, and width_px (≥80) are required", http.StatusBadRequest)
		return
	}
	if z.Plugin == "" {
		z.Plugin = "builtin:placeholder"
	}
	if z.RefreshMs <= 0 {
		z.RefreshMs = 2000
	}
	if z.Align == "" {
		z.Align = "center"
	}

	if err := s.layoutStore.CreateZone(z); err != nil {
		s.respondError(w, "Failed to create zone: "+err.Error(), http.StatusInternalServerError)
		return
	}

	// Don't validate width on create — pages are built up incrementally.
	// Validation runs on updates to catch edits that break the 640px invariant.
	s.triggerLayoutReload()
	s.respondSuccess(w, "Zone created", map[string]any{"id": z.ID})
}

// handleReorderZones sets the display order for zones within a page.
// POST /api/layout/zones/reorder  body: {"page_id": 1, "order": ["zone-a","zone-b"]}
func (s *Server) handleReorderZones(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	var req struct {
		PageID int64    `json:"page_id"`
		Order  []string `json:"order"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	if err := s.layoutStore.ReorderZones(req.PageID, req.Order); err != nil {
		s.respondError(w, "Failed to reorder zones: "+err.Error(), http.StatusInternalServerError)
		return
	}

	s.triggerLayoutReload()
	s.respondSuccess(w, "Zones reordered", nil)
}

// handleLayoutZone handles operations on a specific zone.
// PUT /api/layout/zones/:id  — update zone properties
// DELETE /api/layout/zones/:id  — remove zone
func (s *Server) handleLayoutZone(w http.ResponseWriter, r *http.Request) {
	if s.layoutStore == nil {
		s.respondError(w, "Layout store not available", http.StatusServiceUnavailable)
		return
	}

	zoneID := strings.TrimPrefix(r.URL.Path, "/api/layout/zones/")
	if zoneID == "" {
		s.respondError(w, "Zone ID required", http.StatusBadRequest)
		return
	}

	switch r.Method {
	case http.MethodPut:
		var z store.StoredZone
		if err := json.NewDecoder(r.Body).Decode(&z); err != nil {
			s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
			return
		}
		z.ID = zoneID
		if err := s.layoutStore.UpdateZone(z); err != nil {
			s.respondError(w, "Failed to update zone: "+err.Error(), http.StatusInternalServerError)
			return
		}
		if err := s.validatePageWidth(z.PageID); err != nil {
			s.respondError(w, err.Error(), http.StatusBadRequest)
			return
		}
		s.triggerLayoutReload()
		s.respondSuccess(w, "Zone updated", nil)

	case http.MethodDelete:
		if err := s.layoutStore.DeleteZone(zoneID); err != nil {
			s.respondError(w, "Failed to delete zone: "+err.Error(), http.StatusInternalServerError)
			return
		}
		s.triggerLayoutReload()
		s.respondSuccess(w, "Zone deleted", nil)

	default:
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// triggerLayoutReload rebuilds the live zone config from the DB and reloads
// the zone manager so the hardware display reflects the change immediately.
// Runs synchronously — layout edits are infrequent so the ~10ms rebuild cost
// is acceptable.
func (s *Server) triggerLayoutReload() {
	if s.layoutReloader == nil || s.layoutStore == nil {
		return
	}

	pages, zoneMap, err := s.layoutStore.GetFullLayout()
	if err != nil {
		s.logger.Error("layout reload: failed to load from store", "error", err)
		return
	}

	cfg := storeToZoneConfig(pages, zoneMap, s.layoutReloader.GetConfig())
	if err := s.layoutReloader.ReloadFromConfig(cfg); err != nil {
		s.logger.Error("layout reload: ReloadFromConfig failed", "error", err)
		return
	}

	// Broadcast updated page state to all WS clients.
	go s.BroadcastPageState()
	s.logger.Debug("layout reloaded", "pages", len(pages))
}

// storeToZoneConfig converts DB rows to a zone.Config suitable for ReloadFromConfig.
// It inherits the global theme and nav settings from the current running config.
func storeToZoneConfig(
	pages []store.StoredPage,
	zoneMap map[int64][]store.StoredZone,
	current *zone.Config,
) *zone.Config {
	cfg := &zone.Config{
		Name:    "User Layout",
		Version: "1.0",
	}
	if current != nil {
		cfg.Theme = current.Theme
		cfg.Nav = current.Nav
	} else {
		cfg.Theme = zone.DefaultTheme()
	}

	for _, p := range pages {
		page := zone.Page{Name: p.Name}
		for _, z := range zoneMap[p.ID] {
			zc := zone.ZoneConfig{
				ID:        z.ID,
				Width:     z.WidthPx,
				Plugin:    z.Plugin,
				RefreshMs: z.RefreshMs,
				Align:     zone.Alignment(z.Align),
			}
			// Apply theme overrides if present.
			if len(z.ThemeJSON) > 0 {
				t := zone.DefaultTheme()
				if v, ok := z.ThemeJSON["accent"].(string); ok && v != "" {
					t.Accent = v
				}
				if v, ok := z.ThemeJSON["bg"].(string); ok && v != "" {
					t.Bg = v
				}
				zc.ThemeOverride = &t
			}
			page.Zones = append(page.Zones, zc)
		}
		cfg.Pages = append(cfg.Pages, page)
	}

	if len(cfg.Pages) == 0 {
		// Fallback: keep current pages if DB has none.
		if current != nil {
			cfg.Pages = current.Pages
		}
	}

	return cfg
}

// validatePageWidth checks that zones for a page still sum to 640px.
// Returns a user-facing error if not.
func (s *Server) validatePageWidth(pageID int64) error {
	zones, err := s.layoutStore.GetZonesForPage(pageID)
	if err != nil {
		return nil // can't validate — don't block the operation
	}
	return store.ValidateZoneWidths(zones)
}

// handleLayoutExport exports the current live layout as a downloadable YAML file.
// GET /api/layout/export
func (s *Server) handleLayoutExport(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.layoutReloader == nil {
		s.respondError(w, "Layout not available", http.StatusServiceUnavailable)
		return
	}

	cfg := s.layoutReloader.GetConfig()
	if cfg == nil {
		s.respondError(w, "No active layout", http.StatusInternalServerError)
		return
	}

	data, err := zone.ExportConfigToYAML(cfg)
	if err != nil {
		s.logger.Error("layout export: marshal failed", "error", err)
		s.respondError(w, "Failed to export layout: "+err.Error(), http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/x-yaml")
	w.Header().Set("Content-Disposition", fmt.Sprintf(`attachment; filename="%s.yaml"`, cfg.Name))
	w.Header().Set("Content-Length", fmt.Sprintf("%d", len(data)))
	w.WriteHeader(http.StatusOK)
	w.Write(data) //nolint:errcheck
}
