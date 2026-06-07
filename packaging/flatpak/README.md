# Flatpak Packaging for Nexus Open

This directory contains the Flatpak manifest for building and distributing Nexus Open.

## Prerequisites

```bash
# Install flatpak and flatpak-builder
sudo apt install flatpak flatpak-builder  # Debian/Ubuntu
sudo pacman -S flatpak flatpak-builder    # Arch Linux
sudo dnf install flatpak flatpak-builder  # Fedora

# Add Flathub repository
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo

# Install required runtime and SDK
flatpak install flathub org.freedesktop.Platform//23.08
flatpak install flathub org.freedesktop.Sdk//23.08
flatpak install flathub org.freedesktop.Sdk.Extension.golang//23.08
```

## Building Locally

### Option 1: From Git Repository

```bash
# Clone the repository
cd /path/to/nexus-open

# Build the Flatpak
flatpak-builder --force-clean build-dir packaging/flatpak/com.github.nexusopen.NexusOpen.yaml

# Install locally for testing
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/com.github.nexusopen.NexusOpen.yaml

# Run the installed application
flatpak run com.github.nexusopen.NexusOpen
```

### Option 2: From Local Source (Development)

Edit the manifest to use local sources:

```yaml
sources:
  - type: dir
    path: ../..
```

Then build:

```bash
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/com.github.nexusopen.NexusOpen.yaml
```

## USB Device Access

Flatpak applications run in a sandbox. To access the iCUE Nexus USB device:

### Option 1: Grant USB Access (Recommended)

The manifest includes `--device=all` which grants access to all USB devices. This is necessary for the application to communicate with the iCUE Nexus.

### Option 2: Manual udev Rules (System-wide)

For better security, you can install udev rules system-wide:

```bash
# Copy udev rules from the Flatpak installation
sudo cp ~/.local/share/flatpak/app/com.github.nexusopen.NexusOpen/current/active/files/share/nexus-open/99-corsair-nexus.rules /etc/udev/rules.d/

# Or from the source repository
sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/

# Reload udev rules
sudo udevadm control --reload-rules
sudo udevadm trigger

# Add user to plugdev group
sudo usermod -a -G plugdev $USER

# Log out and back in
```

## Testing

```bash
# Run the Flatpak
flatpak run com.github.nexusopen.NexusOpen

# Check logs
flatpak run --command=sh com.github.nexusopen.NexusOpen
journalctl --user -f | grep nexus-open

# Test with device connected
lsusb | grep 1b1c:1b8e
```

## Creating a Flatpak Bundle

For distribution without Flathub:

```bash
# Build the Flatpak
flatpak-builder --repo=repo --force-clean build-dir packaging/flatpak/com.github.nexusopen.NexusOpen.yaml

# Create a single-file bundle
flatpak build-bundle repo nexus-open.flatpak com.github.nexusopen.NexusOpen

# Users can install with:
# flatpak install nexus-open.flatpak
```

## Submitting to Flathub

1. Fork the [Flathub repository](https://github.com/flathub/flathub)
2. Create a new repository: `com.github.nexusopen.NexusOpen`
3. Add the manifest and metainfo files
4. Submit a pull request to Flathub
5. Wait for review and approval

See [Flathub submission guidelines](https://docs.flathub.org/docs/for-app-authors/submission) for details.

## Updating the Manifest

### Updating App Version

1. Update `tag:` in the manifest to new version
2. Update version in metainfo.xml
3. Add new `<release>` entry in metainfo.xml
4. Rebuild

## Troubleshooting

### Build Fails with "module not found"

Go modules need to be downloaded. This is handled automatically, but you can verify with:

```bash
flatpak run --command=sh org.freedesktop.Sdk//23.08
go mod download
```

### USB Device Not Accessible

1. Check Flatpak permissions:
   ```bash
   flatpak info --show-permissions com.github.nexusopen.NexusOpen
   ```

2. Verify device is visible:
   ```bash
   flatpak run --command=lsusb com.github.nexusopen.NexusOpen
   ```

3. Check udev rules are installed system-wide

### Application Crashes

Check logs:

```bash
flatpak run com.github.nexusopen.NexusOpen --verbose
journalctl --user -xe
```

## Files

- `com.github.nexusopen.NexusOpen.yaml` - Main Flatpak manifest
- `com.github.nexusopen.NexusOpen.metainfo.xml` - AppStream metadata
- `README.md` - This file

## Resources

- [Flatpak Documentation](https://docs.flatpak.org/)
- [Flatpak Builder Documentation](https://docs.flatpak.org/en/latest/flatpak-builder.html)
- [Flathub Submission Guide](https://docs.flathub.org/)
- [AppStream Metadata Guidelines](https://www.freedesktop.org/software/appstream/docs/)
