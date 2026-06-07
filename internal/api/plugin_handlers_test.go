package api

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/mantonx/nexus-open/internal/zone"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

type stubCatalog struct {
	entries []zone.CatalogEntry
}

func (s *stubCatalog) GetCatalog() []zone.CatalogEntry { return s.entries }

func TestPluginCatalog_Empty(t *testing.T) {
	srv := newTestServer(t)
	// No catalog provider wired — should return empty array, not 500.
	req := httptest.NewRequest("GET", "/api/plugins", nil)
	w := httptest.NewRecorder()
	srv.handlePluginCatalog(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", w.Code)
	}
	var body []any
	json.NewDecoder(w.Body).Decode(&body) //nolint:errcheck
	if len(body) != 0 {
		t.Errorf("want empty array, got %v", body)
	}
}

func TestPluginCatalog_ReturnsCatalog(t *testing.T) {
	srv := newTestServer(t)
	srv.SetPluginCatalog(&stubCatalog{
		entries: []zone.CatalogEntry{
			{
				ID:   "builtin:clock",
				Kind: "builtin",
				Descriptor: plugin.Descriptor{
					Name:      "Clock",
					Version:   "1.0.0",
					RefreshMs: 1000,
					Schema: plugin.ConfigSchema{
						Fields: []plugin.ConfigField{
							{Key: "clock_face", Label: "Face style", Type: plugin.FieldTypeEnum, Default: "digital"},
							{Key: "clock_format", Label: "Hour format", Type: plugin.FieldTypeEnum, Default: "12h"},
							{Key: "blink_colon", Label: "Blink colon", Type: plugin.FieldTypeBool, Default: true},
						},
					},
				},
			},
			{
				ID:   "exec:cpu-temp",
				Kind: "exec",
				Descriptor: plugin.Descriptor{
					Name:      "CPU Temperature",
					Version:   "1.0.0",
					RefreshMs: 2000,
				},
			},
		},
	})

	req := httptest.NewRequest("GET", "/api/plugins", nil)
	w := httptest.NewRecorder()
	srv.handlePluginCatalog(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", w.Code)
	}

	var entries []map[string]any
	json.NewDecoder(w.Body).Decode(&entries) //nolint:errcheck
	if len(entries) != 2 {
		t.Fatalf("want 2 entries, got %d", len(entries))
	}
	if entries[0]["id"] != "builtin:clock" {
		t.Errorf("entries[0].id = %v, want builtin:clock", entries[0]["id"])
	}
	if entries[1]["kind"] != "exec" {
		t.Errorf("entries[1].kind = %v, want exec", entries[1]["kind"])
	}

	// Schema should be present and contain fields.
	desc, _ := entries[0]["descriptor"].(map[string]any)
	schema, _ := desc["config_schema"].(map[string]any)
	fields, _ := schema["fields"].([]any)
	if len(fields) != 3 {
		t.Errorf("clock schema fields: want 3, got %d", len(fields))
	}
}

func TestPluginCatalog_MethodNotAllowed(t *testing.T) {
	srv := newTestServer(t)
	req := httptest.NewRequest("POST", "/api/plugins", nil)
	w := httptest.NewRecorder()
	srv.handlePluginCatalog(w, req)
	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("status = %d, want 405", w.Code)
	}
}
