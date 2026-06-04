#!/bin/bash
# Setup udev rules for Corsair iCUE Nexus HID device permissions.
# Detects the distro and writes rules to the correct path.

set -e

RULES_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/packaging/udev/99-corsair-nexus.rules"

if [ ! -f "${RULES_SRC}" ]; then
    echo "Error: rules file not found at ${RULES_SRC}" >&2
    exit 1
fi

# Detect target rules directory:
#   /usr/lib/udev/rules.d/ — Arch and other distros that manage this via packages
#   /etc/udev/rules.d/     — manual installs on all distros
detect_rules_dir() {
    # Arch: uses /usr/lib/udev/rules.d/ for package-managed rules
    if [ -f /etc/arch-release ]; then
        echo "/usr/lib/udev/rules.d"
        return
    fi
    # Default: /etc/udev/rules.d/ (Debian, Fedora, openSUSE, etc.)
    echo "/etc/udev/rules.d"
}

RULES_DIR="$(detect_rules_dir)"
DEST="${RULES_DIR}/99-corsair-nexus.rules"

echo "Installing udev rules for Corsair iCUE Nexus (1b1c:1b8e)..."
echo "  Source: ${RULES_SRC}"
echo "  Dest:   ${DEST}"

sudo mkdir -p "${RULES_DIR}"
sudo cp "${RULES_SRC}" "${DEST}"
sudo chmod 644 "${DEST}"

echo "Reloading udev rules..."
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=usb
sudo udevadm trigger --subsystem-match=hidraw

echo ""
echo "✓ Udev rules installed."
echo ""
echo "If this is your first time, add your user to the plugdev group:"

# Fedora/RHEL don't have plugdev — advise 'input' instead
if [ -f /etc/fedora-release ] || [ -f /etc/redhat-release ]; then
    echo "  sudo usermod -a -G input \$USER"
    echo "  (Fedora/RHEL use the 'input' group)"
else
    echo "  sudo usermod -a -G plugdev \$USER"
fi

echo ""
echo "Then log out and back in, or unplug and replug the device."
