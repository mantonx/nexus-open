# Changelog

All notable changes to Nexus Open are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.3.4] - 2026-06-27

### Added

- Ui): show daemon build info in Device tab
- Ui): show app version in Device tab Software section
- Ui): show app version in navigation rail footer
- Ci): generate AI prose summary for release notes via GitHub Models

### Fixed

- Fix(dev): pass APP_VERSION dart-define in dev-ui target
- Fix(dev): force GDK_BACKEND=x11 for flutter run — avoids epoxy crash on Wayland

## [0.3.3] - 2026-06-27

### Fixed

- Fix(deps): update dependency geocoding to v4 (#34)
- Fix(deps): update dependency google_fonts to v8 (#35)
- Fix(deps): update dependency intl to ^0.20.0 (#22)
- Fix(deps): update module github.com/mantonx/nexus-open to v0.3.2
- Fix(renovate): remove invalid flutter key
- Fix(plugin): remove ineffectual mx assignment in SparkHistory.Normalized
- Fix(ci): generate release notes from tag range, not --unreleased

## [0.3.2] - 2026-06-27

### Fixed

- Fix(app): remove unused dirExists
- Fix(app): skip empty plugin dirs during resolution

### Performance

- Perf(pkg): build plugins once, stage copies for all package formats

## [0.3.1] - 2026-06-27

### Fixed

- Fix(pkg): POSIX-safe lifecycle scripts and comprehensive runtime tests
- Fix(pkg): reliable install/upgrade/uninstall lifecycle across all formats
- Fix(pkg): include plugins in release tarball and PKGBUILD install

## [0.3.0] - 2026-06-27

### Added

- Media): add media plugin source and tap-mock dev utility
- Media): add TMDb poster art lookup and Firefox MPRIS support
- Touch): improve touch handling
- Plugins): migrate to flat plugin layout with nexus- prefix

### Fixed

- Fix(test): replace deadline context with cancellable context in TestApp_Lifecycle
- Fix(lint): resolve golangci-lint failures
- Fix(zone): eliminate clock AM/PM blink caused by shared builtin instances
- Fix(pkg): restart service on upgrade, not just start
- Correct v0.2.0 PKGBUILD sha256; add workflow_dispatch to aur-publish
- Fix(ci): pass exact version to build-package.sh via RELEASE_VERSION
- Fix(ci): resolve version from tag ref, not git describe
- Fix(ci): prevent AUR publish race by never re-firing release:published
- Fix(ci): add --clobber to release step to allow overwriting manual pre-releases

## [0.2.0] - 2026-06-26

### Added

- Dog-ear affordance, tap ripple animation, and backend file splits
- Start service immediately on install for active sessions
- Enable service automatically via systemd user preset

### Fixed

- Claim USB interface before detaching kernel driver
- Enable service on install via post_install, bump pkgrel to 2
- Install udev rule via .install hook to avoid file conflicts
- Add .install file, correct dep names, remove invalid PKGBUILD hooks
- Correct Arch package name for libayatana-appindicator

## [0.0.1] - 2026-06-09

### Added

