package api

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"log/slog"

	"github.com/mantonx/nexus-open/internal/device"
	"github.com/mantonx/nexus-open/internal/store"
	config "github.com/mantonx/nexus-open/internal/settings"
)

// ── helpers ───────────────────────────────────────────────────────────────────

func newTestServer(t *testing.T) *Server {
	t.Helper()
	cfg, err := config.NewManagerFromPath(filepath.Join(t.TempDir(), "test.db"), nil)
	if err != nil {
		t.Fatalf("settings.NewManagerFromPath: %v", err)
	}
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelError}))
	return NewServer(":0", cfg, device.NewMockDevice(), logger)
}

func newTestServerWithStore(t *testing.T) (*Server, *store.DB) {
	t.Helper()
	db, err := store.Open(filepath.Join(t.TempDir(), "layout.db"), nil)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	srv := newTestServer(t)
	srv.SetLayoutStore(db)
	return srv, db
}

// stubZoneCfgManager is an in-memory ConfigManager used in API handler tests.
// It avoids needing real zone rows in the DB while still exercising the handler wiring.
type stubZoneCfgManager struct {
	data map[string]map[string]any
}

func (s *stubZoneCfgManager) GetZoneOverride(zoneID string) map[string]any {
	return s.data[zoneID]
}
func (s *stubZoneCfgManager) SetZoneOverride(zoneID string, cfg map[string]any) error {
	s.data[zoneID] = cfg
	return nil
}
func (s *stubZoneCfgManager) DeleteZoneOverride(zoneID string) error {
	delete(s.data, zoneID)
	return nil
}
func (s *stubZoneCfgManager) BroadcastZoneConfigChange(zoneID string, cfg map[string]any) error {
	return nil
}

func newTestZoneCfgManager(_ *testing.T) *stubZoneCfgManager {
	return &stubZoneCfgManager{data: make(map[string]map[string]any)}
}

// ── Zone config handlers ──────────────────────────────────────────────────────

func TestZoneConfig_GetEmpty(t *testing.T) {
	srv := newTestServer(t)
	srv.SetZoneConfigManager(newTestZoneCfgManager(t))

	req := httptest.NewRequest("GET", "/api/zones/zone-1/config", nil)
	req.SetPathValue("id", "zone-1")
	w := httptest.NewRecorder()
	srv.handleZones(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200", w.Code)
	}
	var resp map[string]interface{}
	json.NewDecoder(w.Body).Decode(&resp) //nolint:errcheck
	if resp["zone_id"] != "zone-1" {
		t.Errorf("zone_id = %v, want zone-1", resp["zone_id"])
	}
}

func TestZoneConfig_SetAndGet(t *testing.T) {
	srv := newTestServer(t)
	srv.SetZoneConfigManager(newTestZoneCfgManager(t))

	body, _ := json.Marshal(map[string]interface{}{"unit": "celsius"})

	// POST config.
	req := httptest.NewRequest("POST", "/api/zones/zone-1/config", bytes.NewReader(body))
	req.SetPathValue("id", "zone-1")
	w := httptest.NewRecorder()
	srv.handleZones(w, req)
	if w.Code != http.StatusOK {
		t.Errorf("POST status = %d, want 200", w.Code)
	}

	// GET it back.
	req = httptest.NewRequest("GET", "/api/zones/zone-1/config", nil)
	req.SetPathValue("id", "zone-1")
	w = httptest.NewRecorder()
	srv.handleZones(w, req)

	var resp map[string]interface{}
	json.NewDecoder(w.Body).Decode(&resp) //nolint:errcheck
	cfg, _ := resp["config"].(map[string]interface{})
	if cfg["unit"] != "celsius" {
		t.Errorf("unit = %v, want celsius", cfg["unit"])
	}
}

func TestZoneConfig_Delete(t *testing.T) {
	srv := newTestServer(t)
	mgr := newTestZoneCfgManager(t)
	srv.SetZoneConfigManager(mgr)

	// Set then delete.
	body, _ := json.Marshal(map[string]interface{}{"unit": "fahrenheit"})
	req := httptest.NewRequest("POST", "/api/zones/z/config", bytes.NewReader(body))
	req.SetPathValue("id", "z")
	srv.handleZones(httptest.NewRecorder(), req)

	req = httptest.NewRequest("DELETE", "/api/zones/z/config", nil)
	req.SetPathValue("id", "z")
	w := httptest.NewRecorder()
	srv.handleZones(w, req)
	if w.Code != http.StatusOK {
		t.Errorf("DELETE status = %d, want 200", w.Code)
	}
}

func TestZoneConfig_InvalidMethod(t *testing.T) {
	srv := newTestServer(t)
	srv.SetZoneConfigManager(newTestZoneCfgManager(t))

	req := httptest.NewRequest("PATCH", "/api/zones/z/config", nil)
	w := httptest.NewRecorder()
	srv.handleZones(w, req)
	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}

