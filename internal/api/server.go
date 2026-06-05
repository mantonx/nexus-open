// Package api provides the HTTP API server for configuration and image management.
package api

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"time"

	settings "github.com/mantonx/nexus-next/internal/settings"
	"github.com/mantonx/nexus-next/internal/zone"
)

// DeviceController provides an interface for controlling device features.
type DeviceController interface {
	SetBrightness(brightness int) error
	GetFirmwareVersion() (string, error)
	IsConnected() bool
}

// ZoneConfigNotifier can notify zones about config changes.
type ZoneConfigNotifier interface {
	BroadcastZoneConfigChange(zoneID string, config map[string]interface{}) error
}

// ZoneStatusProvider returns per-zone health status from the sampler.
type ZoneStatusProvider interface {
	GetZoneStatus(zoneID string) zone.ZoneStatus
	AllZoneStatuses() map[string]zone.ZoneStatus
}

// Server manages the HTTP API server.
type Server struct {
	server          *http.Server
	logger          *slog.Logger
	cfg             *settings.Manager
	zoneCfg         *zone.ConfigManager
	device          DeviceController
	zoneNotifier    ZoneConfigNotifier   // Notifies zones of config changes
	zoneStatus      ZoneStatusProvider   // Reports module error state
	windowState     string               // "shown" or "hidden"
	windowStateCh   chan string
	hub             *hub
	lastConnectErr  error // last device connect error, shown in /api/device/info
}

// NewServer creates a new API server instance.
func NewServer(addr string, cfg *settings.Manager, device DeviceController, logger *slog.Logger) *Server {
	if logger == nil {
		logger = slog.Default()
	}

	s := &Server{
		logger:        logger,
		cfg:           cfg,
		device:        device,
		windowState:   "shown",
		windowStateCh: make(chan string, 10),
		hub:           newHub(logger),
	}

	// Create router
	mux := http.NewServeMux()
	s.registerRoutes(mux)

	// Create HTTP server with timeouts
	s.server = &http.Server{
		Addr:        addr,
		Handler:     s.middleware(mux),
		ReadTimeout: 10 * time.Second,
		// WriteTimeout must be 0 for WebSocket: Go's net/http fires it after
		// the deadline regardless of whether the connection was hijacked, which
		// tears down long-lived WS connections after 10 s.
		WriteTimeout: 0,
		IdleTimeout:  60 * time.Second,
	}

	return s
}

// Start begins listening for HTTP requests.
func (s *Server) Start() error {
	s.logger.Info("starting API server", "addr", s.server.Addr)

	if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		return fmt.Errorf("server error: %w", err)
	}

	return nil
}

// Shutdown gracefully stops the API server.
func (s *Server) Shutdown(ctx context.Context) error {
	s.logger.Info("shutting down API server")

	if err := s.server.Shutdown(ctx); err != nil {
		return fmt.Errorf("server shutdown error: %w", err)
	}

	return nil
}

// SetZoneConfigManager sets the zone config manager.
func (s *Server) SetZoneConfigManager(zoneCfg *zone.ConfigManager) {
	s.zoneCfg = zoneCfg
	s.logger.Debug("zone config manager registered")
}

// SetZoneConfigNotifier sets the zone config notifier.
func (s *Server) SetZoneConfigNotifier(notifier ZoneConfigNotifier) {
	s.zoneNotifier = notifier
	s.logger.Debug("zone config notifier registered")
}

// Hub returns the WebSocket hub so callers can broadcast messages (e.g. frames).
func (s *Server) Hub() *hub {
	return s.hub
}

// SetLastConnectError stores the most recent device connect error so it can be
// surfaced in API responses (e.g. GET /api/device/info).
func (s *Server) SetLastConnectError(err error) {
	s.lastConnectErr = err
}

// SetZoneStatusProvider wires in the sampler so zone status can be queried.
func (s *Server) SetZoneStatusProvider(p ZoneStatusProvider) {
	s.zoneStatus = p
}

// registerRoutes sets up all API endpoints.
func (s *Server) registerRoutes(mux *http.ServeMux) {
	// API endpoints
	mux.HandleFunc("/api/health", s.handleHealth)
	mux.HandleFunc("/api/config", s.handleConfig)
	mux.HandleFunc("/api/images/upload", s.handleImageUpload)
	mux.HandleFunc("/api/images/delete", s.handleDeleteImage)
	mux.HandleFunc("/api/images/{filename}", s.handleServeImage)
	mux.HandleFunc("/api/images", s.handleListImages)

	// Zone and module config endpoints
	mux.HandleFunc("/api/modules/", s.handleModuleConfig)
	mux.HandleFunc("/api/zones/", s.handleZones)
	mux.HandleFunc("/api/zones/{id}/status", s.handleZoneStatus)

	// HID feature endpoints
	mux.HandleFunc("/api/device/brightness", s.handleBrightness)
	mux.HandleFunc("/api/device/info", s.handleDeviceInfo)

	// WebSocket endpoint — live frame + state push
	mux.HandleFunc("/api/ws", s.handleWS)

	// Window control endpoints
	mux.HandleFunc("/api/window/state", s.handleWindowState)
	mux.HandleFunc("/api/window/show", s.handleWindowShow)
	mux.HandleFunc("/api/window/hide", s.handleWindowHide)

	// OpenAPI 3.0 spec endpoints
	mux.HandleFunc("/openapi.yaml", s.handleOpenAPISpec)
	mux.HandleFunc("/openapi.json", s.handleOpenAPISpecJSON)
	mux.HandleFunc("/docs", s.handleSwaggerUI)
}

// handleOpenAPISpec serves the OpenAPI 3.0 YAML spec
func (s *Server) handleOpenAPISpec(w http.ResponseWriter, r *http.Request) {
	specPath := "api/openapi.yaml"
	data, err := os.ReadFile(specPath)
	if err != nil {
		s.logger.Error("failed to read OpenAPI spec", "error", err)
		http.Error(w, "OpenAPI spec not found", http.StatusNotFound)
		return
	}
	w.Header().Set("Content-Type", "application/x-yaml")
	w.Write(data)
}

// handleOpenAPISpecJSON serves the OpenAPI 3.0 JSON spec
func (s *Server) handleOpenAPISpecJSON(w http.ResponseWriter, r *http.Request) {
	// For now, we only have YAML, but we could convert it
	// Or serve the JSON version if we generate it
	http.Redirect(w, r, "/openapi.yaml", http.StatusTemporaryRedirect)
}

// handleSwaggerUI serves the Swagger UI for the OpenAPI spec
func (s *Server) handleSwaggerUI(w http.ResponseWriter, r *http.Request) {
	html := `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Nexus Open API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: "/openapi.yaml",
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout"
            });
        };
    </script>
</body>
</html>`
	w.Header().Set("Content-Type", "text/html")
	w.Write([]byte(html))
}
