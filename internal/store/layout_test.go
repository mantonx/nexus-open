package store

import (
	"testing"
)

func makeZone(id string, pageID int64, ord, width int) StoredZone {
	return StoredZone{
		ID:        id,
		PageID:    pageID,
		Ord:       ord,
		WidthPx:   width,
		Plugin:    "builtin:placeholder",
		RefreshMs: 2000,
		Align:     "center",
	}
}

// ── Pages ─────────────────────────────────────────────────────────────────────

func TestPages_CreateGet(t *testing.T) {
	db := openTestDB(t)

	id, err := db.CreatePage("Main", 0)
	if err != nil {
		t.Fatalf("CreatePage: %v", err)
	}

	pages, err := db.GetPages()
	if err != nil {
		t.Fatalf("GetPages: %v", err)
	}
	if len(pages) != 1 || pages[0].ID != id || pages[0].Name != "Main" {
		t.Errorf("GetPages = %v, want 1 page named Main with id %d", pages, id)
	}
}

func TestPages_Update(t *testing.T) {
	db := openTestDB(t)

	id, _ := db.CreatePage("Old", 0)
	if err := db.UpdatePage(id, "New", 1); err != nil {
		t.Fatalf("UpdatePage: %v", err)
	}

	pages, _ := db.GetPages()
	if pages[0].Name != "New" || pages[0].Ord != 1 {
		t.Errorf("after update: %+v", pages[0])
	}
}

func TestPages_Delete(t *testing.T) {
	db := openTestDB(t)

	id, _ := db.CreatePage("Temp", 0)
	if err := db.DeletePage(id); err != nil {
		t.Fatalf("DeletePage: %v", err)
	}

	pages, _ := db.GetPages()
	if len(pages) != 0 {
		t.Errorf("expected 0 pages after delete, got %d", len(pages))
	}
}

func TestPages_Reorder(t *testing.T) {
	db := openTestDB(t)

	a, _ := db.CreatePage("A", 0)
	b, _ := db.CreatePage("B", 1)

	// Swap order.
	if err := db.ReorderPages([]int64{b, a}); err != nil {
		t.Fatalf("ReorderPages: %v", err)
	}

	pages, _ := db.GetPages()
	if pages[0].ID != b || pages[1].ID != a {
		t.Errorf("expected B then A, got %v", pages)
	}
}

func TestPages_OrderedByOrd(t *testing.T) {
	db := openTestDB(t)

	db.CreatePage("Z", 2) //nolint:errcheck
	db.CreatePage("A", 0) //nolint:errcheck
	db.CreatePage("M", 1) //nolint:errcheck

	pages, _ := db.GetPages()
	names := make([]string, len(pages))
	for i, p := range pages {
		names[i] = p.Name
	}
	if names[0] != "A" || names[1] != "M" || names[2] != "Z" {
		t.Errorf("pages out of order: %v", names)
	}
}

// ── Zones ─────────────────────────────────────────────────────────────────────

func TestZones_CreateGet(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	z := makeZone("cpu", pageID, 0, 320)

	if err := db.CreateZone(z); err != nil {
		t.Fatalf("CreateZone: %v", err)
	}

	zones, err := db.GetZonesForPage(pageID)
	if err != nil {
		t.Fatalf("GetZonesForPage: %v", err)
	}
	if len(zones) != 1 || zones[0].ID != "cpu" || zones[0].WidthPx != 320 {
		t.Errorf("GetZonesForPage = %v", zones)
	}
}

func TestZones_Update(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	db.CreateZone(makeZone("z1", pageID, 0, 320)) //nolint:errcheck

	updated := makeZone("z1", pageID, 0, 480)
	updated.Plugin = "exec:./plugins/cpu-load/cpu-load"
	if err := db.UpdateZone(updated); err != nil {
		t.Fatalf("UpdateZone: %v", err)
	}

	zones, _ := db.GetZonesForPage(pageID)
	if zones[0].WidthPx != 480 || zones[0].Plugin != "exec:./plugins/cpu-load/cpu-load" {
		t.Errorf("after update: %+v", zones[0])
	}
}

func TestZones_UpdateMissing(t *testing.T) {
	db := openTestDB(t)
	pageID, _ := db.CreatePage("P", 0)

	err := db.UpdateZone(makeZone("ghost", pageID, 0, 320))
	if err == nil {
		t.Error("expected error updating non-existent zone")
	}
}

func TestZones_Delete(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	db.CreateZone(makeZone("z1", pageID, 0, 320)) //nolint:errcheck

	if err := db.DeleteZone("z1"); err != nil {
		t.Fatalf("DeleteZone: %v", err)
	}

	zones, _ := db.GetZonesForPage(pageID)
	if len(zones) != 0 {
		t.Errorf("expected 0 zones after delete, got %d", len(zones))
	}
}

