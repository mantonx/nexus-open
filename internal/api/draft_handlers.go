package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"github.com/google/uuid"
	"github.com/mantonx/nexus-open/internal/zone"
)

// handleGetDraft returns the current draft, opening one from the committed
// DB state if none is active.
// GET /api/layout/draft
func (s *Server) handleGetDraft(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}
	draft, err := s.draft.OpenDraft()
	if err != nil {
		s.respondError(w, "Failed to open draft: "+err.Error(), http.StatusInternalServerError)
		return
	}
	s.respondJSON(w, map[string]any{"active": true, "layout": draft}, http.StatusOK)
}

// handlePutDraft replaces the entire draft with the body.
// PUT /api/layout/draft
func (s *Server) handlePutDraft(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPut {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}

	var cfg zone.Config
	if err := json.NewDecoder(r.Body).Decode(&cfg); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	// Validate width sums for non-empty pages — empty pages are allowed in draft
	// but zones within a page must sum to the display width.
	for _, p := range cfg.Pages {
		if len(p.Zones) == 0 {
			continue
		}
		total := 0
		for _, z := range p.Zones {
			total += z.Width
		}
		if total != zone.DisplayWidthPx {
			s.respondError(w, fmt.Sprintf("zone widths must sum to %d, got %d", zone.DisplayWidthPx, total), http.StatusUnprocessableEntity)
			return
		}
	}
	if err := s.draft.UpdateDraft(&cfg); err != nil {
		s.respondError(w, "Draft update failed: "+err.Error(), http.StatusInternalServerError)
		return
	}
	s.respondSuccess(w, "Draft updated", nil)
}

// handleDraftZones handles zone creation in the draft.
// POST /api/layout/draft/zones
func (s *Server) handleDraftZones(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}

	var req struct {
		PageIndex      int    `json:"page_index"`
		Plugin         string `json:"plugin"`
		RefreshMs      int    `json:"refresh_ms"`
		InsertBeforeID string `json:"insert_before_id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	if req.Plugin == "" {
		s.respondError(w, "plugin is required", http.StatusBadRequest)
		return
	}
	if req.RefreshMs <= 0 {
		req.RefreshMs = 1000
	}

	draft, err := s.draft.OpenDraft()
	if err != nil {
		s.respondError(w, "Failed to open draft: "+err.Error(), http.StatusInternalServerError)
		return
	}

	if req.PageIndex < 0 || req.PageIndex >= len(draft.Pages) {
		s.respondError(w, fmt.Sprintf("page_index %d out of range", req.PageIndex), http.StatusBadRequest)
		return
	}
	page := &draft.Pages[req.PageIndex]
	if len(page.Zones) >= zone.MaxZonesPerPage {
		s.respondError(w, "ZoneCapExceeded: page already has the maximum number of zones", http.StatusUnprocessableEntity)
		return
	}

	newID := uuid.NewString()
	newZone := zone.ZoneConfig{
		ID:        newID,
		Plugin:    zone.NormalizePluginID(req.Plugin),
		RefreshMs: req.RefreshMs,
	}
	if req.InsertBeforeID != "" {
		insertAt := -1
		for i, z := range page.Zones {
			if z.ID == req.InsertBeforeID {
				insertAt = i
				break
			}
		}
		if insertAt >= 0 {
			page.Zones = append(page.Zones[:insertAt], append([]zone.ZoneConfig{newZone}, page.Zones[insertAt:]...)...)
		} else {
			page.Zones = append(page.Zones, newZone)
		}
	} else {
		page.Zones = append(page.Zones, newZone)
	}
	if err := page.RedistributeWidths(zone.DisplayWidthPx, zone.MinZoneWidthPx); err != nil {
		s.respondError(w, "Width redistribution failed: "+err.Error(), http.StatusUnprocessableEntity)
		return
	}

	if err := s.draft.UpdateDraft(draft); err != nil {
		s.respondError(w, "Draft update failed: "+err.Error(), http.StatusInternalServerError)
		return
	}
	s.respondSuccess(w, "Zone added to draft", map[string]any{"id": newID})
}

// handleDraftZone handles single-zone operations in the draft.
// DELETE /api/layout/draft/zones/:id
// PATCH  /api/layout/draft/zones/:id
func (s *Server) handleDraftZone(w http.ResponseWriter, r *http.Request) {
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}

	path := r.URL.Path
	const prefix = "/api/layout/draft/zones/"
	if !strings.HasPrefix(path, prefix) {
		s.respondError(w, "Not found", http.StatusNotFound)
		return
	}
	zoneID := strings.TrimSuffix(strings.TrimPrefix(path, prefix), "/")
	if zoneID == "" {
		s.respondError(w, "Zone ID required", http.StatusBadRequest)
		return
	}

	draft, err := s.draft.OpenDraft()
	if err != nil {
		s.respondError(w, "Failed to open draft: "+err.Error(), http.StatusInternalServerError)
		return
	}

	switch r.Method {
	case http.MethodDelete:
		s.draftDeleteZone(w, draft, zoneID)
	case http.MethodPatch:
		s.draftPatchZone(w, r, draft, zoneID)
	default:
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

func (s *Server) draftDeleteZone(w http.ResponseWriter, draft *zone.Config, zoneID string) {
	found := false
	for pi := range draft.Pages {
		page := &draft.Pages[pi]
		for zi, z := range page.Zones {
			if z.ID != zoneID {
				continue
			}
			page.Zones = append(page.Zones[:zi], page.Zones[zi+1:]...)
			if len(page.Zones) > 0 {
				if err := page.RedistributeWidths(zone.DisplayWidthPx, zone.MinZoneWidthPx); err != nil {
					s.respondError(w, "Width redistribution failed: "+err.Error(), http.StatusInternalServerError)
					return
				}
			}
			found = true
			break
		}
		if found {
			break
		}
	}
	if !found {
		s.respondError(w, "Zone not found in draft", http.StatusNotFound)
		return
	}
	if err := s.draft.UpdateDraft(draft); err != nil {
		s.respondError(w, "Draft update failed: "+err.Error(), http.StatusInternalServerError)
		return
	}
	s.respondSuccess(w, "Zone removed from draft", nil)
}

func (s *Server) draftPatchZone(w http.ResponseWriter, r *http.Request, draft *zone.Config, zoneID string) {
	var patch struct {
		Plugin       *string        `json:"plugin,omitempty"`
		RefreshMs    *int           `json:"refresh_ms,omitempty"`
		Width        *int           `json:"width,omitempty"`
		Align        *string        `json:"align,omitempty"`
		PluginConfig map[string]any `json:"plugin_config,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&patch); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	found := false
	for pi := range draft.Pages {
		for zi := range draft.Pages[pi].Zones {
			z := &draft.Pages[pi].Zones[zi]
			if z.ID != zoneID {
				continue
			}
			if patch.Plugin != nil {
				z.Plugin = *patch.Plugin
			}
			if patch.RefreshMs != nil {
				z.RefreshMs = *patch.RefreshMs
			}
			if patch.Width != nil {
				z.Width = *patch.Width
			}
			if patch.Align != nil {
				z.Align = zone.Alignment(*patch.Align)
			}
			if patch.PluginConfig != nil {
				z.PluginConfig = patch.PluginConfig
			}
			found = true
			break
		}
		if found {
			break
		}
	}
	if !found {
		s.respondError(w, "Zone not found in draft", http.StatusNotFound)
		return
	}
	if err := s.draft.UpdateDraft(draft); err != nil {
		s.respondError(w, "Draft update failed: "+err.Error(), http.StatusInternalServerError)
		return
	}

	// Live-preview: push plugin config to the running sampler immediately so
	// the device display updates without requiring a commit first.
	if patch.PluginConfig != nil && s.zoneNotifier != nil {
		if err := s.zoneNotifier.BroadcastZoneConfigChange(zoneID, patch.PluginConfig); err != nil {
			s.logger.Warn("live preview config push failed", "zone_id", zoneID, "error", err)
		}
	}

	s.respondSuccess(w, "Zone updated in draft", nil)
}

