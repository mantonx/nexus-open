#!/usr/bin/env bash
# build-package.sh — build installable packages for all supported formats
#
# Usage:
#   ./build-package.sh              # build deb + rpm + pacman
#   ./build-package.sh --deb        # deb only
#   ./build-package.sh --rpm        # rpm only
#   ./build-package.sh --pacman     # pacman only
#   ./build-package.sh --test       # build then smoke-test each in Docker
#
# Prerequisites (host):
#   - go 1.25+
#   - fpm (gem install fpm)
#   - rpm-build (for --rpm on non-RPM hosts): sudo pacman -S rpm-tools
#   - docker (for --test)
#
# The Flutter UI bundle is included if already built:
#   cd ui && flutter build linux --release

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

# ── config ────────────────────────────────────────────────────────────────────

PKG_NAME="nexus-open"
PKG_VERSION="1.0.0"
PKG_ARCH="amd64"
PKG_DESCRIPTION="Linux controller for Corsair iCUE Nexus display"
PKG_URL="https://github.com/mantonx/nexus-next"
PKG_MAINTAINER="Matt <matthew.panton@gmail.com>"
PKG_LICENSE="MIT"

OUT_DIR="$REPO_DIR/dist"
STAGING_DIR="$REPO_DIR/.pkg-staging"

DAEMON_BIN="$REPO_DIR/nexus-open"
UI_BUNDLE="$REPO_DIR/ui/build/linux/x64/release/bundle"
ICON_DIR="$REPO_DIR/internal/tray"
UDEV_RULES="$REPO_DIR/packaging/udev/99-corsair-nexus.rules"
DESKTOP_FILE="$REPO_DIR/packaging/desktop/nexus-open.desktop"
SERVICE_FILE="$REPO_DIR/packaging/systemd/nexus-open.service"
POSTINST="$REPO_DIR/packaging/deb/DEBIAN/postinst"
PRERM="$REPO_DIR/packaging/deb/DEBIAN/prerm"

FPM="$HOME/.local/share/gem/ruby/3.4.0/bin/fpm"
[[ -x "$FPM" ]] || FPM="$(command -v fpm 2>/dev/null)" || { echo "fpm not found — gem install fpm"; exit 1; }

# ── helpers ───────────────────────────────────────────────────────────────────

info()  { echo "  $*"; }
ok()    { echo "✓ $*"; }
warn()  { echo "! $*" >&2; }
die()   { echo "✗ $*" >&2; exit 1; }

# ── build Go binary ───────────────────────────────────────────────────────────

build_binary() {
    info "Building Go daemon (native)..."
    COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    go build \
        -trimpath \
        -ldflags "-X main.version=${PKG_VERSION} -X main.commit=${COMMIT}" \
        -o "$DAEMON_BIN" \
        ./cmd/nexus-open
    ok "Built: $DAEMON_BIN"
}

# Build inside Ubuntu 22.04 to target its glibc (2.35), so the binary runs
# on Ubuntu 22.04, 24.04, and Debian 12. Binaries built on Arch link against
# glibc 2.38+ which is too new for Ubuntu 22.04.
build_binary_ubuntu() {
    info "Building Go daemon inside Ubuntu 22.04 (glibc 2.35 target)..."
    COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    docker run --rm \
        -v "$REPO_DIR:/src" \
        -w /src \
        ubuntu:22.04 bash -c "
            set -e
            apt-get update -qq
            apt-get install -y --no-install-recommends \
                curl ca-certificates \
                libusb-1.0-0-dev \
                libayatana-appindicator3-dev \
                pkg-config gcc 2>&1 | tail -2
            curl -sSL https://go.dev/dl/go1.25.0.linux-amd64.tar.gz | tar -C /usr/local -xz
            export PATH=/usr/local/go/bin:\$PATH
            go build -trimpath \
                -ldflags \"-X main.version=${PKG_VERSION} -X main.commit=${COMMIT}\" \
                -o nexus-open ./cmd/nexus-open
        "
    ok "Built: $DAEMON_BIN (glibc 2.35 compatible)"
}

# ── staging area ──────────────────────────────────────────────────────────────

