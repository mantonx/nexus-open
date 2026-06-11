package api

import (
	"encoding/json"
	"net/http"
)

// handleNavigateState returns current page index + page list for the Flutter preview UI.
// GET /api/navigate/state
func (s *Server) handleNavigateState(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.navigator == nil {
		s.respondError(w, "Navigator not available", http.StatusServiceUnavailable)
		return
	}
	s.respondJSON(w, map[string]any{
		"current_page": s.navigator.GetCurrentPage(),
		"num_pages":    s.navigator.NumPages(),
		"pages":        s.navigator.GetPageInfos(),
	}, http.StatusOK)
}

// handleNavigatePage switches to a specific page index.
// POST /api/navigate/page  body: {"page": 2}
func (s *Server) handleNavigatePage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.navigator == nil {
		s.respondError(w, "Navigator not available", http.StatusServiceUnavailable)
		return
	}

	var req struct {
		Page int `json:"page"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	if err := s.navigator.SwitchPage(req.Page); err != nil {
		s.respondError(w, err.Error(), http.StatusBadRequest)
		return
	}

	s.BroadcastPageState()
	s.respondSuccess(w, "Page switched", map[string]any{"page": req.Page})
}
