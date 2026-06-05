# Nexus Open — Master Plan
_Last updated: 2026-06-05_

Previous plan documents archived to `docs/archive/`.

---

## Progress

### Step 7 — Zone/page layout editor ✅ 2026-06-05

**Go backend:**
- `internal/store` migration v2: `pages` + `zones` tables (640px constraint,
  cascade delete on page removal, ord-indexed)
- Full CRUD API: `GET /api/layout`, `POST /api/layout/pages`,
  `PUT/DELETE /api/layout/pages/:id`, `POST /api/layout/pages/reorder`,
  `POST /api/layout/zones`, `PUT/DELETE /api/layout/zones/:id`,
  `POST /api/layout/zones/reorder`
- `zone.Manager.ReloadFromConfig(*Config)` — replaces running layout live,
  invalidates page cache, re-initialises current page. No restart required.
- `triggerLayoutReload()` in API layer converts DB rows → `zone.Config` and
  calls `ReloadFromConfig` on every write so the hardware display updates
  immediately
- `internal/app/layout_import.go` seeds layout DB from YAML on first run
- Zone creation no longer validates 640px sum (intermediate state is valid);
  zone updates do validate to catch post-edit invalidity

**Flutter:**
- `LayoutPage` + `LayoutZone` model classes (plain Dart, not freezed — mutable
  for in-place editor state)
- `NexusApiService` gains full layout CRUD methods
- `LayoutEditorTab` widget:
  - Live hardware frame header (same mockup as preview tab, thinner)
  - Page tab bar — click to preview, long-press to rename, × to delete, + to add
  - Zone width proportional bar showing current layout at a glance
  - `ReorderableListView` of zone cards, each with:
    - Width slider (adjusts adjacent zone to maintain 640px invariant)
    - Module dropdown (builtin + exec modules)
    - Align dropdown (left/center/right)
    - Delete button
  - "Add zone" button + 640px validity indicator
- Wired as second nav rail item (index 1) between Preview and Display

**Known issue fixed:** Zone creation validation now skips 640px check on
creation (it only applies to updates). This prevented seeding a page with
multiple zones via the API.

### Step 6 — SQLite config backend ✅ 2026-06-05

New `internal/store` package: SQLite via `modernc.org/sqlite` (pure Go, no CGO).
Single database at `~/.config/nexus-open/nexus.db` replaces three config stores:

- `config.yaml` (UI settings via viper) → `settings` key/value table
- `zone-configs.yaml` (module configs) → `zone_module_config` table
- Module-default path-keyed configs → `zone_module_config` with `module:` prefix

**API surface unchanged** — `settings.Manager`, `zone.ConfigManager`, and all API
handlers have identical signatures. Tests updated in-place.

**On first run**: the store detects a fresh DB and imports existing YAML files
so upgrades are seamless. Viper is now used only in `yaml_import.go` (legacy
import path, not the hot path).

**Schema migrations**: versioned via `schema_version` table. Adding a new column
or table means adding a new `migrationN` function — no manual SQL needed.

All 7 test packages passing.

### Step 3 — manager.go split ✅ 2026-06-05

1,280-line `zone.Manager` split into four focused files, all in `package zone`
so the single `Manager` struct is shared with no API changes:

- `manager.go` (184 lines) — struct definition, `NewManager`, lifecycle
- `manager_page.go` (368 lines) — config/navigation/cache: `initializePage`,
  `mergeTheme`, `GetConfig/CurrentPage/NumPages/PageInfos/Zones`, `SwitchPage`,
  `NextPage/PrevPage`, `CycleZoneModule`, callbacks, pre-render cache
- `manager_render.go` (260 lines) — payloads/theme/compositing: `UpdatePayload`,
  `UpdateTheme`, `RenderFrame`, `GetLastFrame`, `IsTransitioning`,
  `renderImmediateFrameForCurrentPage`, `renderPageFrame`
- `manager_swipe.go` (453 lines) — gesture/transition: `UpdateLiveSwipe`,
  `FinalizeLiveSwipe`, `CancelLiveSwipe`

No callers changed (app.go, api/server.go, touch/handler.go all unmodified).
All tests passing. Bonus: `TestApp_ContextCancellation` had a 2s timeout that
was only marginally safe even on the original code; bumped to 5s.

