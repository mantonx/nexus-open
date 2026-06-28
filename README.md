# Nexus Open

Linux controller for the Corsair iCUE Nexus companion display — a 640×48 pixel
LCD strip with a resistive touch surface. Corsair doesn't support Linux; this
project reverse-engineered the USB protocol from usbmon captures.

[![Release](https://img.shields.io/github/v/release/mantonx/nexus-open)](https://github.com/mantonx/nexus-open/releases/latest)
[![AUR](https://img.shields.io/aur/version/nexus-open)](https://aur.archlinux.org/packages/nexus-open)
![Go](https://img.shields.io/badge/go-1.26+-00ADD8)
![Flutter](https://img.shields.io/badge/flutter-3.24+-02569B)
![Platform](https://img.shields.io/badge/platform-Linux-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Screenshots

**Hardware display** — clock, weather, CPU, GPU temp, and network on the 640×48 strip:

![640×48 hardware display showing clock, weather, CPU, GPU, and network zones](docs/screenshots/display.png)

**Settings UI** — layout editor with live hardware preview at the top:

![Flutter settings UI with layout editor and live hardware preview strip](docs/screenshots/settings-ui.png)

## How it works

The Go daemon talks to the device directly via Linux usbfs ioctls
(`/dev/bus/usb`) — no libusb, no CGo, no shared libraries. Each frame is split
into 121 × 1024-byte packets with an 8-byte header and an RGBA→BGRA swap, then
written to EP 0x02 OUT. Touch events come back on EP 0x81 IN as HID reports.
The full protocol is in [docs/USB_PROTOCOL.md](docs/USB_PROTOCOL.md).

Each display zone runs a separate Go binary over
[go-plugin](https://github.com/hashicorp/go-plugin) (net/RPC). The daemon gives
each sample call a deadline, shows a timeout payload if the plugin hangs, and
restarts it with exponential backoff if it crashes. Adding a data source is a
dropped binary and a line in the layout YAML — no daemon restart needed.

The Flutter settings UI gets the live 640×48 RGBA framebuffer over WebSocket.
There's no separate preview renderer; the preview is the exact bytes going to
the hardware.

Every release builds `.deb`, `.rpm`, `.pkg.tar.zst`, and `.tar.gz`, then CI
installs each into a matching Docker container (Ubuntu, Fedora, Arch) and runs
the daemon against a mock device to confirm the binary, udev rule, and systemd
unit all work. Other CI gates: race detector, `govulncheck`, OpenAPI drift,
sqlc drift, and Flutter analyze + test.

## Architecture

```text
┌─────────────────┐   WebSocket (raw RGBA frames)   ┌──────────────────┐
│   Flutter UI    │ ◄───────────────────────────────  │   Go daemon      │
│  (settings,     │                                   │                  │
│   live preview) │ ──── REST API :1985 ────────────► │  usbfs driver    │
└─────────────────┘                                   └────────┬─────────┘
                                                               │ go-plugin (net/RPC)
                                              ┌────────────────▼──────────────────┐
                                              │  Plugin subprocesses (one per zone) │
                                              │  cpu-temp  cpu-load  gpu-temp      │
                                              │  gpu-load  network   weather        │
                                              └─────────────────────────────────────┘
```

## Features

- Live CPU/GPU temperature, load, and network throughput
- Weather via open-meteo (configurable location and units)
- Per-zone graph types: sparkline, bar, area, segmented, combo
- Multi-page layouts with spring-physics swipe transitions
- Flutter settings UI with live 640×48 hardware preview
- Layout editor — change zones and plugins without restarting
- REST API with OpenAPI 3.0 spec
- Plugin system — drop a Go binary, reference it in YAML
- Runs headless as a systemd user service

## Quick Start

### Prerequisites

- Go 1.26+
- Flutter 3.24+
- Corsair iCUE Nexus (USB VID `0x1b1c`, PID `0x1b8e`)

### Run from source

```bash
git clone https://github.com/mantonx/nexus-open.git
cd nexus-open

# Install dev tools (air, overmind, watchexec)
make setup

# One-time: install udev rules
sudo nexus-open --setup-udev

# Start Go + Flutter with hot-reload
make dev
```

Go files rebuild in ~3 s; Dart files hot-reload in under a second. To run
without hardware:

```bash
NEXUS_MOCK_DEVICE=1 make dev
```

### Install from package

Download from the [releases page](https://github.com/mantonx/nexus-open/releases/latest).

| Format         | Distro                                                 |
| -------------- | ------------------------------------------------------ |
| `.deb`         | Debian, Ubuntu 24.04+                                  |
| `.rpm`         | Fedora, RHEL, openSUSE                                 |
| `.pkg.tar.zst` | Arch Linux                                             |
| `.tar.gz`      | Manual / headless (static binary + udev rule)          |

Unplug and replug the Nexus after installing — the bundled udev rule grants
access automatically without group membership.

See [docs/INSTALLATION.md](docs/INSTALLATION.md) for details.

## Build Commands

```bash
make build          # Dev binary
make build-release  # Stripped release binary
make build-ui       # Flutter UI
make build-plugins  # All plugin binaries
make build-all      # Everything

make test           # Tests
make test-race      # Tests with race detector
make coverage       # Coverage report

make dev            # Go + Flutter hot-reload
make dev-backend    # Go hot-reload only
make dev-ui         # Flutter only

make install        # Build + install to ~/.local/bin, restart service
make doctor         # Check runtime health and toolchain

make deb            # DEB package
make rpm            # RPM package
```

## Project Structure

```text
nexus-open/
├── cmd/nexus-open/         # Entry point and CLI flags
├── internal/
│   ├── app/                # Dependency wiring and lifecycle
│   ├── api/                # REST API and WebSocket hub
│   ├── device/             # Pure-Go usbfs driver (real + mock)
│   ├── zone/               # Layout, renderer, sampler, transitions
│   ├── plugins/            # Plugin host and builtin plugins
│   ├── store/              # SQLite (settings, layout, zone configs)
│   ├── settings/           # Settings manager
│   ├── touch/              # Touch reader and gesture handler
│   └── tray/               # System tray
├── pkg/plugin/             # Public plugin interface
├── plugins/
│   ├── cpu-temp/
│   ├── cpu-load/
│   ├── gpu-temp/           # AMD, Intel, NVIDIA
│   ├── gpu-load/
│   ├── network/            # ↓/↑ throughput
│   ├── weather/            # open-meteo
│   └── hello/              # Minimal example
├── ui/                     # Flutter app
├── configs/layouts/        # Layout YAML files
├── packaging/              # DEB, RPM, AUR PKGBUILD
└── docs/
```

## Writing a Plugin

Drop a Go binary anywhere, then reference it in a layout YAML with `exec:`.
The interface:

```go
type Plugin interface {
    Describe() (Descriptor, error)
    Sample()   (Payload, error)
    Configure(cfg map[string]any) error
}
```

See `plugins/hello/main.go` for a working example.

## REST API

Listens on `127.0.0.1:1985`. Full spec at `/openapi.yaml` or `api/openapi.yaml`.

| Endpoint                     | Description                                 |
| ---------------------------- | ------------------------------------------- |
| `GET /api/health`            | Health check and device status              |
| `GET /api/config`            | Settings                                    |
| `POST /api/config`           | Update settings                             |
| `GET /api/layout`            | Current layout (pages + zones)              |
| `GET /api/plugins`           | Plugin catalog with per-zone status         |
| `GET /api/zones/{id}/status` | Zone health and last error                  |
| `GET /api/device/info`       | Connection state                            |
| `GET /api/ws`                | WebSocket — live 640×48 RGBA frames         |

## Troubleshooting

**Device not found** — `lsusb | grep 1b1c` to confirm it's visible. If it is
but the daemon can't open it, run `make doctor` to check USB permissions.

**Permission denied** — reinstall the udev rule:

```bash
sudo nexus-open --setup-udev
# Log out and back in
```

See [DEVICE_SETUP.md](DEVICE_SETUP.md) for per-distro instructions.

**Port 1985 in use** — `ss -tlnp | grep 1985`, or use `nexus-open --port 1986`.

**Plugin shows blank** — check `GET /api/zones/{id}/status` for the error.

## Known Limitations

**No "release to native" command.** On exit, the daemon sends a black frame
but the firmware has no command to restore the iCUE boot screen. Touching the
device after shutdown triggers a firmware USB reset — unplug and replug to
reconnect.

**Reconnect needs a settle delay.** The firmware takes ~2 seconds to reset its
USB state after a disconnect. Reconnecting sooner causes repeated failed opens
that require a physical replug. The reconnect loop waits automatically.

**Settings UI requires XWayland.** The Flutter GTK3 embedder crashes on native
Wayland. The packaged binary wraps the Flutter binary to unset `WAYLAND_DISPLAY`,
so XWayland is used automatically. Tested compositors: GNOME, KDE Plasma, Sway,
Hyprland. The daemon and hardware display are unaffected by display server.

**Tested distros:** Arch Linux (primary), Ubuntu 24.04, Fedora 40. Older
distros with glibc < 2.34 or GTK < 3.24 may have issues with the Flutter UI;
the daemon has no such dependency.

**x86-64 only.** The usbfs ioctl numbers in `internal/device/usbfs.go` are
amd64-specific. Other architectures would need verified values.

**Not affiliated with Corsair.** iCUE Nexus is a Corsair trademark. The
protocol was reverse-engineered from usbmon captures with no reference
implementation.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [DEVELOPMENT.md](DEVELOPMENT.md).

## License

MIT — see [LICENSE](LICENSE).
