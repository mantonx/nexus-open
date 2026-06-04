#!/bin/bash
# Development script for working with real hardware
# This uses sudo to run ONLY the device access part, while keeping hot-reload

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "🔌 Nexus Open - Hardware Development Mode"
echo ""

# Check if device is accessible
if [ -w /dev/hidraw0 ]; then
    echo "✓ HID device is accessible (no sudo needed)"
    NEED_SUDO=0
else
    echo "⚠ HID device requires elevated privileges"
    echo "  Running with sudo for device access..."
    NEED_SUDO=1
fi

echo ""

# Kill any existing instances
killall nexus-open ui air 2>/dev/null || true
sleep 1

# Start air with hot-reload
echo "Starting Go backend with air (hot-reload enabled)..."
export NEXUS_MOCK_DEVICE=0

# Find air binary
AIR_BIN=""
if command -v air &> /dev/null; then
    AIR_BIN="air"
elif [ -f "$HOME/go/bin/air" ]; then
    AIR_BIN="$HOME/go/bin/air"
else
    echo "Error: air not found. Install with: go install github.com/air-verse/air@latest"
    exit 1
fi

# Run with or without sudo based on permissions
if [ $NEED_SUDO -eq 1 ]; then
    echo "Running with sudo (password may be required)..."
    sudo -E $AIR_BIN
else
    $AIR_BIN
fi