### Step 2 — Remaining swipe backlog (items 3 & 4) ✅ 2026-06-05

Already implemented before this session. Direction reversal (`manager.go:~1006`)
and rubber-band resistance (`manager.go:~995`) were both done during the June
swipe tuning session. `docs/swipe-improvements.md` updated to reflect actual
status. All 4 high-impact items complete.

### Step 1 — Fix failing test + finish tap gesture ✅ 2026-06-05

**`TestHealthHandler` fixed** (`internal/api/handlers_test.go`): mock device was
starting disconnected so the health endpoint correctly returned `"degraded"`.
Fix: call `mockDev.Connect()` before the request. Added `context` import.

**Tap gesture implemented** end-to-end:
- `touch.Event` gains `TapX int` — display pixel X position (0–639), populated
  by the `HIDTouchReader` from the smoothed `xi` value at lift time
  (`internal/touch/event.go`, `internal/touch/reader.go`)
- `touch.Handler.handleTap(event Event)` now routes by X coordinate: walks the
  current page's zones (after `ComputeOffsets()`), finds the zone containing
  `TapX`, and dispatches its `OnTap` action (`internal/touch/handler.go`)
- `TapActionCycle` implemented: `zone.Manager.CycleZoneModule(zoneID)` advances
  the zone's `Choices` list modulo its length, updates `Module` on the config,
  and fires the `onZoneCycle` callback (`internal/zone/manager.go`)
- `zone.Sampler.RestartZone(zoneConfig)` stops the old sampling goroutine for
  a single zone and starts a new one with the updated module spec
  (`internal/zone/sampler.go`)
- `app.go` wires `SetOnZoneCycle` → `sampler.RestartZone`, keeping manager and
  sampler decoupled via callback (same pattern as `onPageChange`)

All tests passing (`go test ./...` — 7 packages, 0 failures).

**Pre-existing flaky test noted:** `TestApp_ContextCancellation` fails
intermittently when `go test ./...` is run from the repo root (working directory
issue with relative `exec:` module paths). Passes when run as
`go test ./internal/app/`. Not caused by our changes; tracked for future fix.

---

## Codebase State

### What is genuinely solid

- **HID device layer** (`internal/device/nexus.go`, 498 lines) — reconnect
  backoff, health monitoring, clean `Device` interface. Well-structured.
- **Touch/swipe pipeline** — One-Euro filter, velocity trailing window (4-sample
  weighted, iOS-style), spring physics in `transition.go` with proper stiffness/
  damping constants. Most of the swipe backlog items in
  `docs/swipe-improvements.md` are already resolved. Items 3 (direction change)
  and 4 (rubber-band) are the main remaining ones.
- **Module plugin system** — the RPC-over-subprocess design is questionable
  (see below) but the implementation is sound: proper lifecycle, config
  notification interface, clean `pkg/module` types.
- **Zone renderer** (`internal/zone/renderer.go`, 1,270 lines) — layout engine
  is flexible: multi-line, icons, graphs (sparkline/bar/area/line), progress
  bars, label positioning. More capable than it appears from the UI.
- **Compositor** (`internal/zone/compositor.go`) — clean 114 lines. Does one
  thing.
- **Font manager** (`internal/fonts/manager.go`) — embeds Go fonts, caches
  faces by name+size, falls back gracefully. Works well.
- **WS frame stream** — base64 PNG at up to 30fps, subsample to 10fps when
  idle, full rate during transitions. Page state broadcast on connect and change.
  Good design.

### Structural problems found

**1. manager.go is a god object (1,280 lines, 30+ methods)**
`zone.Manager` handles: config loading from YAML, renderer lifecycle, payload
caching, page switching, live swipe gesture tracking, transition animation
state, frame compositing, page cache pre-rendering, and frame output. These are
at least four distinct concerns. The size makes the swipe backlog hard to fix
cleanly and will make the zone editor impossible to build without introducing
more entanglement.

**2. Three separate config stores, no single source of truth**
At runtime, config lives in:
- `~/.config/nexus-open/config.yaml` — UI preferences (colors, units, location)
  managed by `settings.Manager` via viper
- `configs/layouts/multi-page.yaml` — page/zone layout loaded by `zone.Manager`
  at startup from a hardcoded relative path
- `~/.config/nexus-open/zone-configs.yaml` — module config overrides managed by
  `zone.ConfigManager`
