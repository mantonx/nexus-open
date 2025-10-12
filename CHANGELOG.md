# Changelog

All notable changes to Nexus Open will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-10-12

### Added

#### Core Features
- **Display System**: 24 FPS rendering to Corsair iCUE Nexus (640x48 display)
- **System Monitoring**: Real-time CPU and GPU temperature monitoring
- **Network Statistics**: Upload/download speed tracking with psutil integration
- **Weather Integration**: Location-based weather with Open-Meteo API and Nominatim geocoding
- **Configuration Management**: YAML-based config with hot reload support
- **REST API**: Full HTTP API on port 1985 for programmatic control
  - GET/POST `/api/config` - Configuration management
  - POST `/api/images/upload` - Background image upload
  - GET `/api/images` - List uploaded images
  - POST `/api/images/delete` - Delete images
  - GET `/api/health` - Health check endpoint

#### Architecture
- **Clean Go Structure**: Standard cmd/, internal/, pkg/ layout
- **Dependency Injection**: Functional options pattern throughout
- **Interface-Based Design**: Device, Instrument, Renderer interfaces for testability
- **Context-Based Cancellation**: Proper shutdown and cleanup
- **Thread Safety**: sync.RWMutex and sync.Once for concurrent access
- **Structured Logging**: log/slog integration
- **Zero Global Variables**: All state managed in structs

#### Instrumentation
- **CPU Temperature**: Cross-platform support (Linux /sys, Windows wmic, macOS sysctl)
- **GPU Temperature**: nvidia-smi integration for NVIDIA GPUs
- **Network Stats**: Real-time bandwidth monitoring
- **Weather**: 30-minute update interval with weather codes and icons
- **Registry Pattern**: Unified instrument lifecycle management with 1-second aggregation

#### Testing
- **64.9% Code Coverage**: 65 unit tests across all packages
- **Table-Driven Tests**: Comprehensive test coverage
- **Mock Device**: Full mock implementation for testing
- **Race Detector**: All tests pass with -race flag
- **Context Testing**: Proper cancellation and timeout testing

#### Linux Packaging
- **DEB Package**: For Debian, Ubuntu, Linux Mint, Pop!_OS, elementary OS
  - Automated build script with binary stripping
  - Post-install script for udev and group setup
  - systemd user service integration
- **AUR Package**: For Arch Linux, Manjaro, EndeavourOS
  - Full PKGBUILD with build flags and checks
  - Post-install instructions
- **AppImage**: Universal self-contained executable
  - Works on all Linux distributions
  - Bundled dependencies (libusb)
- **Flatpak**: Sandboxed universal package
  - Ready for Flathub submission
  - AppStream metadata included
  - Proper USB and network permissions
- **Snap**: Ubuntu/Canonical package format
  - Strict confinement with security hardening
  - Systemd daemon with auto-restart
  - Raw USB interface with VID/PID filtering

#### System Integration
- **udev Rules**: USB device permissions for non-root access
- **Desktop Entry**: Application menu integration
- **systemd Service**: User service with resource limits
  - Memory limit: 100MB
  - CPU quota: 20%
  - Security: NoNewPrivileges, PrivateTmp, ProtectSystem
- **Configuration**: ~/.config/nexus-open/config.yaml

#### Documentation
- **README.md**: Project overview and quick start
- **PROJECT_PLAN.md**: Complete development history and architecture
- **docs/INSTALLATION.md**: Comprehensive installation guide
  - All package formats
  - Troubleshooting section
  - Building from source
- **docs/RELEASE_CHECKLIST.md**: Complete release procedure
- **packaging/*/README.md**: Package-specific build guides

### Changed
- Migrated from Wails to Flutter for UI framework
- Replaced global variables with dependency injection
- Refactored display rendering for better performance
- Improved error handling with custom error types
- Updated to Go 1.23+ with modern patterns

### Removed
- Wails dependencies (47 files, 5,195 lines removed)
- React/TypeScript frontend
- Global state management
- log.Fatal calls in library code

### Fixed
- USB device connection race conditions
- Configuration race conditions with proper locking
- Memory leaks in instrument goroutines
- Context cancellation in all subsystems

### Security
- USB device access via udev rules (no root required)
- Resource limits in systemd service
- Sandboxing in Flatpak and Snap packages
- NoNewPrivileges in systemd service

## [Unreleased]

### Planned Features
- Flutter UI completion
- Touch input migration
- System tray integration
- Auto-start configuration
- Additional instrument types (CPU load, memory, disk)
- Custom widget/layout system
- Profile support for different display modes

### Future Packaging
- RPM package (Fedora, RHEL, CentOS)
- NixOS package
- Gentoo ebuild

## Release Statistics

### v1.0.0 Metrics
- **Lines of Code**: ~4,000 (new internal/ packages)
- **Test Coverage**: 64.9% overall, 73.9% for internal/ packages
- **Tests**: 65 unit tests
- **Build Time**: ~3 seconds
- **Binary Size**: 8.8 MB (stripped), 13 MB (with debug symbols)
- **Package Formats**: 5 (DEB, AUR, AppImage, Flatpak, Snap)
- **Supported Distributions**: 95%+ of Linux desktop users

### Development Timeline
- **Phase 1**: Foundation & Cleanup (Week 1)
- **Phase 2**: Backend Refactoring (Week 2)
- **Phase 3**: API Layer (Week 2-3)
- **Phase 4**: UI Integration (Week 3)
- **Phase 5**: Core Functionality Integration (Week 4)
- **Phase 6**: Testing & Quality (Week 5)
- **Phase 7**: Linux Packaging (Week 6)
- **Phase 8**: CI/CD & Release (Week 6-7)

## Links

- [GitHub Repository](https://github.com/yourusername/nexus-open)
- [Issue Tracker](https://github.com/yourusername/nexus-open/issues)
- [AUR Package](https://aur.archlinux.org/packages/nexus-open)
- [Flathub](https://flathub.org/apps/com.github.nexusopen.NexusOpen)
- [Snap Store](https://snapcraft.io/nexus-open)
