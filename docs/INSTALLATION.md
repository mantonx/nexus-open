# Installation Guide

This guide covers installation of Nexus Open on various Linux distributions.

## Quick Install (Recommended Methods)

### Flatpak (Recommended - Universal)

Flatpak works on virtually all Linux distributions and provides sandboxed execution with automatic updates.

**Install from Flathub:**
```bash
# Add Flathub repository (if not already added)
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo

# Install Nexus Open
flatpak install flathub com.github.nexusopen.NexusOpen

# Run the application
flatpak run com.github.nexusopen.NexusOpen
```

**USB Permissions:**
The Flatpak has USB access configured, but you may need to install udev rules system-wide:
```bash
# Optional: Install udev rules for better integration
sudo cp ~/.local/share/flatpak/app/com.github.nexusopen.NexusOpen/current/active/files/share/nexus-open/99-corsair-nexus.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -a -G plugdev $USER
# Log out and back in
```

**Configuration:**
```bash
# Config location for Flatpak
~/.var/app/com.github.nexusopen.NexusOpen/config/nexus-open/config.yaml
```

### Snap (Ubuntu & Snap-enabled Distros)

Snap packages are popular on Ubuntu and provide automatic updates with strong sandboxing.

**Install from Snap Store:**
```bash
# Install the snap
sudo snap install nexus-open

# Connect USB interface (required for device access)
sudo snap connect nexus-open:raw-usb

# The service starts automatically
```

**Managing the Service:**
```bash
# Check status
snap services nexus-open

# View logs
sudo snap logs nexus-open -f

# Restart service
sudo snap restart nexus-open
```

**Configuration:**
```bash
# Config location for Snap
~/snap/nexus-open/common/.config/nexus-open/config.yaml
```

### Debian/Ubuntu (DEB Package)

> **Minimum requirement: Ubuntu 24.04 or later.**
> Ubuntu 22.04 and Debian 12 ship glib < 2.75 which Flutter requires.
> Use the **Flatpak** above on those distros instead.

