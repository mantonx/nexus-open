# Nexus Open v2 — Implementation Plan

> Grounded in the actual codebase as of 2026-06-07. Every claim below was verified
> by reading the source. Corrections to the AI-generated plan precede each phase
> where relevant.

---

## What the AI plan got wrong (corrections)

These are places where the source contradicts the plan. Reading the plan without
this section leads to duplicated or misguided work.

**B4 (crash recovery) is already done.** `internal/zone/sampler.go:248-299`
implements `handlePluginCrash` with exponential backoff (1s → 2s → … cap 30s),
`Evict` + relaunch, and stale-payload display during restart. Do not re-implement
this. The plan incorrectly lists it as not yet built.

**Layout CRUD endpoints already exist.** `internal/api/layout_handlers.go` has
`GET /api/layout`, `POST /api/layout/pages`, `GET/PUT/DELETE` for individual
pages, and zone CRUD. Phase 2's "zone CRUD endpoints" requirement is partially
satisfied. What's missing: `RedistributeWidths`, the 6-zone cap, and proper
round-tripping of `PluginConfig` from the struct through the DB.

**The Flutter UI has 6 tabs, not 7.** `settings_page.dart:35-43` defines exactly
6 destinations: Preview, Layout, Display, Location, Plugins, Images. The plan says
"seven tabs" — this is wrong.

**`MagicCookieValue` says `"nexus-open-v2"` but `ProtocolVersion` is `1`.**
`pkg/plugin/plugin.go:21`. The cookie value is misleading. Bumping
`ProtocolVersion` to `2` is the right signal for the new `Configure` contract.

**`ZoneConfig` in memory has no `PluginConfig` field.** The `zones` DB table has
`config_json` (read into `StoredZone.ConfigJSON`), and `zone_plugin_config` has a
parallel per-zone record. Neither is loaded into the `ZoneConfig` struct
(`internal/zone/config.go:37-47`) that the running manager actually uses. This is
Gap #2 from the plan and the first concrete thing to fix.

**`PluginConfigNotifier.OnConfigChanged` is already the delivery mechanism.** The
sampler calls it at start-up and on zone-config changes. The new `Configure`
method in the plan is effectively this interface renamed. Keep the name
`Configure` for v2 clarity, but note the existing call sites in `sampler.go:157-170`
and `sampler.go:511-538` are the exact hook points.

---

## Goal

Make app configuration coherent and intentional. Users build pages of up to 6
zones, assign plugins to zones, and configure each plugin in place. Changes
preview live on the device. An explicit Confirm step writes to the store. A
single source of truth (SQLite) replaces the current three overlapping surfaces.

## Non-goals (v2)

- An open plugin marketplace or hosted binary index.
- 2D zone layouts; zones stay horizontal partitions of the 640×48 strip.
- Resizable drag handles directly on the device.
- Remote/multi-device sync.

---

## Current state (accurate)

| Concern | File(s) | State |
|---|---|---|
| Plugin contract | `pkg/plugin/types.go`, `plugin.go` | `Describe()` + `Sample()`. `PluginConfigNotifier.OnConfigChanged` is optional, global-scoped. No schema declaration. `ProtocolVersion: 1`. |
| Zone model | `internal/zone/config.go` | `ZoneConfig` has `ThemeOverride *Theme` but **no `PluginConfig` field**. No zone-count cap. |
| Config delivery | `internal/zone/sampler.go` | `applyInitialZoneConfig` + `BroadcastZoneConfigChange` deliver config via `OnConfigChanged` at launch and on API edits. Works today. |
| Crash recovery | `internal/zone/sampler.go:248-299` | **Already implemented.** Exponential backoff restart with stale-payload cue. |
| Persistence | `internal/store/store.go`, `layout.go` | SQLite is the store. `zone_plugin_config` table holds per-zone JSON. `zones.config_json` is a second (overlapping) per-zone store. Need to consolidate. |
| Global config | `internal/settings/config.go` | `BackgroundColor`, `BackgroundImage`, `TextColor`, `Display` (fonts, layout, date format). Already narrowed vs. original; `location`/`unit`/`time_format` have been removed. |
| API | `internal/api/` | REST on `:1985`. Layout CRUD exists. `GET/POST /api/plugins/:name/config` exists (plugin-level defaults, not schema). `GET /api/zones/:id/status` exists. |
| Flutter UI | `ui/lib/src/widgets/settings/` | 6-tab `NavigationRail`. Live 640×48 frame from WS already works. Typed models via `freezed`. |

