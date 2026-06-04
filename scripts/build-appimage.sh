#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building Nexus Open AppImage...${NC}"

# Configuration
VERSION="1.0.0"
ARCH="x86_64"
APP_NAME="nexus-open"
BUILD_DIR="build/appimage"
APPDIR="${BUILD_DIR}/${APP_NAME}.AppDir"

# Clean previous build
echo -e "${YELLOW}Cleaning previous build...${NC}"
rm -rf "${BUILD_DIR}"
mkdir -p "${APPDIR}"

# Create AppDir structure
echo -e "${YELLOW}Creating AppImage structure...${NC}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

# Build the Go binary
echo -e "${YELLOW}Building Go binary...${NC}"
CGO_ENABLED=1 go build -o "${APPDIR}/usr/bin/${APP_NAME}" \
    -ldflags "-X main.version=${VERSION} -X main.commit=$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')" \
    ./cmd/nexus-open

# Strip binary to reduce size
strip "${APPDIR}/usr/bin/${APP_NAME}"

# Copy desktop file
echo -e "${YELLOW}Copying desktop file...${NC}"
cp packaging/desktop/nexus-open.desktop "${APPDIR}/usr/share/applications/"


# Create AppRun script
echo -e "${YELLOW}Creating AppRun script...${NC}"
cat > "${APPDIR}/AppRun" << 'EOF'
#!/bin/bash
APPDIR="$(dirname "$(readlink -f "$0")")"
export PATH="${APPDIR}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${APPDIR}/usr/lib:${LD_LIBRARY_PATH}"

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    echo "Warning: Running as root. USB device access should work."
fi

# Check for plugdev group membership
if ! groups | grep -q '\bplugdev\b'; then
    echo "Warning: User not in 'plugdev' group. USB device access may fail."
    echo "Run: sudo usermod -a -G plugdev \$USER"
    echo "Then log out and back in."
fi

exec "${APPDIR}/usr/bin/nexus-open" "$@"
EOF

chmod +x "${APPDIR}/AppRun"

# Create .desktop file symlink at root
ln -sf usr/share/applications/nexus-open.desktop "${APPDIR}/nexus-open.desktop"

# Copy necessary libraries
echo -e "${YELLOW}Copying library dependencies...${NC}"

# Locate libusb dynamically: try pkg-config, then ldd, then common paths
LIBUSB_PATH=""
if pkg-config --exists libusb-1.0 2>/dev/null; then
    LIBUSB_DIR="$(pkg-config --variable=libdir libusb-1.0)"
    LIBUSB_PATH="${LIBUSB_DIR}/libusb-1.0.so.0"
fi

if [ -z "${LIBUSB_PATH}" ] || [ ! -f "${LIBUSB_PATH}" ]; then
    # Fall back to ldd output on the built binary
    LIBUSB_PATH="$(ldd "${APPDIR}/usr/bin/${APP_NAME}" 2>/dev/null \
        | awk '/libusb-1\.0/ { print $3 }' | head -1)"
fi

if [ -n "${LIBUSB_PATH}" ] && [ -f "${LIBUSB_PATH}" ]; then
    cp "${LIBUSB_PATH}"* "${APPDIR}/usr/lib/" 2>/dev/null || true
    echo -e "${GREEN}✓ Copied libusb from ${LIBUSB_PATH}${NC}"
else
    echo -e "${RED}ERROR: libusb-1.0.so.0 not found. Install libusb-1.0-0-dev and retry.${NC}"
    exit 1
fi

# Fail loudly if desktop icon is missing rather than silently building a broken AppImage
ICON_SRC="packaging/icons/64.png"
if [ ! -f "${ICON_SRC}" ]; then
    echo -e "${RED}ERROR: Icon not found at ${ICON_SRC}. Build the icon set first (see packaging/icons/).${NC}"
    exit 1
fi
cp "${ICON_SRC}" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"
ln -sf "usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png" "${APPDIR}/${APP_NAME}.png"

# Download appimagetool if not present
APPIMAGETOOL="build/appimagetool-${ARCH}.AppImage"
if [ ! -f "${APPIMAGETOOL}" ]; then
    echo -e "${YELLOW}Downloading appimagetool...${NC}"
    wget -q "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-${ARCH}.AppImage" \
        -O "${APPIMAGETOOL}"
    chmod +x "${APPIMAGETOOL}"
fi

# Build AppImage
echo -e "${YELLOW}Building AppImage...${NC}"
mkdir -p dist
ARCH=${ARCH} "${APPIMAGETOOL}" "${APPDIR}" "dist/${APP_NAME}-${VERSION}-${ARCH}.AppImage"

echo ""
echo -e "${GREEN}✓ AppImage built successfully!${NC}"
echo -e "Output: ${GREEN}dist/${APP_NAME}-${VERSION}-${ARCH}.AppImage${NC}"
echo ""
echo "To run:"
echo "  chmod +x dist/${APP_NAME}-${VERSION}-${ARCH}.AppImage"
echo "  ./dist/${APP_NAME}-${VERSION}-${ARCH}.AppImage"
echo ""
echo "Note: Ensure you have USB permissions set up (plugdev group)"
