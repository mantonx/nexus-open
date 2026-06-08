package api

import (
	"encoding/json"
	"net/http"
	"strings"
)

// handleZones handles zone-specific config endpoints.
// Routes: GET/POST/DELETE /api/zones/:id/config
func (s *Server) handleZones(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimPrefix(r.URL.Path, "/api/zones/")
	parts := strings.Split(path, "/")

	if len(parts) < 2 || parts[1] != "config" {
		s.respondError(w, "Invalid path format. Use /api/zones/:id/config", http.StatusBadRequest)
		return
	}

	zoneID := parts[0]
	if zoneID == "" {
		s.respondError(w, "Zone ID is required", http.StatusBadRequest)
		return
	}

	switch r.Method {
	case http.MethodGet:
		s.handleGetZoneConfig(w, r, zoneID)
	case http.MethodPost:
		s.handleSetZoneConfig(w, r, zoneID)
	case http.MethodDelete:
		s.handleDeleteZoneConfig(w, r, zoneID)
	default:
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// handleGetZoneConfig returns the plugin config for a zone.
func (s *Server) handleGetZoneConfig(w http.ResponseWriter, r *http.Request, zoneID string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	config := s.zoneCfg.GetZoneOverride(zoneID)
	s.respondJSON(w, map[string]any{
		"zone_id": zoneID,
		"config":  config,
	}, http.StatusOK)
}

// handleSetZoneConfig sets the plugin config for a zone.
func (s *Server) handleSetZoneConfig(w http.ResponseWriter, r *http.Request, zoneID string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	var config map[string]any
	if err := json.NewDecoder(r.Body).Decode(&config); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	if err := s.zoneCfg.SetZoneOverride(zoneID, config); err != nil {
		s.logger.Error("failed to set zone config", "zone_id", zoneID, "error", err)
		s.respondError(w, "Failed to save zone config", http.StatusInternalServerError)
		return
	}

	if s.zoneNotifier != nil {
		if err := s.zoneNotifier.BroadcastZoneConfigChange(zoneID, config); err != nil {
			s.logger.Warn("failed to broadcast zone config change", "zone_id", zoneID, "error", err)
		}
	}

	s.logger.Info("zone config updated", "zone_id", zoneID)
	s.respondSuccess(w, "Zone config updated successfully", map[string]any{
		"zone_id": zoneID,
	})
}

// handleDeleteZoneConfig clears the plugin config for a zone.
func (s *Server) handleDeleteZoneConfig(w http.ResponseWriter, r *http.Request, zoneID string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	if err := s.zoneCfg.DeleteZoneOverride(zoneID); err != nil {
		s.logger.Error("failed to delete zone config", "zone_id", zoneID, "error", err)
		s.respondError(w, "Failed to delete zone config", http.StatusInternalServerError)
		return
	}

	s.logger.Info("zone config cleared", "zone_id", zoneID)
	s.respondSuccess(w, "Zone config cleared successfully", map[string]any{
		"zone_id": zoneID,
	})
}

// handleZoneStatus returns the current health of a zone's plugin.
// GET /api/zones/:id/status
func (s *Server) handleZoneStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	zoneID := r.PathValue("id")
	if zoneID == "" {
		s.respondError(w, "Zone ID required", http.StatusBadRequest)
		return
	}

	if s.zoneStatus == nil {
		s.respondJSON(w, map[string]string{"status": "loading"}, http.StatusOK)
		return
	}

	st := s.zoneStatus.GetZoneStatus(zoneID)
	s.respondJSON(w, map[string]string{
		"status": st.Status,
		"error":  st.Error,
	}, http.StatusOK)
}
