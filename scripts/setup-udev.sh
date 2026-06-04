#!/bin/bash
# Setup script for Corsair iCUE Nexus HID device permissions
# This script creates udev rules to allow non-root access to the device

set -e

echo "Setting up udev rules for Corsair iCUE Nexus (1b1c:1b8e)..."

# Create the udev rule
echo 'SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b8e", MODE="0666", TAG+="uaccess"' | sudo tee /etc/udev/rules.d/99-nexus.rules

# Reload udev rules
echo "Reloading udev rules..."
sudo udevadm control --reload-rules

# Trigger device events
echo "Triggering device events..."
sudo udevadm trigger

echo ""
echo "✓ Udev rules installed successfully!"
echo ""
echo "The Corsair iCUE Nexus device should now be accessible without root."
echo "You may need to unplug and replug the device, or run:"
echo "  sudo udevadm trigger"
echo ""
echo "You can now run the application with:"
echo "  ./bin/nexus-open"
echo ""