# Populate a staging tree that fpm maps into the package.
# Returns an array of "localpath=/destpath" fpm arguments via stdout.
build_staging() {
    rm -rf "$STAGING_DIR"

    # Daemon binary
    install -Dm755 "$DAEMON_BIN" "$STAGING_DIR/usr/bin/nexus-open"

    # Flutter UI bundle (optional)
    if [[ -d "$UI_BUNDLE" ]]; then
        mkdir -p "$STAGING_DIR/usr/lib/nexus-open"
        cp -r "$UI_BUNDLE/." "$STAGING_DIR/usr/lib/nexus-open/"
        # Wrap the Flutter binary for XWayland (same as install.sh)
        mv "$STAGING_DIR/usr/lib/nexus-open/ui" "$STAGING_DIR/usr/lib/nexus-open/ui.real"
        cat > "$STAGING_DIR/usr/lib/nexus-open/ui" << 'WRAPPER'
#!/usr/bin/env bash
exec env WAYLAND_DISPLAY= "$(dirname "$0")/ui.real" "$@"
WRAPPER
        chmod +x "$STAGING_DIR/usr/lib/nexus-open/ui"
        ok "Included Flutter UI bundle"
    else
        warn "Flutter UI not built — package will be daemon-only"
        warn "Build with: cd ui && flutter build linux --release"
    fi

    # systemd user service
    install -Dm644 "$SERVICE_FILE" \
        "$STAGING_DIR/usr/lib/systemd/user/nexus-open.service"

    # Desktop file
    install -Dm644 "$DESKTOP_FILE" \
        "$STAGING_DIR/usr/share/applications/nexus-open.desktop"

    # udev rules
    install -Dm644 "$UDEV_RULES" \
        "$STAGING_DIR/usr/lib/udev/rules.d/99-corsair-nexus.rules"

    # Icons
    for size in 16 22 32 48 64 128 256; do
        src="$ICON_DIR/icon-${size}.png"
        if [[ -f "$src" ]]; then
            install -Dm644 "$src" \
                "$STAGING_DIR/usr/share/icons/hicolor/${size}x${size}/apps/nexus-open.png"
        fi
    done
}

# ── common fpm args ───────────────────────────────────────────────────────────

fpm_common() {
    local target="$1"
    "$FPM" \
        -s dir \
        -t "$target" \
        -n "$PKG_NAME" \
        -v "$PKG_VERSION" \
        --architecture "$PKG_ARCH" \
        --description "$PKG_DESCRIPTION" \
        --url "$PKG_URL" \
        --maintainer "$PKG_MAINTAINER" \
        --license "$PKG_LICENSE" \
        --category "utils" \
        --vendor "Nexus Open" \
        --chdir "$STAGING_DIR" \
        --package "$OUT_DIR/" \
        "${@:2}" \
        .
}

# ── deb ───────────────────────────────────────────────────────────────────────

build_deb() {
    # Build the binary inside Ubuntu 22.04 so it links against glibc 2.35,
    # making it compatible with Ubuntu 22.04, 24.04 and Debian 12.
    build_binary_ubuntu
    build_staging  # re-stage with the freshly built binary
    info "Building .deb..."
    mkdir -p "$OUT_DIR"
    fpm_common deb \
        --depends "libusb-1.0-0 (>= 2:1.0.21)" \
        --depends "libayatana-appindicator3-1" \
        --after-install "$POSTINST" \
        --before-remove "$PRERM" \
        --deb-systemd-enable \
        --deb-systemd-auto-start
    DEB_FILE=$(ls -t "$OUT_DIR"/${PKG_NAME}_*.deb 2>/dev/null | head -1)
    ok "Built: $DEB_FILE"
}

# ── rpm ───────────────────────────────────────────────────────────────────────

build_rpm() {
    command -v rpmbuild &>/dev/null || die "rpmbuild not found — sudo pacman -S rpm-tools"
    info "Building .rpm..."
    mkdir -p "$OUT_DIR"
    fpm_common rpm \
        --depends "libusb1" \
        --depends "libayatana-appindicator3" \
        --rpm-summary "$PKG_DESCRIPTION" \
        --after-install "$POSTINST" \
        --before-remove "$PRERM"
    RPM_FILE=$(ls -t "$OUT_DIR"/${PKG_NAME}-*.rpm 2>/dev/null | head -1)
    ok "Built: $RPM_FILE"
}

# ── pacman ────────────────────────────────────────────────────────────────────

build_pacman() {
    info "Building .pacman..."
    mkdir -p "$OUT_DIR"
    fpm_common pacman \
        --depends "libusb" \
        --depends "libayatana-appindicator" \
        --after-install "$POSTINST" \
        --before-remove "$PRERM"
    PACMAN_FILE=$(ls -t "$OUT_DIR"/${PKG_NAME}-*.pkg.tar.* 2>/dev/null | head -1)
    ok "Built: $PACMAN_FILE"
}

