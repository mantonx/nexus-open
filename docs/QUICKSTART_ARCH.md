# Quick Start Guide for Arch Linux

This guide will help you build and test Nexus Open on Arch Linux.

## Prerequisites

Install required dependencies:

```bash
# Install build dependencies
sudo pacman -S go libusb git base-devel

# Optional: Install Flutter for the UI (if you want to test the GUI)
yay -S flutter
# OR follow: https://docs.flutter.dev/get-started/install/linux
```

## Build and Run

### 1. Navigate to the project directory

```bash
cd /home/fictional/Projects/nexus-next
```

### 2. Run tests (optional but recommended)

```bash
# Run all tests
go test -v ./...

# Run with coverage
go test -v -coverprofile=coverage.out ./...
go tool cover -html=coverage.out  # Opens in browser
```

### 3. Build the binary

```bash
# Build with version info
CGO_ENABLED=1 go build -v -o nexus-open \
  -ldflags "-X main.version=1.0.0 -X main.commit=$(git rev-parse --short HEAD)" \
  ./cmd/nexus-open

# Verify it built correctly
./nexus-open --version
./nexus-open --help
```

### 4. Set up USB permissions

```bash
# Install udev rules
sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/

# Reload udev
sudo udevadm control --reload-rules
sudo udevadm trigger

# Add yourself to plugdev group
sudo groupadd plugdev 2>/dev/null || true
sudo usermod -a -G plugdev $USER

# IMPORTANT: Log out and back in for group changes to take effect
# You can verify with: groups
```

### 5. Run the application

#### Option A: Without Device (Testing)

The application will run without the physical device, but won't display anything. This is useful for testing the API and configuration:

```bash
# Run in foreground
./nexus-open

# You should see output like:
# time=... level=INFO msg="Starting Nexus Open" version=1.0.0
# time=... level=WARN msg="failed to connect to device" error="device not found"
# time=... level=INFO msg="API server listening" address=":1985"
```

In another terminal, test the API:

```bash
# Health check
curl http://localhost:1985/api/health

# Get config
curl http://localhost:1985/api/config

# Update config
curl -X POST http://localhost:1985/api/config \
  -H "Content-Type: application/json" \
  -d '{
    "location": "Los Angeles, CA",
    "time_format": "24h",
    "unit": "metric",
    "background_color": "#000000",
    "text_color": "#00FF00"
  }'
```

Stop with Ctrl+C.

#### Option B: With Device (Real Hardware)

If you have a Corsair iCUE Nexus device:

```bash
# Check device is visible
lsusb | grep 1b1c:1b8e
# Should show: Bus XXX Device XXX: ID 1b1c:1b8e Corsair iCUE NEXUS

# Run the application
./nexus-open

# You should see the display update with:
# - Current time
# - CPU/GPU temperatures
# - Network stats
# - Weather (after ~30 seconds)
```

### 6. Check the configuration

The application creates a config file on first run:

```bash
# View config
cat ~/.config/nexus-open/config.yaml

# Edit config
nano ~/.config/nexus-open/config.yaml
# Changes will be auto-reloaded (no restart needed!)
```

### 7. Run as a systemd service (optional)

```bash
# Copy service file
mkdir -p ~/.config/systemd/user/
cp packaging/systemd/nexus-open.service ~/.config/systemd/user/

# Edit ExecStart path if needed
nano ~/.config/systemd/user/nexus-open.service
# Change: ExecStart=/usr/bin/nexus-open
# To: ExecStart=/home/fictional/Projects/nexus-next/nexus-open

# Enable and start service
systemctl --user daemon-reload
systemctl --user enable nexus-open.service
systemctl --user start nexus-open.service

# Check status
systemctl --user status nexus-open.service

# View logs
journalctl --user -u nexus-open.service -f

# Stop service
systemctl --user stop nexus-open.service
```

## Building Packages

### Build AUR Package Locally

