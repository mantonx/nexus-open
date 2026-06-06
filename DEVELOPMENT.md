# Development Guide

## Quick Start

### Using Mock Device Mode (Recommended for Development)

Mock device mode allows you to develop without needing the physical Corsair iCUE Nexus hardware:

```bash
# Start development environment with mock device (default)
./dev.sh

# Or explicitly enable mock mode
NEXUS_MOCK_DEVICE=1 ./dev.sh

# To use real hardware instead
NEXUS_MOCK_DEVICE=0 ./dev.sh
```

### Development Launcher

Install the development launcher to your application menu:

```bash
./scripts/install-dev-launcher.sh
```

Then launch "Nexus Open (Development)" from your application menu!

## Development Features

### Mock Device Mode

When `NEXUS_MOCK_DEVICE=1` is set (default in `./dev.sh`):
- No physical hardware required
- No permission issues
- Instant startup
- Perfect for UI development and plugin testing

The mock device:
- Accepts all frame data
- Simulates touch events if needed
- Provides mock firmware version
- Auto-connects without errors

### Live Reload with Air

The `dev.sh` script starts Air, which watches for Go code changes and automatically rebuilds:

- **Backend**: Auto-reloads on `.go` file changes
- **Frontend**: Flutter UI built and launched
- **Plugins**: Built before backend starts

### Running Individual Components

```bash
# Backend only (with mock device)
NEXUS_MOCK_DEVICE=1 ~/go/bin/air

# Backend with real hardware
NEXUS_MOCK_DEVICE=0 ~/go/bin/air

# Flutter UI only (backend must be running)
cd ui && flutter run -d linux
```

## HID Device Connection

### Why the Connection Failed Before

The application tried to open the first HID interface found, but that interface didn't have the correct permissions. The improved code now:

1. **Tries all HID interfaces** until one opens successfully
2. **Logs detailed information** about each attempt
3. **Handles permission errors gracefully**
4. **Falls back to mock device** when `NEXUS_MOCK_DEVICE=1`

### Using Real Hardware

If you want to use the physical device:

1. Ensure udev rules are installed:
   ```bash
   sudo cp packaging/debian/usr/lib/udev/rules.d/99-corsair-nexus.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```

2. Check device permissions:
   ```bash
   ls -la /dev/hidraw* | grep "rw-rw-rw-"
   ```

3. Run with hardware:
   ```bash
   NEXUS_MOCK_DEVICE=0 ./dev.sh
   ```

## Project Structure

```
nexus-next/
├── cmd/nexus-open/        # Main application entry point
├── internal/
│   ├── app/               # Application orchestration
│   ├── device/            # Device abstraction (HID + Mock)
│   ├── zone/              # Zone and plugin management
│   └── api/               # REST API server
├── plugins/               # External plugins (CPU, GPU, Weather, etc.)
├── ui/                    # Flutter UI
├── configs/               # Configuration files
└── scripts/               # Development scripts
```

## Common Tasks

### Adding a New Plugin

See [plugins/README.md](plugins/README.md) for plugin development guide.

### Debugging

```bash
# Run with debug logging
./bin/nexus-open --debug

# Or with Air
NEXUS_MOCK_DEVICE=1 ~/go/bin/air -- --debug
```

### Testing

```bash
# Run all tests
go test ./...

# Test specific package
go test ./internal/device

# With coverage
go test -cover ./...
```

## Troubleshooting

### "go: updates to go.mod needed"

```bash
go mod tidy
```

### Air not detecting changes

Kill Air and restart:
```bash
pkill air
./dev.sh
```

### Flutter build fails

```bash
cd ui
flutter pub get
flutter clean
flutter build linux --debug
```

## Environment Variables

- `NEXUS_MOCK_DEVICE` - Enable mock device mode (1=enabled, 0=disabled)
- `NEXUS_DEBUG` - Enable debug logging
- `NEXUS_CONFIG_PATH` - Override config file path
- `NEXUS_API_PORT` - Override API port (default: 1985)