// handleCommitDraft persists the current draft to the store and reloads.
// POST /api/layout/commit
func (s *Server) handleCommitDraft(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}
	if !s.draft.HasDraft() {
		s.respondError(w, "No active draft to commit", http.StatusConflict)
		return
	}

	err := s.draft.Commit(func(cfg *zone.Config) error {
		return zone.SaveConfigToDB(s.layoutStore, cfg)
	})
	if err != nil {
		s.respondError(w, "Commit failed: "+err.Error(), http.StatusInternalServerError)
		return
	}

	s.triggerLayoutReload()
	s.hub.Broadcast(WSMessage{Type: "committed_state", Data: map[string]any{"committed": true}})
	s.respondSuccess(w, "Draft committed", nil)
}

// handleDiscardDraft reverts the draft to the committed state.
// POST /api/layout/discard
func (s *Server) handleDiscardDraft(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}
	s.draft.Discard()
	s.respondSuccess(w, "Draft discarded", nil)
}

// handleDraftReorderZones reorders zones within a page of the current draft.
// POST /api/layout/draft/zones/reorder  body: {"page_index": 0, "order": ["id-a","id-b"]}
func (s *Server) handleDraftReorderZones(w http.ResponseWriter, r *http.Request) {
	if s.draft == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}
	var req struct {
		PageIndex int      `json:"page_index"`
		Order     []string `json:"order"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}
	draft, err := s.draft.OpenDraft()
	if err != nil {
		s.respondError(w, "Failed to open draft: "+err.Error(), http.StatusInternalServerError)
		return
	}
	if req.PageIndex < 0 || req.PageIndex >= len(draft.Pages) {
		s.respondError(w, fmt.Sprintf("page_index %d out of range", req.PageIndex), http.StatusBadRequest)
		return
	}
	page := &draft.Pages[req.PageIndex]
	byID := make(map[string]zone.ZoneConfig, len(page.Zones))
	for _, z := range page.Zones {
		byID[z.ID] = z
	}
	reordered := make([]zone.ZoneConfig, 0, len(req.Order))
	for _, id := range req.Order {
		if z, ok := byID[id]; ok {
			reordered = append(reordered, z)
		}
	}
	page.Zones = reordered
	if err := s.draft.UpdateDraft(draft); err != nil {
		s.respondError(w, "Draft update failed: "+err.Error(), http.StatusInternalServerError)
		return
	}
	s.respondSuccess(w, "Draft zones reordered", nil)
}

func normalizeConfigPluginIDs(cfg *zone.Config) {
	for i := range cfg.Pages {
		for j := range cfg.Pages[i].Zones {
			cfg.Pages[i].Zones[j].Plugin = zone.NormalizePluginID(cfg.Pages[i].Zones[j].Plugin)
		}
	}
}
