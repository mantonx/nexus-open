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
	t.Cleanup(func() { db.Close() })
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
		db.Close()
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

// ── ZoneConfig ────────────────────────────────────────────────────────────────

func TestZoneConfig_MissingReturnsNil(t *testing.T) {
	db := openTestDB(t)
	cfg, err := db.GetZoneConfig("nonexistent")
	if err != nil {
		t.Fatalf("GetZoneConfig: %v", err)
	}
	if cfg != nil {
		t.Errorf("expected nil for missing zone, got %v", cfg)
	}
}

func TestZoneConfig_SetGet(t *testing.T) {
	db := openTestDB(t)

	want := map[string]interface{}{"unit": "celsius", "graph": true}
	if err := db.SetZoneConfig("zone-1", want); err != nil {
		t.Fatalf("SetZoneConfig: %v", err)
	}

	got, err := db.GetZoneConfig("zone-1")
	if err != nil {
		t.Fatalf("GetZoneConfig: %v", err)
	}
	if got["unit"] != "celsius" {
		t.Errorf("unit = %v, want celsius", got["unit"])
	}
}

func TestZoneConfig_Upsert(t *testing.T) {
	db := openTestDB(t)

	db.SetZoneConfig("z", map[string]interface{}{"v": "1"}) //nolint:errcheck
	db.SetZoneConfig("z", map[string]interface{}{"v": "2"}) //nolint:errcheck

	got, _ := db.GetZoneConfig("z")
	if got["v"] != "2" {
		t.Errorf("after upsert v = %v, want 2", got["v"])
	}
}

func TestZoneConfig_Delete(t *testing.T) {
	db := openTestDB(t)

	db.SetZoneConfig("z", map[string]interface{}{"k": "v"}) //nolint:errcheck
	if err := db.DeleteZoneConfig("z"); err != nil {
		t.Fatalf("DeleteZoneConfig: %v", err)
	}

	got, err := db.GetZoneConfig("z")
	if err != nil || got != nil {
		t.Errorf("expected nil after delete, got %v (err %v)", got, err)
	}
}

func TestZoneConfig_GetAll(t *testing.T) {
	db := openTestDB(t)

	db.SetZoneConfig("a", map[string]interface{}{"n": 1}) //nolint:errcheck
	db.SetZoneConfig("b", map[string]interface{}{"n": 2}) //nolint:errcheck

	all, err := db.GetAllZoneConfigs()
	if err != nil {
		t.Fatalf("GetAllZoneConfigs: %v", err)
	}
	if len(all) != 2 {
		t.Errorf("len = %d, want 2", len(all))
	}
}
