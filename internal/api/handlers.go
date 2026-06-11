package api

import (
	"encoding/json"
	"net/http"

	config "github.com/mantonx/nexus-open/internal/settings"
)

// ErrorResponse represents an API error response.
// openapi:schema ErrorResponse
type ErrorResponse struct {
	// openapi:description The error type/category
	// openapi:example Bad Request
	Error string `json:"error"`
	// openapi:description Detailed error message
	// openapi:example Invalid brightness value provided
	Message string `json:"message,omitempty"`
}

// SuccessResponse represents a successful API response.
// openapi:schema SuccessResponse
type SuccessResponse struct {
	// openapi:description Status of the operation
	// openapi:example success
	Status string `json:"status"`
	// openapi:description Human-readable success message
	// openapi:example Configuration updated successfully
	Message string `json:"message,omitempty"`
	// openapi:description Additional response data
	Data interface{} `json:"data,omitempty"`
}

// handleHealth returns server health status.
// openapi:operation GET /api/health getHealth
// openapi:summary Health check endpoint
// openapi:description Returns the health status of the API server including version information
// openapi:tag Device
// openapi:produces application/json
// openapi:response 200 SuccessResponse --- Health status retrieved successfully
func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	deviceConnected := s.device != nil && s.device.IsConnected()

	// "ok" — API up and device connected
	// "degraded" — API up but device not connected
	status := "ok"
	if !deviceConnected {
		status = "degraded"
	}

	response := map[string]interface{}{
		"status":           status,
		"version":          "1.0.0",
		"first_run":        s.cfg.IsFirstRun,
		"device_connected": deviceConnected,
	}

	s.respondJSON(w, response, http.StatusOK)
}

// handleConfig handles GET and POST requests for configuration.
func (s *Server) handleConfig(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		s.handleGetConfig(w, r)
	case http.MethodPost:
		s.handleUpdateConfig(w, r)
	default:
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// handleGetConfig returns the current configuration.
// openapi:operation GET /api/config getConfig
// openapi:summary Get current configuration
// openapi:description Returns the current application configuration including display settings, location, and modules
// openapi:tag Config
// openapi:produces application/json
// openapi:response 200 Config --- Configuration retrieved successfully
// openapi:response 500 ErrorResponse --- Internal server error
func (s *Server) handleGetConfig(w http.ResponseWriter, r *http.Request) {
	cfg := s.cfg.Get()
	s.respondJSON(w, cfg, http.StatusOK)
}

// handleUpdateConfig updates the configuration.
// openapi:operation POST /api/config updateConfig
// openapi:summary Update configuration
// openapi:description Updates the application configuration with new settings
// openapi:tag Config
// openapi:consumes application/json
// openapi:produces application/json
// openapi:param config body Config true --- Configuration object to update
// openapi:response 200 SuccessResponse --- Configuration updated successfully
// openapi:response 400 ErrorResponse --- Invalid configuration provided
// openapi:response 500 ErrorResponse --- Failed to save configuration
func (s *Server) handleUpdateConfig(w http.ResponseWriter, r *http.Request) {
	var newConfig config.Config

	if err := json.NewDecoder(r.Body).Decode(&newConfig); err != nil {
		s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
		return
	}

	// Validate configuration
	if err := newConfig.Validate(); err != nil {
		s.respondError(w, "Invalid configuration: "+err.Error(), http.StatusBadRequest)
		return
	}

	// Update configuration
	if err := s.cfg.Update(newConfig); err != nil {
		s.logger.Error("failed to update config", "error", err)
		s.respondError(w, "Failed to save configuration", http.StatusInternalServerError)
		return
	}

	// NOTE: Global config is now UI-only (colors, fonts, display settings)
	// Module configs are managed per-zone via /api/zones/:id/config

	s.respondSuccess(w, "Configuration updated successfully", nil)
}

// respondJSON sends a JSON response.
func (s *Server) respondJSON(w http.ResponseWriter, data interface{}, status int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)

	if err := json.NewEncoder(w).Encode(data); err != nil {
		s.logger.Error("failed to encode JSON response", "error", err)
	}
}

// respondError sends an error response.
func (s *Server) respondError(w http.ResponseWriter, message string, status int) {
	response := ErrorResponse{
		Error:   http.StatusText(status),
		Message: message,
	}
	s.respondJSON(w, response, status)
}

// respondSuccess sends a success response.
func (s *Server) respondSuccess(w http.ResponseWriter, message string, data interface{}) {
	response := SuccessResponse{
		Status:  "success",
		Message: message,
		Data:    data,
	}
	s.respondJSON(w, response, http.StatusOK)
}

