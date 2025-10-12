# Nexus Open - Roadmap

## Current Version: 0.1.0 (Development)

**Status:** Core functionality complete, preparing for alpha release

---

## Version 0.2.0 - Touch Input & Interactivity (2-3 weeks)

**Theme:** Making the device interactive with touch gestures and actions

### Critical Features

#### 1. Touch Input Integration ⭐ HIGH PRIORITY
- [ ] **Integrate TouchReader into app lifecycle**
  - Wire touch.go into internal/app/app.go
  - Start touch event monitoring goroutine
  - Handle touch events in display manager

- [ ] **Touch Event Actions**
  - Define configurable actions for gestures (tap, swipe left/right)
  - Examples: cycle displays, toggle brightness, launch apps
  - Add touch configuration to config.yaml

- [ ] **UI Configuration for Touch**
  - Add touch configuration tab in Flutter UI
  - Map gestures to actions (dropdown selection)
  - Test touch actions from UI

- [ ] **Testing**
  - Unit tests for TouchReader
  - Integration tests for touch events → actions
  - Manual testing with actual device

**Benefits:** Make the device truly interactive, not just a passive display

#### 2. Multiple Display Modes ⭐ HIGH PRIORITY
- [ ] **Display Mode System**
  - Create DisplayMode interface (System Monitor, Weather, Clock, Custom)
  - Allow cycling through modes with touch gestures or API
  - Persist current mode in state

- [ ] **Built-in Display Modes**
  - System Monitor (current implementation)
  - Large Clock display
  - Weather-only display
  - Custom text display

- [ ] **Mode Configuration**
  - UI to enable/disable modes
  - Order modes for cycling
  - Configure what's shown in each mode

**Benefits:** Users can customize what information they want to see

#### 3. CPU Load Monitoring
- [ ] Implement CPU load percentage tracking (currently shows 0)
- [ ] Add to instruments/cpu_load.go
- [ ] Display in system monitor mode
- [ ] Unit tests for CPU load

**Benefits:** Complete the system monitoring feature set

---

## Version 0.3.0 - Polish & Packaging (2 weeks)

**Theme:** Production-ready release with proper packaging and documentation

### Critical Features

#### 1. Old Code Cleanup ⭐ HIGH PRIORITY
- [ ] **Delete nexus/ package (~2,737 lines)**
  - Verify all functionality migrated to internal/
  - Remove nexus/ directory entirely
  - Update imports if needed
  - Verify tests still pass

**Benefits:** Cleaner codebase, no legacy code confusion

#### 2. Application Icon & Branding
- [ ] **Create application icon**
  - Design icon (640x48 device theme?)
  - Export in all sizes: 16, 32, 48, 64, 128, 256, 512
  - Add to packaging/icons/

- [ ] **Update desktop integration**
  - Install icons with packages
  - Update .desktop file with icon
  - Test icon appears in menus

**Benefits:** Professional appearance in system menus

#### 3. Auto-start Configuration
- [ ] **Systemd user service**
  - Document enabling auto-start: `systemctl --user enable nexus-open`
  - Add UI toggle for auto-start (calls systemctl via API)
  - Test on fresh system

- [ ] **Alternative: XDG autostart**
  - Create .desktop file for autostart
  - Add to ~/.config/autostart/
  - Works on non-systemd systems

**Benefits:** Device works immediately on login

#### 4. Package Testing & Distribution
- [ ] **Test packages on clean VMs**
  - Ubuntu 22.04, 24.04 (DEB)
  - Arch Linux (AUR)
  - Fedora (AppImage)

- [ ] **Submit to repositories**
  - AUR package submission
  - Consider Flathub submission
  - Consider Snap store submission

- [ ] **Release v0.3.0**
  - Tag in git
  - Build all packages
  - GitHub Release with assets
  - Announcement

**Benefits:** Easy installation for users

---

## Version 0.4.0 - Advanced Features (3-4 weeks)

**Theme:** Power user features and customization

### Features