- Add AppImage packaging
- Ui): add design tokens, detail overlay widget, and settings polish
- Plugins): add graph types, captions, and testdata fixtures
- Plugin): extend payload API with caption, graph types, and validation
- Design): add hardware display design token system
- Debug): add GET /api/debug/frame endpoint to snapshot current display
- Editor): location field typeahead + map preview, live config updates
- Editor): fix zone colour picker — use accent for text, add weather plugin (#16)
- Ui): preview strip with swipe arrows, zone tap, and detail close highlight
- Zone tap, detail overlay cache, detail_state WS broadcast
- Security): bind to loopback, add capability token auth
- Tray lifecycle, perf optimisations, and stability fixes
- Ui): start maximized with 1280×800 minimum, minimize instead of hide
- Clock): analog face, multi-style config, live preview, and blink fix
- Settings UI overhaul, draft layout system, and device tab fixes
- Phase-2): zone model v2 — 6-zone cap + auto width redistribution
- Phase-1): plugin config schema contract + Configure interface
- Phase-0): consolidate plugin config into zones.config_json
- Fpm-based package builder + distro install tests (Ubuntu 22/24, Debian 12, Fedora 40, Arch)
- KDE desktop integration — single-instance, systemd service, icon set
- Resolve exec: plugin paths relative to pluginsDir, not CWD
- Desktop integration — autostart, app launcher, install script
- Add plugin reliability — validation, timeout, error surface, crash restart
- SQLite-first layout loading with YAML export
- Complete renderer overhaul + plugin rename + display design polish
- Complete swipe pipeline overhaul
- Broadcast every frame during transitions for full-resolution WS analysis
- Add -analyse flag to swipe-sim for frame-by-frame smoothness report
- Swipe simulation endpoint and CLI for transition tuning
- Reflect hardware state in health endpoint
- Background device watcher — connect without restarting the app
- --setup-udev flag and improved onboarding connect step
- Onboarding overlay redesign, VM screenshot extension, module rebuilds
- Onboarding overlay redesign, VM screenshot extension, module rebuilds
- Hardware preview, module layout fix, full tour coverage
- Expand tour coverage and fix form field styling
- Tighten layout, auto-scroll screenshots, self-cleaning tour
- Unify page headers and complete Display tab
- Visual polish pass — depth, instrument-panel aesthetic
- Integration test screenshot pipeline and dark mode persistence
- Flutter UI — design system, live preview, and component library
- Complete improvement plan — backend, CI, packaging, and docs
- Hardware preview, module layout fix, full tour coverage
- Expand tour coverage and fix form field styling
- Tighten layout, auto-scroll screenshots, self-cleaning tour
- Unify page headers and complete Display tab
- Visual polish pass — depth, instrument-panel aesthetic
- Integration test screenshot pipeline and dark mode persistence
- Flutter UI — design system, live preview, and component library
- Complete improvement plan — backend, CI, packaging, and docs
- Improve graph visibility and module reliability
- Enhance graph visibility and add dev workflow docs
- Implement graph type rendering (Part 2)
- Add configurable graph types for module visualizations (Part 1)
- Implement per-zone configuration system
- Migrate weather module to ConfigNotifier interface
- Implement event-driven config notification system for modules
- Implement annotation-based OpenAPI 3.0 spec generation
- Add proper OpenAPI 3.0 specification
- Add OpenAPI 3 specification (Phase 0)
- Add zone-test live device testing tool
- REFACTOR-Phase4): implement multi-page navigation and touch interactions
- REFACTOR-Phase3): migrate CPU, Network, and Weather modules
- Gpu-temp): add multi-vendor GPU support (AMD, Intel, generic sysfs)
- REFACTOR-Phase3): GPU temperature module complete
- Clock): add blinking colon for visual feedback
- REFACTOR-Phase2): implement plugin system with RPC
- REFACTOR-Phase1): implement core zone system for modular layout
- FL-013): implement system tray with window show/hide control
- V0.1.5): implement Font Awesome icons and display polish
- V0.1.5): implement TrueType font rendering system
- FL-001-012): Phase 4 - Flutter integration complete

### Changed

- Refactor(renderer): adopt design tokens and expand graph rendering
- Replace cgo+libusb with pure-Go usbfs implementation
- Rename module from nexus-next to nexus-open
- Decouple draft from zone manager; fix race condition and layout bugs
- Code organisation pass — dead code, duplication, package layout
- Rename module → plugin throughout codebase
- Remove legacy display and instruments packages
- Reorganize config packages for clarity
- Restructure project architecture and improve touch handling
- Refactor(API-001-007): Phase 3 - Modern API server with middleware
- Refactor(BP-005,BP-006,BP-009,BP-011): Phase 2 - Backend refactoring complete
- Refactor(BP-001,BP-003,BP-004): establish foundation with new structure
- Refactor(BP-002): remove Wails artifacts and clean up main.go

### Fixed