- `shared_preferences` on the Flutter side — theme mode only

Each store has its own file, its own lock, its own save/load path. None are
transactional. The layout file is not user-editable because the path is relative
and loaded once at startup with no reload mechanism.

**3. The module plugin system has real costs for no current benefit**
Every data module (weather, CPU temp, GPU temp, network) is a compiled binary
launched as a child process over hashicorp/go-plugin (gRPC + protobuf). This
is the right architecture if you want third-party modules in any language — but
that isn't happening and there are no external module authors. The costs are
real: ~50ms startup per module, process management overhead, RPC serialization
for a struct that's three strings and a float array, and hashicorp/go-plugin +
gRPC as dependencies. The modules themselves are already Go — they could be
compiled in directly behind the `module.Module` interface with zero API change.
This is a deliberate trade-off worth revisiting, not a bug.

**4. Test suite has a failing test**
`TestHealthHandler` in `internal/api` fails because it expects `"ok"` but gets
`"degraded"` (no mock device in the test context). This test has probably been
failing silently since the health endpoint was updated to reflect device state.
CI would catch this — not clear if CI is configured.

**5. Tap gesture is a stub**
`handler.go:handleTap()` logs "tap detected - zone-specific action would go here"
and does nothing. Tap-to-zone routing (which zone was tapped based on X
coordinate) is designed but not implemented. This is called out in a comment
but is invisible unless you read the source.

**6. Light mode is maintained but broken in key places**
The hardware mockup (black device on white canvas) looks wrong in light mode.
The `light_tab_preview.png` screenshot confirms this. Light mode adds
maintenance cost for a use case that doesn't match the product context (hardware
control software used alongside gaming/monitoring tools, always dark).

**7. The Flutter app information architecture doesn't match user mental model**
The nav rail presents: Preview / Display / Location / Modules / Images. This is
organized around config categories. The user's actual mental model is: "I have
a display. It has pages. Pages have zones. Zones show things." The zone editor
plan fixes this, but the top-level IA should change too — not just add a new
tab.

**8. ~~viper is a heavy dependency~~ — not actionable**
viper is already compiled in with no conflicts or bugs. Replacing it would mean
rewriting `settings.Manager`'s file watching and env binding for zero
user-visible benefit. Leave it.

---

## Work Areas

Ordered by recommended sequence. Each area is a prerequisite for the next.

---

### 1. Fix the failing test + finish tap gesture

Small. `TestHealthHandler` needs a mock device injected. The tap handler needs
X-coordinate routing: given a tap X position (0–639), compute which zone it
falls in on the current page and dispatch the zone's `on_tap` action. This
completes the touch input feature that's been stubbed since the original
implementation.

---

### 2. Remaining swipe backlog items (items 3 & 4)

Items 1 (velocity trailing window) and 2 (spring physics) from
`docs/swipe-improvements.md` are already implemented. Remaining:

**Item 3 — Direction change mid-swipe** (`manager.go:~820`):
`liveSwipeLeft` locks at first touch and never updates. Fix: update direction
from the current signed delta on each `UpdateLiveSwipe` call, but only
re-initialize the target frame if direction actually flips.

**Item 4 — Rubber-band at page boundaries** (`manager.go:~840`):
Already has a `liveSwipeBoundary` flag. Needs the resistance scale applied:
when boundary is true, report `progress * 0.3` to the transition instead of
raw progress, and always cancel (never commit) on lift.

Items 5–9 (cache miss async — already done, snap magic constants — replaced by
spring, One-Euro beta — device-specific tuning, cooldown wall-clock —
acceptable) can be deferred.

---

### 3. manager.go split

Before building the zone editor, split `zone.Manager` into three focused types:

**`PageManager`** — owns: config (pages + zones), current page index, page
switching logic, page cache, `GetConfig()`, `GetCurrentPage()`, `NumPages()`,
`GetPageInfos()`, `SwitchPage()`, `Reload()`.

**`RenderManager`** — owns: renderer pool, compositor, payload map, frame
output, `UpdatePayload()`, `RenderFrame()`, `UpdateTheme()`, `GetLastFrame()`.

**`SwipeController`** — owns: transition state, live swipe tracking, spring
animation, `UpdateLiveSwipe()`, `FinalizeLiveSwipe()`, `CancelLiveSwipe()`,
`IsTransitioning()`.

