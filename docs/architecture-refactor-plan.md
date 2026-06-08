# Nexus Open — Architecture Refactor Plan

## Background

Several bugs found during touch investigation revealed deeper structural problems: the DB/YAML
seeding logic fires on the wrong trigger, migration functions are out of order in the source
file, domain boundaries are violated in three places, and the forecast cache lives only in
process memory. This document captures each problem, its root cause, and the planned fix.
Implement phases in order — each one unblocks or simplifies the next.

---

## Phase 1 — Fix DB/YAML Seeding ✓ Done

### Problem

`LoadConfigFromDB` returns an error when the pages table is empty. `NewManager` catches that
error and re-seeds from the YAML file. This is the wrong trigger for two reasons:

1. An empty pages table can happen for reasons other than first run (deliberate wipe, failed
   migration, partial import). Seeding silently on any such error overwrites whatever state the
   user intended.
2. `IsFirstRun()` in `store.go` is supposed to detect genuine first-run but it always returns
   false. `Open()` calls `migrate()` before returning, which bumps `schema_version` to 6. By the
   time any caller checks `IsFirstRun()`, the sentinel is already set. The settings YAML import
   path (`internal/app/app.go`) therefore never runs.

### Fix

**`internal/store/store.go`**

Add a single boolean query that runs *before* `migrate()` and reports whether this is a fresh
database (i.e., `schema_version` did not exist yet):

```go
// HasLayout reports whether the pages table contains at least one page row.
// Use this — not IsFirstRun — to decide whether to seed from YAML.
func (s *Store) HasLayout() (bool, error) {
    var count int
    err := s.db.QueryRow(`SELECT COUNT(*) FROM pages`).Scan(&count)
    if err != nil {
        // Table may not exist yet on a brand-new DB; treat as no layout.
        return false, nil
    }
    return count > 0, nil
}
```

Fix `IsFirstRun()` by capturing the pre-migration state at `Open()` time:

```go
type Store struct {
    db          *sql.DB
    firstRun    bool   // set once in Open(), before migrate() runs
}

func Open(path string) (*Store, error) {
    // ... open DB ...
    var ver int
    _ = db.QueryRow(`SELECT version FROM schema_version LIMIT 1`).Scan(&ver)
    s := &Store{db: db, firstRun: ver == 0}
    if err := s.migrate(); err != nil { ... }
    return s, nil
}

func (s *Store) IsFirstRun() bool { return s.firstRun }
```

**`internal/zone/manager.go`** (`NewManager`)

Replace the error-based seeding trigger:

```go
// Before: seeds if LoadConfigFromDB returns any error.
// After:
hasLayout, _ := store.HasLayout()
if !hasLayout {
    if err := seedFromYAML(store, layoutPath); err != nil {
        return nil, fmt.Errorf("seed layout: %w", err)
    }
}
cfg, err := store.LoadConfigFromDB()
if err != nil {
    return nil, fmt.Errorf("load config: %w", err)
}
```

This makes YAML the definitive factory default and DB the runtime source of truth, with no
ambiguity about when seeding fires.

---

## Phase 2 — Clean Up Migration System ✓ Done

### Problem

Three structural problems coexist in `internal/store/store.go`:

1. **Out-of-order functions.** `migration6` is defined at line 327, `migration5` at line 337.
   This is a code-smell rather than a runtime bug today (they're called by index, not by file
   order), but it will cause a real bug the next time someone adds a migration and assumes
   "add it after the last one in the file."

2. **Non-descriptive names.** `migration1` through `migration6` convey no intent. A future
   developer (or future-you) can't tell what any migration does without reading its body.

3. **No idempotency guards.** Migrations use raw DDL (`CREATE TABLE`, `ALTER TABLE`) without
   `IF NOT EXISTS` / `IF NOT EXISTS column`. If a migration partially succeeds and the process
   crashes, re-running will fail on the already-created object.

### Fix

Rename migrations to convey execution order *and* intent. Keep them in strict numeric order in
the file. Add `IF NOT EXISTS` guards where practical.

Naming scheme: `migrateV<N>_<short_description>`

