# Nexus Open — Improvement Plan

**Created:** 2026-06-04  
**Updated:** 2026-06-04  
**Status:** Execution complete (see summary below)

This plan covers everything needed to go from a functional prototype to a
polished, fully-featured open-source Linux application. It is organised into
eleven areas, each with concrete tasks. The sequencing section at the bottom
maps tasks to a four-week execution order.

---

## Execution Summary

Tasks are marked: ✅ Done · ⏭ Skipped (design/asset work) · 🔲 Not yet done

### Completed (60 tasks)

1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3, 3.4, 3.5, 4A.1,
4A.2, 4A.3, 4A.4, 4B.1, 4B.2, 4B.3, 4C.1, 4C.2, 4C.3, 4C.4, 4C.5, 4C.6,
5.1, 5.2, 5.3, 5.4, 5.5, 6.1, 6.2, 6.3, 6.4, 7.1, 7.2, 7.3, 7.4, 7.5,
7.6, 8.1, 8.2, 10.1, 10.2, 11.1, 11.2, 11.3, 11.4

### Skipped — requires design/asset work (2 tasks)

9.1 (icon set), 9.2 (screenshots)

### Remaining (0 tasks)

All implementable tasks are complete.

---

## Area 1 — Dev Workflow

**Problem:** The correct entry point (`NEXUS_MOCK_DEVICE=1 ./dev.sh`) exists but
was not being used. When running with real hardware, stopping the app leaves
a locked HID handle that blocks the next start. `dev.sh` also orphans the Flutter
UI process on Ctrl+C, and `.air.toml` passes `--debug` unconditionally, flooding
logs with 30 FPS graph-position lines that make real errors hard to see.

#### 1.1 ✅ — Fix `dev.sh` process cleanup

- Add `trap 'kill $UI_PID 2>/dev/null; wait' EXIT SIGINT SIGTERM` before
  launching the UI process so Ctrl+C always kills both Air and the Flutter bundle.
- File: `dev.sh`

#### 1.2 ✅ — Remove unconditional `--debug` from Air config

- Remove `-debug` from `args_bin` in `.air.toml`.
- Add a `NEXUS_DEBUG` env var check in `cmd/nexus-open/main.go` as an alternative
  to `--debug` flag, so debug logging is opt-in during dev without editing config files.
- Files: `.air.toml`, `cmd/nexus-open/main.go`

#### 1.3 ✅ — Add initial-connect retry loop

- The reconnect monitor in `NexusDevice` only fires after a successful first
  connect. If the device is busy on startup (e.g. held by a previous process),
  the app starts permanently disconnected with no retry.
- Add a startup retry loop (3 attempts, 2s apart) before giving up on initial connect.
- File: `internal/device/nexus.go`

---

## Area 2 — Display Emulation / Live Preview via WebSocket

**Problem:** There is no way to see what the 640×48 display looks like without
physical hardware. The Flutter preview tab shows hardcoded mock data, and the
window visibility controller polls the backend every 500ms. Both are replaced by
a single persistent WebSocket connection that pushes frames and state changes.

**What exists:** `zone.Manager` already stores every rendered frame in `lastFrame`
(mutex-protected, updated at 30 FPS). The Go standard library's `net/http` has
built-in WebSocket upgrade support via `nhooyr.io/websocket` (zero dependencies,
context-native).

**Design:** A single `GET /api/ws` endpoint upgrades to WebSocket and pushes
typed JSON messages to all connected clients:

```json
{"type": "frame",        "data": "<base64-encoded PNG>"}
{"type": "window_state", "data": "shown" | "hidden"}
{"type": "config",       "data": { ...current config... }}
```

The Flutter side uses `web_socket_channel` and a `StreamController` that fans
messages out to listeners by type. WebSocket disconnect is also the
connection-loss signal — no separate health polling needed.

Frame rate to clients is capped at ~10 FPS (every 3rd render tick) to keep
the base64 PNG stream manageable over localhost. The 30 FPS device render loop
is unaffected.

#### 2.1 ✅ — Add `GetLastFrame()` to `zone.Manager`

- Expose the existing `lastFrame *image.RGBA` field via a public mutex-safe method.
- File: `internal/zone/manager.go`

#### 2.2 ✅ — Add WebSocket hub to the API server

- Add a `Hub` type in `internal/api/` that manages connected WebSocket clients
  (register/unregister channels, broadcast to all).