### Actual gaps (not the plan's list of five; corrected here)

1. **No `PluginConfig` on `ZoneConfig` struct.** The DB stores it; the in-memory
   model doesn't carry it; plugins receive it only if `OnConfigChanged` is called
   separately. YAML export/import does not round-trip it.

2. **No plugin config schema declaration.** Plugins can't describe their options.
   The UI can't build a form. Config is set blind via raw JSON.

3. **`zone_plugin_config` and `zones.config_json` are two overlapping per-zone
   config stores.** The `ConfigManager` uses `zone_plugin_config`; the layout
   API's `GET /api/layout` reads `zones.config_json`. They can diverge.

4. **No zone-count cap, no auto width redistribution.** Adding/removing zones
   requires manual pixel arithmetic. Min-80px floor is validated but not enforced
   at the add level.

5. **No draft/preview/commit separation.** Saving applies and persists in one
   step. No undo or live-preview without commitment.

---

## Architecture decisions (locked)

**Single config store.** `zones.config_json` is the canonical per-zone plugin
config location. `zone_plugin_config` is removed (migration adds a DB migration
that copies any distinct rows into `zones.config_json` then drops the table).
`ConfigManager` is updated to read/write `zones.config_json` via new store
methods.

**`PluginConfig` on `ZoneConfig`.** Add `PluginConfig map[string]any` to
`ZoneConfig` (yaml: `plugin_config`, json: `plugin_config`). The YAML import/export
and DB round-trip carry it. This closes gap #1.

**Plugin schema via `Descriptor`.** Add `ConfigSchema` to `Descriptor` and a
`Configure(cfg map[string]any) error` method to the `Plugin` interface.
`OnConfigChanged` becomes a deprecated alias in the RPC layer for one release.
Bump `ProtocolVersion` to `2`; reject v1 binaries at handshake. Since no external
community plugins exist yet, there is no backward-compat obligation.

**`MaxZonesPerPage = 6`.** Constant in `internal/zone/config.go`. `Page.Validate`
enforces it. A `RedistributeWidths()` helper auto-splits on add/remove.

**Draft layout in memory.** Host holds a draft `*zone.Config` separate from the
committed config in the store. Mutations over the draft API update it and
re-render live. `POST /api/layout/commit` flushes draft → store. `POST
/api/layout/discard` reverts. Client disconnect and 60s idle revert to committed.
Draft is not persisted across host restarts (simplest approach; sufficient for v2).

**Flutter: canvas + inspector model.** Replace the 6-tab rail with a 3-mode
rail (Editor, Global, Device). The editor surface is: persistent live preview
strip at top → page strip → zone canvas → inline inspector (per selected zone).
Draft/confirm bar is persistent while there are unsaved changes.

---

## Phased delivery

### Phase 0 — Config surface consolidation (no behavior change) ✅ COMPLETE (2026-06-07, commit 8379b8c)

**Goal:** close the two-store divergence, add `PluginConfig` to the in-memory
model, and make YAML round-trip complete.

**Changes:**

1. **Store migration (v4 → v5):** copy rows from `zone_plugin_config` into
   `zones.config_json` where `zone_id` matches a known zone ID; rows with a
   `plugin:` prefix (plugin-level defaults stored by `SetPluginDefault`) are
   kept in a new `plugin_defaults` table. Drop `zone_plugin_config`.

2. **`ZoneConfig` struct:** add `PluginConfig map[string]any` with tags
   `yaml:"plugin_config,omitempty" json:"plugin_config,omitempty"`.

3. **`store.StoredZone` ↔ `zone.ZoneConfig` round-trip:** update
   `internal/store/layout.go` `GetFullLayout` / zone write helpers to
   read/write `ConfigJSON` ↔ `ZoneConfig.PluginConfig`.

4. **YAML import/export:** `internal/zone/yaml_import.go` and `db_import.go`
   already handle `ZoneConfig` fields; adding `PluginConfig` to the struct
   is sufficient — verify with an import/export test.

5. **`ConfigManager` update:** remove the `zone_plugin_config` read path;
   `Get(zoneID)` now reads `zones.config_json` via a new
   `store.DB.GetZonePluginConfig(zoneID)` method that queries the `zones`
   table directly.

