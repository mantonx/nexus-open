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

// ── mock reloader ─────────────────────────────────────────────────────────────

type mockReloader struct {
	cfg *zone.Config
}

func (m *mockReloader) ReloadFromConfig(cfg *zone.Config) error {
	m.cfg = cfg
	return nil
}

func (m *mockReloader) GetConfig() *zone.Config {
	return m.cfg
}

func (m *mockReloader) NumPages() int {
	if m.cfg == nil {
		return 0
	}
	return len(m.cfg.Pages)
}

// ── mock store (adds ImportLayout to the existing real store) ─────────────────

type mockImportStore struct {
	*store.DB
	imported bool
}

func (m *mockImportStore) ImportLayout(_ []store.StoredPage, _ map[int64][]store.StoredZone) error {
	m.imported = true
	return nil
}

// ── helpers ───────────────────────────────────────────────────────────────────

func simpleConfig() *zone.Config {
	return &zone.Config{
		Name:    "Test",
		Version: "1.0",
		Theme:   zone.DefaultTheme(),
		Pages: []zone.Page{
			{
				Name: "Main",
				Zones: []zone.ZoneConfig{
					{ID: "z1", Width: 320, Plugin: "builtin:clock", RefreshMs: 1000},
					{ID: "z2", Width: 320, Plugin: "builtin:clock", RefreshMs: 1000},
				},
			},
		},
	}
}

// seedStore writes cfg into db so OpenDraft (which reads from DB) has data.
func seedStore(t *testing.T, db *store.DB, cfg *zone.Config) {
	t.Helper()
	for pi, p := range cfg.Pages {
		pageID, err := db.CreatePage(p.Name, pi)
		if err != nil {
			t.Fatalf("seed page: %v", err)
		}
		for zi, z := range p.Zones {
			if err := db.CreateZone(store.StoredZone{
				ID: z.ID, PageID: pageID, Plugin: z.Plugin,
				WidthPx: z.Width, RefreshMs: z.RefreshMs, Ord: zi,
			}); err != nil {
				t.Fatalf("seed zone: %v", err)
			}
		}
	}
}

func newDraftTestServer(t *testing.T) (*Server, *mockReloader) {
	t.Helper()
	srv, db := newTestServerWithStore(t)
	cfg := simpleConfig()
	seedStore(t, db, cfg)
	mr := &mockReloader{cfg: cfg}
	srv.SetLayoutReloader(mr)
	return srv, mr
}

// ── tests ─────────────────────────────────────────────────────────────────────

func TestDraft_GetOpensFromCommitted(t *testing.T) {
	srv, _ := newDraftTestServer(t)

	req := httptest.NewRequest(http.MethodGet, "/api/layout/draft", nil)
	w := httptest.NewRecorder()
	srv.handleGetDraft(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("GET /api/layout/draft: got %d, want 200", w.Code)
	}
	var resp map[string]any
	if err := json.Unmarshal(w.Body.Bytes(), &resp); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	if active, _ := resp["active"].(bool); !active {
		t.Error("expected active=true in draft response")
	}
	if resp["layout"] == nil {
		t.Error("expected layout in draft response")
	}
}

func TestDraft_GetReturnsDraftAfterOpen(t *testing.T) {
	srv, _ := newDraftTestServer(t)

	// Open the draft first.
	req := httptest.NewRequest(http.MethodGet, "/api/layout/draft", nil)
	w := httptest.NewRecorder()
	srv.handleGetDraft(w, req)
	if w.Code != http.StatusOK {
		t.Fatalf("first GET: %d", w.Code)
	}

	// Second GET should return same active draft.
	req2 := httptest.NewRequest(http.MethodGet, "/api/layout/draft", nil)
	w2 := httptest.NewRecorder()
	srv.handleGetDraft(w2, req2)
	if w2.Code != http.StatusOK {
		t.Fatalf("second GET: %d", w2.Code)
	}
	if !srv.draft.HasDraft() {
		t.Error("expected draft to be active after two GETs")
	}
}

