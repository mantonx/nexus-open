# Changelog

All notable changes to Nexus Open are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.3.7] - 2026-06-28

### Fixed

- Improve process lifecycle and connection UX
- Kill orphaned UI processes before starting Flutter

## [0.3.6] - 2026-06-28

### Fixed

- Include Flutter UI bundle in AUR tarball and PKGBUILD
- Include Flutter UI bundle in AUR tarball and PKGBUILD
- Grant models: read permission for AI release summary

## [0.3.5] - 2026-06-27

### Fixed

- Stamp buildTime ldflag in build-package.sh
- Polish credibility issues
- Harden USB write/close lifecycle

## [0.3.4] - 2026-06-27

### Added

- Show daemon build info in Device tab
- Show app version in Device tab Software section
- Show app version in navigation rail footer
- Generate AI prose summary for release notes via GitHub Models

### Fixed

- Pass APP_VERSION dart-define in dev-ui target
- Force GDK_BACKEND=x11 for flutter run — avoids epoxy crash on Wayland

## [0.3.3] - 2026-06-27

### Fixed

- Update dependency geocoding to v4 (#34)
- Update dependency google_fonts to v8 (#35)
- Update dependency intl to ^0.20.0 (#22)
- Update module github.com/mantonx/nexus-open to v0.3.2
- Remove invalid flutter key
- Remove ineffectual mx assignment in SparkHistory.Normalized
- Generate release notes from tag range, not --unreleased

## [0.3.2] - 2026-06-27

### Fixed

- Remove unused dirExists
- Skip empty plugin dirs during resolution

### Performance

- Build plugins once, stage copies for all package formats

## [0.3.1] - 2026-06-27

### Fixed

- POSIX-safe lifecycle scripts and comprehensive runtime tests
- Reliable install/upgrade/uninstall lifecycle across all formats
- Include plugins in release tarball and PKGBUILD install

## [0.3.0] - 2026-06-27

### Added

- Add media plugin source and tap-mock dev utility
- Add TMDb poster art lookup and Firefox MPRIS support
- Improve touch handling
- Migrate to flat plugin layout with nexus- prefix

### Fixed

- Replace deadline context with cancellable context in TestApp_Lifecycle
- Resolve golangci-lint failures
- Eliminate clock AM/PM blink caused by shared builtin instances
- Restart service on upgrade, not just start
- Correct v0.2.0 PKGBUILD sha256; add workflow_dispatch to aur-publish
- Pass exact version to build-package.sh via RELEASE_VERSION
- Resolve version from tag ref, not git describe
- Prevent AUR publish race by never re-firing release:published
- Add --clobber to release step to allow overwriting manual pre-releases

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
- Add design tokens, detail overlay widget, and settings polish
- Add graph types, captions, and testdata fixtures
- Extend payload API with caption, graph types, and validation
- Add hardware display design token system
- Add GET /api/debug/frame endpoint to snapshot current display
- Location field typeahead + map preview, live config updates
- Fix zone colour picker — use accent for text, add weather plugin (#16)
- Preview strip with swipe arrows, zone tap, and detail close highlight
- Zone tap, detail overlay cache, detail_state WS broadcast
- Bind to loopback, add capability token auth
- Tray lifecycle, perf optimisations, and stability fixes
- Start maximized with 1280×800 minimum, minimize instead of hide
- Analog face, multi-style config, live preview, and blink fix
- Settings UI overhaul, draft layout system, and device tab fixes
- Zone model v2 — 6-zone cap + auto width redistribution
- Plugin config schema contract + Configure interface
- Consolidate plugin config into zones.config_json
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
- Implement multi-page navigation and touch interactions
- Migrate CPU, Network, and Weather modules
- Add multi-vendor GPU support (AMD, Intel, generic sysfs)
- GPU temperature module complete
- Add blinking colon for visual feedback
- Implement plugin system with RPC
- Implement core zone system for modular layout
- Implement system tray with window show/hide control
- Implement Font Awesome icons and display polish
- Implement TrueType font rendering system
- Phase 4 - Flutter integration complete

### Changed

- Adopt design tokens and expand graph rendering
- Replace cgo+libusb with pure-Go usbfs implementation
- Rename module from nexus-next to nexus-open
- Decouple draft from zone manager; fix race condition and layout bugs
- Code organisation pass — dead code, duplication, package layout
- Rename module → plugin throughout codebase
- Remove legacy display and instruments packages
- Reorganize config packages for clarity
- Restructure project architecture and improve touch handling
- Phase 3 - Modern API server with middleware
- Phase 2 - Backend refactoring complete
- Establish foundation with new structure
- Remove Wails artifacts and clean up main.go

### Fixed

- Desktop file Actions key and HardwareSettings category
- Ensure Run cleans up on context cancellation; fix test isolation
- Eliminate flicker via vendor caching and TTL headroom
- Correct tap coordinates and derive detail action from plugin
- Govulncheck in CI, harden systemd unit, drop config echo
- Tighten device permissions, clarify plugdev is headless-only
- Plugin path allowlist, file confinement, body limits, systemd hardening
- Resolve errcheck and unused symbols flagged by golangci-lint
- Set window size to 1400x800
- Install builds and deploys both backend and Flutter UI
- Install to ~/.local/bin and restart systemd service
- Scale fonts and layout to zone width
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
- Improve gesture detection with press/release tracking

### Performance

- Skip USB send when frame is unchanged, remove stale chunk constants
- Skip expensive jobs on unrelated changes via paths-filter + build_runner cache
- Cache pub deps and fpm gem in CI to reduce build times

[0.3.7]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.7
[0.3.6]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.6
[0.3.5]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.5
[0.3.4]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.4
[0.3.3]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.3
[0.3.2]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.2
[0.3.1]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.1
[0.3.0]: https://github.com/mantonx/nexus-open/releases/tag/v0.3.0
[0.2.0]: https://github.com/mantonx/nexus-open/releases/tag/v0.2.0
[0.0.1]: https://github.com/mantonx/nexus-open/releases/tag/v0.0.1

