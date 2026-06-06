#!/usr/bin/env bash
# install.sh — install Nexus Open for the current user
#
# Usage:
#   ./install.sh          # install binaries, desktop entry, autostart
#   ./install.sh --udev   # also install udev rules (requires sudo)
#   ./install.sh --remove # uninstall

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/nexus-open"
APP_DIR="$HOME/.local/share/applications"
AUTOSTART_DIR="$HOME/.config/autostart"
ICON_DIR="$HOME/.local/share/icons/hicolor/256x256/apps"

DAEMON_BIN="$REPO_DIR/nexus-open"
UI_BIN="$REPO_DIR/ui/build/linux/x64/release/bundle/nexus_open"
PLUGINS_DIR="$REPO_DIR/plugins"

# ── helpers ───────────────────────────────────────────────────────────────────

info()  { echo "  $*"; }
ok()    { echo "✓ $*"; }
warn()  { echo "! $*" >&2; }
die()   { echo "✗ $*" >&2; exit 1; }

require_binary() {
    [[ -f "$1" ]] || die "Binary not found: $1
Run 'go build -o nexus-open ./cmd/nexus-open' first."
}

# ── remove ────────────────────────────────────────────────────────────────────

do_remove() {
    info "Removing Nexus Open..."
    rm -f  "$BIN_DIR/nexus-open"
    rm -rf "$DATA_DIR"
    rm -f  "$APP_DIR/nexus-open.desktop"
    rm -f  "$AUTOSTART_DIR/nexus-open-autostart.desktop"
    rm -f  "$ICON_DIR/nexus-open.png"
    ok "Nexus Open removed."
    info "udev rules (if installed) must be removed manually with sudo:"
    info "  sudo rm /usr/lib/udev/rules.d/60-nexus-open.rules && sudo udevadm control --reload"
}

# ── udev ──────────────────────────────────────────────────────────────────────

do_udev() {
    if [[ $EUID -eq 0 ]]; then
        "$DAEMON_BIN" --setup-udev
    else
        info "Installing udev rules requires sudo..."
        sudo "$DAEMON_BIN" --setup-udev
    fi
    ok "udev rules installed — replug the Nexus device."
}

# ── install ───────────────────────────────────────────────────────────────────

do_install() {
    require_binary "$DAEMON_BIN"

    info "Installing to $BIN_DIR and $DATA_DIR..."
    mkdir -p "$BIN_DIR" "$DATA_DIR/plugins" "$APP_DIR" "$AUTOSTART_DIR" "$ICON_DIR"

    # Daemon binary
    install -m755 "$DAEMON_BIN" "$BIN_DIR/nexus-open"
    ok "Installed daemon → $BIN_DIR/nexus-open"

    # Flutter UI binary + its data bundle
    if [[ -f "$UI_BIN" ]]; then
        UI_BUNDLE_DIR="$(dirname "$UI_BIN")"
        cp -r "$UI_BUNDLE_DIR/." "$DATA_DIR/"
        ok "Installed UI bundle → $DATA_DIR"
    else
        warn "Flutter UI release binary not found — tray 'Show' will not work."
        warn "Build it with: cd ui && flutter build linux --release"
    fi

    # Plugins
    if [[ -d "$PLUGINS_DIR" ]]; then
        cp -r "$PLUGINS_DIR/." "$DATA_DIR/plugins/"
        ok "Installed plugins → $DATA_DIR/plugins/"
    else
        warn "No plugins/ directory found — zones will show errors."
    fi

    # Icon (use the tray icon PNG if available)
    ICON_SRC="$REPO_DIR/internal/tray/icon.png"
    if [[ -f "$ICON_SRC" ]]; then
        install -m644 "$ICON_SRC" "$ICON_DIR/nexus-open.png"
        ok "Installed icon"
    fi

    # App launcher .desktop
    install -m644 "$REPO_DIR/contrib/nexus-open.desktop" "$APP_DIR/nexus-open.desktop"
    ok "Installed app launcher → $APP_DIR/nexus-open.desktop"

    # Autostart .desktop
    install -m644 "$REPO_DIR/contrib/nexus-open-autostart.desktop" "$AUTOSTART_DIR/nexus-open-autostart.desktop"
    ok "Installed autostart entry → $AUTOSTART_DIR/nexus-open-autostart.desktop"

    # Ensure ~/.local/bin is in PATH
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        warn "$HOME/.local/bin is not in your PATH."
        warn "Add this to your ~/.bashrc or ~/.zshrc:"
        warn '  export PATH="$HOME/.local/bin:$PATH"'
    fi

    echo ""
    ok "Install complete."
    info "Nexus Open will start automatically on next login."
    info "To start it now: nexus-open --tray &"
    info ""
    info "If the device is not detected, install udev rules (once, requires sudo):"
    info "  ./install.sh --udev"
}

# ── main ──────────────────────────────────────────────────────────────────────

case "${1:-}" in
    --remove) do_remove ;;
    --udev)   require_binary "$DAEMON_BIN"; do_udev ;;
    "")       do_install ;;
    *)        die "Unknown option: $1. Usage: ./install.sh [--udev|--remove]" ;;
esac