#### 1. System Tray Integration
- [ ] Add system tray icon
- [ ] Menu: Show/Hide UI, Restart, Quit
- [ ] Show connection status in tray
- [ ] Click to open settings

#### 2. Brightness Control
- [ ] Implement SetBrightness in NexusDevice (HID feature report)
- [ ] Add brightness slider to UI
- [ ] Auto-brightness based on time of day
- [ ] Keyboard shortcuts for brightness

#### 3. Background Image Management
- [ ] Image slideshow mode (rotate through images)
- [ ] Configure slideshow interval
- [ ] Image effects (blur, tint, opacity)
- [ ] Support GIF animations properly

#### 4. Advanced Display Modes
- [ ] Spotify/Music display (show now playing)
- [ ] Calendar/Agenda display
- [ ] System resource graph (CPU/RAM over time)
- [ ] Custom layout editor (drag & drop widgets)

#### 5. Plugin System (Experimental)
- [ ] Define plugin interface
- [ ] Load plugins from ~/.config/nexus-open/plugins/
- [ ] Allow custom data sources
- [ ] Allow custom display modes

---

## Version 0.5.0 - Performance & Quality (2 weeks)

**Theme:** Optimization and stability

### Features

#### 1. Performance Profiling
- [ ] CPU profiling of main loops
- [ ] Memory profiling (reduce allocations)
- [ ] Optimize rendering pipeline
- [ ] Benchmark critical paths

#### 2. Advanced Testing
- [ ] Integration tests with test fixtures
- [ ] End-to-end API tests
- [ ] Stress testing (24hr+ runs)
- [ ] Memory leak detection

#### 3. Improved Error Handling
- [ ] Better error messages for users
- [ ] Automatic reconnection on USB disconnect
- [ ] Fallback modes when sensors unavailable
- [ ] Logging levels configurable via UI

#### 4. Documentation
- [ ] API documentation (OpenAPI/Swagger)
- [ ] Architecture diagrams (mermaid)
- [ ] Contributing guide
- [ ] Plugin development guide

---

## Version 1.0.0 - Stable Release (After 0.5.0 stabilizes)

**Theme:** Production-ready, stable, well-documented

### Release Criteria
- ✅ All core features working
- ✅ 70%+ test coverage
- ✅ No known crashes or data loss bugs
- ✅ Comprehensive documentation
- ✅ Available in major package repositories
- ✅ Active community (GitHub stars, issues, discussions)
- ✅ Performance profiled and optimized
- ✅ Touch input fully functional
- ✅ Multiple display modes implemented

---

## Future (Post 1.0) - Considerations

### Potential Features
- **Windows support** - Port to Windows with WinUSB
- **macOS support** - Port to macOS with IOKit
- **Web UI** - Browser-based configuration (alternative to Flutter)
- **Mobile app** - Android/iOS remote control app
- **Cloud sync** - Sync settings across devices
- **Community display modes** - Share custom displays
- **Hardware support** - Support other similar devices
- **Raspberry Pi optimization** - Run on Pi as dedicated device

### Community Requests
- Track feature requests in GitHub Issues
- Vote on features via discussions
- Consider contributions for advanced features

---

## Development Principles

1. **Stability First** - Don't break working features
2. **Test Coverage** - Maintain 60%+ coverage at all times
3. **User Feedback** - Listen to issues and feature requests
4. **Clean Code** - Keep the architecture clean as we grow
5. **Documentation** - Document as we build, not after
6. **Backwards Compatibility** - Don't break user configs

---

## Version Numbering

- **0.x.x** - Development/Alpha releases (breaking changes allowed)
- **1.x.x** - Stable releases (backwards compatible)
- **x.Y.x** - Minor features, bug fixes
- **x.x.Z** - Patch releases, critical bug fixes

---

## Contributing to Roadmap

Want to help? Pick a task from the current version milestone:
1. Comment on related GitHub issue (or create one)
2. Fork the repo and create a feature branch
3. Implement with tests
4. Submit PR with reference to issue

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development setup.

---

**Last Updated:** 2025-10-12
**Current Focus:** v0.2.0 - Touch Input & Interactivity
