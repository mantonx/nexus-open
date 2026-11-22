#!/usr/bin/env bash
# Builds the Arch package from this checkout, with no root: the source is
# `git archive HEAD` (committed files only) in place of the tag tarball.
#
# Usage: packaging/nexusd/arch/build-local.sh [OUTDIR]   (default target/dist)
# Extra makepkg flags go in MAKEPKG_FLAGS, e.g. MAKEPKG_FLAGS=--nocheck.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="$(mkdir -p "${1:-$repo/target/dist}" && cd "${1:-$repo/target/dist}" && pwd)"
ver="$(sed -n 's/^pkgver=//p' "$here/PKGBUILD")"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here/PKGBUILD" "$here/nexus-open.install" "$work/"
git -C "$repo" archive --format=tar.gz --prefix="nexus-open-$ver/" \
    -o "$work/nexus-open-$ver.tar.gz" HEAD

cd "$work"
# shellcheck disable=SC2086 # MAKEPKG_FLAGS is a list of flags
PKGDEST="$out" SRCDEST="$work" BUILDDIR="$work/build" makepkg -f ${MAKEPKG_FLAGS:-}