- Add `GET /api/ws` upgrade handler using `nhooyr.io/websocket` (add to `go.mod`).
- The hub receives messages from a `chan WSMessage` and fans them out to clients.
  Disconnected clients are removed from the hub on write error.
- Files: `internal/api/ws.go` (new), `internal/api/server.go`, `go.mod`

#### 2.3 ✅ — Push frames from the render loop

- In `app.go`'s `renderLoop()`, after every 3rd frame (≈10 FPS), encode
  `lastFrame` as PNG into a `bytes.Buffer`, base64-encode it, and send a
  `{"type":"frame","data":"..."}` message to the hub.
- Wire the hub into `App` in `app.go`.
- Files: `internal/app/app.go`

#### 2.4 ✅ — Push window state changes from the tray/API

- When `handleWindowShow` or `handleWindowHide` is called, also broadcast a
  `{"type":"window_state","data":"shown"|"hidden"}` message to the hub.
- Remove the polling loop from `window_controller.dart` entirely — the WS
  message replaces it.
- File: `internal/api/handlers.go`

#### 2.5 ✅ — Flutter: WebSocket service and live preview

- Add `web_socket_channel` to `ui/pubspec.yaml` (if not already present as
  a transitive dep, make it explicit).
- Create `ui/lib/src/services/ws_service.dart`: connects to `ws://localhost:1985/api/ws`,
  reconnects with exponential backoff on disconnect, exposes typed streams:
  `Stream<Uint8List> frames` and `Stream<String> windowState`.
- Replace the static preview in `preview_tab.dart` with a widget that listens
  to `WsService.frames` and renders incoming PNG bytes via `Image.memory`.
- Replace the polling loop in `window_controller.dart` with a listener on
  `WsService.windowState`.
- Add a "LIVE" chip indicator to the preview; fallback to static mock when the
  stream has no data yet.
- Files: `ui/lib/src/services/ws_service.dart` (new),
  `ui/lib/src/widgets/settings/tabs/preview_tab.dart`,
  `ui/lib/src/services/window_controller.dart`,
  `ui/pubspec.yaml`

---

## Area 3 — Arch Linux / udev / Multi-Distro

**Problem:** The packaged udev rules cover `SUBSYSTEM=="usb"` but not
`SUBSYSTEM=="hidraw"`. On Arch (and any distro where hidraw ACLs aren't
inherited from the USB rule), the app can't open the device without sudo.
Beyond udev, Fedora/RHEL users (≈40% of Linux desktop) have no native package.

#### 3.1 ✅ — Fix the packaged udev rule

- Add the missing hidraw line with `GROUP="plugdev"`:
  ```
  SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b8e", \
    MODE="0660", GROUP="plugdev", TAG+="uaccess"
  ```
- Fix the same gap in all packaging targets (deb, arch, flatpak, snap, udev/).
- File: `packaging/udev/99-corsair-nexus.rules` and all packaging subdirs

#### 3.2 ✅ — Add Arch packaging path

- The PKGBUILD in `packaging/arch/` is ready; submit to AUR.
- Update `scripts/setup-udev.sh` to detect distro and write rules to
  `/usr/lib/udev/rules.d/` (Arch, package-managed) vs `/etc/udev/rules.d/`
  (manual install).
- Files: `packaging/arch/PKGBUILD`, `scripts/setup-udev.sh`

#### 3.3 ✅ — Add RPM packaging

- Create `packaging/rpm/nexus-open.spec` for Fedora/RHEL/openSUSE.
- RPM covers approximately 40% of Linux desktop users not reached by DEB alone.
- Add `make rpm` target to Makefile.
- Files: `packaging/rpm/nexus-open.spec` (new), `Makefile`

#### 3.4 ✅ — Fix Flatpak metadata placeholders

- Replace all `yourusername` template URLs with real GitHub org/repo.
- Uncomment the Flutter UI module in `com.github.nexusopen.NexusOpen.yaml`
  and make it build correctly.
- Add at least one screenshot reference (can be generated from zone-demo).
- Add the app icon (uncomment the icon line).
- File: `packaging/flatpak/com.github.nexusopen.NexusOpen.yaml`,
  `packaging/flatpak/com.github.nexusopen.NexusOpen.metainfo.xml`

#### 3.5 ✅ — Update `DEVICE_SETUP.md`