```bash
cd packaging/arch/

# Review the PKGBUILD
cat PKGBUILD

# Build and install
makepkg -si

# This will:
# - Download dependencies
# - Build the binary
# - Create the package
# - Install it

# After installation:
systemctl --user enable nexus-open.service
systemctl --user start nexus-open.service
```

## Testing Flutter UI

```bash
cd ui/

# Install dependencies
flutter pub get

# Run in debug mode
flutter run -d linux

# This will launch the configuration UI
# You can:
# - Change location
# - Toggle time format (12h/24h)
# - Toggle units (imperial/metric)
# - Change colors
# - Upload background images
```

## Troubleshooting

### Device Not Found

```bash
# Check device is connected
lsusb | grep 1b1c:1b8e

# Check permissions
ls -l /dev/bus/usb/$(lsusb | grep 1b1c:1b8e | awk '{print $2,$4}' | tr ' ' '/' | tr -d ':')

# Verify you're in plugdev group
groups | grep plugdev

# If not, add yourself and re-login
sudo usermod -a -G plugdev $USER
# Then log out and back in
```

### Permission Denied

```bash
# Check udev rules are installed
ls -la /etc/udev/rules.d/99-corsair-nexus.rules

# Reload udev
sudo udevadm control --reload-rules
sudo udevadm trigger

# Try running with sudo (not recommended for regular use)
sudo ./nexus-open
```

### Build Errors

```bash
# Ensure CGO is enabled
export CGO_ENABLED=1

# Check Go version
go version  # Should be 1.23+

# Ensure libusb is installed
pacman -Qi libusb

# Clean and rebuild
go clean -cache
go build -v ./cmd/nexus-open
```

### API Not Accessible

```bash
# Check if running
ps aux | grep nexus-open

# Check port is listening
sudo ss -tulpn | grep 1985

# Test locally
curl http://localhost:1985/api/health

# Check logs
journalctl --user -u nexus-open.service -n 50
```

### Temperature Not Showing

```bash
# Check if sensors are available
cat /sys/class/thermal/thermal_zone0/temp
# Should show a number (in millidegrees Celsius)

# For GPU (NVIDIA)
nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader
```

### Weather Not Working

```bash
# Check network connectivity
ping -c 1 api.open-meteo.com

# Check logs for weather errors
journalctl --user -u nexus-open.service | grep -i weather

# Verify location in config
cat ~/.config/nexus-open/config.yaml | grep location
```

## Cleaning Up

```bash
# Stop service
systemctl --user stop nexus-open.service
systemctl --user disable nexus-open.service

# Remove config
rm -rf ~/.config/nexus-open/

# Remove binary
rm ./nexus-open

# Remove build artifacts
go clean -cache
rm -rf build/ dist/

# If installed via makepkg
sudo pacman -R nexus-open
```

## Quick Command Reference

```bash
# Build
CGO_ENABLED=1 go build -o nexus-open ./cmd/nexus-open

# Test
go test ./...

# Run
./nexus-open

# Run with debug logging
./nexus-open -debug

# Run on different port
./nexus-open -port 8080

# Check device
lsusb | grep 1b1c:1b8e

# API health
curl http://localhost:1985/api/health

# View logs
journalctl --user -u nexus-open.service -f
```

## Next Steps

Once you've tested and it's working:

1. **Install permanently**: Use the AUR PKGBUILD or copy binary to /usr/local/bin
2. **Enable service**: `systemctl --user enable nexus-open.service`
3. **Configure**: Edit ~/.config/nexus-open/config.yaml
4. **Customize**: Use the Flutter UI for easier configuration

## Getting Help

- Check logs: `journalctl --user -u nexus-open.service -f`
- Run with debug: `./nexus-open -debug`
- See full installation guide: [docs/INSTALLATION.md](INSTALLATION.md)
- Report issues: [GitHub Issues](https://github.com/mantonx/nexus-next/issues)
