package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"github.com/google/uuid"
	"github.com/mantonx/nexus-next/internal/zone"
)

// handleGetDraft returns the current draft, opening one from the committed
// config if none is active.
// GET /api/layout/draft
func (s *Server) handleGetDraft(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.draft == nil || s.layoutReloader == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}
	committed := s.layoutReloader.GetConfig()
	draft := s.draft.OpenDraft(committed)
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
	if err := cfg.Validate(); err != nil {
		s.respondError(w, "Invalid layout: "+err.Error(), http.StatusUnprocessableEntity)
		return
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
	if s.draft == nil || s.layoutReloader == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}

	var req struct {
		PageIndex int    `json:"page_index"`
		Plugin    string `json:"plugin"`
		RefreshMs int    `json:"refresh_ms"`
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

	committed := s.layoutReloader.GetConfig()
	draft := s.draft.OpenDraft(committed)

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
	page.Zones = append(page.Zones, zone.ZoneConfig{
		ID:        newID,
		Plugin:    req.Plugin,
		RefreshMs: req.RefreshMs,
	})
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
	if s.draft == nil || s.layoutReloader == nil {
		s.respondError(w, "Draft manager not available", http.StatusServiceUnavailable)
		return
	}

	// Extract zone ID from path: /api/layout/draft/zones/{id}
	path := r.URL.Path
	const prefix = "/api/layout/draft/zones/"
	if !strings.HasPrefix(path, prefix) {
		s.respondError(w, "Not found", http.StatusNotFound)
		return
	}
	zoneID := strings.TrimPrefix(path, prefix)
	zoneID = strings.TrimSuffix(zoneID, "/")
	if zoneID == "" {
		s.respondError(w, "Zone ID required", http.StatusBadRequest)
		return
	}

	committed := s.layoutReloader.GetConfig()
	draft := s.draft.OpenDraft(committed)

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

	// Reload from the newly committed DB state so the manager has fresh IDs.
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

	committed := s.layoutReloader.GetConfig()
	s.draft.Discard(committed)
	s.respondSuccess(w, "Draft discarded", nil)
}
