# Changelog

All notable changes to Nexus Open are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-06-07

First public release. Complete rewrite of the original Wails prototype into a
production Go daemon with a Flutter Linux UI.

### Added

- Go daemon (`nexus-open`) with USB HID communication to Corsair iCUE Nexus
- Mock device mode (`NEXUS_MOCK_DEVICE=1`) for development without hardware
- REST API on port 1985: health, config, device info, zone management, animations
- Zone-based display rendering with plugin system for live data (CPU, GPU, weather, network)
- TrueType font rendering pipeline for zone text
- Swipe gesture handling with touch input via evdev
- Flutter Linux UI: Preview, Layout, Display, Location, Plugins, Images tabs
- System tray integration (`--tray` flag)
- Systemd user service (`nexus-open.service`)
- udev rules for unprivileged USB access (`99-corsair-nexus.rules`)
- `.desktop` file for application launcher integration
- Installable packages for Debian/Ubuntu (`.deb`), Fedora/RHEL (`.rpm`), and Arch Linux (`.pkg.tar.zst`)
- CI pipeline: lint, race-detector tests, Flutter analyze/test, OpenAPI drift check
- Runtime installation tests on Ubuntu 24.04, Fedora 40, and Arch Linux via Docker
- Version derived from git tag across Go binary, Flutter UI, and package filenames

### Changed

- Replaced Wails frontend with Flutter Linux (native, no Electron/WebView)
- Replaced ad-hoc USB polling with structured evdev touch handling

[0.1.0]: https://github.com/mantonx/nexus-open/releases/tag/v0.1.0