6. **Docs:** add `docs/TERMINOLOGY.md` section "Config surface hierarchy"
   documenting: global (`settings` table) → per-zone (`zones.config_json`) →
   per-plugin defaults (`plugin_defaults` table). No `config.yaml` in v2.

**Acceptance tests:**
- YAML → DB → YAML round-trip preserves `plugin_config` for all
  `configs/layouts/*.yaml` example layouts.
- Existing layouts load unchanged after migration.
- `internal/store` and `internal/zone` tests cover the new round-trip.
- `zone_plugin_config` table no longer exists post-migration.

---

### Phase 1 — Plugin config schema contract
**Goal:** plugins declare their configurable fields; the host exposes a catalog;
Flutter can render a form from the schema.

**Changes:**

1. **`pkg/plugin/types.go`:** add `ConfigField`, `ConfigSchema`, update
   `Descriptor` with `ConfigSchema ConfigSchema`:

   ```go
   type FieldType string
   const (
       FieldTypeString FieldType = "string"
       FieldTypeEnum   FieldType = "enum"
       FieldTypeInt    FieldType = "int"
       FieldTypeBool   FieldType = "bool"
       FieldTypeColor  FieldType = "color"
   )

   type FieldOption struct {
       Value string `json:"value"`
       Label string `json:"label"`
   }

   type ConfigField struct {
       Key     string        `json:"key"`
       Label   string        `json:"label"`
       Type    FieldType     `json:"type"`
       Default any           `json:"default,omitempty"`
       Options []FieldOption `json:"options,omitempty"`
       Min     *int          `json:"min,omitempty"`
       Max     *int          `json:"max,omitempty"`
       Help    string        `json:"help,omitempty"`
   }

   type ConfigSchema struct {
       Fields []ConfigField `json:"fields"`
   }
   ```

2. **`Plugin` interface:** add `Configure(cfg map[string]any) error`. Remove
   `PluginConfigNotifier` from the public interface — the RPC layer keeps an
   internal shim that calls `Configure` when `OnConfigChanged` arrives from a
   v1 binary, but externally `Configure` is the contract.

3. **`pkg/plugin/plugin.go`:** bump `ProtocolVersion` to `2`. Add
   `Configure` RPC method to `rpcClient` and `pluginRPC`. Add `ConfigSchema`
   to `Descriptor` gob registration. Reject v1 handshakes with a clear error
   message.

4. **Builtin plugins** (`internal/plugins/builtin/clock.go`): implement
   `Configure` (clock has no config; return nil). Add `ConfigSchema` to its
   `Describe()` response (empty schema).

5. **Exec plugins** — implement schemas and `Configure` for:
   - `plugins/weather/`: fields `location` (string), `unit` (enum: imperial/metric).
   - `plugins/cpu-temp/`: fields `unit` (enum), `graph_type` (enum).
   - `plugins/gpu-temp/`: same as cpu-temp.
   - `plugins/cpu-load/`: fields `graph_type` (enum).
   - `plugins/gpu-load/`: fields `graph_type` (enum).
   - `plugins/network/`: fields `interface` (string, default "auto"),
     `graph_type` (enum).
   In each plugin, `Configure` sets the same fields currently set by
   `OnConfigChanged`; the method replaces that code path directly.

6. **`internal/zone/sampler.go`:** replace `applyInitialZoneConfig` /
   `BroadcastZoneConfigChange` calls to `OnConfigChanged` with calls to
   `Configure`. The call sites are `sampler.go:157-170` and `sampler.go:511-538`.

7. **`GET /api/plugins` catalog endpoint** (`internal/api/handlers.go` or a
   new `plugin_handlers.go`): returns all registered builtin + discovered exec
   plugins as `[]{id, kind, descriptor, config_schema, status}`. Status is
   `"ok"` | `"error"` | `"loading"` derived from sampler zone statuses.

8. **Update `api/openapi.yaml`** with the new `ConfigField`, `ConfigSchema`,
   and catalog endpoint. Run `scripts/generate-flutter-api.sh`.

**Acceptance tests:**
- `GET /api/plugins` returns schema for all 6 exec plugins and `builtin:clock`.
- Editing a zone's `plugin_config` via `PUT /api/layout/draft/zones/:id` reaches
  the running plugin and changes the next `Sample()`.
- Unit tests for `ConfigField` JSON marshaling and `Configure` round-trip.

---

### Phase 2 — Zone model v2
**Goal:** enforce the 6-zone cap, auto-redistribute widths, and expose clean
zone CRUD that keeps the 640px invariant without manual arithmetic.

