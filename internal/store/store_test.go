package store

import (
	"path/filepath"
	"testing"
)

func openTestDB(t *testing.T) *DB {
	t.Helper()
	db, err := Open(filepath.Join(t.TempDir(), "test.db"), nil)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })
	return db
}

// ── Migration ─────────────────────────────────────────────────────────────────

func TestMigration_IdempotentOpen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "test.db")

	// Open twice — second open should not fail even though all tables already exist.
	for i := range 2 {
		db, err := Open(path, nil)
		if err != nil {
			t.Fatalf("Open #%d: %v", i+1, err)
		}
		_ = db.Close()
	}
}

func TestMigration_ModulesToPluginsPathRewrite(t *testing.T) {
	db := openTestDB(t)

	// Simulate a pre-migration4 zone inserted after migration5 has already
	// renamed the column to "plugin". Migration4 updates the path value only.
	if _, err := db.db.Exec(
		`INSERT INTO pages(id, name, ord) VALUES (99, 'Test', 0)`,
	); err != nil {
		t.Fatalf("insert page: %v", err)
	}
	if _, err := db.db.Exec(
		`INSERT INTO zones(id, page_id, ord, width_px, plugin, refresh_ms, align, config_json, theme_json)
		 VALUES ('z', 99, 0, 640, 'exec:./plugins/cpu-temp/cpu-temp', 2000, 'center', '{}', '{}')`,
	); err != nil {
		t.Fatalf("insert zone: %v", err)
	}

	zones, _ := db.GetZonesForPage(99)
	if len(zones) == 0 {
		t.Fatal("no zones found")
	}
	if want := "exec:./plugins/cpu-temp/cpu-temp"; zones[0].Plugin != want {
		t.Errorf("plugin = %q, want %q", zones[0].Plugin, want)
	}
}

func TestMigration_ModulesToPlugins_SkipsNonModules(t *testing.T) {
	db := openTestDB(t)

	if _, err := db.db.Exec(
		`INSERT INTO pages(id, name, ord) VALUES (98, 'Test', 0)`,
	); err != nil {
		t.Fatalf("insert page: %v", err)
	}
	if _, err := db.db.Exec(
		`INSERT INTO zones(id, page_id, ord, width_px, plugin, refresh_ms, align, config_json, theme_json)
		 VALUES ('z', 98, 0, 640, 'builtin:clock', 1000, 'center', '{}', '{}')`,
	); err != nil {
		t.Fatalf("insert zone: %v", err)
	}

	zones, _ := db.GetZonesForPage(98)
	if zones[0].Plugin != "builtin:clock" {
		t.Errorf("builtin plugin was incorrectly rewritten to %q", zones[0].Plugin)
	}
}

func TestMigration_SchemaVersion(t *testing.T) {
	db := openTestDB(t)

	var v int
	if err := db.db.QueryRow(`SELECT version FROM schema_version`).Scan(&v); err != nil {
		t.Fatalf("read schema_version: %v", err)
	}
	if v != currentSchemaVersion {
		t.Errorf("schema_version = %d, want %d", v, currentSchemaVersion)
	}
}

func TestMigration5_ZonePluginConfigTableDropped(t *testing.T) {
	db := openTestDB(t)

	var n int
	err := db.db.QueryRow(
		`SELECT count(*) FROM sqlite_master WHERE type='table' AND name='zone_plugin_config'`,
	).Scan(&n)
	if err != nil {
		t.Fatalf("query sqlite_master: %v", err)
	}
	if n != 0 {
		t.Errorf("zone_plugin_config table still exists after migration5")
	}
}

// ── Settings ──────────────────────────────────────────────────────────────────

func TestSettings_GetDefault(t *testing.T) {
	db := openTestDB(t)
	got := db.GetSetting("missing_key", "fallback")
	if got != "fallback" {
		t.Errorf("GetSetting missing key = %q, want %q", got, "fallback")
	}
}

func TestSettings_SetGet(t *testing.T) {
	db := openTestDB(t)

	if err := db.SetSetting("theme", "dark"); err != nil {
		t.Fatalf("SetSetting: %v", err)
	}
	if got := db.GetSetting("theme", ""); got != "dark" {
		t.Errorf("GetSetting = %q, want %q", got, "dark")
	}
}