Download the latest `.deb` package from the [releases page](https://github.com/mantonx/nexus-open/releases):

```bash
# Install the package
sudo dpkg -i nexus-open_1.0.0_amd64.deb

# Fix any dependency issues (if needed)
sudo apt-get install -f

# Add your user to the plugdev group
sudo usermod -a -G plugdev $USER

# Log out and back in for group changes to take effect
```

### Arch Linux (AUR)

Using an AUR helper like `yay`:

```bash
# Install from AUR
yay -S nexus-open

# Add your user to the plugdev group
sudo gpasswd -a $USER plugdev

# Reload udev rules
sudo udevadm control --reload-rules
sudo udevadm trigger

# Log out and back in
```

Or manually:

```bash
# Clone the AUR repository
git clone https://aur.archlinux.org/nexus-open.git
cd nexus-open

# Build and install
makepkg -si

# Follow the post-install instructions
```

### AppImage (Universal)

AppImages work on most Linux distributions without installation:

```bash
# Download the AppImage
wget https://github.com/mantonx/nexus-open/releases/download/v1.0.0/nexus-open-1.0.0-x86_64.AppImage

# Make it executable
chmod +x nexus-open-1.0.0-x86_64.AppImage

# Set up USB permissions (one-time setup)
sudo wget https://raw.githubusercontent.com/mantonx/nexus-open/main/packaging/udev/99-corsair-nexus.rules \
    -O /etc/udev/rules.d/99-corsair-nexus.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -a -G plugdev $USER

# Log out and back in

# Run the application
./nexus-open-1.0.0-x86_64.AppImage
```

## Building from Source

### Prerequisites

- Go 1.25 or later
- libusb-1.0-dev
- libayatana-appindicator3-dev (for system tray)
- Flutter 3.24+ (optional, for GUI)
- Git

### Install Dependencies

**Debian/Ubuntu:**

> **Note:** Ubuntu 24.04's `golang-go` package is Go 1.22, which is too old.
> Install Go from [go.dev/dl](https://go.dev/dl/) instead:
>
> ```bash
> curl -sSL https://go.dev/dl/go1.25.0.linux-amd64.tar.gz | sudo tar -C /usr/local -xz
> echo 'export PATH=/usr/local/go/bin:$PATH' >> ~/.bashrc && source ~/.bashrc
> ```

```bash
sudo apt update
sudo apt install libusb-1.0-0-dev libayatana-appindicator3-dev pkg-config gcc git
```

**Arch Linux:**
```bash
sudo pacman -S go libusb git
```

**Fedora:**
```bash
sudo dnf install golang libusb-devel git
```

### Build Steps

```bash
# Clone the repository
git clone https://github.com/mantonx/nexus-open.git
cd nexus-open

# Build the Go backend
go build -o nexus-open ./cmd/nexus-open

# Install to /usr/local/bin (optional)
sudo install -m 755 nexus-open /usr/local/bin/nexus-open

# Set up USB permissions
sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -a -G plugdev $USER

# Log out and back in for group changes

# Run the application
./nexus-open
```

### Build Flutter UI (Optional)

```bash
cd ui
flutter pub get
flutter run -d linux
```

## Configuration

Nexus Open stores its configuration in `~/.config/nexus-open/config.yaml`.

### Default Configuration

On first run, a default configuration will be created:

```yaml
location: "New York, NY"
time_format: "12h"
unit: "imperial"
background_color: "#000000"
text_color: "#FFFFFF"
image_paths: []
```

### Configuration Options

- `location`: City name for weather display (e.g., "Jersey City, NJ")
- `time_format`: Time display format - `"12h"` or `"24h"`
- `unit`: Temperature unit - `"imperial"` (°F) or `"metric"` (°C)
- `background_color`: Background color in hex format (e.g., "#000000")
- `text_color`: Text color in hex format (e.g., "#FFFFFF")
- `image_paths`: List of background image paths

### Editing Configuration

You can edit the config file directly:

```bash
nano ~/.config/nexus-open/config.yaml
```

Or use the Flutter UI for a graphical interface:

```bash
cd ui && flutter run -d linux
```

## Running as a Service

### systemd User Service (Recommended)

The package installation automatically sets up a systemd user service.

**Enable and start the service:**

```bash
# Enable service to start at login
systemctl --user enable nexus-open.service

# Start the service now
systemctl --user start nexus-open.service

# Check service status
systemctl --user status nexus-open.service

# View logs
journalctl --user -u nexus-open.service -f
```

**Stop or disable the service:**

```bash
# Stop the service
systemctl --user stop nexus-open.service

# Disable auto-start
systemctl --user disable nexus-open.service
```

### Manual Background Execution

If you don't want to use systemd:

```bash
# Run in background with nohup
nohup nexus-open > ~/.config/nexus-open/nexus-open.log 2>&1 &

# Or use screen/tmux
screen -dmS nexus-open nexus-open
```

## Troubleshooting

### Permission Denied (USB Access)

**Error:** `libusb: bad access [code -3]` or `permission denied`

**Solution:**

1. Verify udev rules are installed:
   ```bash
   ls -l /etc/udev/rules.d/99-corsair-nexus.rules
   ```

2. Check if you're in the `plugdev` group:
   ```bash
   groups
   ```

3. Add yourself to the group if missing:
   ```bash
   sudo usermod -a -G plugdev $USER
   ```

4. Reload udev rules:
   ```bash
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```

5. **Important:** Log out and back in (or reboot)

6. Verify device is accessible:
   ```bash
   lsusb | grep 1b1c:1b8e
   ```

### Device Not Found

**Error:** `device not found` or `failed to open device`

**Possible causes:**

1. **Device not connected**: Ensure the iCUE Nexus is plugged in
2. **Wrong USB port**: Try a different USB port
3. **USB hub issues**: Connect directly to the computer
4. **Device in use**: Close Corsair iCUE if running (Windows/dual-boot)

**Check device presence:**
```bash
lsusb | grep Corsair
# Should show: Bus XXX Device XXX: ID 1b1c:1b8e Corsair iCUE NEXUS
```

### Configuration Not Saving

**Issue:** Changes to config.yaml are not reflected

**Solution:**

1. Check file permissions:
   ```bash
   ls -la ~/.config/nexus-open/config.yaml
   ```

2. Ensure the file is writable:
   ```bash
   chmod 644 ~/.config/nexus-open/config.yaml
   ```

3. Restart the application:
   ```bash
   systemctl --user restart nexus-open.service
   # or kill and restart the process
   ```

### Service Won't Start

**Error:** `systemctl --user status nexus-open.service` shows failed

**Solution:**

1. Check logs for specific error:
   ```bash
   journalctl --user -u nexus-open.service -n 50
   ```

2. Verify binary exists and is executable:
   ```bash
   which nexus-open
   ls -l $(which nexus-open)
   ```

3. Test running manually:
   ```bash
   /usr/bin/nexus-open
   ```

### Temperature/Weather Not Showing

**Issue:** System metrics or weather not displaying

**Possible causes:**

1. **Temperature sensors:** Not all systems expose temperature via sysfs
2. **Weather API:** Network connectivity or API rate limits
3. **Invalid location:** Check location string in config.yaml

**Debugging:**

```bash
# Check CPU temperature sensor
cat /sys/class/thermal/thermal_zone0/temp

# Test network connectivity
curl -I https://api.open-meteo.com

# View application logs
journalctl --user -u nexus-open.service -f
```

### High CPU Usage

**Issue:** nexus-open consuming excessive CPU

**Solution:**

The display renders at 24 FPS which requires continuous processing. This is normal, but you can:

1. Check if GPU temperature monitoring is causing issues:
   - GPU temp requires vendor-specific tools (nvidia-smi, etc.)
   - Disable if not needed

2. Reduce update intervals in code (requires rebuilding)

3. Use systemd CPU quota (already set to 20% in service file)

## Uninstallation

### Debian/Ubuntu

```bash
sudo apt remove nexus-open
# or
sudo dpkg -r nexus-open
```

### Arch Linux

```bash
sudo pacman -R nexus-open
```

### Manual Installation

```bash
# Stop the service
systemctl --user stop nexus-open.service
systemctl --user disable nexus-open.service

# Remove files
sudo rm /usr/local/bin/nexus-open
sudo rm /etc/udev/rules.d/99-corsair-nexus.rules
sudo rm /usr/lib/systemd/user/nexus-open.service
rm -rf ~/.config/nexus-open

# Reload udev
sudo udevadm control --reload-rules
```

## Getting Help

- **Documentation:** See the [docs/](../docs/) directory
- **Issues:** [GitHub Issues](https://github.com/mantonx/nexus-open/issues)
- **Project Plan:** See [PROJECT_PLAN.md](../PROJECT_PLAN.md) for development details

## Next Steps

After installation:

1. **Configure your preferences** - Edit `~/.config/nexus-open/config.yaml`
2. **Set up the service** - Enable with systemd for automatic startup
3. **Test the UI** - Run the Flutter interface for easy configuration
4. **Upload backgrounds** - Use the API or UI to customize display backgrounds

See [USAGE.md](USAGE.md) for detailed usage instructions.