func TestDraft_PutReplacesLayout(t *testing.T) {
	srv, mr := newDraftTestServer(t)

	cfg := simpleConfig()
	cfg.Pages[0].Zones[0].Plugin = "builtin:debug"

	body, _ := json.Marshal(cfg)
	req := httptest.NewRequest(http.MethodPut, "/api/layout/draft", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	srv.handlePutDraft(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("PUT /api/layout/draft: got %d, want 200; body: %s", w.Code, w.Body)
	}
	// Verify the reloader received the updated config (live preview).
	if mr.cfg == nil || mr.cfg.Pages[0].Zones[0].Plugin != "builtin:debug" {
		t.Error("expected reloader to have received updated config with builtin:debug")
	}
}

func TestDraft_PutInvalidLayoutReturns422(t *testing.T) {
	srv, _ := newDraftTestServer(t)

	// Config with zone widths that don't sum to 640.
	bad := &zone.Config{
		Name:    "Bad",
		Version: "1.0",
		Theme:   zone.DefaultTheme(),
		Pages: []zone.Page{
			{Name: "p", Zones: []zone.ZoneConfig{
				{ID: "z1", Width: 100, Plugin: "builtin:clock", RefreshMs: 1000},
			}},
		},
	}
	body, _ := json.Marshal(bad)
	req := httptest.NewRequest(http.MethodPut, "/api/layout/draft", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	srv.handlePutDraft(w, req)

	if w.Code != http.StatusUnprocessableEntity {
		t.Fatalf("expected 422, got %d", w.Code)
	}
}

func TestDraft_AddZone(t *testing.T) {
	srv, _ := newDraftTestServer(t)

	body := bytes.NewBufferString(`{"page_index":0,"plugin":"builtin:debug","refresh_ms":500}`)
	req := httptest.NewRequest(http.MethodPost, "/api/layout/draft/zones", body)
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	srv.handleDraftZones(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("POST /api/layout/draft/zones: got %d, want 200; body: %s", w.Code, w.Body)
	}
	draft := srv.draft.GetDraft()
	if len(draft.Pages[0].Zones) != 3 {
		t.Errorf("expected 3 zones after add, got %d", len(draft.Pages[0].Zones))
	}
	// Widths must sum to 640.
	sum := 0
	for _, z := range draft.Pages[0].Zones {
		sum += z.Width
	}
	if sum != 640 {
		t.Errorf("zone widths sum to %d, want 640", sum)
	}
}

func TestDraft_AddZone_CapEnforced(t *testing.T) {
	// Build a full-page config (MaxZonesPerPage zones) and load it into the draft directly.
	full := &zone.Config{
		Name: "Full", Version: "1.0", Theme: zone.DefaultTheme(),
		Pages: []zone.Page{{Name: "p"}},
	}
	for i := range zone.MaxZonesPerPage {
		full.Pages[0].Zones = append(full.Pages[0].Zones, zone.ZoneConfig{
			ID: "z" + string(rune('a'+i)), Width: 640 / zone.MaxZonesPerPage,
			Plugin: "builtin:clock", RefreshMs: 1000,
		})
	}
	if err := full.Pages[0].RedistributeWidths(zone.DisplayWidthPx, zone.MinZoneWidthPx); err != nil {
		t.Fatalf("setup: %v", err)
	}

	srv, _ := newDraftTestServer(t)
	// Bypass OpenDraft — seed the in-memory draft directly with the full config.
	if err := srv.draft.UpdateDraft(full); err != nil {
		t.Fatalf("seed draft: %v", err)
	}

	body := bytes.NewBufferString(`{"page_index":0,"plugin":"builtin:debug","refresh_ms":500}`)
	req := httptest.NewRequest(http.MethodPost, "/api/layout/draft/zones", body)
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	srv.handleDraftZones(w, req)

	if w.Code != http.StatusUnprocessableEntity {
		t.Fatalf("expected 422 for zone cap, got %d; body: %s", w.Code, w.Body)
	}
}

func TestDraft_DeleteZone(t *testing.T) {
	srv, _ := newDraftTestServer(t)

	// Open draft first.
	getReq := httptest.NewRequest(http.MethodGet, "/api/layout/draft", nil)
	srv.handleGetDraft(httptest.NewRecorder(), getReq)

	req := httptest.NewRequest(http.MethodDelete, "/api/layout/draft/zones/z1", nil)
	w := httptest.NewRecorder()
	srv.handleDraftZone(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("DELETE zone: got %d, want 200; body: %s", w.Code, w.Body)
	}
	draft := srv.draft.GetDraft()
	if len(draft.Pages[0].Zones) != 1 {
		t.Errorf("expected 1 zone after delete, got %d", len(draft.Pages[0].Zones))
	}
	if draft.Pages[0].Zones[0].Width != 640 {
		t.Errorf("remaining zone width = %d, want 640", draft.Pages[0].Zones[0].Width)
	}
}

func TestDraft_DeleteZone_NotFound(t *testing.T) {
	srv, _ := newDraftTestServer(t)
	srv.draft.UpdateDraft(simpleConfig()) //nolint:errcheck

	req := httptest.NewRequest(http.MethodDelete, "/api/layout/draft/zones/nope", nil)
	w := httptest.NewRecorder()
	srv.handleDraftZone(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404 for unknown zone, got %d", w.Code)
	}
}

func TestDraft_PatchZone(t *testing.T) {
	srv, _ := newDraftTestServer(t)
	srv.draft.UpdateDraft(simpleConfig()) //nolint:errcheck

	plugin := "builtin:debug"
	body, _ := json.Marshal(map[string]any{"plugin": plugin})
	req := httptest.NewRequest(http.MethodPatch, "/api/layout/draft/zones/z1", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	srv.handleDraftZone(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("PATCH zone: got %d; body: %s", w.Code, w.Body)
	}
	draft := srv.draft.GetDraft()
	if draft.Pages[0].Zones[0].Plugin != plugin {
		t.Errorf("plugin not updated: got %q, want %q", draft.Pages[0].Zones[0].Plugin, plugin)
	}
}

func TestDraft_Discard(t *testing.T) {
	srv, _ := newDraftTestServer(t)
	srv.draft.UpdateDraft(simpleConfig()) //nolint:errcheck

	req := httptest.NewRequest(http.MethodPost, "/api/layout/discard", nil)
	w := httptest.NewRecorder()
	srv.handleDiscardDraft(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("POST /api/layout/discard: got %d", w.Code)
	}
	if srv.draft.HasDraft() {
		t.Error("expected draft to be cleared after discard")
	}
}

func TestDraft_CommitNoActive(t *testing.T) {
	srv, _ := newDraftTestServer(t)

	req := httptest.NewRequest(http.MethodPost, "/api/layout/commit", nil)
	w := httptest.NewRecorder()
	srv.handleCommitDraft(w, req)

	if w.Code != http.StatusConflict {
		t.Fatalf("expected 409 for commit with no draft, got %d", w.Code)
	}
}

func TestDraft_NoReloaderReturns503(t *testing.T) {
	srv := newTestServer(t)
	// Do NOT call SetLayoutReloader — draft is nil.

	req := httptest.NewRequest(http.MethodGet, "/api/layout/draft", nil)
	w := httptest.NewRecorder()
	srv.handleGetDraft(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503 with no reloader, got %d", w.Code)
	}
}