# ── docker test ───────────────────────────────────────────────────────────────

test_deb() {
    local deb_file="$1"
    local deb_base; deb_base="$(basename "$deb_file")"

    for distro in ubuntu:22.04 ubuntu:24.04 debian:12; do
        info "Testing .deb in ${distro}..."
        docker run --rm \
            -v "$OUT_DIR:/pkgs:ro" \
            "$distro" bash -c "
                set -e
                apt-get update -qq
                apt-get install -y --no-install-recommends \
                    file libusb-1.0-0 libayatana-appindicator3-1 2>&1 | tail -2
                dpkg -i /pkgs/${deb_base}
                echo '=== Binary check ===' && file /usr/bin/nexus-open
                echo '=== ldd ===' && ldd /usr/bin/nexus-open | grep 'not found' || echo 'all libs satisfied'
                echo '=== --version ===' && /usr/bin/nexus-open --version
            " && ok "${distro} .deb test passed" || warn "${distro} .deb test FAILED"
    done
}

test_rpm() {
    local rpm_file="$1"
    info "Testing .rpm in Fedora 40..."
    docker run --rm \
        -v "$OUT_DIR:/pkgs:ro" \
        fedora:40 bash -c "
            set -e
            dnf install -y libusb libayatana-appindicator3 2>&1 | tail -3
            rpm -ivh /pkgs/$(basename "$rpm_file") 2>&1
            echo '=== Installed files ===' && rpm -ql nexus-open
            echo '=== --version ===' && /usr/bin/nexus-open --version
        " && ok "Fedora 40 .rpm test passed" || warn "Fedora 40 .rpm test FAILED"
}

test_pacman() {
    local pkg_file="$1"
    info "Testing .pacman in Arch Linux..."
    docker run --rm \
        -v "$OUT_DIR:/pkgs:ro" \
        archlinux:latest bash -c "
            set -e
            pacman -Sy --noconfirm libusb libayatana-appindicator 2>&1 | tail -3
            pacman -U --noconfirm /pkgs/$(basename "$pkg_file") 2>&1
            echo '=== --version ===' && /usr/bin/nexus-open --version
        " && ok "Arch .pacman test passed" || warn "Arch .pacman test FAILED"
}

# ── main ──────────────────────────────────────────────────────────────────────

BUILD_DEB=false
BUILD_RPM=false
BUILD_PACMAN=false
RUN_TESTS=false

# Default: build all
if [[ $# -eq 0 ]]; then
    BUILD_DEB=true; BUILD_RPM=true; BUILD_PACMAN=true
fi

for arg in "$@"; do
    case "$arg" in
        --deb)    BUILD_DEB=true ;;
        --rpm)    BUILD_RPM=true ;;
        --pacman) BUILD_PACMAN=true ;;
        --test)   BUILD_DEB=true; BUILD_RPM=true; BUILD_PACMAN=true; RUN_TESTS=true ;;
        *) die "Unknown option: $arg. Usage: ./build-package.sh [--deb] [--rpm] [--pacman] [--test]" ;;
    esac
done

echo "Building Nexus Open v${PKG_VERSION} packages..."
echo ""

# deb builds its own binary inside Ubuntu 22.04 for glibc compatibility.
# rpm and pacman use a native binary (the host distro's glibc is fine for
# Fedora/Arch users who will have a recent glibc).
if $BUILD_RPM || $BUILD_PACMAN; then
    [[ -f "$DAEMON_BIN" ]] || build_binary
    build_staging
    $BUILD_RPM    && build_rpm
    $BUILD_PACMAN && build_pacman
fi

# deb: build_deb calls build_binary_ubuntu + build_staging internally
$BUILD_DEB && build_deb

if $RUN_TESTS; then
    echo ""
    echo "Running Docker install tests..."
    command -v docker &>/dev/null || die "docker not found — needed for --test"
    $BUILD_DEB    && [[ -n "${DEB_FILE:-}" ]]    && test_deb "$DEB_FILE"
    $BUILD_RPM    && [[ -n "${RPM_FILE:-}" ]]    && test_rpm "$RPM_FILE"
    $BUILD_PACMAN && [[ -n "${PACMAN_FILE:-}" ]] && test_pacman "$PACMAN_FILE"
fi

echo ""
ok "Done. Packages in $OUT_DIR/"
ls -lh "$OUT_DIR/"
