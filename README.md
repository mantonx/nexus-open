# Nexus Open

Linux controller for the Corsair iCUE Nexus companion display — a 640×48 pixel
LCD strip that sits above your keyboard. Corsair doesn't support Linux; this
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

## What it does

Shows live stats — CPU/GPU temperature and load, network throughput, weather —
on the physical display strip, with a touch surface for switching pages. A
Flutter settings app lets you rearrange zones and change plugins without
restarting anything. The live preview in the settings app is the exact bytes
going to the hardware, so what you see is what you get.

The daemon runs headless as a systemd user service and talks directly to the
device via Linux usbfs — no libusb, no CGo, no shared libraries required.
Plugins are separate Go binaries; adding a data source is dropping a binary and
editing a YAML file.

## Install

Download from the [releases page](https://github.com/mantonx/nexus-open/releases/latest):

| Format         | Distro                                                 |
| -------------- | ------------------------------------------------------ |
| `.pkg.tar.zst` | Arch Linux (or `yay -S nexus-open`)                    |
| `.deb`         | Ubuntu 24.04+, Debian 13+                              |
| `.rpm`         | Fedora 40+, RHEL 9+                                    |
| `.tar.gz`      | Any distro — static binary, no runtime dependencies    |

After installing, unplug and replug the Nexus. The bundled udev rule grants
access automatically without adding your user to any group.

See [docs/INSTALLATION.md](docs/INSTALLATION.md) for full details and build-from-source instructions.

## Known limitations

The firmware has no command to restore the iCUE boot screen on exit — the daemon
sends a black frame when it stops. Touching the device afterwards triggers a USB
reset; unplug and replug to reconnect.

Reconnecting too soon after a disconnect causes repeated failed opens. The daemon
waits automatically, but if you get stuck, a physical replug fixes it.

The Flutter settings UI requires XWayland. The packaged binary handles this
automatically (tested on GNOME, KDE, Sway, Hyprland). The daemon itself is
unaffected by display server.

Tested distros: Arch Linux (primary), Ubuntu 24.04, Fedora 40. The daemon has
no glibc or GTK dependency; the Flutter UI requires GTK 3.24+.

x86-64 only. The usbfs ioctl numbers in the driver are amd64-specific.

Not affiliated with Corsair. Protocol was reverse-engineered from usbmon captures.

## Building from source

Requires Go 1.26+ and Flutter 3.24+. No C libraries or pkg-config needed.

```bash
git clone https://github.com/mantonx/nexus-open.git
cd nexus-open
make setup   # installs air, overmind, watchexec
make dev     # Go + Flutter with hot-reload
```

See [DEVELOPMENT.md](DEVELOPMENT.md) for the full workflow, or [docs/INSTALLATION.md](docs/INSTALLATION.md) for a production build.

## More

- [docs/INSTALLATION.md](docs/INSTALLATION.md) — install, build from source, configuration
- [DEVICE_SETUP.md](DEVICE_SETUP.md) — USB permissions and troubleshooting
- [DEVELOPMENT.md](DEVELOPMENT.md) — architecture, hot-reload workflow, testing
- [CONTRIBUTING.md](CONTRIBUTING.md) — writing plugins, submitting PRs

## License

MIT — see [LICENSE](LICENSE).
