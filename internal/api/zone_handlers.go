package api

import (
	"encoding/json"
	"net/http"
	"strings"
)

// handlePluginConfig handles GET and POST for plugin default configs.
// Routes: GET/POST /api/plugins/:pluginName/config
func (s *Server) handlePluginConfig(w http.ResponseWriter, r *http.Request) {
	// Parse plugin name from path: /api/plugins/{name}/config
	path := strings.TrimPrefix(r.URL.Path, "/api/plugins/")
	parts := strings.Split(path, "/")

	if len(parts) < 2 || parts[1] != "config" {
		s.respondError(w, "Invalid path format. Use /api/plugins/:name/config", http.StatusBadRequest)
		return
	}

	pluginName := parts[0]
	if pluginName == "" {
		s.respondError(w, "Plugin name is required", http.StatusBadRequest)
		return
	}

	// Convert plugin name to full path (e.g., "cpu-temp" -> "exec:./plugins/cpu-temp/cpu-temp")
	pluginPath := pluginNameToPath(pluginName)

	switch r.Method {
	case http.MethodGet:
		s.handleGetPluginConfig(w, r, pluginPath)
	case http.MethodPost:
		s.handleSetPluginConfig(w, r, pluginPath)
	default:
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// handleGetPluginConfig returns the default config for a plugin.
func (s *Server) handleGetPluginConfig(w http.ResponseWriter, r *http.Request, pluginPath string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	config := s.zoneCfg.GetPluginDefault(pluginPath)
	s.respondJSON(w, map[string]interface{}{
		"plugin": pluginPath,
		"config": config,
	}, http.StatusOK)
}

// handleSetPluginConfig sets the default config for a plugin.
func (s *Server) handleSetPluginConfig(w http.ResponseWriter, r *http.Request, pluginPath string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	var config map[string]interface{}
	if err := json.NewDecoder(r.Body).Decode(&config); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	if err := s.zoneCfg.SetPluginDefault(pluginPath, config); err != nil {
		s.logger.Error("failed to set plugin config", "plugin", pluginPath, "error", err)
		s.respondError(w, "Failed to save plugin config", http.StatusInternalServerError)
		return
	}

	s.logger.Info("plugin default config updated", "plugin", pluginPath, "config", config)
	s.respondSuccess(w, "Plugin config updated successfully", map[string]interface{}{
		"plugin": pluginPath,
		"config": config,
	})
}

// handleZones handles zone-specific endpoints.
// Routes: GET/POST/DELETE /api/zones/:id/config
func (s *Server) handleZones(w http.ResponseWriter, r *http.Request) {
	// Parse zone ID from path: /api/zones/{id}/config
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

// handleGetZoneConfig returns the config override for a zone.
func (s *Server) handleGetZoneConfig(w http.ResponseWriter, r *http.Request, zoneID string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	config := s.zoneCfg.GetZoneOverride(zoneID)
	s.respondJSON(w, map[string]interface{}{
		"zone_id": zoneID,
		"config":  config,
	}, http.StatusOK)
}

// handleSetZoneConfig sets a zone-specific config override.
func (s *Server) handleSetZoneConfig(w http.ResponseWriter, r *http.Request, zoneID string) {
	if s.zoneCfg == nil {
		s.respondError(w, "Zone config manager not initialized", http.StatusInternalServerError)
		return
	}

	var config map[string]interface{}
	if err := json.NewDecoder(r.Body).Decode(&config); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	if err := s.zoneCfg.SetZoneOverride(zoneID, config); err != nil {
		s.logger.Error("failed to set zone config", "zone_id", zoneID, "error", err)
		s.respondError(w, "Failed to save zone config", http.StatusInternalServerError)
		return
	}

	// Broadcast config change to the specific zone
	if s.zoneNotifier != nil {
		if err := s.zoneNotifier.BroadcastZoneConfigChange(zoneID, config); err != nil {
			s.logger.Warn("failed to broadcast zone config change", "zone_id", zoneID, "error", err)
			// Don't fail the request - config is saved, just notify failed
		}
	}

	s.logger.Info("zone config override set", "zone_id", zoneID, "config", config)
	s.respondSuccess(w, "Zone config updated successfully", map[string]interface{}{
		"zone_id": zoneID,
		"config":  config,
	})
}

// handleDeleteZoneConfig removes a zone-specific config override.
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

	s.logger.Info("zone config override removed", "zone_id", zoneID)
	s.respondSuccess(w, "Zone config override removed successfully", map[string]interface{}{
		"zone_id": zoneID,
	})
}

// handleZoneStatus returns the current health status of a zone's module.
// GET /api/zones/:id/status → {"status":"ok"|"error"|"timeout"|"loading","error":"..."}
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

// pluginNameToPath converts a short plugin name to full path.
// e.g., "cpu-temp" -> "exec:./plugins/cpu-temp/cpu-temp"
func pluginNameToPath(name string) string {
	// Check if it's already a full path
	if strings.HasPrefix(name, "exec:") || strings.HasPrefix(name, "builtin:") {
		return name
	}

	// Convert short name to full exec path
	return "exec:./plugins/" + name + "/" + name
}
