#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building Nexus Open DEB package...${NC}"

# Configuration
VERSION="1.0.0"
ARCH="amd64"
PKG_NAME="nexus-open_${VERSION}_${ARCH}"
BUILD_DIR="build/deb/${PKG_NAME}"

# Clean previous build
echo -e "${YELLOW}Cleaning previous build...${NC}"
rm -rf build/deb
mkdir -p "${BUILD_DIR}"

# Create directory structure
echo -e "${YELLOW}Creating package structure...${NC}"
mkdir -p "${BUILD_DIR}/DEBIAN"
mkdir -p "${BUILD_DIR}/usr/bin"
mkdir -p "${BUILD_DIR}/usr/share/applications"
mkdir -p "${BUILD_DIR}/usr/share/doc/nexus-open"
mkdir -p "${BUILD_DIR}/etc/udev/rules.d"
mkdir -p "${BUILD_DIR}/usr/lib/systemd/user"

# Build the Go binary
echo -e "${YELLOW}Building Go binary...${NC}"
CGO_ENABLED=1 go build -o "${BUILD_DIR}/usr/bin/nexus-open" \
    -ldflags "-X main.version=${VERSION} -X main.commit=$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')" \
    ./cmd/nexus-open

# Strip binary to reduce size
strip "${BUILD_DIR}/usr/bin/nexus-open"

# Build and install external plugins
echo -e "${YELLOW}Building external plugins...${NC}"
mkdir -p "${BUILD_DIR}/usr/lib/nexus-open/plugins"
for mod in cpu-temp gpu-temp network weather cpu-load gpu-load; do
    if [ -d "plugins/${mod}" ]; then
        echo "  → plugins/${mod}"
        (cd "plugins/${mod}" && go build -o "${mod}" .) || exit 1
        cp "plugins/${mod}/${mod}" "${BUILD_DIR}/usr/lib/nexus-open/plugins/"
        strip "${BUILD_DIR}/usr/lib/nexus-open/plugins/${mod}" 2>/dev/null || true
    fi
done
echo -e "${GREEN}✓ Plugins built${NC}"

# Copy control files
echo -e "${YELLOW}Copying control files...${NC}"
cp packaging/deb/DEBIAN/control "${BUILD_DIR}/DEBIAN/"
cp packaging/deb/DEBIAN/postinst "${BUILD_DIR}/DEBIAN/"
cp packaging/deb/DEBIAN/prerm "${BUILD_DIR}/DEBIAN/"
chmod 755 "${BUILD_DIR}/DEBIAN/postinst"
chmod 755 "${BUILD_DIR}/DEBIAN/prerm"

# Copy desktop file
echo -e "${YELLOW}Copying desktop file...${NC}"
cp packaging/desktop/nexus-open.desktop "${BUILD_DIR}/usr/share/applications/"

# Copy udev rules
echo -e "${YELLOW}Copying udev rules...${NC}"
cp packaging/udev/99-corsair-nexus.rules "${BUILD_DIR}/etc/udev/rules.d/"

# Copy systemd service
echo -e "${YELLOW}Copying systemd service...${NC}"
cp packaging/systemd/nexus-open.service "${BUILD_DIR}/usr/lib/systemd/user/"

# Copy documentation
echo -e "${YELLOW}Copying documentation...${NC}"
cp README.md "${BUILD_DIR}/usr/share/doc/nexus-open/"
cp PROJECT_PLAN.md "${BUILD_DIR}/usr/share/doc/nexus-open/" 2>/dev/null || true

# Create copyright file
cat > "${BUILD_DIR}/usr/share/doc/nexus-open/copyright" << EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: nexus-open
Source: https://github.com/mantonx/nexus-next

Files: *
Copyright: $(date +%Y) Nexus Open Team
License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a copy
 of this software and associated documentation files (the "Software"), to deal
 in the Software without restriction, including without limitation the rights
 to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included in all
 copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 SOFTWARE.
EOF

# Set permissions
echo -e "${YELLOW}Setting permissions...${NC}"
find "${BUILD_DIR}" -type d -exec chmod 755 {} \;
find "${BUILD_DIR}" -type f -exec chmod 644 {} \;
chmod 755 "${BUILD_DIR}/usr/bin/nexus-open"
chmod 755 "${BUILD_DIR}/DEBIAN/postinst"
chmod 755 "${BUILD_DIR}/DEBIAN/prerm"

# Calculate installed size
INSTALLED_SIZE=$(du -sk "${BUILD_DIR}" | cut -f1)
sed -i "s/^Installed-Size:.*/Installed-Size: ${INSTALLED_SIZE}/" "${BUILD_DIR}/DEBIAN/control" || \
    echo "Installed-Size: ${INSTALLED_SIZE}" >> "${BUILD_DIR}/DEBIAN/control"

# Build the package
echo -e "${YELLOW}Building DEB package...${NC}"
dpkg-deb --build --root-owner-group "${BUILD_DIR}"

# Move to output directory
mkdir -p dist
mv "build/deb/${PKG_NAME}.deb" dist/

echo ""
echo -e "${GREEN}✓ Package built successfully!${NC}"
echo -e "Output: ${GREEN}dist/${PKG_NAME}.deb${NC}"
echo ""
echo "To install:"
echo "  sudo dpkg -i dist/${PKG_NAME}.deb"
echo "  sudo apt-get install -f  # Fix any dependency issues"
echo ""
echo "Package info:"
dpkg-deb --info "dist/${PKG_NAME}.deb"
