package api

import "net/http"

// handlePluginCatalog serves GET /api/plugins — returns all available plugins
// with their descriptors and config schemas.
func (s *Server) handlePluginCatalog(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	if s.pluginCatalog == nil {
		s.respondJSON(w, []any{}, http.StatusOK)
		return
	}

	entries := s.pluginCatalog.GetCatalog()
	if entries == nil {
		s.respondJSON(w, []struct{}{}, http.StatusOK)
		return
	}
	s.respondJSON(w, entries, http.StatusOK)
}