| Old name    | New name                                   |
|-------------|--------------------------------------------|
| migration1  | migrateV1_settingsAndZonePluginConfig      |
| migration2  | migrateV2_pagesAndZones                    |
| migration3  | migrateV3_renameTable                      |
| migration4  | migrateV4_rewriteExecPaths                 |
| migration5  | migrateV5_consolidateConfigAndRenameColumn |
| migration6  | migrateV6_addOnTapAndChoicesJson           |

The migration runner slice becomes:

```go
var migrations = []func(*sql.Tx) error{
    migrateV1_settingsAndZonePluginConfig,
    migrateV2_pagesAndZones,
    migrateV3_renameTable,
    migrateV4_rewriteExecPaths,
    migrateV5_consolidateConfigAndRenameColumn,
    migrateV6_addOnTapAndChoicesJson,
}
```

This is the canonical order; the file order must match the slice order. Adding a new migration
means: append to the slice, write the function immediately below the previous one, increment
`currentSchemaVersion`.

---

## Phase 3 — Domain Boundary: Touch Handler Decoupling ✓ Done

### Problem

`internal/touch/handler.go` reaches directly into the plugin layer via `DetailProvider`:

```go
type DetailProvider interface {
    GetPlugin(zoneID string) (plugin.Plugin, bool)
}
```

The handler then type-asserts `plugin.Tapper` itself. This means the touch package knows about
plugin mechanics. If the plugin interface changes, the touch handler must change too. The touch
handler's job is to classify gestures and route them — not to know how to fetch forecast data.

### Fix

Add a single method to `zone.Manager` that encapsulates the full tap-→-detail flow:

```go
// HandleZoneTap looks up the plugin for zoneID, calls OnTap if it implements
// Tapper, and shows the result as a detail overlay. It is safe to call from
// any goroutine.
func (m *Manager) HandleZoneTap(zoneID string) error
```

The touch handler's `executeTapAction` becomes:

```go
case zone.TapActionDetail:
    if h.detailInFlight.CompareAndSwap(false, true) {
        go func() {
            defer h.detailInFlight.Store(false)
            if err := h.zoneManager.HandleZoneTap(z.ID); err != nil {
                h.logger.Warn("zone tap failed", "zone", z.ID, "error", err)
            }
        }()
    }
```

Remove `DetailProvider` interface, `SetDetailProvider`, and `handleDetailTap` from
`internal/touch/handler.go` entirely. Move that logic into `Manager.HandleZoneTap` in
`internal/zone/manager.go`. The touch package import list loses `pkg/plugin`.

---

## Phase 4 — Domain Boundary: Zone Dimensions Contract ✓ Done

### Problem

The weather plugin (and presumably others) uses config keys `_zone_width` and `_zone_height`
to read its allocated display area at runtime. These keys are injected by the sampler but are
not declared anywhere in `pkg/plugin`. A plugin author has no way to know these keys exist
without reading the sampler source code. This is an implicit contract.

### Fix

Declare zone dimensions as a first-class concept in `pkg/plugin/types.go`:

```go
// ZoneDimensions is injected into every plugin's Configure call via the
// standard keys below. Plugins may read these to adapt their rendering.
const (
    ConfigKeyZoneWidth  = "_zone_width"
    ConfigKeyZoneHeight = "_zone_height"
)
```

Alternatively, add a `ZoneDimensions` struct to `Plugin.Configure`:

```go
// Option A — explicit struct (cleaner, but changes the Configure signature)
Configure(cfg map[string]string, dims ZoneDimensions) error

// Option B — declared constants (no signature change, easier migration)
// Use ConfigKeyZoneWidth / ConfigKeyZoneHeight in map[string]string
```

**Recommendation: Option B first.** Declare the constants; update the sampler to use them;
update plugin docs. A signature change can come later if needed.

---

## Phase 5 — Domain Boundary: CycleZonePlugin Persistence ✓ Done

### Problem

`CycleZonePlugin` in `internal/zone/manager_page.go` (previously `manager.go`) mutates the
in-memory `Config.Pages[].Zones[].Plugin` field and triggers a re-render, but never writes the
new plugin choice to the database. On restart, the zone reverts to whatever was last written to
the DB — meaning user-initiated cycles are lost across restarts.

### Fix

After updating the in-memory config, write the new plugin ID to the DB:

