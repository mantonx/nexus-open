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
SYSTEMD_DIR="$HOME/.config/systemd/user"

DAEMON_BIN="$REPO_DIR/nexus-open"
UI_BIN="$REPO_DIR/ui/build/linux/x64/release/bundle/ui"
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
    systemctl --user disable --now nexus-open.service 2>/dev/null || true
    rm -f  "$SYSTEMD_DIR/nexus-open.service"
    systemctl --user daemon-reload 2>/dev/null || true
    rm -f  "$BIN_DIR/nexus-open"
    rm -rf "$DATA_DIR"
    rm -f  "$APP_DIR/nexus-open.desktop"
    rm -f  "$AUTOSTART_DIR/nexus-open-autostart.desktop"
    for size in 16 22 32 48 64 128 256; do
        rm -f "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/nexus-open.png"
    done
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
    mkdir -p "$BIN_DIR" "$DATA_DIR/plugins" "$APP_DIR" "$AUTOSTART_DIR" "$SYSTEMD_DIR"

    # Daemon binary
    install -m755 "$DAEMON_BIN" "$BIN_DIR/nexus-open"
    ok "Installed daemon → $BIN_DIR/nexus-open"

    # Flutter UI binary + its data bundle
    if [[ -f "$UI_BIN" ]]; then
        UI_BUNDLE_DIR="$(dirname "$UI_BIN")"
        cp -r "$UI_BUNDLE_DIR/." "$DATA_DIR/"
        # Wrap the Flutter binary in a script that forces EGL over GLX.
        # Flutter's GTK embedder defaults to GLX which crashes on Wayland
        # sessions where XWayland hasn't initialized yet. A wrapper is more
        # reliable than env var inheritance through KDE's launcher chain.
        mv "$DATA_DIR/ui" "$DATA_DIR/ui.real"
        cat > "$DATA_DIR/ui" << 'WRAPPER'
#!/usr/bin/env bash
# Flutter's GTK3 embedder crashes on native Wayland (wl_proxy race in GDK GL
# context init). Force XWayland by clearing WAYLAND_DISPLAY so GTK uses X11.
exec env WAYLAND_DISPLAY= "$(dirname "$0")/ui.real" "$@"
WRAPPER
        chmod +x "$DATA_DIR/ui"
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

    # Icons — install all available sizes into the hicolor theme
    ICON_DIR_BASE="$HOME/.local/share/icons/hicolor"
    for size in 16 22 32 48 64 128 256; do
        ICON_SRC="$REPO_DIR/internal/tray/icon-${size}.png"
        if [[ -f "$ICON_SRC" ]]; then
            mkdir -p "$ICON_DIR_BASE/${size}x${size}/apps"
            install -m644 "$ICON_SRC" "$ICON_DIR_BASE/${size}x${size}/apps/nexus-open.png"
        fi
    done
    ok "Installed icons (hicolor theme)"

    # App launcher .desktop — substitute absolute binary path so KDE/GNOME
    # launchers work even if ~/.local/bin isn't in the session PATH.
    sed "s|Exec=nexus-open|Exec=$BIN_DIR/nexus-open|g" \
        "$REPO_DIR/contrib/nexus-open.desktop" > "$APP_DIR/nexus-open.desktop"
    chmod 644 "$APP_DIR/nexus-open.desktop"
    ok "Installed app launcher → $APP_DIR/nexus-open.desktop"

    # Autostart .desktop — same substitution
    sed "s|Exec=nexus-open|Exec=$BIN_DIR/nexus-open|g" \
        "$REPO_DIR/contrib/nexus-open-autostart.desktop" > "$AUTOSTART_DIR/nexus-open-autostart.desktop"
    chmod 644 "$AUTOSTART_DIR/nexus-open-autostart.desktop"
    ok "Installed autostart entry → $AUTOSTART_DIR/nexus-open-autostart.desktop"

    # systemd user service — stops any running instance, installs, re-enables
    install -m644 "$REPO_DIR/contrib/nexus-open.service" "$SYSTEMD_DIR/nexus-open.service"
    systemctl --user daemon-reload
    systemctl --user enable --now nexus-open.service 2>/dev/null || true
    ok "Installed systemd user service → $SYSTEMD_DIR/nexus-open.service"

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
