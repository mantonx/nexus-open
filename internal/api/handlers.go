package api

import (
	"encoding/json"
	"errors"
	"net/http"
	"path/filepath"

	"github.com/mantonx/nexus-next/internal/assets"
	"github.com/mantonx/nexus-next/internal/device"
	"github.com/mantonx/nexus-next/internal/settings"
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
	defer func() { _ = file.Close() }()

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

// handleServeImage serves a single image file by filename.
func (s *Server) handleServeImage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		s.respondError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Path is /api/images/<filename>
	filename := r.PathValue("filename")
	if filename == "" {
		s.respondError(w, "Filename required", http.StatusBadRequest)
		return
	}

	imagesDir, err := assets.GetImagesDir()
	if err != nil {
		s.respondError(w, "Images directory unavailable", http.StatusInternalServerError)
		return
	}

	http.ServeFile(w, r, filepath.Join(imagesDir, filename))
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

// BrightnessRequest represents a request to set device brightness.
// openapi:schema BrightnessRequest
type BrightnessRequest struct {
	// openapi:description Brightness level (0-100)
	// openapi:example 75
	Brightness int `json:"brightness"`
}

// handleBrightness handles brightness control (POST only).
// openapi:operation POST /api/device/brightness setBrightness
// openapi:summary Set display brightness
// openapi:description Sets the brightness of the iCUE Nexus device display (0-100)
// openapi:tag Device
// openapi:consumes application/json
// openapi:produces application/json
// openapi:param brightness body BrightnessRequest true --- Brightness level to set
// openapi:response 200 SuccessResponse --- Brightness updated successfully
// openapi:response 400 ErrorResponse --- Invalid brightness value
// openapi:response 503 ErrorResponse --- Device not available
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
// openapi:operation GET /api/device/info getDeviceInfo
// openapi:summary Get device information
// openapi:description Returns information about the connected iCUE Nexus device including firmware version and hardware details
// openapi:tag Device
// openapi:produces application/json
// openapi:response 200 SuccessResponse --- Device information retrieved successfully
// openapi:response 503 ErrorResponse --- Device not available
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
		"firmware":  firmware,
		"vendorId":  "0x1b1c",
		"productId": "0x1b8e",
		"model":     "iCUE Nexus",
	}

	// Surface the last connect error as a human-readable action hint.
	if s.lastConnectErr != nil {
		switch {
		case errors.Is(s.lastConnectErr, device.ErrPermissionDenied):
			info["connect_error"] = "USB permission denied. Run: sudo usermod -a -G plugdev $USER then log out and back in."
		case errors.Is(s.lastConnectErr, device.ErrDeviceBusy):
			info["connect_error"] = "Device is in use by another application. Close iCUE or other Nexus software."
		case errors.Is(s.lastConnectErr, device.ErrDeviceNotFound):
			info["connect_error"] = "iCUE Nexus not found. Is it plugged in?"
		default:
			info["connect_error"] = s.lastConnectErr.Error()
		}
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
