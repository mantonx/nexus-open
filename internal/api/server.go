// Package api provides the HTTP API server for configuration and image management.
package api

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"time"

	"github.com/mantonx/nexus-open/internal/device"
	settings "github.com/mantonx/nexus-open/internal/settings"
	"github.com/mantonx/nexus-open/internal/store"
	"github.com/mantonx/nexus-open/internal/zone"
)

// DeviceController provides an interface for controlling device features.
type DeviceController interface {
	SetBrightness(brightness int) error
	GetFirmwareVersion() (string, error)
	GetDeviceInfo() device.DeviceInfo
	IsConnected() bool
}

// ZoneConfigNotifier can notify zones about config changes.
type ZoneConfigNotifier interface {
	BroadcastZoneConfigChange(zoneID string, config map[string]any) error
}

// ZoneCfgManager reads and writes per-zone plugin configuration.
type ZoneCfgManager interface {
	GetZoneOverride(zoneID string) map[string]any
	SetZoneOverride(zoneID string, cfg map[string]any) error
	DeleteZoneOverride(zoneID string) error
	BroadcastZoneConfigChange(zoneID string, cfg map[string]any) error
}

// ZoneStatusProvider returns per-zone health status from the sampler.
type ZoneStatusProvider interface {
	GetZoneStatus(zoneID string) zone.ZoneStatus
	AllZoneStatuses() map[string]zone.ZoneStatus
}

// PluginCatalogProvider returns the list of available plugins with their schemas.
type PluginCatalogProvider interface {
	GetCatalog() []zone.CatalogEntry
}

// SwipeSimulator is the subset of zone.Manager needed to drive synthetic swipes.
type SwipeSimulator interface {
	UpdateLiveSwipe(progress float32, isLeft bool) error
	FinalizeLiveSwipe(progress float32, velocity float32, isLeft bool) error
	CancelLiveSwipe() error
}

// Navigator is the subset of zone.Manager needed for page navigation from the UI.
type Navigator interface {
	GetCurrentPage() int
	NumPages() int
	GetPageInfos() []zone.PageInfo
	SwitchPage(pageIndex int) error
}

// LayoutEditor is the subset of interfaces needed to edit the live layout.
// The store and zone manager are wired separately so neither depends on the other.
type LayoutStore interface {
	GetPages() ([]store.StoredPage, error)
	GetZonesForPage(pageID int64) ([]store.StoredZone, error)
	GetFullLayout() ([]store.StoredPage, map[int64][]store.StoredZone, error)
	CreatePage(name string, ord int) (int64, error)
	UpdatePage(id int64, name string, ord int) error
	DeletePage(id int64) error
	ReorderPages(order []int64) error
	GetZonePageID(zoneID string) (int64, error)
	CreateZone(z store.StoredZone) error
	UpdateZone(z store.StoredZone) error
	DeleteZone(id string) error
	ReorderZones(pageID int64, order []string) error
	HasLayout() (bool, error)
	ImportLayout(pages []store.StoredPage, zonesByPage map[int64][]store.StoredZone) error
}

// LayoutReloader applies a new layout config to the live zone manager without restarting.
type LayoutReloader interface {
	ReloadFromConfig(config *zone.Config) error
	GetConfig() *zone.Config
	NumPages() int
}

// Server manages the HTTP API server.
type Server struct {
	server          *http.Server
	logger          *slog.Logger
	cfg             *settings.Manager
	zoneCfg         ZoneCfgManager
	device          DeviceController
	zoneNotifier    ZoneConfigNotifier
	zoneStatus      ZoneStatusProvider
	swipeSim        SwipeSimulator        // for /api/debug/swipe
	navigator       Navigator             // for /api/navigate/page
	layoutStore     LayoutStore           // for /api/layout/*
	layoutReloader  LayoutReloader        // for live reloads after layout edits
	pluginCatalog   PluginCatalogProvider // for /api/plugins
	draft           *DraftManager         // live draft session
	windowState     string                // "shown" or "hidden"
	windowStateCh   chan string
	windowClosedCh  chan struct{}
	hub             *hub
	lastConnectErr  error
	token           string // capability token for X-Nexus-Token validation
}

