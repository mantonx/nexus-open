# Snap Packaging for Nexus Open

This directory contains the Snap configuration for building and distributing Nexus Open.

## Why Snap?

Snaps are:
- Popular on Ubuntu and Ubuntu-based distributions
- Easy to install and auto-update
- Sandboxed for security
- Available on many Linux distributions via snapd

## Prerequisites

```bash
# Install snapd
sudo apt install snapd        # Ubuntu/Debian
sudo pacman -S snapd          # Arch Linux
sudo dnf install snapd        # Fedora

# Install snapcraft (build tool)
sudo snap install snapcraft --classic

# Install LXD for building (recommended)
sudo snap install lxd
sudo lxd init --auto
sudo usermod -a -G lxd $USER
# Log out and back in
```

## Building the Snap

### Option 1: Build with LXD (Recommended)

```bash
cd /path/to/nexus-open

# Build using LXD container (clean environment)
snapcraft --use-lxd

# Result: nexus-open_1.0.0_amd64.snap
```

### Option 2: Build Locally

```bash
cd /path/to/nexus-open

# Build directly on host
snapcraft

# Result: nexus-open_1.0.0_amd64.snap
```

### Option 3: Remote Build (Launchpad)

```bash
# Login to Snapcraft
snapcraft login

# Push to Launchpad for building
snapcraft remote-build

# This builds for multiple architectures (amd64, arm64, etc.)
```

## Installing the Snap

### Install Locally

```bash
# Install the built snap
sudo snap install nexus-open_1.0.0_amd64.snap --dangerous

# OR if you want devmode (less restricted)
sudo snap install nexus-open_1.0.0_amd64.snap --dangerous --devmode
```

### Install from Snap Store (After Publishing)

```bash
sudo snap install nexus-open
```

## Connecting Interfaces

Snaps require explicit permission to access hardware:

```bash
# Required: USB device access
sudo snap connect nexus-open:raw-usb

# Verify connections
snap connections nexus-open
```

### Interface Details

The snap uses these interfaces:
- `network` - For weather API calls (auto-connected)
- `network-bind` - For REST API server (auto-connected)
- `raw-usb` - For iCUE Nexus USB device (manual connection required)
- `hardware-observe` - For CPU/GPU temperature (auto-connected)
- `network-observe` - For network statistics (auto-connected)
- `system-observe` - For system monitoring (auto-connected)

## Using the Snap

### As a Daemon (Recommended)

The snap runs as a daemon by default:

```bash
# Start the service
sudo snap start nexus-open

# Stop the service
sudo snap stop nexus-open

# Restart the service
sudo snap restart nexus-open

# Check status
snap services nexus-open

# View logs
sudo snap logs nexus-open -f
```

### As a Command

```bash
# Run directly (stops daemon first)
sudo snap stop nexus-open
nexus-open.cli

# Check configuration location
echo $SNAP_USER_COMMON
# Config is at: ~/snap/nexus-open/common/.config/nexus-open/config.yaml
```

## Configuration

The snap stores configuration in:
```
~/snap/nexus-open/common/.config/nexus-open/config.yaml
```

You can edit this file directly or use the REST API on http://localhost:1985

## Testing the Snap

```bash
# Install the snap
sudo snap install --dangerous nexus-open_1.0.0_amd64.snap

# Connect USB interface
sudo snap connect nexus-open:raw-usb

# Check if device is visible
lsusb | grep 1b1c:1b8e

# Start the service
sudo snap start nexus-open

# Check logs
sudo snap logs nexus-open

# Test the API
curl http://localhost:1985/api/health

# Check configuration
cat ~/snap/nexus-open/common/.config/nexus-open/config.yaml
```

## Publishing to Snap Store

### 1. Register the Name

```bash
snapcraft login
snapcraft register nexus-open
```

### 2. Build and Upload

```bash
# Build all architectures
snapcraft remote-build

# Upload to store (edge channel)
snapcraft upload nexus-open_1.0.0_amd64.snap --release=edge

# Promote to stable after testing
snapcraft release nexus-open 1 stable
```

### 3. Store Listing

Update the store listing at https://snapcraft.io/nexus-open/listing:
- Description
- Screenshots
- Categories: utilities, system
- Website and contact info

## Troubleshooting

### USB Device Not Found

```bash
# Check if raw-usb is connected
snap connections nexus-open | grep raw-usb

# Connect if not connected
sudo snap connect nexus-open:raw-usb

# Verify device is visible
lsusb | grep 1b1c:1b8e

# Check snap logs
sudo snap logs nexus-open -f
```

### Service Won't Start

```bash
# Check service status
snap services nexus-open

# View detailed logs
sudo journalctl -u snap.nexus-open.nexus-open.service -f

# Try running manually to see errors
sudo snap stop nexus-open
nexus-open.cli
```

### Configuration Not Saving

```bash
# Check file permissions
ls -la ~/snap/nexus-open/common/.config/nexus-open/

# Verify snap has access
snap interfaces nexus-open
```

### Permission Denied Errors

If running in strict confinement:

```bash
# Option 1: Connect required interfaces
sudo snap connect nexus-open:raw-usb
sudo snap connect nexus-open:hardware-observe

# Option 2: Install in devmode (not recommended for production)
sudo snap install --devmode nexus-open_1.0.0_amd64.snap
```

## Development Tips

### Quick Iteration

```bash
# Clean build artifacts
snapcraft clean

# Rebuild just your part
snapcraft build nexus-open

# Test without reinstalling
snapcraft prime
sudo snap try prime/ --devmode
```

### Debug Shell

```bash
# Enter snap environment
snap run --shell nexus-open.cli

# Check environment variables
env | grep SNAP

# Test commands
$SNAP/bin/nexus-open
```

## Snap vs Other Formats

| Feature | Snap | Flatpak | AppImage | DEB |
|---------|------|---------|----------|-----|
| Sandboxing | ✅ Strong | ✅ Strong | ❌ None | ❌ None |
| Auto-updates | ✅ Yes | ✅ Yes | ❌ No | ⚠️ Via apt |
| USB Access | ⚠️ Manual | ⚠️ Manual | ✅ Easy | ✅ Easy |
| Size | ~15 MB | ~15 MB | ~10 MB | ~9 MB |
| Speed | Fast | Fast | Fastest | Fastest |
| Distro Support | Many | Many | All | Debian/Ubuntu |

### When to Use Snap

✅ Good for:
- Ubuntu and Ubuntu-based distributions
- Users wanting auto-updates
- Store distribution via Snap Store
- Sandboxed execution

❌ Limitations:
- Requires manual interface connections for USB
- Larger package size (includes runtime)
- Slower first launch (due to squashfs mount)

## Files

- `snapcraft.yaml` - Snap manifest
- `README.md` - This file

## Resources

- [Snapcraft Documentation](https://snapcraft.io/docs)
- [Snap Store](https://snapcraft.io/store)
- [Snapcraft Forum](https://forum.snapcraft.io/)
- [Go Plugin Guide](https://snapcraft.io/docs/go-plugin)