func TestSettings_Upsert(t *testing.T) {
	db := openTestDB(t)

	db.SetSetting("key", "v1") //nolint:errcheck
	db.SetSetting("key", "v2") //nolint:errcheck

	if got := db.GetSetting("key", ""); got != "v2" {
		t.Errorf("GetSetting after upsert = %q, want v2", got)
	}
}

func TestSettings_GetAll(t *testing.T) {
	db := openTestDB(t)

	db.SetSetting("a", "1") //nolint:errcheck
	db.SetSetting("b", "2") //nolint:errcheck

	all, err := db.GetSettings()
	if err != nil {
		t.Fatalf("GetSettings: %v", err)
	}
	if all["a"] != "1" || all["b"] != "2" {
		t.Errorf("GetSettings = %v, want a=1 b=2", all)
	}
}

func TestSettings_SetMultiple(t *testing.T) {
	db := openTestDB(t)

	if err := db.SetSettings(map[string]string{"x": "10", "y": "20"}); err != nil {
		t.Fatalf("SetSettings: %v", err)
	}
	if got := db.GetSetting("x", ""); got != "10" {
		t.Errorf("x = %q, want 10", got)
	}
	if got := db.GetSetting("y", ""); got != "20" {
		t.Errorf("y = %q, want 20", got)
	}
}

// ── ZonePluginConfig ──────────────────────────────────────────────────────────
// Per-zone plugin config now lives in zones.config_json (single source of
// truth after migration5 dropped zone_plugin_config).

func TestZonePluginConfig_MissingReturnsNil(t *testing.T) {
	db := openTestDB(t)
	cfg, err := db.GetZonePluginConfig("nonexistent")
	// zone doesn't exist → sql.ErrNoRows; nil config is acceptable
	if err == nil && cfg != nil {
		t.Errorf("expected nil for missing zone, got %v", cfg)
	}
}

func TestZonePluginConfig_SetGet(t *testing.T) {
	db := openTestDB(t)
	pageID, _ := db.CreatePage("P", 0)
	z := StoredZone{
		ID: "zone-1", PageID: pageID, Ord: 0, WidthPx: 640,
		Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
		ConfigJSON: map[string]any{"unit": "celsius", "graph": true},
	}
	if err := db.CreateZone(z); err != nil {
		t.Fatalf("CreateZone: %v", err)
	}

	got, err := db.GetZonePluginConfig("zone-1")
	if err != nil {
		t.Fatalf("GetZonePluginConfig: %v", err)
	}
	if got["unit"] != "celsius" {
		t.Errorf("unit = %v, want celsius", got["unit"])
	}
}

func TestZonePluginConfig_Update(t *testing.T) {
	db := openTestDB(t)
	pageID, _ := db.CreatePage("P", 0)
	z := StoredZone{
		ID: "z", PageID: pageID, Ord: 0, WidthPx: 640,
		Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
		ConfigJSON: map[string]any{"v": "1"},
	}
	db.CreateZone(z) //nolint:errcheck

	if err := db.SetZonePluginConfig("z", map[string]any{"v": "2"}); err != nil {
		t.Fatalf("SetZonePluginConfig: %v", err)
	}
	got, _ := db.GetZonePluginConfig("z")
	if got["v"] != "2" {
		t.Errorf("after update v = %v, want 2", got["v"])
	}
}

func TestZonePluginConfig_Clear(t *testing.T) {
	db := openTestDB(t)
	pageID, _ := db.CreatePage("P", 0)
	z := StoredZone{
		ID: "z", PageID: pageID, Ord: 0, WidthPx: 640,
		Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
		ConfigJSON: map[string]any{"k": "v"},
	}
	db.CreateZone(z) //nolint:errcheck

	if err := db.SetZonePluginConfig("z", map[string]any{}); err != nil {
		t.Fatalf("SetZonePluginConfig (clear): %v", err)
	}
	got, err := db.GetZonePluginConfig("z")
	if err != nil {
		t.Fatalf("GetZonePluginConfig after clear: %v", err)
	}
	if got != nil {
		t.Errorf("expected nil after clearing config, got %v", got)
	}
}