**Changes:**

1. **`internal/zone/config.go`:** add `const MaxZonesPerPage = 6` and enforce
   `len(page.Zones) <= MaxZonesPerPage` in `Page.Validate()`.

2. **`RedistributeWidths(totalPx, floorPx int)`** method on `Page`: equal split
   by default, respecting the 80px floor. Returns an error if
   `len(Zones) * floorPx > totalPx`.

3. **`internal/api/layout_handlers.go`:** update the existing zone-add and
   zone-delete handlers to call `RedistributeWidths` after mutation. Return a
   typed error `{"error":"ZoneCapExceeded","message":"..."}` when the 7th zone
   is requested.

   > Note: the zone add/delete endpoints already exist in `layout_handlers.go`.
   > This phase updates their behavior, not their routing.

4. **Draft-aware zone CRUD (prep for Phase 3):** the existing handlers write
   directly to the store; in this phase, add a `--dry-run` test mode that
   verifies redistribution logic without committing. The full draft model
   ships in Phase 3.

**Acceptance tests:**
- Add/remove a zone; `GET /api/layout` returns redistributed widths summing to
  640 with all zones ≥ 80px.
- Adding a 7th zone returns HTTP 422 with `ZoneCapExceeded`.
- `internal/zone` tests cover redistribution edge cases: 6-zone floor collision,
  unequal starting widths.

---

### Phase 3 — Live draft + confirm
**Goal:** mutations update a draft (rendered live on device) without touching
the committed store until the user explicitly confirms.

**Changes:**

1. **`internal/zone/manager.go` / `app.go`:** add a `draft *zone.Config` field
   to the zone manager (or a new `DraftManager` wrapper). Draft starts as a
   deep copy of the committed config at session open.

2. **Draft API surface** in `internal/api/layout_handlers.go`:
   - `GET /api/layout/draft` — returns current draft.
   - `PUT /api/layout/draft` — replaces draft (full config body).
   - `POST /api/layout/draft/zones` — add zone to draft (calls
     `RedistributeWidths`).
   - `DELETE /api/layout/draft/zones/:id` — remove zone from draft.
   - `PATCH /api/layout/draft/zones/:id` — update zone fields in draft.
   - `POST /api/layout/commit` — write draft → store, broadcast
     `committed_state` WS message.
   - `POST /api/layout/discard` — revert draft to committed state.

3. **Draft rendering:** after any draft mutation, call
   `sampler.RestartForPage` (already exists) with the draft config so the
   device shows the draft immediately. This reuses the existing render pipeline
   unchanged.

4. **Auto-revert:** idle timeout (default 60s, configurable via a new
   `draft_idle_timeout_s` key in `settings`) and client disconnect (WS close)
   trigger discard. The timeout resets on any draft mutation.

5. **WS message:** add `draft_state` WS event (same shape as `page_state`)
   broadcast on every draft mutation so all connected clients stay in sync.

6. **Update `api/openapi.yaml`** with draft endpoints. Regenerate Dart client.

**Acceptance tests:**
- Mutating the draft via API renders on the device without writing the store.
- `POST /api/layout/commit` persists; config survives host restart.
- Client disconnect reverts within 1s (test with mock WS close).
- Idle timeout reverts after configured duration.

---

### Phase 4 — Flutter UI consolidation
**Goal:** replace the 6-tab settings page with a preview-first, canvas+inspector
editor wired to the new API.

**Surface layout:**

```
┌──┬──────────────────────────────────────────────────┐
│  │  ████   live 640×48 preview strip  (amber)  ████  │
│R │  Main · Work · + page              ● live         │
│a ├──────────┬──────────────────────────┬─────────────┤
│i │ Library  │  Page canvas             │  Inspector  │
│l │ (plugins)│  [Weather][CPU*][GPU] +  │  selected   │
│  │          │  zones sized by width    │  zone only  │
│  ├──────────┴──────────────────────────┴─────────────┤
│  │  ● Unsaved changes on device    [Discard][Confirm] │
└──┴──────────────────────────────────────────────────┘
Rail: Editor | Global | Device
```

**Changes:**

