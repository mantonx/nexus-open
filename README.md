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

## Current Status: Refactoring in Progress 🚧

**Branch:** `refactor/flutter-migration`

We are actively refactoring the codebase to improve quality and migrate from Wails to Flutter. See [PROJECT_PLAN.md](PROJECT_PLAN.md) for details.

### Completed (Phase 1)
- [x] Remove Wails dependencies
- [x] Create standard Go project structure
- [x] Add dependency injection pattern
- [x] Set up structured logging
- [x] Graceful shutdown handling

### In Progress
- [ ] Refactor backend (remove globals, add interfaces)
- [ ] Complete Flutter UI integration
- [ ] Linux packaging (DEB, AppImage, AUR)
- [ ] Comprehensive testing
- [ ] Documentation

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

### Build & Run

```bash
# Clone repository
git clone https://github.com/yourusername/nexus-open.git
cd nexus-open

# Build Go backend
go build -o nexus-open ./cmd/nexus-open

# Set up USB permissions (one-time)
sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -a -G plugdev $USER
# Log out and back in for group changes

# Run backend
./nexus-open

# In another terminal, run Flutter UI (development)
cd ui
flutter run -d linux
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

## Development Roadmap

See [PROJECT_PLAN.md](PROJECT_PLAN.md) for the comprehensive development plan.

**Current Phase:** Phase 1 - Foundation & Cleanup (Week 1)

### Upcoming Milestones

- **Week 2:** Backend refactoring complete
- **Week 3:** Flutter UI integration complete
- **Week 4:** Linux packages ready
- **Week 5:** Testing and documentation
- **Week 6:** v1.0.0 release

## Contributing

Contributions are welcome! This project is under active refactoring. Please:

1. Check [PROJECT_PLAN.md](PROJECT_PLAN.md) for current status
2. Open an issue to discuss major changes
3. Follow the existing code style
4. Add tests for new features
5. Update documentation

## License

MIT License - see LICENSE file for details

## Credits

- **Original Device Communication:** Reverse-engineered USB protocol
- **System Monitoring:** github.com/shirou/gopsutil
- **USB Library:** github.com/google/gousb
- **UI Framework:** Flutter

## Support

- **Issues:** [GitHub Issues](https://github.com/yourusername/nexus-open/issues)
- **Documentation:** See `docs/` directory
- **Troubleshooting:** See [PROJECT_PLAN.md](PROJECT_PLAN.md#troubleshooting)

---

**Note:** This project is not affiliated with Corsair. iCUE Nexus is a trademark of Corsair.
