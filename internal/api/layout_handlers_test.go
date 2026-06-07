package api

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/mantonx/nexus-next/internal/store"
	"github.com/mantonx/nexus-next/internal/zone"
)

// ── storeToZoneConfig ─────────────────────────────────────────────────────────

func TestStoreToZoneConfig_BasicConversion(t *testing.T) {
	pages := []store.StoredPage{
		{ID: 1, Name: "Main", Ord: 0},
	}
	zones := map[int64][]store.StoredZone{
		1: {
			{ID: "z1", PageID: 1, WidthPx: 320, Plugin: "builtin:clock", RefreshMs: 1000, Align: "center"},
			{ID: "z2", PageID: 1, WidthPx: 320, Plugin: "builtin:cpu", RefreshMs: 2000, Align: "left"},
		},
	}
	current := &zone.Config{Theme: zone.DefaultTheme()}

	cfg := storeToZoneConfig(pages, zones, current)

	if len(cfg.Pages) != 1 {
		t.Fatalf("want 1 page, got %d", len(cfg.Pages))
	}
	p := cfg.Pages[0]
	if p.Name != "Main" {
		t.Errorf("page name = %q, want Main", p.Name)
	}
	if len(p.Zones) != 2 {
		t.Fatalf("want 2 zones, got %d", len(p.Zones))
	}
	if p.Zones[0].ID != "z1" || p.Zones[0].Width != 320 {
		t.Errorf("zone[0] = %+v, unexpected", p.Zones[0])
	}
	if p.Zones[1].Plugin != "builtin:cpu" {
		t.Errorf("zone[1].Plugin = %q, want builtin:cpu", p.Zones[1].Plugin)
	}
}

func TestStoreToZoneConfig_NilCurrentGetsDefaultTheme(t *testing.T) {
	pages := []store.StoredPage{{ID: 1, Name: "P"}}
	zones := map[int64][]store.StoredZone{
		1: {{ID: "z1", PageID: 1, WidthPx: 640, Plugin: "builtin:clock", RefreshMs: 1000}},
	}

	cfg := storeToZoneConfig(pages, zones, nil)

	if cfg.Theme.Accent == "" {
		t.Error("expected non-empty default theme accent when current is nil")
	}
}

func TestStoreToZoneConfig_EmptyPagesKeepsCurrentPages(t *testing.T) {
	current := &zone.Config{
		Theme: zone.DefaultTheme(),
		Pages: []zone.Page{{Name: "Preserved"}},
	}

	cfg := storeToZoneConfig(nil, nil, current)

	if len(cfg.Pages) != 1 || cfg.Pages[0].Name != "Preserved" {
		t.Errorf("expected preserved page, got %v", cfg.Pages)
	}
}

func TestStoreToZoneConfig_ThemeOverrideApplied(t *testing.T) {
	pages := []store.StoredPage{{ID: 1, Name: "P"}}
	zones := map[int64][]store.StoredZone{
		1: {
			{
				ID: "z1", PageID: 1, WidthPx: 640, Plugin: "builtin:clock", RefreshMs: 1000,
				ThemeJSON: map[string]any{"accent": "#ff0000", "bg": "#000000"},
			},
		},
	}

	cfg := storeToZoneConfig(pages, zones, nil)

	z := cfg.Pages[0].Zones[0]
	if z.ThemeOverride == nil {
		t.Fatal("expected ThemeOverride to be set")
	}
	if z.ThemeOverride.Accent != "#ff0000" {
		t.Errorf("accent = %q, want #ff0000", z.ThemeOverride.Accent)
	}
	if z.ThemeOverride.Bg != "#000000" {
		t.Errorf("bg = %q, want #000000", z.ThemeOverride.Bg)
	}
}

// ── handleReorderPages ────────────────────────────────────────────────────────

func TestReorderPages_Valid(t *testing.T) {
	srv, db := newTestServerWithStore(t)

	id1, _ := db.CreatePage("A", 0)
	id2, _ := db.CreatePage("B", 1)

	body, _ := json.Marshal(map[string]any{"order": []int64{id2, id1}})
	req := httptest.NewRequest("POST", "/api/layout/pages/reorder", bytes.NewReader(body))
	w := httptest.NewRecorder()
	srv.handleReorderPages(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}
}

func TestReorderPages_MethodNotAllowed(t *testing.T) {
	srv, _ := newTestServerWithStore(t)

	req := httptest.NewRequest("GET", "/api/layout/pages/reorder", nil)
	w := httptest.NewRecorder()
	srv.handleReorderPages(w, req)

	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}

func TestReorderPages_InvalidJSON(t *testing.T) {
	srv, _ := newTestServerWithStore(t)

	req := httptest.NewRequest("POST", "/api/layout/pages/reorder", bytes.NewReader([]byte("notjson")))
	w := httptest.NewRecorder()
	srv.handleReorderPages(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want 400", w.Code)
	}
}

// ── handleReorderZones ────────────────────────────────────────────────────────

func TestReorderZones_Valid(t *testing.T) {
	srv, db := newTestServerWithStore(t)

	pageID, _ := db.CreatePage("P", 0)
	_ = db.CreateZone(store.StoredZone{ID: "z1", PageID: pageID, WidthPx: 320, Plugin: "builtin:clock", RefreshMs: 1000, Align: "center"})
	_ = db.CreateZone(store.StoredZone{ID: "z2", PageID: pageID, WidthPx: 320, Plugin: "builtin:clock", RefreshMs: 1000, Align: "center"})

	body, _ := json.Marshal(map[string]any{"page_id": pageID, "order": []string{"z2", "z1"}})
	req := httptest.NewRequest("POST", "/api/layout/zones/reorder", bytes.NewReader(body))
	w := httptest.NewRecorder()
	srv.handleReorderZones(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}
}

