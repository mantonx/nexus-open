# Nexus Open

Open-source Linux controller for the Corsair iCUE Nexus display device.

![Status](https://img.shields.io/badge/status-in%20development-yellow)
![License](https://img.shields.io/badge/license-MIT-blue)
![Go](https://img.shields.io/badge/go-1.23+-00ADD8)
![Flutter](https://img.shields.io/badge/flutter-3.24+-02569B)

## Overview

Nexus Open provides a native Linux desktop application to control and configure your Corsair iCUE Nexus (640x48 pixel display). Display system metrics, weather information, and custom backgrounds on your device.

### Features

- ✅ **System Monitoring** - Real-time CPU/GPU temperature and network statistics
- ✅ **Weather Display** - Location-based weather with configurable units
- ✅ **Custom Backgrounds** - Upload and manage background images
- ✅ **Touch Input** - Responsive button controls
- ✅ **Configuration UI** - Flutter-based desktop settings application
- ✅ **REST API** - HTTP API for programmatic control
- ✅ **Headless Mode** - Run as background service

## Current Status: Ready for v1.0 🚀

The refactoring from Wails to Flutter is complete! See [PROJECT_PLAN.md](PROJECT_PLAN.md) for full development history.

### What's Working
- ✅ **Core Functionality** - Display rendering at 24 FPS with system metrics
- ✅ **Data Collection** - CPU/GPU temperature, network stats, weather data
- ✅ **USB Communication** - Stable device interface with proper lifecycle management
- ✅ **REST API** - Full HTTP API for configuration and control
- ✅ **Testing** - 64.9% code coverage with 65 unit tests
- ✅ **Linux Packages** - DEB, AppImage, and AUR (Arch) packages ready
- ✅ **Documentation** - Comprehensive installation and usage guides

### Recent Milestones
- **Phase 1-4:** Foundation, backend refactoring, API layer, UI integration (COMPLETE)
- **Phase 5:** Core functionality integration with instruments and display (COMPLETE)
- **Phase 6:** Testing coverage from 37.8% to 64.9% (COMPLETE)
- **Phase 7:** Production-ready Linux packaging (COMPLETE)

### Next Steps
- [ ] Release v1.0.0
- [ ] Publish to AUR
- [ ] Community feedback and bug fixes
- [ ] Additional features based on user requests

## Architecture

```
┌──────────────┐     HTTP :1985      ┌──────────────┐
│  Flutter UI  │ ◄──────────────────► │  Go Backend  │
│  (Desktop)   │   JSON REST API     │  + USB Dev   │
└──────────────┘                      └──────────────┘
```

## Quick Start (Development)

### Prerequisites

- Go 1.23+
- Flutter 3.24+
- libusb-1.0-dev
- Corsair iCUE Nexus device (VID: 0x1b1c, PID: 0x1b8e)
- make (optional, for using Makefile)

### Build & Run

```bash
# Clone repository
git clone https://github.com/mantonx/nexus-next.git
cd nexus-next

# Build Go backend (using Make)
make build                 # Development build (with debug info)
make build-release         # Optimized release build (stripped, smaller)

# Or build manually
go build -o bin/nexus-open ./cmd/nexus-open

# Set up USB permissions (one-time)
sudo bash scripts/setup-udev.sh
# Log out and back in for group changes

# Run backend
make run                   # Build and run
# Or run directly
./bin/nexus-open

# In another terminal, run Flutter UI (development)
cd ui
flutter run -d linux
```

### Build Commands

The project includes a Makefile for standardized builds:

```bash
# Development
make build         # Build development binary (with debug info)
make build-release # Build optimized release binary (stripped)
make run           # Build and run the application
make clean         # Remove all build artifacts

# Testing
make test          # Run all tests
make test-race     # Run tests with race detector
make coverage      # Generate test coverage report

# Packaging
make deb           # Build DEB package
make appimage      # Build AppImage
make all           # Build all packages

# Maintenance
make install       # Install to /usr/local/bin (requires sudo)
make uninstall     # Remove from /usr/local/bin (requires sudo)

# See all available commands
make help
```

## Project Structure

```
nexus-open/
├── cmd/
│   └── nexus-open/          # Application entry point
├── internal/                 # Private application code
│   ├── app/                 # Application orchestration
│   ├── device/              # USB device abstraction
│   ├── display/             # Display rendering
│   ├── instruments/         # Data sources (temp, network, weather)
│   ├── config/              # Configuration management
│   └── api/                 # HTTP API server
├── pkg/                      # Public reusable libraries
├── ui/                       # Flutter frontend
├── nexus/                    # Legacy code (being migrated)
├── packaging/                # Linux distribution files
│   ├── deb/                 # Debian/Ubuntu packages
│   ├── udev/                # USB permissions
│   └── desktop/             # Desktop integration
├── scripts/                  # Build and deployment scripts
├── docs/                     # Documentation
└── PROJECT_PLAN.md          # Comprehensive development plan
```

## Configuration

Configuration is stored in `~/.config/nexus-open/config.yaml`:

```yaml
location: "Jersey City, NJ"
time_format: "12h"           # or "24h"
unit: "imperial"             # or "metric"
background_color: "#000000"
text_color: "#FFFFFF"
image_paths:
  - "background1.png"
  - "background2.gif"
```

## API Endpoints

The backend provides a REST API on port 1985:

- `GET /api/config` - Get current configuration
- `POST /api/config` - Update configuration
- `POST /api/images/upload` - Upload background image
- `GET /api/images` - List uploaded images
- `POST /api/images/delete` - Delete image
- `GET /api/health` - Health check

## Installation

See [docs/INSTALLATION.md](docs/INSTALLATION.md) for detailed installation instructions.

### Quick Install

**Flatpak (Recommended - All Distributions):**
```bash
flatpak install flathub com.github.nexusopen.NexusOpen
flatpak run com.github.nexusopen.NexusOpen
```

**Snap (Ubuntu & Others):**
```bash
sudo snap install nexus-open
sudo snap connect nexus-open:raw-usb
```

**Debian/Ubuntu (DEB Package):**
```bash
sudo dpkg -i nexus-open_1.0.0_amd64.deb
sudo usermod -a -G plugdev $USER
# Log out and back in
```

**Arch Linux (AUR):**
```bash
yay -S nexus-open
```

**AppImage (Universal Binary):**
```bash
chmod +x nexus-open-1.0.0-x86_64.AppImage
./nexus-open-1.0.0-x86_64.AppImage
```

**From Source:**
```bash
# Using Make
make build-release
make install
# Or manually
go build -o bin/nexus-open ./cmd/nexus-open
sudo cp bin/nexus-open /usr/local/bin/
sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -a -G plugdev $USER
# Log out and back in
nexus-open
```

## Contributing

Contributions are welcome! Please:

1. Open an issue to discuss major changes before submitting PRs
2. Follow the existing code style and project structure
3. Add tests for new features (maintain 60%+ coverage)
4. Update documentation as needed
5. Test on your hardware if possible (Corsair iCUE Nexus)

See [PROJECT_PLAN.md](PROJECT_PLAN.md) for development history and architecture details.

## License

MIT License - see LICENSE file for details

## Credits

- **Original Device Communication:** Reverse-engineered USB protocol
- **System Monitoring:** github.com/shirou/gopsutil
- **USB Library:** github.com/google/gousb
- **UI Framework:** Flutter

## Troubleshooting

### Device not found

The app shows "Device not found" or "Disconnected" immediately on launch.

- Make sure the iCUE Nexus is plugged in via USB.
- Run `lsusb | grep 1b1c` — the device should appear as `1b1c:1b8e`.
- If it doesn't appear, try a different USB port or cable.

### USB permission denied

The app connects but immediately fails with a permission error, or you see `hidapi: failed to open` in logs.

- Add your user to the `plugdev` group and log out/in:

  ```sh
  sudo usermod -a -G plugdev $USER
  ```

- Reload udev rules:

  ```sh
  sudo udevadm control --reload-rules && sudo udevadm trigger
  ```
- On **Arch Linux**, udev rules go to `/usr/lib/udev/rules.d/` when installed via package. Run `sudo setup-udev.sh` for a manual install.
- On **Fedora/RHEL**, use the `input` group instead of `plugdev`: `sudo usermod -a -G input $USER`.

### Backend won't start (port 1985 in use)

Another process is using port 1985.

- Find and stop it: `ss -tlnp | grep 1985`
- Or run on a different port: `nexus-open --port 1986`

### Flutter UI won't connect

The settings window opens but shows "Backend not responding".

- Make sure the Go backend is running: `pgrep nexus-open`
- If using a custom port, the UI always connects to `localhost:1985`. Run the backend on the default port or wait for the WebSocket support (Week 2).

### Plugin shows blank

A zone displays nothing or a placeholder instead of data.

- Confirm the plugin binary is built and present next to the `nexus-open` binary.
- From the project root: `make plugins` (or `cd plugins/<name> && go build -o <name>`).
- Check logs for `plugin error` or `plugin timeout` lines.

## Support

- **Installation:** [docs/INSTALLATION.md](docs/INSTALLATION.md)
- **Device setup:** [DEVICE_SETUP.md](DEVICE_SETUP.md)
- **Issues:** [GitHub Issues](https://github.com/mantonx/nexus-next/issues)
- **API Documentation:** See REST API endpoints section above
- **Development:** [PROJECT_PLAN.md](PROJECT_PLAN.md)

---

**Note:** This project is not affiliated with Corsair. iCUE Nexus is a trademark of Corsair.
