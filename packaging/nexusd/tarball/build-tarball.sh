#!/usr/bin/env bash
# Builds nexus-open-VERSION-ARCH-linux.tar.gz from binaries already built:
# the package's files (packaging/nexusd/stage.sh) under usr/, plus
# install.sh, which puts them under ~/.local for one user.
#
# Usage: packaging/nexusd/tarball/build-tarball.sh [BINDIR] [OUTDIR]
#   BINDIR  the built nexusd and nexusctl (default target/release); see
#           deb/build-deb.sh on where to build them
#   OUTDIR  default target/dist
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin="$(cd "${1:-$repo/target/release}" && pwd)"
out="$(mkdir -p "${2:-$repo/target/dist}" && cd "${2:-$repo/target/dist}" && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -1)"
name="nexus-open-$version-$(uname -m)-linux"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
"$repo/packaging/nexusd/stage.sh" "$work/$name" "$bin"
# The package scripts' helper and the preset are for system-wide installs.
rm -r "$work/$name/usr/lib/nexus-open" "$work/$name/usr/lib/systemd/user-preset"
install -m755 "$here/install.sh" "$work/$name/install.sh"
install -m644 "$repo/docs/INSTALL.md" "$work/$name/INSTALL.md"
install -m644 "$repo/LICENSE" "$work/$name/LICENSE"

tarball="$out/$name.tar.gz"
tar -C "$work" --owner=0 --group=0 --numeric-owner --sort=name \
    ${SOURCE_DATE_EPOCH:+--mtime=@$SOURCE_DATE_EPOCH} -czf "$tarball" "$name"
echo "$tarball"