```go
func (m *Manager) CycleZonePlugin(zoneID string) error {
    // ... existing in-memory cycle logic ...

    // Persist the new plugin choice.
    if err := m.store.SetZonePlugin(zoneID, newPluginID); err != nil {
        return fmt.Errorf("persist zone cycle: %w", err)
    }
    return nil
}
```

Add to `internal/store/store.go`:

```go
// SetZonePlugin updates the plugin_id for a single zone row.
func (s *Store) SetZonePlugin(zoneID, pluginID string) error {
    _, err := s.db.Exec(
        `UPDATE zones SET plugin_id = ? WHERE id = ?`,
        pluginID, zoneID,
    )
    return err
}
```

---

## Phase 6 — Forecast Cache Persistence ✓ Done

### Problem

`WeatherPlugin.cachedForecast` lives in process memory. On restart it is gone. The first tap
after restart always hits the network — the user sees a spinner or stale data for however long
the HTTP round-trip takes (typically 1–3s on a good connection, much longer on a slow one).

The five-minute in-memory TTL is also invisible to operators: there is no way to see when the
cache was last populated or how stale it is.

### Fix

Add a `payload_cache` table to the database (via a new migration):

```sql
CREATE TABLE IF NOT EXISTS payload_cache (
    zone_id    TEXT PRIMARY KEY,
    payload    TEXT NOT NULL,       -- JSON-encoded DetailPayload
    plugin_id  TEXT NOT NULL,       -- which plugin produced it
    fetched_at INTEGER NOT NULL     -- Unix seconds
);
```

**Store contract additions** (`internal/store/store.go`):

```go
func (s *Store) SavePayloadCache(zoneID, pluginID string, payload []byte) error
func (s *Store) LoadPayloadCache(zoneID string) (payload []byte, fetchedAt time.Time, err error)
```

**Flow changes:**

1. On `HandleZoneTap` (Phase 3), after a successful `OnTap`:
   - Encode `DetailPayload` as JSON
   - Call `store.SavePayloadCache(zoneID, pluginID, encoded)`
2. On startup, in `Manager.HandleZoneTap`:
   - Check `store.LoadPayloadCache(zoneID)`
   - If cache is younger than TTL (e.g. 30 minutes for weather), show the cached payload
     immediately while issuing a background refresh
   - If cache is older than TTL, fetch fresh and update cache

**TTL policy:** The initial 5-minute in-memory TTL was too aggressive for a display that wakes
up from sleep. The DB-backed cache should use a longer TTL (30 minutes default) because the
point of DB persistence is to survive restarts, not to be a high-frequency refresh mechanism.
The WeatherPlugin can still enforce a separate shorter in-memory TTL for repeated taps during
the same session.

**Remove:** `WeatherPlugin.cachedForecast` and `WeatherPlugin.forecastLastUpdate` from
`plugins/weather/main.go` once the DB cache is in place. The plugin's `OnTap` becomes a pure
fetch with no caching concern — caching moves up to the host's `HandleZoneTap`.

---

## Execution Order Summary

| Phase | File(s) Changed | Prerequisite |
|-------|----------------|--------------|
| 1 — Seeding fix | `store.go`, `manager.go` | none |
| 2 — Migration cleanup | `store.go` | none (can run in parallel with Phase 1) |
| 3 — Touch decoupling | `handler.go`, `manager.go` | Phase 1 (Manager must be stable) |
| 4 — Zone dims contract | `pkg/plugin/types.go`, `sampler.go` | none |
| 5 — Cycle persistence | `manager_page.go`, `store.go` | Phase 1 |
| 6 — Forecast DB cache | `store.go`, `manager.go`, `weather/main.go` | Phase 1, Phase 3 |

Phases 1 and 2 can go in together as a single PR. Phase 4 is independent and can be its own
small PR. Phases 3, 5, 6 each touch Manager and should be sequential to keep diffs reviewable.

---

## What We Are Not Doing

- **YAML hot-reload.** The YAML is factory default only. Runtime config lives in the DB. We
  will not poll or watch the YAML file for changes after first seed.
- **Multi-file layout support.** One canonical layout file per install. The export path
  (`ExportConfigToYAML`) already handles backup.
- **Plugin-side caching.** After Phase 6, plugins are pure data sources. Cache is the host's
  responsibility. Plugin authors should not need to think about TTLs.