func TestZoneConfig_InvalidJSON(t *testing.T) {
	srv := newTestServer(t)
	srv.SetZoneConfigManager(newTestZoneCfgManager(t))

	req := httptest.NewRequest("POST", "/api/zones/z/config", bytes.NewReader([]byte("not-json")))
	w := httptest.NewRecorder()
	srv.handleZones(w, req)
	if w.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want 400", w.Code)
	}
}

func TestZoneConfig_NilManagerReturns503(t *testing.T) {
	srv := newTestServer(t) // no SetZoneConfigManager

	req := httptest.NewRequest("GET", "/api/zones/z/config", nil)
	w := httptest.NewRecorder()
	srv.handleZones(w, req)
	if w.Code != http.StatusInternalServerError {
		t.Errorf("status = %d, want 500", w.Code)
	}
}

// ── Zone status handler ───────────────────────────────────────────────────────

func TestZoneStatus_NoProvider(t *testing.T) {
	srv := newTestServer(t) // no zoneStatus wired in

	req := httptest.NewRequest("GET", "/api/zones/z/status", nil)
	req.SetPathValue("id", "z")
	w := httptest.NewRecorder()
	srv.handleZoneStatus(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200", w.Code)
	}
	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp) //nolint:errcheck
	if resp["status"] != "loading" {
		t.Errorf("status = %q, want loading", resp["status"])
	}
}

func TestZoneStatus_MethodNotAllowed(t *testing.T) {
	srv := newTestServer(t)

	req := httptest.NewRequest("POST", "/api/zones/z/status", nil)
	req.SetPathValue("id", "z")
	w := httptest.NewRecorder()
	srv.handleZoneStatus(w, req)
	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}

// ── Layout handlers ───────────────────────────────────────────────────────────

func TestGetLayout_Empty(t *testing.T) {
	srv, _ := newTestServerWithStore(t)

	req := httptest.NewRequest("GET", "/api/layout", nil)
	w := httptest.NewRecorder()
	srv.handleGetLayout(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200", w.Code)
	}
	var pages []interface{}
	json.NewDecoder(w.Body).Decode(&pages) //nolint:errcheck
	if len(pages) != 0 {
		t.Errorf("expected empty layout, got %d pages", len(pages))
	}
}

func TestGetLayout_MethodNotAllowed(t *testing.T) {
	srv, _ := newTestServerWithStore(t)

	req := httptest.NewRequest("POST", "/api/layout", nil)
	w := httptest.NewRecorder()
	srv.handleGetLayout(w, req)
	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}

func TestGetLayout_NoStore(t *testing.T) {
	srv := newTestServer(t) // no layoutStore

	req := httptest.NewRequest("GET", "/api/layout", nil)
	w := httptest.NewRecorder()
	srv.handleGetLayout(w, req)
	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("status = %d, want 503", w.Code)
	}
}

func TestCreatePage(t *testing.T) {
	srv, db := newTestServerWithStore(t)

	body, _ := json.Marshal(map[string]interface{}{"name": "Stats", "ord": 0})
	req := httptest.NewRequest("POST", "/api/layout/pages", bytes.NewReader(body))
	w := httptest.NewRecorder()
	srv.handleLayoutPages(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}

	pages, _ := db.GetPages()
	if len(pages) != 1 || pages[0].Name != "Stats" {
		t.Errorf("expected 1 page named Stats, got %v", pages)
	}
}

func TestCreatePage_MissingName(t *testing.T) {
	srv, _ := newTestServerWithStore(t)

	body, _ := json.Marshal(map[string]interface{}{"ord": 0})
	req := httptest.NewRequest("POST", "/api/layout/pages", bytes.NewReader(body))
	w := httptest.NewRecorder()
	srv.handleLayoutPages(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want 400", w.Code)
	}
}

func TestCreateZone_Valid(t *testing.T) {
	srv, db := newTestServerWithStore(t)

	pageID, _ := db.CreatePage("P", 0)

	body, _ := json.Marshal(map[string]interface{}{
		"id":         "cpu",
		"page_id":    pageID,
		"width_px":   640,
		"plugin":     "builtin:placeholder",
		"refresh_ms": 2000,
		"align":      "center",
	})
	req := httptest.NewRequest("POST", "/api/layout/zones", bytes.NewReader(body))
	w := httptest.NewRecorder()
	srv.handleCreateZone(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}

	zones, _ := db.GetZonesForPage(pageID)
	if len(zones) != 1 || zones[0].ID != "cpu" {
		t.Errorf("expected 1 zone 'cpu', got %v", zones)
	}
}

func TestDeletePage(t *testing.T) {
	srv, db := newTestServerWithStore(t)

	db.CreatePage("Temp", 0) //nolint:errcheck
	pages, _ := db.GetPages()
	id := pages[0].ID

	req := httptest.NewRequest("DELETE", "/api/layout/pages/"+string(rune('0'+id)), nil)
	req.SetPathValue("pageID", "1")
	w := httptest.NewRecorder()
	srv.handleLayoutPage(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}

	remaining, _ := db.GetPages()
	if len(remaining) != 0 {
		t.Errorf("expected 0 pages after delete, got %d", len(remaining))
	}
}