The existing `Manager` becomes a thin facade that composes all three and
satisfies the existing interface. No API changes required — it's an internal
refactor. This makes items 2 (swipe fixes) and 5 (zone editor) tractable.

---

### 4. ~~Drop viper, drop light mode~~ — RECONSIDERED, REMOVED

Both recommendations were wrong:

**Viper stays**: It's already compiled in; removing it would mean rewriting
`settings.Manager` (file I/O, fsnotify watching, env binding) for zero
user-visible benefit. No version conflict, no bug — no reason to touch it.

**Light mode stays**: The mockup looking odd on white is a problem with the
mockup widget, not with light mode. Fix the mockup (darker border, adjusted
shadow) rather than removing a legitimate UI mode that some users rely on or
that the system may enforce. Fixing the mockup is part of the design refresh
(step 5) anyway.

---

### 5. Design system refresh ✅ 2026-06-05

**Delivered across multiple sessions:**

- Two-accent system: `accent` electric blue (`#4F9EFF`) for all interactive
  UI elements; `hardwareAccent` amber (`#E07B20`) reserved for hardware-display
  elements only (preview frame border, zone colours, severity indicators)
- Depth hierarchy: `darkRail` (`#0A0A0C`) → `darkBg` (`#131316`) → `darkSurface`
  (`#1C1C21`) → `darkElevated` (`#252529`) — rail matches hardware housing colour
- `dataAccent` cyan (`#00C8FF`) retained for hardware data display
- `NEXUS` wordmark: amber glow (`hardwareAccent` with 70% shadow blur) — ties
  rail header to the physical device
- Typography split: Poppins for display/headline/title (identity text at ≥15px);
  Inter for titleSmall/body/label (better hinting at small sizes on Linux HiDPI)
- Full `ColorScheme` wiring in `app_theme.dart` — all Material3 components
  derive from tokens, no raw colour literals in widget files
- Light mode preserved; `lightRail` keeps navy (`#1A2236`) for the sidebar

---

### 6. SQLite config backend

Replace all three config stores with a single SQLite database at
`~/.config/nexus-open/nexus.db`.

**Why SQLite over consolidated YAML:**
- Transactional writes — zone editor changes are atomic
- Schema migrations — versioned via a `schema_version` table; old installs
  upgrade automatically on first run
- Queryable — "give me all zones on page 2" without YAML parsing
- Single file — one backup target, portable between machines

**Driver choice:** `modernc.org/sqlite` (pure Go, no CGO required) preferred
over `mattn/go-sqlite3` (requires CGO and a C toolchain on the build machine).

**Schema:**

```sql
CREATE TABLE schema_version (version INTEGER NOT NULL);

-- UI preferences: replaces config.yaml + shared_preferences
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Layout: replaces layouts/*.yaml
CREATE TABLE pages (
  id    INTEGER PRIMARY KEY AUTOINCREMENT,
  name  TEXT    NOT NULL,
  ord   INTEGER NOT NULL
);

CREATE TABLE zones (
  id           TEXT    PRIMARY KEY,
  page_id      INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  ord          INTEGER NOT NULL,
  width_px     INTEGER NOT NULL CHECK(width_px >= 80 AND width_px <= 640),
  module       TEXT    NOT NULL,
  refresh_ms   INTEGER NOT NULL DEFAULT 2000,
  align        TEXT    NOT NULL DEFAULT 'center',
  config_json  TEXT    NOT NULL DEFAULT '{}',
  theme_json   TEXT    NOT NULL DEFAULT '{}'
);

CREATE INDEX zones_page_ord ON zones(page_id, ord);
```

**Migration path:**
1. On first run with new binary: detect existing YAML files, import them into
   the DB, rename originals to `.bak`
2. Each schema change increments `schema_version` and runs the corresponding
   `ALTER TABLE` / data migration

**Go side:** `settings.Manager` and `zone.ConfigManager` merge into a single
`store.DB` type. `zone.Manager` loads layout from the DB on startup and holds
a reference for live edits. The existing `GET /api/config` / `POST /api/config`
API contract is unchanged — it reads/writes the `settings` table.

**Flutter side:** `NexusApiService` gains layout endpoints (see zone editor
below). `SettingsState` gains a `loadLayout()` path alongside `loadFromBackend()`.