1. **3-mode `NavigationRail`:** Editor (current canvas), Global (background
   color, background image, device defaults — replaces Display + Images tabs),
   Device (device health, firmware, brightness — replaces the hardware
   preview's device section).

2. **Persistent preview strip** (`_DisplayStrip` already exists at
   `settings_page.dart:479`): move it above the rail content and keep it
   always visible. Wrap it in an amber border (`AppColors.hardwareAccent`).

3. **Page strip:** horizontal scrollable list of page thumbnail chips. Add/remove/
   reorder pages. Selecting a page loads its zones into the canvas below.

4. **Zone canvas:** horizontal row of zone chips, proportional widths, matching
   the device aspect. Drag to reorder (use `ReorderableListView` or
   `flutter_reorderable_grid_view`). Tap to select (highlights in both canvas
   and preview strip via an overlay outline). Add-zone button opens the plugin
   library; tapping a zone's × calls `DELETE /api/layout/draft/zones/:id`.

5. **Inspector panel** (right column on wide; bottom sheet on narrow): shows
   config form for the selected zone, rendered from `GET /api/plugins` schema.
   Use `flutter_form_builder` or a minimal hand-rolled schema renderer. Fields:
   schema-declared plugin config + `theme_override.accent` color + width slider
   (80–640, constrained by neighbors) + alignment + refresh interval.

6. **Plugin library** (left column or drawer): flat list of plugins from
   `GET /api/plugins`. Each entry shows icon, name, description. Tap or drag
   into the canvas to add to a zone.

7. **Draft/confirm bar:** persistent `AnimatedContainer` at the bottom, visible
   when `_hasDraftChanges` is true. "● Unsaved changes — live on device"
   label + **Discard** (calls `POST /api/layout/discard`) + **Confirm** (calls
   `POST /api/layout/commit`). This replaces the existing `_hasUnsavedChanges`
   check in `settings_page.dart:67-71`.

8. **Global mode:** background color picker, background image selector
   (reuse `images_tab.dart` logic), and device defaults (time format, units).
   These write directly to `POST /api/config` — no draft needed.

9. **Device mode:** health view (per-zone status from `GET /api/zones/:id/status`),
   firmware, brightness slider, connection status. Fold `hardware_preview_tab`
   here.

10. **Fold old tabs:** Preview → persistent strip. Layout → canvas. Display →
    Global. Location → Global (weather zone now configures location per-zone via
    the inspector). Plugins → library. Images → Global.

11. **Responsive breakpoint:** `< 900px` collapses inspector to a bottom sheet,
    library to a drawer. `>= 900px` shows three-column layout.

12. **Retarget `onboarding_overlay.dart`** at the new flow: Welcome → Plugin
    library → Add first zone → Confirm.

**Acceptance tests:**
- Widget test: add a page, drag a plugin into a zone, edit the zone config,
  confirm; verify WS draft_state messages were sent.
- Widget test: discard reverts canvas to pre-edit state.
- `ui/integration_test` covers the full add-configure-commit flow.
- Narrow viewport renders inspector as bottom sheet (breakpoint test).

---

### Phase 5 — Catalog polish, migration, docs
**Goal:** complete schema coverage, finish terminology, update docs.

**Changes:**

1. Schemas for any remaining plugins not covered in Phase 1.

2. `docs/TERMINOLOGY.md`: finish the Instrument → Plugin cleanup table; add the
   "Config surface hierarchy" section from Phase 0.

3. `README.md`: fix quick-start (still references `nexus-next` repo name). Add
   a GIF or screenshot at the top.

4. `DEVICE_SETUP.md`: verify all steps still apply post-v2.

5. Coverage check: maintain or exceed the current ~65% line coverage in
   `internal/store`, `internal/zone`, `internal/api`.

**Acceptance:** every shipped plugin has a non-empty `ConfigSchema`. Docs match
the v2 flow. CI passes without skipped tests.

---

## Part B — Usefulness and polish

Items are independent of Part A unless noted. Recommended interleaving is at the
end of this section.

### B1 — Scriptable / HTTP generic plugin
**Slots after Phase 1.** A builtin plugin (`builtin:http` and `builtin:command`)
that fetches a URL or runs a shell command and maps the result to a `Payload` via
a declared schema. This makes most bespoke data sources into config rather than
code. Shell command execution is intentionally gated to local UI config only —
never accepted over the network API.

### B2 — Threshold-driven attention
**Slots after Phase 1.** Add `warn_above`/`crit_above` schema fields to metric
plugins; plugins set `Payload.Severity` accordingly. Add an attention policy
per zone (`on_crit: none|flash|switch_page`). Default: `flash`. `switch_page`
debounced at 10s to prevent thrashing on flapping values.

### B3 — Plugins beyond system metrics (priority order)
**Slots after Phase 1.**
1. **Media / MPRIS** (roadmapped) — D-Bus `org.mpris.MediaPlayer2` via
   `github.com/godbus/dbus/v5`. Tap toggles play/pause.
2. **Notifications passthrough** — `org.freedesktop.Notifications` ambient
   ticker zone.
3. **Timer / Pomodoro** — stateful builtin, tap to start/stop.
4. **Calendar next-event** and **CI status** — as B1 HTTP config examples.
5. **Docker/homelab status** — Docker socket.

### B4 — Plugin crash recovery *(already implemented)*
`internal/zone/sampler.go:248-299`. No action needed. The stale-payload dim cue
mentioned in the original plan is the only unimplemented sub-item; the zone shows
"Timeout / Restarting…" text rather than a dimmed last-good value. That visual
polish can be done in Phase 4 without a separate phase.

### B5 — Automation control surface
**Slots after Phase 3 (for layout API stability).** CLI subcommands on the
`nexus-open` binary as thin REST client against `:1985`. MQTT / Home Assistant
is deferred — large surface, evaluate after CLI is in place.

### B6 — `nexus-open doctor`
**Independent; can land any time.** Checks: USB presence (VID `0x1b1c`, PID
`0x1b8e`), `plugdev` group membership, port `1985` availability, plugin binary
existence and executability, udev rule installation. Prints exact fix for each
failure. Extend `GET /api/health` with per-plugin last-sample time, error count,
and host FPS; surface in Device mode (Phase 4).

### Adoption (high ROI, no feature work)
- **Cut a tagged v1.0 release.** `release.yml` is tag-triggered but no tags have
  been pushed. `git tag v1.0.0 && git push --tags` unblocks the CI release flow.
- **Fix README quick-start.** The `cd nexus-next` line and the Support link
  point at the old repo name.
- **Add a GIF or screenshot** of the strip in motion to the top of README.
  Worth more for discoverability than the next several features.

### Recommended interleaving
1. Land Phase 0 + Phase 1.
2. B6 (`doctor`) and adoption items at any point.
3. Phase 2 + Phase 3.
4. B1 (scriptable), B2 (thresholds), B3 (MPRIS first) after Phase 1.
5. Phase 4 (Flutter consolidation) after Phase 3.
6. B5 (CLI/MQTT) after Phase 3.
7. Phase 5 (polish + docs) last.

---

## Plugin `plugin.yaml` manifest (future: L1 local library)

Each plugin directory gains a static manifest so the catalog doesn't need to
launch a binary to describe it. This is scaffolding for the install system;
the catalog falls back to launching `Describe()` if the manifest is absent.

```yaml
# plugins/weather/plugin.yaml
id: weather
name: Weather
version: 1.0.0
author: Nexus Team
description: Current conditions via Open-Meteo
icon: cloud
kind: exec
entry: ./weather
recommended_refresh_ms: 300000
min_protocol: 2
permissions: [network]
config_schema:
  - { key: location, label: Location, type: string, default: "Jersey City, NJ" }
  - { key: unit, label: Units, type: enum, default: imperial,
      options: [{value: imperial, label: "°F"}, {value: metric, label: "°C"}] }
```

The Phase 1 catalog endpoint reads `plugin.yaml` if present, falls back to
`Describe()` otherwise.

---

## Open decisions

1. **Install scope this cycle.** L2 (recipe/YAML import) only, or also L3
   (signed binary index)? L2 is safe and cheap. L3 adds meaningful security
   work. Recommendation: L2 only in v2; L3 is a v2.1 item.

2. **B1 command execution trust boundary.** Confirm: configured commands run
   with user privileges and are only accepted through the local UI or config
   file, never via the network API. This is the current design; confirm it
   before B1 lands.

3. **B5 scope.** CLI only (cheap, slots after Phase 3), or also MQTT? MQTT is
   a larger integration surface. Recommendation: CLI first, MQTT as a v2.1
   item after the CLI validates the pattern.

4. **B2 default `on_crit` policy.** `flash` (subtle) vs. `switch_page`
   (assertive). Recommendation: `flash` by default, `switch_page` opt-in per
   zone. This avoids surprising users whose disk-usage plugin briefly spikes.

---

*Last updated: 2026-06-07. Update this file when decisions are locked.*
