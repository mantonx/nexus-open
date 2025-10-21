package api

import (
	"encoding/json"
	"net/http"

	"nexus-open/internal/assets"
	"nexus-open/internal/config"
)

// ErrorResponse represents an API error response.
type ErrorResponse struct {
	Error   string `json:"error"`
	Message string `json:"message,omitempty"`
}

// SuccessResponse represents a successful API response.
type SuccessResponse struct {
	Status  string      `json:"status"`
	Message string      `json:"message,omitempty"`
	Data    interface{} `json:"data,omitempty"`
}

// handleHealth returns server health status.
func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	response := map[string]interface{}{
		"status":  "ok",
		"version": "1.0.0",
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
func (s *Server) handleGetConfig(w http.ResponseWriter, r *http.Request) {
	cfg := s.cfg.Get()
	s.respondJSON(w, cfg, http.StatusOK)
}

// handleUpdateConfig updates the configuration.
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

	s.respondSuccess(w, "Configuration updated successfully", nil)
}

// handleImageUpload processes image uploads.
func (s *Server) handleImageUpload(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Parse multipart form (max 10MB)
	if err := r.ParseMultipartForm(10 << 20); err != nil {
		s.respondError(w, "Failed to parse form: "+err.Error(), http.StatusBadRequest)
		return
	}

	file, header, err := r.FormFile("image")
	if err != nil {
		s.respondError(w, "Failed to read image file: "+err.Error(), http.StatusBadRequest)
		return
	}
	defer file.Close()

	// Save image
	if err := assets.SaveImage(header.Filename, file); err != nil {
		s.logger.Error("failed to save image", "error", err, "filename", header.Filename)
		s.respondError(w, "Failed to save image", http.StatusInternalServerError)
		return
	}

	s.logger.Info("image uploaded", "filename", header.Filename, "size", header.Size)
	s.respondSuccess(w, "Image uploaded successfully", map[string]string{
		"filename": header.Filename,
	})
}

// handleListImages returns a list of all uploaded images.
func (s *Server) handleListImages(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	images, err := assets.GetImages()
	if err != nil {
		s.logger.Error("failed to list images", "error", err)
		s.respondError(w, "Failed to list images", http.StatusInternalServerError)
		return
	}

	s.respondJSON(w, images, http.StatusOK)
}

// handleDeleteImage deletes an uploaded image.
func (s *Server) handleDeleteImage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost && r.Method != http.MethodDelete {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	filename := r.FormValue("filename")
	if filename == "" {
		s.respondError(w, "Missing filename parameter", http.StatusBadRequest)
		return
	}

	if err := assets.DeleteImage(filename); err != nil {
		s.logger.Error("failed to delete image", "error", err, "filename", filename)
		s.respondError(w, "Failed to delete image", http.StatusInternalServerError)
		return
	}

	s.logger.Info("image deleted", "filename", filename)
	s.respondSuccess(w, "Image deleted successfully", nil)
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

// handleBrightness handles brightness control (POST only).
func (s *Server) handleBrightness(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	if s.device == nil {
		s.respondError(w, "Device not available", http.StatusServiceUnavailable)
		return
	}

	var request struct {
		Brightness int `json:"brightness"`
	}

	if err := json.NewDecoder(r.Body).Decode(&request); err != nil {
		s.respondError(w, "Invalid request body", http.StatusBadRequest)
		return
	}

	if request.Brightness < 0 || request.Brightness > 100 {
		s.respondError(w, "Brightness must be between 0 and 100", http.StatusBadRequest)
		return
	}

	if err := s.device.SetBrightness(request.Brightness); err != nil {
		s.logger.Error("failed to set brightness", "error", err)
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}

	s.respondSuccess(w, "Brightness updated", map[string]int{"brightness": request.Brightness})
}

// handleDeviceInfo handles device information queries (GET only).
func (s *Server) handleDeviceInfo(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	if s.device == nil {
		s.respondError(w, "Device not available", http.StatusServiceUnavailable)
		return
	}

	firmware, err := s.device.GetFirmwareVersion()
	if err != nil {
		s.logger.Error("failed to get firmware version", "error", err)
		s.respondError(w, err.Error(), http.StatusInternalServerError)
		return
	}

	info := map[string]interface{}{
		"firmware": firmware,
		"vendorId":  "0x1b1c",
		"productId": "0x1b8e",
		"model":     "iCUE Nexus",
	}

	s.respondSuccess(w, "Device information", info)
}

// Window control handlers

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

	s.respondSuccess(w, "Window hide command sent", map[string]string{"state": "hidden"})
}
