package api

import "net/http"

// handleWindowState returns the current window state.
func (s *Server) handleWindowState(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	response := map[string]string{
		"state": s.windowState,
	}
	s.respondJSON(w, response, http.StatusOK)
}

// handleWindowShow sets window state to "show".
func (s *Server) handleWindowShow(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	s.windowState = "shown"
	select {
	case s.windowStateCh <- "show":
	default:
	}
	s.broadcastWindowState("shown")

	s.respondSuccess(w, "Window show command sent", map[string]string{"state": "shown"})
}

// handleWindowHide sets window state to "hide".
func (s *Server) handleWindowHide(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	s.windowState = "hidden"
	select {
	case s.windowStateCh <- "hide":
	default:
	}
	s.broadcastWindowState("hidden")

	s.respondSuccess(w, "Window hide command sent", map[string]string{"state": "hidden"})
}

// handleWindowClosed is called by Flutter just before it exits on close.
// It signals the tray to kill the Flutter process and restart it on next Show.
func (s *Server) handleWindowClosed(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	s.windowState = "hidden"
	select {
	case s.windowClosedCh <- struct{}{}:
	default:
	}

	s.respondSuccess(w, "Window closed", map[string]string{"state": "hidden"})
}

// WindowClosedChannel returns the channel that fires when Flutter reports it is closing.
func (s *Server) WindowClosedChannel() <-chan struct{} {
	return s.windowClosedCh
}
