#!/bin/bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

VERSION="1.0.0"
APP_NAME="nexus-open"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "${SCRIPT_DIR}")"

echo -e "${GREEN}Building Nexus Open RPM...${NC}"

if ! command -v rpmbuild &>/dev/null; then
    echo -e "${RED}ERROR: rpmbuild not found. Install rpm-build:${NC}"
    echo "  Fedora/RHEL: sudo dnf install rpm-build"
    echo "  openSUSE:    sudo zypper install rpm-build"
    exit 1
fi

# Create rpmbuild tree
RPM_DIR="${HOME}/rpmbuild"
mkdir -p "${RPM_DIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Create source tarball
TARBALL="${RPM_DIR}/SOURCES/${APP_NAME}-${VERSION}.tar.gz"
echo -e "${YELLOW}Creating source tarball...${NC}"
git archive --format=tar.gz --prefix="${APP_NAME}-${VERSION}/" HEAD \
    -o "${TARBALL}"

# Copy spec
cp "${ROOT_DIR}/packaging/rpm/${APP_NAME}.spec" "${RPM_DIR}/SPECS/"

# Build
echo -e "${YELLOW}Running rpmbuild...${NC}"
rpmbuild -ba "${RPM_DIR}/SPECS/${APP_NAME}.spec"

# Copy to dist/
mkdir -p "${ROOT_DIR}/dist"
find "${RPM_DIR}/RPMS" -name "*.rpm" -exec cp {} "${ROOT_DIR}/dist/" \;

echo -e "${GREEN}✓ RPM built successfully!${NC}"
ls -lh "${ROOT_DIR}/dist/"*.rpm
