#!/usr/bin/env bash
# Puts every file the Nexus Open package installs into a staging root, at
# its final path. The Arch, deb and rpm packages and the tarball all pack
# this tree, so the file list is written once, here.
#
# Usage: packaging/nexusd/stage.sh DESTDIR [BINDIR]
#   DESTDIR  the staging root; files land in DESTDIR/usr/...
#   BINDIR   where the built nexusd and nexusctl are (default target/release)
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
dest="${1:?usage: stage.sh DESTDIR [BINDIR]}"
bin="${2:-$repo/target/release}"
licenses="$dest/usr/share/licenses/nexus-open"

cd "$repo"
install -Dm755 "$bin/nexusd" "$dest/usr/bin/nexusd"
install -Dm755 "$bin/nexusctl" "$dest/usr/bin/nexusctl"

# The service, enabled for every user by the preset; the udev rule that
# gives the user at the seat the strip.
install -Dm644 packaging/systemd/nexusd.service "$dest/usr/lib/systemd/user/nexusd.service"
install -Dm644 packaging/systemd/80-nexus-open.preset \
    "$dest/usr/lib/systemd/user-preset/80-nexus-open.preset"
install -Dm644 packaging/udev/70-nexus-open.rules "$dest/usr/lib/udev/rules.d/70-nexus-open.rules"

# The settings window in the app menu and in software centres.
install -Dm644 packaging/desktop/io.nexus.open.desktop \
    "$dest/usr/share/applications/io.nexus.open.desktop"
install -Dm644 packaging/desktop/io.nexus.open.metainfo.xml \
    "$dest/usr/share/metainfo/io.nexus.open.metainfo.xml"
install -Dm644 crates/nexus-ui/assets/icon.png \
    "$dest/usr/share/icons/hicolor/256x256/apps/io.nexus.open.png"

# What the package scripts run (see the file).
install -Dm755 packaging/nexusd/package-hook "$dest/usr/lib/nexus-open/package-hook"

# The app's licence, and the licences of the fonts compiled into it. The
# window's fonts (crates/nexus-ui) are Rubik and Phosphor, whose licences
# are in the strip's set too.
install -Dm644 LICENSE "$licenses/LICENSE"
install -Dm644 -t "$licenses/fonts" crates/nexus-core/assets/fonts/licenses/*
