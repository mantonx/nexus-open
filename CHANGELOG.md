# Changelog

All notable changes to Nexus Open are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] — 2026-06-09

First public release. Nexus Open is a native Linux controller for the Corsair iCUE Nexus
(640×48) that was previously unsupported on Linux.

### Added

- **Hardware support** — reverse-engineered USB protocol for Corsair iCUE Nexus (VID `0x1b1c`, PID `0x1b8e`); background device watcher reconnects without restarting
- **Plugin system** — external Go binaries over net/RPC (hashicorp/go-plugin); crash restart, timeout, and error surface; per-zone configuration schema
- **Builtin plugins** — `clock` (digital, analog, date-first faces; 12/24h; blink colon), CPU temperature, GPU temperature (AMD, Intel, NVIDIA sysfs), CPU load, network throughput (↓/↑ sparklines), weather (open-meteo)
- **Renderer** — sparkline, bar, area, segmented, and combo graph types; design-token colour system; caption rendering for secondary metrics
- **Multi-page layouts** — spring-physics swipe transitions; page pre-render; touch event pipeline
- **Flutter settings UI** — dark-mode dashboard; live 640×48 hardware preview via WebSocket RGBA stream; onboarding flow; layout editor
- **REST API** — full HTTP API on `127.0.0.1:1985`; OpenAPI 3.0 spec at `/openapi.yaml`; health endpoint reflecting real device state
- **SQLite persistence** — layout and zone config stored in SQLite; YAML export; schema migrations
- **Systemd user service** — headless mode; PID file; single-instance enforcement
- **Desktop integration** — autostart entry, app launcher icon, KDE integration
- **USB permissions** — `--setup-udev` flag; cross-distro udev rule install
- **Packaging** — DEB, RPM, AppImage, Flatpak, Snap, AUR; fpm-based builder; distro install tests
- **Developer tooling** — overmind hot-reload environment (`make dev`); air for Go, watchexec + SIGUSR1 for Dart; `NEXUS_MOCK_DEVICE=1` for hardware-free development; golden frame regression tests; `make doctor` health check

### Changed

- Renamed *module* → *plugin* throughout the codebase and API surface
- Plugin IPC protocol switched from gRPC to net/RPC (smaller dependency, no protobuf codegen)
- Layout YAML is the sole source of truth for theme; removed global theme injection from settings
- Exec plugin paths resolved relative to `pluginsDir`, not working directory
- Removed legacy `display` and `instruments` packages from earlier prototype

### Fixed

- Race condition in `Sampler.recordFirstSample`
- Swipe dead zone and `Loading` flash on page change
- Page cache invalidation on theme update — swipe no longer shows stale colours
- Goroutine leak in `app.Run()` — context cancellation now always calls `Shutdown()`
- HID handle reliably released on shutdown; reconnects immediately on write failure
- GPU vendor cached at startup to eliminate per-frame file reads and flicker

[1.0.0]: https://github.com/mantonx/nexus-open/releases/tag/v1.0.0