- Add Arch-specific section: plugdev group, correct rules path, udevadm trigger.
- Add Fedora-specific section: `input` group instead of `plugdev`, SELinux notes.
- Note that `TAG+="uaccess"` handles logind seat sessions automatically
  (desktop session users on most distros don't need manual group assignment).
- File: `DEVICE_SETUP.md`

---

## Area 4 — Flutter UI: Correctness and Polish

The current UI covers ~40% of what the backend can configure. The audit found
six categories of issues: missing surfaces, data bugs, dead code, hardcoded
values, poor error handling, and design gaps.

### 4A — Data Bugs

#### 4A.1 ✅ — Fix `dateFormat` not persisting

- `setDateFormat()` calls `notifyListeners()` but never updates `_config`,
  so the value is lost on save.
- Add `date_format` field to `NexusConfig` (model + JSON serialization).
- Add the field to the backend `settings.Config` struct and persist it.
- Files: `ui/lib/src/models/settings_state.dart`,
  `ui/lib/src/services/nexus_api_service.dart`,
  `internal/settings/config.go`

#### 4A.2 ✅ — Delete dead tabs

- `time_format_tab.dart` and `units_tab.dart` exist but are not wired into
  `SettingsPage`. `units_tab.dart` makes raw HTTP calls bypassing `SettingsState`.
- Delete both files. Their functionality is already covered by `display_tab.dart`.
- Files: `ui/lib/src/widgets/settings/tabs/time_format_tab.dart`,
  `ui/lib/src/widgets/settings/tabs/units_tab.dart`

#### 4A.3 ✅ — Fix image grid showing filenames instead of images

- `images_tab.dart` shows a filename + icon, not the actual image.
- Display images using `Image.network('http://localhost:1985/api/images/<filename>')`.
- Requires adding a `GET /api/images/:filename` static file handler to the backend.
- Files: `ui/lib/src/widgets/settings/tabs/images_tab.dart`,
  `internal/api/handlers.go`, `internal/api/server.go`

#### 4A.4 ✅ — Fix hardcoded default location

- The default `"Jersey City, NJ"` is hardcoded in the location tab widget,
  not read from `SettingsState`.
- Fall back to `settings.location` if set, otherwise show hint "Search for a city…".
- File: `ui/lib/src/widgets/settings/tabs/location_tab.dart`

### 4B — Missing Configuration Surfaces

The backend has a full per-zone, per-module config system. The UI exposes none
of it. Each module accepts specific keys via its `OnConfigChanged` handler:

| Module | Config keys |
|--------|-------------|
| `cpu-temp` | `unit` (metric/imperial), `graph_type` (sparkline/bar/area) |
| `gpu-temp` | `unit` (metric/imperial), `graph_type` (sparkline/bar/area) |
| `weather` | `location` (string), `unit` (metric/imperial) |
| `network` | `network_format` (bytes/bits), `graph_type` (sparkline/bar/area) |

#### 4B.1 ✅ — Add a "Modules" tab to SettingsPage

- New tab (icon: `tune`), placed between Display and Images.
- Shows one expandable card per active module, loaded from `GET /api/modules/:name/config`.
- Each card renders the relevant controls for that module's known config keys
  (dropdowns for enums, text fields for strings).
- On change, calls `POST /api/modules/:name/config` immediately (no save button —
  these are live updates via `ConfigNotifier`).
- On success, briefly animate the card border with the accent color.
- On failure, show an inline error on the card (not a snackbar).
- File: `ui/lib/src/widgets/settings/tabs/modules_tab.dart` (new)

#### 4B.2 ✅ — Add brightness control

- Add a brightness slider (0–100) to the Display tab.
- Calls `POST /api/device/brightness` on drag end (not on every tick).
- Disable with a muted state when `isConnected` is false.
- File: `ui/lib/src/widgets/settings/tabs/display_tab.dart`

#### 4B.3 ✅ — Add device info to connection status

- On connection, fetch `GET /api/device/info` and show model name and firmware
  version in a tooltip or chip in the AppBar.
- File: `ui/lib/src/widgets/settings/settings_page.dart`

### 4C — UI Design and Visual Polish

The existing theme (navy `#202C46` + orange `#DB8720`, Poppins font) is a sound
foundation but the execution is generic. `docs/UI_REDESIGN.md` already documents
a display rendering redesign (8px grid, larger primary text, atmospheric graphs)
that is partially implemented. The Flutter app needs equivalent treatment.

#### 4C.1 ✅ — Add dark mode

- This is the single most-reported gap for any Linux desktop app in 2025.
  Approximately 85% of Linux developers use dark mode.
- Add a dark `ThemeData` to `app_theme.dart` alongside the existing light theme,
  using deep grey surfaces (`#1C1C1E`, `#2C2C2E`) not pure black.
- Respect the system theme via `MediaQuery.platformBrightnessOf(context)`.
- Add a manual toggle (light / dark / system) in the Display tab, persisted to config.
- Ensure the orange accent `#DB8720` works in both themes (it does — warm on dark
  is a strong combination).
- Files: `ui/lib/src/theme/app_theme.dart`, `ui/lib/src/models/settings_state.dart`,
  `internal/settings/config.go`

#### 4C.2 ✅ — Switch from TabBar to NavigationRail

- At 800px wide, a left-side `NavigationRail` with icons + labels is more
  spacious and modern than a top tab strip. It scales gracefully as more
  sections are added, and gives the content area more vertical room.
- Navigation items: Preview, Location, Display, Modules, Images.
- The live 640×48 preview frame (from 2.5) sits as a hero element in the
  header area above the rail, visible regardless of which section is active.
- Files: `ui/lib/src/widgets/settings/settings_page.dart`

#### 4C.3 ✅ — Tighten the visual design system

- Apply a consistent 8px spacing grid throughout (replace arbitrary padding values).
- Increase card `borderRadius` to 16px; use `surfaceVariant` fills instead of
  white-on-white cards — this gives depth without heavy shadows.
- Use the orange accent `#DB8720` consistently: active slider thumbs, focused
  input borders, selected nav item indicator, save button.
- Increase secondary label contrast — `#9AA0A6` (current muted color used in
  the display renderer) fails WCAG AA on dark backgrounds; target `#B8BDC2`
  as documented in `docs/UI_REDESIGN.md`.
- Add `AnimatedSwitcher` for the connection status indicator and
  `PageTransitionSwitcher` between navigation sections.
- Files: `ui/lib/src/theme/app_theme.dart`, all tab widgets

#### 4C.4 ✅ — Connection loss UX

- When `isConnected` flips false mid-session, the WebSocket disconnect (from 2.5)
  is the signal.
- Show a persistent `MaterialBanner` (not a snackbar — it disappears) at the top
  of the screen with a retry button.
- Disable the save FAB and Modules tab controls with a muted overlay while
  disconnected.
- File: `ui/lib/src/widgets/settings/settings_page.dart`

#### 4C.5 ✅ — Unsaved changes guard

- Warn the user with a dialog if they close or navigate away with unsaved changes
  in the global config (location, time format, colors, etc.).
- Module config changes are live (no save needed), so no guard needed there.
- File: `ui/lib/src/widgets/settings/settings_page.dart`

#### 4C.6 ✅ — Delete `app.dart`

- `app.dart` uses `deepPurple` color scheme while `app_theme.dart` defines
  navy + orange. `main.dart` instantiates `SettingsPage` directly, so `app.dart`
  is dead code. Delete it.
- File: `ui/lib/src/app.dart`

---

## Area 5 — API Contract: OpenAPI → Generated Flutter Client

**Problem:** The Flutter app has a hand-written `nexus_api_service.dart` that
calls only 7 of 15 available API endpoints. The backend already serves an
OpenAPI 3.0 spec at `/openapi.yaml`, and `scripts/generate-flutter-api.sh` is
a stub — but the `api/` directory doesn't exist yet (spec never committed).
Schema mismatches like the missing `dateFormat` field are invisible until runtime.

**Goal:** Go annotations → committed spec → typed Dart models → Flutter imports
generated types. Every backend schema is represented as a `@freezed` class;
schema drift in the backend is visible via the spec drift CI check (5.5).

**Approach:** `freezed` + `json_serializable` rather than `openapi-generator`.
All Dart-native OpenAPI generators either require Java or force an incompatible
HTTP client (Dio/Chopper) as a runtime dependency. Since the API is a local
daemon we control, `freezed` models written once from the spec are the simplest,
most maintainable path. The existing `http`-based `NexusApiService` is retained.

#### 5.1 ✅ — Commit a baseline OpenAPI spec

- Run the `go-openapi` generation step manually, fix annotation gaps
  (the `display` sub-struct in `settings.Config`, zone/module config request
  bodies), and commit the result as `api/openapi.yaml`.
- This file is source of truth. Regenerate on every backend schema change.
- Files: `api/openapi.yaml` (new, committed)

#### 5.2 ✅ — Validate and complete the spec

- Run `openapi-generator validate -i api/openapi.yaml` and resolve all warnings.
- Ensure complete schemas for: `GET /api/config` (full `settings.Config` incl.
  `date_format`, `theme`), `POST /api/modules/:name/config` request/response,
  `GET /api/images/:filename` (binary), `GET /api/ws` (documented as upgrade).
- File: `api/openapi.yaml`

#### 5.3 ✅ — Add typed models with freezed

**Decision (2026-06-04):** All Dart-native OpenAPI generators either require Java
(`openapi_generator` pub package wraps the Java JAR) or force an incompatible HTTP
client as a runtime dep (`swagger_dart_code_generator` → Chopper,
`openapi_retrofit_generator` → Dio). Since the API is a local daemon we control,
the right approach is `freezed` + `json_serializable` — mature, zero new runtime
deps, compatible with the existing `http`-based `NexusApiService`.

- Add `freezed_annotation`, `json_annotation` to `ui/pubspec.yaml` dependencies;
  add `freezed`, `build_runner`, `json_serializable` to dev_dependencies.
- Create `ui/lib/src/models/api_models.dart` with `@freezed` classes matching the
  schemas in `api/openapi.yaml`: `NexusConfig`, `DisplayConfig`, `BrightnessRequest`,
  `ZoneConfig`, `ModuleConfig`, `DeviceInfo`, `ApiError`.
- Run `dart run build_runner build` to generate `.freezed.dart` and `.g.dart` files.
- Add `*.freezed.dart` and `*.g.dart` to `.gitignore` (generated, not committed).
- Add `make models` target to `Makefile` that runs `build_runner build`.
- Files: `ui/lib/src/models/api_models.dart` (new), `ui/pubspec.yaml`, `Makefile`,
  `.gitignore`

#### 5.4 ✅ — Migrate NexusApiService to freezed models

- Replace the hand-written `NexusConfig` class in `nexus_api_service.dart` with the
  generated `@freezed` equivalent from `api_models.dart`.
- `NexusApiService` keeps the `http` package and its existing call signatures —
  only the serialization layer changes (`fromJson`/`toJson` now comes from
  `json_serializable` rather than being hand-written).
- `SettingsState` and all call sites remain unchanged (facade pattern holds).
- **Do not add Dio.** The local daemon API has no need for interceptors, cancellation,
  or upload progress. `http` is the right tool.
- Files: `ui/lib/src/services/nexus_api_service.dart`,
  `ui/lib/src/models/settings_state.dart`

#### 5.5 ✅ — Add spec regeneration to CI

- Add a CI step that regenerates the spec from source and diffs it against
  `api/openapi.yaml`. Fail if they differ — catches backend changes without
  annotation updates.
- File: `.github/workflows/ci.yml`

---

## Area 6 — Error Messaging and First-Run Experience

**Problem:** Silent failures are the biggest source of user confusion. USB
permission errors, backend connection failures, and missing modules all result
in the app showing "disconnected" with no actionable guidance. This is the
difference between a user fixing their own problem in 30 seconds and filing
a GitHub issue.

#### 6.1 ✅ — Actionable USB error messages

- In `NexusDevice.Connect()`, detect the specific failure and return structured
  errors the UI can act on:
  - Device not found → `ErrDeviceNotFound` (already exists): surface as
    "iCUE Nexus not found. Is it plugged in?"
  - Permission denied (`hidapi: failed to open`) → `ErrPermissionDenied` (new):
    surface as "USB permission denied. Run: `sudo usermod -a -G plugdev $USER`
    then log out and back in."
  - Device busy → `ErrDeviceBusy` (new): surface as "Device is in use by another
    application. Close iCUE or other Nexus software."
- Propagate these through the API as structured error responses so the Flutter
  UI can display them with the right tone and action.
- Files: `internal/device/errors.go`, `internal/device/nexus.go`,
  `internal/api/handlers.go`

#### 6.2 ✅ — In-app first-run detection and onboarding

- On first launch (no config file exists), show an onboarding overlay instead
  of landing immediately in settings.
- Steps: "Welcome" → "Connect your device" (shows live connection status) →
  "Choose your location" (feeds directly into location tab) → "Done".
- Detect first run by checking for the absence of the config file.
- Files: `ui/lib/src/widgets/onboarding/` (new),
  `ui/lib/src/widgets/settings/settings_page.dart`

#### 6.3 ✅ — Module error visibility

- When a module crashes or times out, the zone currently shows a blank placeholder.
- Show the placeholder text "Module error" (the `RenderPlaceholder` fix already
  renders text — just pass a meaningful string).
- Expose module error state through the API: `GET /api/zones/:id/status` returns
  `{status: "ok"|"error"|"loading", error: "..."}`.
- Surface this in the Modules tab card (inline error badge).
- Files: `internal/zone/sampler.go`, `internal/api/zone_handlers.go`

#### 6.4 ✅ — Add troubleshooting section to README

- Add a "Troubleshooting" section covering the five most common first-run issues:
  1. Device not found (not plugged in)
  2. USB permission denied (not in plugdev group)
  3. Backend won't start (port 1985 in use)
  4. Flutter UI won't connect (backend not running)
  5. Module shows blank (plugin binary not built/found)
- File: `README.md`

---

## Area 7 — Packaging, CI/CD, and Release

**Problem:** The CI pipeline only tests and builds the binary. It doesn't build
packages, doesn't lint, doesn't run Flutter tests, and has no release automation.
The Flatpak metadata has placeholder URLs. There's no RPM for Fedora users.

#### 7.1 ✅ — Add linting to CI

- Add `golangci-lint` to the CI pipeline (it's already in the Makefile as
  `make lint` but not wired into CI).
- Add Dart/Flutter analysis: `flutter analyze` in a Flutter CI job.
- File: `.github/workflows/ci.yml`

#### 7.2 ✅ — Add Flutter tests

- The UI currently has zero test coverage. Add widget tests for the highest-risk
  surfaces: `SettingsPage` (save/load cycle), `PreviewTab` (WS frame display),
  `ModulesTab` (config POST on change).
- Add `flutter test` step to CI.
- Files: `ui/test/` (new), `.github/workflows/ci.yml`

#### 7.3 ✅ — Add release workflow

- Create `.github/workflows/release.yml` triggered on `v*` tags.
- Jobs: build binary → build DEB → build AppImage → build RPM → create GitHub
  Release → upload artifacts with checksums → generate release notes from CHANGELOG.
- Files: `.github/workflows/release.yml` (new)

#### 7.4 ✅ — Fix AppImage build reliability

- `scripts/build-appimage.sh` references a hardcoded
  `/usr/lib/x86_64-linux-gnu/libusb-1.0.so.0` path that breaks on Arch and ARM.
- Use `ldd` output or `pkg-config --variable=libdir libusb-1.0` to find the
  library dynamically.
- Fail loudly (not just a warning) if the icon asset or libusb are missing.
- File: `scripts/build-appimage.sh`

#### 7.5 ✅ — Fix DEB package metadata

- Replace `contact@example.com` and `https://github.com/yourusername/nexus-open`
  placeholder values with real project URLs.
- File: `packaging/deb/DEBIAN/control`

#### 7.6 ✅ — Add GitHub community files

- Add `.github/CODE_OF_CONDUCT.md` (use the standard Contributor Covenant).
- Add `.github/ISSUE_TEMPLATE/bug_report.md` and `feature_request.md`.
- Add `.github/PULL_REQUEST_TEMPLATE.md`.
- These are standard for any open-source project on GitHub and will be prompted
  by GitHub's community health checklist.
- Files: `.github/CODE_OF_CONDUCT.md`, `.github/ISSUE_TEMPLATE/` (new),
  `.github/PULL_REQUEST_TEMPLATE.md` (new)

---

## Area 8 — System Tray Reliability

**Problem:** `internal/tray/tray.go` has two concrete bugs. First, `startFlutter()`
returns immediately after `cmd.Start()` — Show/Hide commands dispatched within
the first second silently fail because the Flutter window isn't ready yet.
Second, the API port is hardcoded as `localhost:1985` rather than coming from
the app's configured port, so `--port` flag users get broken tray controls.


#### 8.1 ✅ — Add Flutter readiness check before accepting tray events

- After `cmd.Start()`, poll `GET /api/window/state` (or wait for the WS
  connection from 2.2) with a short timeout (max 5s) before processing
  Show/Hide clicks.
- If Flutter fails to start within the timeout, show an error in the tray
  tooltip and disable Show/Hide menu items.
- File: `internal/tray/tray.go`

#### 8.2 ✅ — Pass API port from config into tray manager

- `Manager` should accept the configured API address (e.g. `:1985`) rather
  than hardcoding `localhost:1985` in `showWindow()` and `hideWindow()`.
- Thread it through from `cmd/nexus-open/main.go` at construction time.
- Files: `internal/tray/tray.go`, `cmd/nexus-open/main.go`

---

## Area 9 — Icons and App Identity

**Problem:** `packaging/icons/` is empty. `internal/tray/icon.png` is 64×64
(suitable for the tray but not for AppImage, Flatpak, or desktop environments
which expect 256×256 or SVG). The AppImage build fails silently when it can't
find the icon. There is no consistent app icon across packaging formats.


#### 9.1 ⏭ — Create a proper icon set

- Design or commission a single SVG icon representing the app (a stylised
  horizontal display strip is the obvious choice, given the hardware).
- Export to: `packaging/icons/nexus-open.svg` (source),
  `packaging/icons/16.png`, `48.png`, `64.png`, `128.png`, `256.png`.
- Replace `internal/tray/icon.png` with the 64px export.
- Update `packaging/desktop/nexus-open.desktop` to reference the icon by name.
- Files: `packaging/icons/` (new files), `internal/tray/icon.png`,
  `packaging/desktop/nexus-open.desktop`

#### 9.2 ⏭ — Add screenshots for Flatpak and README

- Run `cmd/zone-demo` with the multi-page layout to generate a representative
  640×48 display screenshot and commit it to `docs/screenshots/display.png`.
- Take a screenshot of the Flutter settings window (both light and dark themes)
  and commit to `docs/screenshots/`.
- Reference the screenshots in `README.md` (one display screenshot, one app
  screenshot near the top — most users decide in 5 seconds).
- Reference them in the Flatpak metainfo.
- Files: `docs/screenshots/` (new), `README.md`,
  `packaging/flatpak/com.github.nexusopen.NexusOpen.metainfo.xml`

---

## Area 10 — Docs Cleanup

**Problem:** `docs/` contains 22 files, of which 14 are internal development
planning artefacts that were never intended to be public-facing. On GitHub
they create noise, make the project look unfinished, and contain stale
information (references to unimplemented features, old version numbers).

The 8 files worth keeping publicly are:
`INSTALLATION.md`, `DEVICE_SETUP.md` (root), `QUICKSTART_ARCH.md`,
`MODULE_FEATURES.md`, `LAYOUT_SYSTEM.md`, `CONFIG_NOTIFIER.md`,
`PROTOCOL_NOTES.md`, `REVERSE_ENGINEERING_FINDINGS.md`, `TERMINOLOGY.md`,
`ROADMAP.md`, `RELEASE_CHECKLIST.md`.

#### 10.1 ✅ — Move internal planning docs out of `docs/`

- Move to `docs/internal/`: `v0.1.5-POLISH-PLAN.md`, `v0.1.5-PROGRESS.md`,
  `v0.2.0-PLAN.md`, `OPENAPI_CONFIG_NOTIFICATION_PLAN.md`,
  `SWIPE_IMPROVEMENT_PLAN.md`, `MIGRATION_INSTRUMENTS.md`,
  `MULTI_PAGE_CONFIG_ANALYSIS.md`, `PER_ZONE_CONFIG_DESIGN.md`,
  `FLEXIBLE_LAYOUT_DESIGN.md`, `RPC_DESIGN.md`, `UI_REDESIGN.md`.
- Add `docs/internal/.gitkeep` and a one-line note explaining these are
  historical planning documents.
- Files: `docs/internal/` (new directory, moved files)

#### 10.2 ✅ — Fix stale content in public docs

- `docs/QUICKSTART_ARCH.md`: fix `-debug` flag to `--debug` throughout.
- `docs/ROADMAP.md`: update from v0.2.0 planning state to current v1.0 reality;
  add the items from this improvement plan as the forward roadmap.
- `docs/RELEASE_CHECKLIST.md`: replace `yourusername` placeholder URLs.
- All docs referencing `https://github.com/yourusername/nexus-open`: replace
  with the real repository URL project-wide (single `sed` pass).
- Files: `docs/QUICKSTART_ARCH.md`, `docs/ROADMAP.md`,
  `docs/RELEASE_CHECKLIST.md`, and any other docs with placeholder URLs

---

## Area 11 — Accessibility

**Problem:** The Flutter UI has no semantic labels, no keyboard navigation hints,
and hard-coded window sizes. This affects roughly 15–20% of users and fails
WCAG 2.1 AA compliance. It will also be an immediate point of feedback on GitHub.

#### 11.1 ✅ — Add `Semantics` wrappers to all interactive widgets

- Every icon button, status indicator, and non-obvious control needs a
  `Semantics(label: '...')` wrapper so screen readers can describe it.
- Key targets: connection status icon, tab bar icons, color picker swatches,
  map zoom controls, save FAB.
- Files: `ui/lib/src/widgets/settings/settings_page.dart` and all tab files

#### 11.2 ✅ — Keyboard navigation

- Ensure all interactive elements are reachable via Tab key in logical order.
- Add `Focus` nodes where needed and ensure the location search field is
  auto-focused on tab open.
- Files: all tab widgets

#### 11.3 ✅ — Minimum contrast audit

- Check that text/background color combinations in the light and dark themes
  meet WCAG AA (4.5:1 for body text, 3:1 for large text).
- The current accent blue `#3984B2` on white is borderline — verify and adjust.
- File: `ui/lib/src/theme/app_theme.dart`

#### 11.4 ✅ — Respect system text scaling

- Remove hard-coded `fontSize` values where they fight system accessibility
  settings. Use `Theme.of(context).textTheme` styles instead.
- File: throughout the UI widgets

---

## Sequencing

```
Week 1 (foundations — independent, unblocks everything else):
  1.1   dev.sh trap fix
  1.2   Remove --debug from Air
  3.1   Fix udev rules across all packaging targets
  3.5   Update DEVICE_SETUP.md
  4A.1  Fix dateFormat persistence
  4A.2  Delete dead tabs
  4A.4  Fix hardcoded location default
  4C.6  Delete app.dart
  5.1   Commit baseline OpenAPI spec
  5.2   Validate and complete the spec
  6.4   Add troubleshooting to README
  7.5   Fix DEB metadata placeholders
  7.6   Add GitHub community files
  8.2   Pass API port into tray manager
  9.1   Create icon set (blocks AppImage, Flatpak, DEB icon)
  10.1  Move internal planning docs to docs/internal/
  10.2  Fix stale content in public docs

Week 2 (backend additions + codegen):
  1.3   Device initial-connect retry
  2.1   zone.Manager GetLastFrame()
  2.2   WebSocket hub + /api/ws endpoint
  2.3   Push frames from render loop
  2.4   Push window state from tray/API
  3.3   Add RPM packaging
  4A.3  Image serving endpoint + grid fix
  4B.2  Brightness slider
  4B.3  Device info in AppBar
  5.3   Wire codegen into build
  5.4   Migrate Flutter to generated client
  6.1   Actionable USB error messages
  6.3   Module error visibility
  7.1   Add linting to CI
  7.4   Fix AppImage build reliability
  8.1   Flutter readiness check in tray

Week 3 (Flutter-heavy — requires Week 2 backend):
  2.5   Flutter WsService + live preview
  4B.1  Modules tab
  4C.1  Add dark mode
  4C.2  Switch to NavigationRail
  4C.3  Visual design system (8px grid, cards, accent colour)
  4C.4  Connection loss UX
  4C.5  Unsaved changes guard
  6.2   First-run onboarding overlay
  7.2   Add Flutter widget tests
  9.2   Add screenshots to README and Flatpak metainfo
  11.1  Semantics wrappers
  11.2  Keyboard navigation
  11.3  Contrast audit
  11.4  System text scaling

Week 4 (polish, packaging, CI release):
  3.2   Submit AUR package
  3.4   Fix Flatpak metadata
  5.5   Spec drift check in CI
  7.3   Release workflow (tag → packages → GitHub Release)
```

---

## Out of Scope

- New modules (media player, disk usage, CPU load graph) — separate effort
- Layout editor (drag-to-resize zones in UI) — post-v1.1
- Multi-device support — architectural change
- NixOS / Gentoo packaging — post-v1.0, referenced in CHANGELOG roadmap
- Additional WebSocket message types beyond frame/window_state/config
  (e.g. touch event mirroring) — post-v1.1
- Plugin sandboxing / checksum verification — security hardening post-v1.0