func TestReorderZones_MethodNotAllowed(t *testing.T) {
	srv, _ := newTestServerWithStore(t)

	req := httptest.NewRequest("GET", "/api/layout/zones/reorder", nil)
	w := httptest.NewRecorder()
	srv.handleReorderZones(w, req)

	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}

// ── handleLayoutZone ──────────────────────────────────────────────────────────

func setupZone(t *testing.T, srv *Server, db *store.DB) (pageID int64, zoneID string) {
	t.Helper()
	pageID, _ = db.CreatePage("P", 0)
	zoneID = "zone-alpha"
	err := db.CreateZone(store.StoredZone{
		ID: zoneID, PageID: pageID, WidthPx: 640,
		Plugin: "builtin:placeholder", RefreshMs: 2000, Align: "center",
	})
	if err != nil {
		t.Fatalf("CreateZone: %v", err)
	}
	return pageID, zoneID
}

func TestLayoutZone_PutUpdate(t *testing.T) {
	srv, db := newTestServerWithStore(t)
	pageID, zoneID := setupZone(t, srv, db)

	updated := store.StoredZone{
		ID: zoneID, PageID: pageID, WidthPx: 640,
		Plugin: "builtin:clock", RefreshMs: 3000, Align: "left",
	}
	body, _ := json.Marshal(updated)
	req := httptest.NewRequest("PUT", "/api/layout/zones/"+zoneID, bytes.NewReader(body))
	req.URL.Path = "/api/layout/zones/" + zoneID
	w := httptest.NewRecorder()
	srv.handleLayoutZone(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}

	zones, _ := db.GetZonesForPage(pageID)
	if len(zones) != 1 || zones[0].Plugin != "builtin:clock" {
		t.Errorf("zone after update: %+v", zones)
	}
}

func TestLayoutZone_PutInvalidJSON(t *testing.T) {
	srv, db := newTestServerWithStore(t)
	_, zoneID := setupZone(t, srv, db)

	req := httptest.NewRequest("PUT", "/api/layout/zones/"+zoneID, bytes.NewReader([]byte("notjson")))
	req.URL.Path = "/api/layout/zones/" + zoneID
	w := httptest.NewRecorder()
	srv.handleLayoutZone(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want 400", w.Code)
	}
}

func TestLayoutZone_Delete(t *testing.T) {
	srv, db := newTestServerWithStore(t)
	pageID, zoneID := setupZone(t, srv, db)

	req := httptest.NewRequest("DELETE", "/api/layout/zones/"+zoneID, nil)
	req.URL.Path = "/api/layout/zones/" + zoneID
	w := httptest.NewRecorder()
	srv.handleLayoutZone(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}

	zones, _ := db.GetZonesForPage(pageID)
	if len(zones) != 0 {
		t.Errorf("expected 0 zones after delete, got %d", len(zones))
	}
}

func TestLayoutZone_MethodNotAllowed(t *testing.T) {
	srv, db := newTestServerWithStore(t)
	_, zoneID := setupZone(t, srv, db)

	req := httptest.NewRequest("GET", "/api/layout/zones/"+zoneID, nil)
	req.URL.Path = "/api/layout/zones/" + zoneID
	w := httptest.NewRecorder()
	srv.handleLayoutZone(w, req)

	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}

func TestLayoutZone_NoStore(t *testing.T) {
	srv := newTestServer(t) // no layout store

	req := httptest.NewRequest("DELETE", "/api/layout/zones/z1", nil)
	req.URL.Path = "/api/layout/zones/z1"
	w := httptest.NewRecorder()
	srv.handleLayoutZone(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("status = %d, want 503", w.Code)
	}
}

// ── handleCommitDraft happy path ──────────────────────────────────────────────

func newDraftServerWithImportStore(t *testing.T) (*Server, *mockImportStore) {
	t.Helper()
	dbPath := t.TempDir() + "/layout.db"
	realDB, err := store.Open(dbPath, nil)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { _ = realDB.Close() })

	ms := &mockImportStore{DB: realDB}
	srv := newTestServer(t)
	srv.SetLayoutStore(ms)

	cfg := simpleConfig()
	seedStore(t, realDB, cfg)

	reloader := &mockReloader{cfg: cfg}
	srv.SetLayoutReloader(reloader)
	return srv, ms
}

func TestCommitDraft_HappyPath(t *testing.T) {
	srv, ms := newDraftServerWithImportStore(t)

	// Open draft first so there's something to commit.
	getReq := httptest.NewRequest("GET", "/api/layout/draft", nil)
	srv.handleGetDraft(httptest.NewRecorder(), getReq)

	req := httptest.NewRequest("POST", "/api/layout/commit", nil)
	w := httptest.NewRecorder()
	srv.handleCommitDraft(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}
	if !ms.imported {
		t.Error("expected ImportLayout to have been called")
	}
}

func TestDiscardDraft_HappyPath(t *testing.T) {
	srv, _ := newDraftServerWithImportStore(t)

	// Open a draft.
	getReq := httptest.NewRequest("GET", "/api/layout/draft", nil)
	srv.handleGetDraft(httptest.NewRecorder(), getReq)

	req := httptest.NewRequest("POST", "/api/layout/discard", nil)
	w := httptest.NewRecorder()
	srv.handleDiscardDraft(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("status = %d, want 200; body: %s", w.Code, w.Body.String())
	}
	if srv.draft.HasDraft() {
		t.Error("expected draft to be cleared after discard")
	}
}
