#!/usr/bin/env bash
# Build an AppImage for Nexus Open.
#
# Usage:
#   bash scripts/build-appimage.sh
#
# Prerequisites:
#   - appimagetool in PATH or ~/bin (downloaded from github.com/AppImage/appimagetool)
#   - Flutter UI already built: cd ui && flutter build linux --release
#   - Go daemon already built: go build -o nexus-open ./cmd/nexus-open
#     (or run from repo root after make build-release)
#
# Output: dist/nexus-open-<version>-x86_64.AppImage

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

# ── config ────────────────────────────────────────────────────────────────────

PKG_NAME="nexus-open"
PKG_VERSION="$(git describe --tags --match 'v*' --always --dirty 2>/dev/null | sed 's/^v//' || echo "0.0.0-dev")"
[[ "${CI:-}" == "true" ]] && PKG_VERSION="$(git describe --tags --match 'v*' --always 2>/dev/null | sed 's/^v//' || echo "0.0.0-dev")"

OUT_DIR="$REPO_DIR/dist"
APPDIR="$REPO_DIR/build/appimage/NexusOpen.AppDir"

DAEMON_BIN="$REPO_DIR/nexus-open"
UI_BUNDLE="$REPO_DIR/ui/build/linux/x64/release/bundle"
ICON_DIR="$REPO_DIR/internal/tray"
DESKTOP_FILE="$REPO_DIR/packaging/desktop/nexus-open.desktop"
UDEV_RULES="$REPO_DIR/packaging/udev/99-corsair-nexus.rules"

OUTPUT_FILE="$OUT_DIR/${PKG_NAME}-${PKG_VERSION}-x86_64.AppImage"

info() { echo "  $*"; }
ok()   { echo "✓ $*"; }
die()  { echo "✗ $*" >&2; exit 1; }

# ── locate appimagetool ───────────────────────────────────────────────────────

find_appimagetool() {
    for candidate in \
        "$(command -v appimagetool 2>/dev/null)" \
        "$HOME/bin/appimagetool" \
        "$HOME/.local/bin/appimagetool" \
        "/usr/local/bin/appimagetool"
    do
        [[ -x "$candidate" ]] && { echo "$candidate"; return; }
    done
    echo ""
}

APPIMAGETOOL="$(find_appimagetool)"
if [[ -z "$APPIMAGETOOL" ]]; then
    die "appimagetool not found. Download from:
  https://github.com/AppImage/appimagetool/releases/latest
  e.g.: curl -Lo ~/bin/appimagetool https://github.com/AppImage/appimagetool/releases/latest/download/appimagetool-x86_64.AppImage && chmod +x ~/bin/appimagetool"
fi

# ── pre-flight ────────────────────────────────────────────────────────────────

[[ -x "$DAEMON_BIN" ]] || die "Daemon binary not found at $DAEMON_BIN — run: go build -o nexus-open ./cmd/nexus-open"
[[ -d "$UI_BUNDLE"  ]] || die "Flutter UI bundle not found at $UI_BUNDLE — run: cd ui && flutter build linux --release"

mkdir -p "$OUT_DIR"

# ── assemble AppDir ───────────────────────────────────────────────────────────

info "Assembling AppDir..."
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/lib/nexus-open"
mkdir -p "$APPDIR/usr/share/nexus-open"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/scalable/apps"

# Daemon binary
install -m755 "$DAEMON_BIN" "$APPDIR/usr/bin/nexus-open"

# Flutter UI bundle — same XWayland wrapper as the other packages
cp -r "$UI_BUNDLE/." "$APPDIR/usr/lib/nexus-open/"
mv "$APPDIR/usr/lib/nexus-open/ui" "$APPDIR/usr/lib/nexus-open/ui.real"
cat > "$APPDIR/usr/lib/nexus-open/ui" << 'WRAPPER'
#!/usr/bin/env bash
exec env WAYLAND_DISPLAY= "$(dirname "$0")/ui.real" "$@"
WRAPPER
chmod +x "$APPDIR/usr/lib/nexus-open/ui"

# Plugin binaries
for mod in cpu-temp gpu-temp network weather cpu-load gpu-load; do
    if [[ -f "plugins/$mod/$mod" ]]; then
        mkdir -p "$APPDIR/usr/lib/nexus-open/plugins/$mod"
        install -m755 "plugins/$mod/$mod" "$APPDIR/usr/lib/nexus-open/plugins/$mod/$mod"
    fi
done

# Layout configs
cp -r "$REPO_DIR/configs" "$APPDIR/usr/share/nexus-open/"

# Icons
for size in 16 22 32 48 64 128 256; do
    src="$ICON_DIR/icon-${size}.png"
    if [[ -f "$src" ]]; then
        mkdir -p "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps"
        install -m644 "$src" "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps/nexus-open.png"
    fi
done
[[ -f "$ICON_DIR/icon.svg" ]] && install -m644 "$ICON_DIR/icon.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/nexus-open.svg"

# Desktop file (adjusted Exec path for AppImage)
install -m644 "$DESKTOP_FILE" "$APPDIR/usr/share/applications/nexus-open.desktop"

# AppImage required files at AppDir root
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/nexus-open.png" "$APPDIR/nexus-open.png"
cp "$APPDIR/usr/share/applications/nexus-open.desktop" "$APPDIR/nexus-open.desktop"

# AppRun — entry point executed by the AppImage runtime
cat > "$APPDIR/AppRun" << 'APPRUN'
#!/usr/bin/env bash
#
# AppRun for Nexus Open AppImage.
# Sets up paths so the daemon can find the Flutter UI and plugins
# regardless of where the AppImage is mounted.

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Point the daemon at the bundled UI and plugins inside the AppImage.
export NEXUS_UI_DIR="$HERE/usr/lib/nexus-open"
export NEXUS_PLUGINS_DIR="$HERE/usr/lib/nexus-open/plugins"

exec "$HERE/usr/bin/nexus-open" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

ok "AppDir assembled at $APPDIR"

# ── build AppImage ────────────────────────────────────────────────────────────

info "Building AppImage with $APPIMAGETOOL..."

# appimagetool needs ARCH set when running on a foreign arch or in CI
export ARCH="${ARCH:-x86_64}"

"$APPIMAGETOOL" --no-appstream "$APPDIR" "$OUTPUT_FILE"

ok "AppImage: $OUTPUT_FILE ($(du -h "$OUTPUT_FILE" | cut -f1))"