// NewServer creates a new API server instance.
func NewServer(addr string, cfg *settings.Manager, device DeviceController, logger *slog.Logger) *Server {
	if logger == nil {
		logger = slog.Default()
	}

	tok, err := LoadOrCreateToken()
	if err != nil {
		// Non-fatal: log and continue. Middleware treats an empty token as
		// "deny all protected requests", so the daemon stays up but the API is
		// locked until the token file is readable.
		logger.Warn("could not load capability token, all API requests will be denied", "error", err)
	}

	s := &Server{
		logger:         logger,
		cfg:            cfg,
		device:         device,
		token:          tok,
		windowState:    "shown",
		windowStateCh:  make(chan string, 10),
		windowClosedCh: make(chan struct{}, 1),
		hub:            newHub(logger),
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
func (s *Server) SetZoneConfigManager(zoneCfg ZoneCfgManager) {
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

// SetSwipeSimulator wires in the zone manager for the debug swipe endpoint.
func (s *Server) SetSwipeSimulator(sim SwipeSimulator) {
	s.swipeSim = sim
}

// SetZoneStatusProvider wires in the sampler so zone status can be queried.
func (s *Server) SetZoneStatusProvider(p ZoneStatusProvider) {
	s.zoneStatus = p
}

// SetNavigator wires in the zone manager for page navigation.
func (s *Server) SetNavigator(n Navigator) {
	s.navigator = n
}

// SetLayoutStore wires in the store for layout CRUD.
func (s *Server) SetLayoutStore(ls LayoutStore) {
	s.layoutStore = ls
}

// SetLayoutReloader wires in the zone manager for live layout reloads.
func (s *Server) SetLayoutReloader(lr LayoutReloader) {
	s.layoutReloader = lr
	if s.layoutStore != nil {
		s.draft = NewDraftManager(s.layoutStore, lr, s.hub.Broadcast)
	}
}

// Token returns the capability token required in X-Nexus-Token on all requests
// except GET /api/health. Used by tests to authorise their requests.
func (s *Server) Token() string {
	return s.token
}

// SetPluginCatalog wires in the sampler for GET /api/plugins.
func (s *Server) SetPluginCatalog(p PluginCatalogProvider) {
	s.pluginCatalog = p
}

// BroadcastPageState sends current page index and page list to all WS clients.
// Called by the app's render loop whenever the page changes.
func (s *Server) BroadcastPageState() {
	if s.navigator == nil {
		return
	}
	s.hub.Broadcast(WSMessage{
		Type: "page_state",
		Data: map[string]any{
			"current_page": s.navigator.GetCurrentPage(),
			"num_pages":    s.navigator.NumPages(),
			"pages":        s.navigator.GetPageInfos(),
		},
	})
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

	// Plugin catalog
	mux.HandleFunc("/api/plugins", s.handlePluginCatalog)

	// Zone config endpoints
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
	mux.HandleFunc("/api/window/closed", s.handleWindowClosed)

	// Layout editor endpoints
	mux.HandleFunc("/api/layout", s.handleGetLayout)
	mux.HandleFunc("/api/layout/export", s.handleLayoutExport)
	mux.HandleFunc("/api/layout/pages", s.handleLayoutPages)
	mux.HandleFunc("/api/layout/pages/reorder", s.handleReorderPages)
	mux.HandleFunc("/api/layout/pages/", s.handleLayoutPage)
	mux.HandleFunc("/api/layout/zones", s.handleCreateZone)
	mux.HandleFunc("/api/layout/zones/reorder", s.handleReorderZones)
	mux.HandleFunc("/api/layout/zones/", s.handleLayoutZone)

	// Draft endpoints — live preview before committing to the store
	mux.HandleFunc("GET /api/layout/draft", s.handleGetDraft)
	mux.HandleFunc("PUT /api/layout/draft", s.handlePutDraft)
	mux.HandleFunc("POST /api/layout/draft/zones", s.handleDraftZones)
	mux.HandleFunc("/api/layout/draft/zones/reorder", s.handleDraftReorderZones)
	mux.HandleFunc("/api/layout/draft/zones/", s.handleDraftZone)
	mux.HandleFunc("POST /api/layout/commit", s.handleCommitDraft)
	mux.HandleFunc("POST /api/layout/discard", s.handleDiscardDraft)

	// Debug endpoints — swipe simulation for tuning transition parameters
	mux.HandleFunc("/api/debug/swipe", s.handleDebugSwipe)

	// Preview navigation — page switching from Flutter UI
	mux.HandleFunc("/api/navigate/page", s.handleNavigatePage)
	mux.HandleFunc("/api/navigate/state", s.handleNavigateState)

	// Interactive drag endpoints — called on every drag frame from Flutter preview
	mux.HandleFunc("/api/debug/swipe/update", s.handleSwipeUpdate)
	mux.HandleFunc("/api/debug/swipe/finalize", s.handleSwipeFinalize)
	mux.HandleFunc("/api/debug/swipe/cancel", s.handleSwipeCancel)

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
	_, _ = w.Write(data)
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
	_, _ = w.Write([]byte(html))
}
