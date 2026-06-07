# Changelog

All notable changes to Nexus Open are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

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

- Skip expensive jobs on unrelated changes via paths-filter + build_runner cache
- Cache pub deps and fpm gem in CI to reduce build times