- Desktop file Actions key and HardwareSettings category
- Fix(app): ensure Run cleans up on context cancellation; fix test isolation
- Fix(gpu): eliminate flicker via vendor caching and TTL headroom
- Fix(touch): correct tap coordinates and derive detail action from plugin
- Fix(security): govulncheck in CI, harden systemd unit, drop config echo
- Fix(udev): tighten device permissions, clarify plugdev is headless-only
- Fix(security): plugin path allowlist, file confinement, body limits, systemd hardening
- Fix(lint): resolve errcheck and unused symbols flagged by golangci-lint
- Fix(ui): set window size to 1400x800
- Fix(makefile): install builds and deploys both backend and Flutter UI
- Fix(makefile): install to ~/.local/bin and restart systemd service
- Fix(renderer): scale fonts and layout to zone width
- Restore touch/swipe via libusb EP 0x81 + split C into nexus_usb.c/h
- Replace setup-git-cliff action with direct cargo install + cache
- Skip fpm check in --test-only mode (test jobs don't need fpm)
- Add always() to package-test if condition to prevent skip propagation
- Run package-test whenever package-build succeeds, regardless of skipped deps
- Install libarchive-tools (bsdtar) for pacman package build
- Persist GEM_HOME to GITHUB_ENV so fpm is findable in later steps
- Install systray deps in package-build job
- Resolve remaining 18 errcheck lint violations
- Resolve remaining 18 errcheck lint violations
- Eliminate data race in Sampler.recordFirstSample
- Resolve all 65 golangci-lint issues + flutter test failures
- Resolve all 136 flutter analyze warnings and infos
- Golangci-lint-action@v9 with latest (v2.x supports Go 1.25; v6+v1.65 don't exist)
- Run build_runner before flutter analyze/build (freezed/.g.dart files are gitignored)
- Pin golangci-lint to v1.65.0 (built with Go 1.25, required for go 1.25.x modules)
- Apt-get update before system dep install (mirror 404 on stale index)
- Use go-version 1.25.x in CI (go1.25.0 tag doesn't exist, patches do)
- Quote 'total:' in grep to avoid YAML parse error
- Resolve layout config path for installed packages + package test fixes
- Install.sh uses XDG_DATA_HOME for plugins, matching app resolution logic
- Four plugin reliability gaps found by audit + race detector
- Migration 4 — rewrite exec:./modules/ paths to exec:./plugins/
- Make migration 3 a no-op when zone_plugin_config already exists
- Remove global theme injection from settings — layout YAML is sole source of truth for theme
- Always render fresh frame on page switch, never use stale cache as transition target
- Pre-render adjacent pages using live renderers so ThemeOverride accents are preserved on swipe
- Invalidate page cache on UpdateTheme so swipe doesn't show stale colours
- Hardware resilience — touch reconnect, HID error logging, reconnect backoff (#14)
- Kill all nexus-open instances on startup, not just PID file entry
- Eliminate swipe jank and Loading flash properly
- Eliminate swipe dead zone and Loading flash on page change
- Smooth swipe transitions — eliminate double-easing and initializePage race
- PID file prevents stale nexus-open processes accumulating
- Release HID handle reliably on shutdown
- Eliminate need for manual replug on backend restart
- Reconnect device immediately on write failure
- Propagate theme updates to zone renderers on current page
- Theme changes now apply to every rendered frame on hardware
- Always open HID interface 0 for display writes
- Propagate Flutter UI settings to hardware renderer in real time
- Cross-distro udev setup for all major Linux distros
- Wire up WebSocket endpoint and add integration test suite
- Theme-aware colours throughout — no more dark-only hardcoding
- Wire up WebSocket endpoint and add integration test suite
- Theme-aware colours throughout — no more dark-only hardcoding
- Fix(touch): improve gesture detection with press/release tracking

### Performance

- Skip USB send when frame is unchanged, remove stale chunk constants
- Skip expensive jobs on unrelated changes via paths-filter + build_runner cache
- Cache pub deps and fpm gem in CI to reduce build times

[0.3.4]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.4
[0.3.3]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.3
[0.3.2]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.2
[0.3.1]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.1
[0.3.0]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.0
[0.2.0]: https://github.com/mantonx/nexus-open/releases/tag/v0.2.0
[0.0.1]: https://github.com/mantonx/nexus-open/releases/tag/v0.0.1

