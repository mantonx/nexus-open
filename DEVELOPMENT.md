# Development Guide

## Prerequisites

- Go 1.25+
- Flutter 3.24+

Two additional tools are required for the hot-reload workflow. Run `make setup` to install them:

- **air** — Go live reload (`go install github.com/air-verse/air@latest`)
- **overmind** — process manager (`sudo pacman -S overmind` / `sudo apt install overmind`)
- **watchexec** — Dart file watcher (`sudo pacman -S watchexec` / `cargo install watchexec-cli`)

## Starting the Dev Environment

```bash
make dev
```

This starts three processes under overmind:

| Process | What it does |
| --- | --- |
| `backend` | air watching `cmd/`, `internal/`, `pkg/`, `plugins/` — rebuilds the daemon on any `.go` save (~2–4 s) |
| `ui` | `flutter run -d linux` — Flutter app in debug mode, PID written to `/tmp/nexus-flutter.pid` |
| `ui-reload` | watchexec watching `ui/lib/` — sends `SIGUSR1` to flutter run on any `.dart` save (hot reload, <1 s) |

Attach to a process to see its logs:

```bash
overmind connect backend    # detach with Ctrl-B D
overmind connect ui
```

Restart one process without stopping the others:

```bash
overmind restart backend
```

Stop everything:

```bash
overmind quit
```

## Mock Device Mode

Develop without the physical hardware:

```bash
NEXUS_MOCK_DEVICE=1 make dev
```

The mock device accepts all frame data, returns a canned firmware version, and simulates the USB lifecycle without needing the Corsair iCUE Nexus plugged in.

## Hot Reload Notes

**Go changes** — air rebuilds automatically. The pre-build script (`scripts/build-changed-plugins.sh`) rebuilds only plugin binaries whose source is newer than their output, so a Go-only change in `internal/` doesn't rebuild all plugins.

**Dart changes** — watchexec sends `SIGUSR1` to `flutter run`, which hot-reloads in under a second. State is preserved. For changes that add new imports or types, a hot-restart is needed:

```bash
kill -USR2 $(cat /tmp/nexus-flutter.pid)
```

## Environment Variables

| Variable | Default | Description |
| --- | --- | --- |
| `NEXUS_MOCK_DEVICE` | `0` | Set to `1` to skip USB and use the in-process mock device |
| `NEXUS_DEBUG` | `0` | Set to `1` for verbose structured log output |
| `NEXUS_PLUGINS_DIR` | auto | Override the plugin binary directory (default: `~/.local/share/nexus-open/plugins`) |

## Running Individual Components

```bash
# Backend with air only (no UI)
make dev-backend

# Flutter UI only (backend must already be running)
make dev-ui

# Dart file watcher only (alongside an already-running dev-ui)
make dev-ui-reload
```

## Building

```bash
make build          # Dev binary with debug info → bin/nexus-open
make build-release  # Stripped release binary
make build-ui       # Flutter release bundle → bin/nexus-open-ui-bundle/
make build-plugins  # All external plugin binaries
make build-all      # All of the above
```

## Testing

```bash
make test           # go test ./...
make test-race      # go test -race ./...
make coverage       # Coverage report → coverage.html
```

The test suite uses `NEXUS_MOCK_DEVICE=1` and `WithPluginsDir(t.TempDir())` internally for lifecycle tests so they don't touch installed binaries or require hardware.

Golden frame tests live in `test/fixture/`. Update golden PNGs after intentional renderer changes:

```bash
go test ./test/fixture/ -update
```

## Install / Full Deploy

Use `make install` when you need to update the installed service binary, UI bundle, and plugins together — for example after changing the layout YAML or adding a new plugin for the first time:

```bash
make install   # builds everything, stops service, installs, restarts
```

For day-to-day code changes, `make dev` is faster and doesn't touch the installed service.

## Health Check

```bash
make doctor
```

Checks that the daemon is running, the API is reachable, the token is present, the USB device is detected, and all dev tools are installed.

## USB Permissions (Hardware)

```bash
sudo nexus-open --setup-udev
sudo usermod -a -G plugdev $USER
# Log out and back in
```

See [DEVICE_SETUP.md](DEVICE_SETUP.md) for per-distro details.

## Project Structure

```text
nexus-open/
├── cmd/nexus-open/         # Entry point, CLI flags
├── internal/
│   ├── app/                # Lifecycle orchestration, dependency wiring
│   ├── api/                # HTTP server, WebSocket hub, route handlers
│   ├── device/             # USB HID (real + mock)
│   ├── zone/               # Layout config, renderer, sampler, transitions
│   ├── plugins/            # Plugin host (go-plugin/net-RPC) + builtins
│   ├── store/              # SQLite (settings, layout, zone configs)
│   ├── settings/           # User settings manager
│   ├── touch/              # Touch event reader + handler
│   ├── tray/               # System tray
│   └── design/             # Hardware display tokens (generated from design/)
├── pkg/plugin/             # Public plugin interface — types, errors, protocol
├── plugins/                # External plugin source trees
├── design/                 # Style Dictionary token pipeline (tokens.json → Go + Dart)
├── ui/                     # Flutter application
│   └── lib/src/
│       ├── theme/          # App tokens + NexusColors hardware tokens
│       ├── widgets/        # Settings page, display preview, zone widgets
│       └── services/       # API client, WebSocket, settings state
├── configs/layouts/        # Layout YAML files loaded on first run
├── packaging/              # DEB, RPM, AppImage, Flatpak, Snap, AUR, systemd
├── scripts/                # Build helpers, udev setup
├── testdata/               # Golden PNGs + payload JSON fixtures
└── test/fixture/           # Golden render regression tests
```