---

### 7. Zone/page layout editor

The zone editor is the primary user-facing feature. It replaces the YAML file
as the way to configure what shows on the display.

**Information architecture change:**
The nav rail should present: `Layout` / `Modules` / `Images` / `Display`. The
current "Preview" tab merges into the Layout editor (it becomes the header of
the layout view). Location moves into Display settings.

**Layout editor structure:**

```
┌─ Layout ──────────────────────────────────────────────────────┐
│  ┌─ Live frame (hardware mockup, 640×48, interactive) ───────┐ │
│  │  [weather zone ──────][cpu zone ────][gpu ────][net ─────] │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  Pages:  ● System  ○ Performance  ○ Clock   [+ Add page]       │
│                                                                 │
│  Zones on "System":                                             │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ ⠿ weather     160px  Weather      [────────] [⚙] [✕]  │    │
│  │ ⠿ cpu         160px  CPU Temp     [────────] [⚙] [✕]  │    │
│  │ ⠿ gpu         160px  GPU Temp     [────────] [⚙] [✕]  │    │
│  │ ⠿ network     160px  Network      [────────] [⚙] [✕]  │    │
│  └────────────────────────────────────────────────────────┘    │
│  Total: 640px ✓                    [+ Add zone]                │
└─────────────────────────────────────────────────────────────────┘
```

- Zone width slider in each row; adjacent zones compensate (total must stay 640)
- Drag handle (⠿) for reorder within page
- Gear (⚙) expands module config inline (absorbs the current Modules tab)
- Live frame updates as any change is applied
- Page tabs at top: click to preview, drag to reorder, [+ Add page] appends

**New API endpoints needed:**
```
GET    /api/layout              — full layout: pages + zones
POST   /api/layout/pages        — create page {name}
PUT    /api/layout/pages/:id    — update {name, ord}
DELETE /api/layout/pages/:id    — delete (cascades zones)
PUT    /api/layout/zones/:id    — update {width_px, module, refresh_ms, align, config_json, theme_json}
POST   /api/layout/zones        — add zone {page_id, module, width_px, ...}
DELETE /api/layout/zones/:id    — remove zone
PUT    /api/layout/zones/reorder — reorder [{id, ord}]
```

---

### 8. Module system nomenclature cleanup ✅ 2026-06-05

Eliminated the "plugin" terminology from internal code — everything is a
"module" from the user's perspective. go-plugin is kept as the exec: transport.

**Changes made:**

- `ModulePlugin` → `ExecModule` in `pkg/module/plugin.go` (and all 6 callers in
  `internal/module/host/host.go` and each `modules/*/main.go`)
- `rpcServer` → `moduleRPC` — the net/rpc receiver name now registers as
  `moduleRPC.*` instead of `Plugin.*`, matching the module terminology throughout
- `MagicCookieKey` `"NEXUS_MODULE_PLUGIN"` → `"NEXUS_EXEC_MODULE"`
- Removed "Modules are plugins…" from package doc comment in `types.go`
- Deleted the empty `internal/plugin/` directory (leftover from registry removal)

**Architecture decision:** go-plugin + exec: subprocess model is kept.
Builtin modules (`builtin:clock` etc.) remain compiled in. The `exec:` prefix
is the deliberate extension point for power users and future third-party authors.
No dep removal, no change to the `module.Module` interface.

---

## Recommended Sequence

```
1. Fix failing test + finish tap gesture  ✅ done
2. Remaining swipe fixes (3 & 4)         ✅ already done
3. manager.go split                      ✅ done
4. ~~Drop viper + drop light mode~~      removed — bad recommendations
5. Design system refresh                 ✅ done
6. SQLite config backend                 (3-4 days)
7. Zone/page layout editor               (1-2 weeks)
8. Module system nomenclature cleanup    ✅ done
```

Steps 1-4 are largely independent of each other and could be parallelised.
Step 5 should finish before 7 (new UI should use the new tokens).
Step 6 must finish before 7 (editor writes to the store).

---

## Explicitly out of scope

- Auto-rotate / scheduled page switching
- Multiple device support
- Cloud sync
- The old "display modes" concept (clock mode, weather mode) — the zone system
  achieves this more flexibly
- Windows / macOS support
