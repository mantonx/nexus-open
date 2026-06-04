#!/bin/bash
# Install development launcher to local applications menu

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Installing Nexus Open development launcher..."

# Create local applications directory if it doesn't exist
mkdir -p ~/.local/share/applications

# Copy desktop file to local applications
cp "$PROJECT_DIR/nexus-open-dev.desktop" ~/.local/share/applications/

# Update desktop database
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database ~/.local/share/applications
fi

echo "✓ Development launcher installed!"
echo ""
echo "You can now launch Nexus Open (Development) from your application menu."
echo "It will run in mock device mode by default (no physical hardware required)."
echo ""
echo "To uninstall: rm ~/.local/share/applications/nexus-open-dev.desktop"
