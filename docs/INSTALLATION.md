# Installation

## Package install (recommended)

Download the latest release from the [GitHub releases page](https://github.com/mantonx/nexus-open/releases/latest).

| Format         | Distro                                               |
| -------------- | ---------------------------------------------------- |
| `.pkg.tar.zst` | Arch Linux (or install via AUR: `yay -S nexus-open`) |
| `.deb`         | Ubuntu 24.04+, Debian 13+                            |
| `.rpm`         | Fedora 40+, RHEL 9+                                  |
| `.tar.gz`      | Any distro - static binary, no runtime dependencies  |

After installing, unplug and replug the Nexus. The bundled udev rule
(`99-corsair-nexus.rules`) grants access automatically on any
`systemd-logind` desktop without adding users to a group.

If auto-access doesn't work, see [DEVICE_SETUP.md](../DEVICE_SETUP.md).

## Build from source

### Prerequisites

- Go 1.26+
- Flutter 3.24+ (only needed for the settings UI)
- Git

The Go backend is pure Go (`CGO_ENABLED=0`) — no C libraries, no pkg-config.

**Arch Linux:**

```bash
sudo pacman -S go git
```

**Debian/Ubuntu:**

```bash
sudo apt install git
# Install Go from go.dev/dl — the distro package is too old
curl -sSL https://go.dev/dl/go1.26.0.linux-amd64.tar.gz | sudo tar -C /usr/local -xz
echo 'export PATH=/usr/local/go/bin:$PATH' >> ~/.bashrc && source ~/.bashrc
```

**Fedora:**

```bash
sudo dnf install golang git
```

### Build and install

```bash
git clone https://github.com/mantonx/nexus-open.git
cd nexus-open

# Build everything and install system-wide (same layout as the distro packages)
make install
```

`make install` builds the Go backend, Flutter UI, and all plugins, then installs
them to `/usr/bin`, `/usr/lib/nexus-open/`, and `/usr/lib/systemd/user/` using
`sudo`. The service is enabled and started automatically.

Unplug and replug the Nexus after installing so the udev rule takes effect.

## Running as a service

```bash
# Enable and start (done automatically by make install)
systemctl --user enable --now nexus-open.service

# View logs
journalctl --user -u nexus-open.service -f

# Stop
systemctl --user stop nexus-open.service
```

## Configuration

Settings are stored in SQLite at `~/.local/share/nexus-open/nexus-open.db`
and managed through the Flutter UI or the REST API (`http://localhost:1985/api/config`).

The layout is a YAML file, default at
`~/.local/share/nexus-open/layouts/multi-page.yaml`.
On first run, the built-in default layout is written there automatically.

## Uninstall

**Arch (pacman/AUR):**

```bash
sudo pacman -R nexus-open
```

**Debian/Ubuntu:**

```bash
sudo apt remove nexus-open
```

**Fedora:**

```bash
sudo dnf remove nexus-open
```

**Built from source:**

```bash
make uninstall
```
