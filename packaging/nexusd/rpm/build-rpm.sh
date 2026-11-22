#!/usr/bin/env bash
# Builds the .rpm from binaries already built, with rpmbuild and no root.
# The source tarball is `git archive HEAD` (committed files only).
#
# Usage: packaging/nexusd/rpm/build-rpm.sh [BINDIR] [OUTDIR]
#   BINDIR  the built nexusd and nexusctl (default target/release); see
#           deb/build-deb.sh on where to build them
#   OUTDIR  default target/dist
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin="$(cd "${1:-$repo/target/release}" && pwd)"
out="$(mkdir -p "${2:-$repo/target/dist}" && cd "${2:-$repo/target/dist}" && pwd)"
version="$(sed -n 's/^Version: *//p' "$here/nexus-open.spec")"

top="$(mktemp -d)"
trap 'rm -rf "$top"' EXIT
mkdir -p "$top/SOURCES"
git -C "$repo" archive --format=tar.gz --prefix="nexus-open-$version/" \
    -o "$top/SOURCES/nexus-open-$version.tar.gz" HEAD

rpmbuild -bb "$here/nexus-open.spec" \
    --define "_topdir $top" \
    --define "prebuilt $bin"
cp "$top"/RPMS/*/nexus-open-*.rpm "$out/"
ls "$out"/nexus-open-"$version"-*.rpm