func TestZones_CascadeDeleteWithPage(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	db.CreateZone(makeZone("z1", pageID, 0, 320)) //nolint:errcheck
	db.CreateZone(makeZone("z2", pageID, 1, 320)) //nolint:errcheck

	db.DeletePage(pageID) //nolint:errcheck

	zones, err := db.GetZonesForPage(pageID)
	if err != nil {
		t.Fatalf("GetZonesForPage after page delete: %v", err)
	}
	if len(zones) != 0 {
		t.Errorf("expected zones to cascade-delete with page, got %d", len(zones))
	}
}

func TestZones_Reorder(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	db.CreateZone(makeZone("a", pageID, 0, 320)) //nolint:errcheck
	db.CreateZone(makeZone("b", pageID, 1, 320)) //nolint:errcheck

	if err := db.ReorderZones(pageID, []string{"b", "a"}); err != nil {
		t.Fatalf("ReorderZones: %v", err)
	}

	zones, _ := db.GetZonesForPage(pageID)
	if zones[0].ID != "b" || zones[1].ID != "a" {
		t.Errorf("expected b then a, got %v", zones)
	}
}

func TestZones_ConfigSyncedToZonePluginConfig(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	z := makeZone("z1", pageID, 0, 320)
	z.ConfigJSON = map[string]interface{}{"unit": "fahrenheit"}

	db.CreateZone(z) //nolint:errcheck

	// CreateZone should have synced ConfigJSON → zone_plugin_config.
	cfg, err := db.GetZoneConfig("z1")
	if err != nil || cfg == nil || cfg["unit"] != "fahrenheit" {
		t.Errorf("zone_plugin_config not synced: cfg=%v err=%v", cfg, err)
	}
}

// ── HasLayout / GetFullLayout ─────────────────────────────────────────────────

func TestHasLayout_Empty(t *testing.T) {
	db := openTestDB(t)
	has, err := db.HasLayout()
	if err != nil || has {
		t.Errorf("expected HasLayout=false on empty DB, got %v err=%v", has, err)
	}
}

func TestHasLayout_WithPage(t *testing.T) {
	db := openTestDB(t)
	db.CreatePage("P", 0) //nolint:errcheck
	has, err := db.HasLayout()
	if err != nil || !has {
		t.Errorf("expected HasLayout=true after CreatePage, got %v err=%v", has, err)
	}
}

func TestGetFullLayout(t *testing.T) {
	db := openTestDB(t)

	pageID, _ := db.CreatePage("P", 0)
	db.CreateZone(makeZone("z1", pageID, 0, 320)) //nolint:errcheck
	db.CreateZone(makeZone("z2", pageID, 1, 320)) //nolint:errcheck

	pages, zoneMap, err := db.GetFullLayout()
	if err != nil {
		t.Fatalf("GetFullLayout: %v", err)
	}
	if len(pages) != 1 {
		t.Fatalf("expected 1 page, got %d", len(pages))
	}
	if len(zoneMap[pageID]) != 2 {
		t.Errorf("expected 2 zones, got %d", len(zoneMap[pageID]))
	}
}

// ── ImportLayout ──────────────────────────────────────────────────────────────

func TestImportLayout_ReplacesExisting(t *testing.T) {
	db := openTestDB(t)

	// Seed initial layout.
	db.CreatePage("Old", 0) //nolint:errcheck

	// Import a fresh layout.
	pages := []StoredPage{{ID: 1, Name: "New", Ord: 0}}
	zoneMap := map[int64][]StoredZone{
		1: {makeZone("z1", 1, 0, 640)},
	}

	if err := db.ImportLayout(pages, zoneMap); err != nil {
		t.Fatalf("ImportLayout: %v", err)
	}

	result, _, _ := db.GetFullLayout()
	if len(result) != 1 || result[0].Name != "New" {
		t.Errorf("expected imported layout, got %v", result)
	}
}

// ── ValidateZoneWidths ────────────────────────────────────────────────────────

func TestValidateZoneWidths_Valid(t *testing.T) {
	zones := []StoredZone{
		{WidthPx: 320},
		{WidthPx: 320},
	}
	if err := ValidateZoneWidths(zones); err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestValidateZoneWidths_Invalid(t *testing.T) {
	zones := []StoredZone{
		{WidthPx: 320},
		{WidthPx: 160},
	}
	if err := ValidateZoneWidths(zones); err == nil {
		t.Error("expected error for zones summing to 480px")
	}
}
