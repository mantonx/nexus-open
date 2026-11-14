#!/usr/bin/env bash
# Builds the Rust daemon and runs it as this user's service, for daily use
# before there is a package. Safe to rerun after every change: it rebuilds,
# replaces the binaries and restarts the service.
#
# Installs, all under your home directory (no sudo):
#   ~/.local/bin/nexusd, ~/.local/bin/nexusctl
#   ~/.config/systemd/user/nexusd.service   (the packaged unit, pointed at ~/.local/bin)
#   ~/.local/share/applications/io.nexus.open.desktop and its icon
#
# Undo: systemctl --user disable --now nexusd, then delete those files.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
bin="$HOME/.local/bin"
units="$HOME/.config/systemd/user"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/256x256/apps"

cargo build --release --locked --manifest-path "$repo/Cargo.toml" -p nexusd -p nexusctl

mkdir -p "$bin" "$units" "$apps" "$icons"
# install(1) writes a new file and renames it into place, so replacing the
# binary of a running daemon is safe.
install -m 755 "$repo/target/release/nexusd" "$repo/target/release/nexusctl" "$bin/"
sed "s|/usr/bin/nexusd|$bin/nexusd|" "$repo/packaging/systemd/nexusd.service" > "$units/nexusd.service"
sed "s|^Exec=nexusd|Exec=$bin/nexusd|" "$repo/packaging/desktop/io.nexus.open.desktop" > "$apps/io.nexus.open.desktop"
install -m 644 "$repo/crates/nexus-ui/assets/icon.png" "$icons/io.nexus.open.png"

if systemctl --user is-active --quiet nexus-open.service; then
    echo "Stopping the old Go service (nexus-open) so the two do not fight over the strip."
    systemctl --user disable --now nexus-open.service
fi

systemctl --user daemon-reload
systemctl --user enable nexusd.service >/dev/null
systemctl --user restart nexusd.service
sleep 1
"$bin/nexusctl" status
