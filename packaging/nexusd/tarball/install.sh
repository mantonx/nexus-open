#!/usr/bin/env bash
# Installs Nexus Open for you alone, under your home folder, from this
# unpacked tarball. Run it again to update.
#
#   ./install.sh               install or update, and start the service
#   ./install.sh --uninstall   remove what install.sh put in place
#
# No root is needed, except for one step: the udev rule that lets you open
# the strip. If it is not installed yet, this script prints the one command
# for it, to run with sudo. Nothing here runs sudo for you.
#
# What goes where (the same places as scripts/nexusd-dev-install.sh):
#   ~/.local/bin/nexusd, ~/.local/bin/nexusctl
#   ~/.config/systemd/user/nexusd.service      the service, enabled for you
#   ~/.local/share/applications/io.nexus.open.desktop, its icon and metainfo
#   ~/.local/share/nexusd/70-nexus-open.rules  the udev rule, for sudo to copy
# Your layouts are kept in ~/.local/state/nexusd; uninstalling leaves them.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
usr="$here/usr"
bin="$HOME/.local/bin"
config="${XDG_CONFIG_HOME:-$HOME/.config}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
units="$config/systemd/user"
apps="$data/applications"
icons="$data/icons/hicolor/256x256/apps"
metainfo="$data/metainfo"
rule_copy="$data/nexusd/70-nexus-open.rules"

files=(
    "$bin/nexusd" "$bin/nexusctl"
    "$units/nexusd.service"
    "$apps/io.nexus.open.desktop"
    "$icons/io.nexus.open.png"
    "$metainfo/io.nexus.open.metainfo.xml"
    "$rule_copy"
)

# Whether this user's service manager can be reached (not, say, over a
# plain su or in a container).
have_user_manager() {
    systemctl --user show-environment >/dev/null 2>&1
}

rule_installed() {
    local d r
    for d in /etc/udev/rules.d /usr/lib/udev/rules.d /lib/udev/rules.d; do
        for r in 70-nexus-open.rules 70-nexus.rules; do
            [[ -e $d/$r ]] && return 0
        done
    done
    return 1
}

uninstall() {
    if have_user_manager; then
        systemctl --user disable --now nexusd.service 2>/dev/null || true
    fi
    rm -f "${files[@]}"
    rmdir "$data/nexusd" 2>/dev/null || true
    if have_user_manager; then
        systemctl --user daemon-reload
    fi
    echo "Nexus Open is uninstalled. Your layouts are still in ~/.local/state/nexusd."
    if [[ -e /etc/udev/rules.d/70-nexus-open.rules ]]; then
        echo
        echo "The udev rule is still installed. To remove it too (needs root):"
        echo
        echo "  sudo rm /etc/udev/rules.d/70-nexus-open.rules && sudo udevadm control --reload"
    fi
}

install_files() {
    if [[ -x /usr/bin/nexusd && -e /usr/lib/systemd/user/nexusd.service && ${FORCE:-} != 1 ]]; then
        echo "Nexus Open is already installed from a package. This copy would hide it:"
        echo "remove the package first, or run FORCE=1 $0 to install anyway."
        exit 1
    fi

    mkdir -p "$bin" "$units" "$apps" "$icons" "$metainfo" "$(dirname "$rule_copy")"
    # install(1) writes a new file and renames it into place, so replacing
    # the binary of a running daemon is safe.
    install -m755 "$usr/bin/nexusd" "$usr/bin/nexusctl" "$bin/"
    sed "s|/usr/bin/nexusd|$bin/nexusd|" "$usr/lib/systemd/user/nexusd.service" >"$units/nexusd.service"
    sed "s|^Exec=nexusd|Exec=$bin/nexusd|" "$usr/share/applications/io.nexus.open.desktop" \
        >"$apps/io.nexus.open.desktop"
    install -m644 "$usr/share/icons/hicolor/256x256/apps/io.nexus.open.png" "$icons/"
    install -m644 "$usr/share/metainfo/io.nexus.open.metainfo.xml" "$metainfo/"
    install -m644 "$usr/lib/udev/rules.d/70-nexus-open.rules" "$rule_copy"
    echo "Installed nexusd and nexusctl in $bin."

    if have_user_manager; then
        if systemctl --user is-active --quiet nexus-open.service; then
            echo "Stopping the old Nexus Open (nexus-open.service) so the two do not fight over the strip."
            echo "If the strip stays dark afterwards, unplug it and plug it back in once."
            systemctl --user disable --now nexus-open.service
        fi
        systemctl --user daemon-reload
        systemctl --user enable nexusd.service >/dev/null
        systemctl --user restart nexusd.service
        echo "The service is running, and starts by itself whenever you log in."
    else
        echo "Your service manager could not be reached from here. In your desktop session, run:"
        echo
        echo "  systemctl --user enable --now nexusd.service"
    fi

    if ! rule_installed; then
        echo
        echo "One step needs root: the udev rule that lets you, at this computer, use the strip."
        echo "Run this once; sudo asks for your password:"
        echo
        echo "  sudo install -m 644 $rule_copy /etc/udev/rules.d/ && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw"
        echo
        echo "The strip lights up a few seconds later, with no replug."
    fi

    case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo && echo "Note: $bin is not on your PATH, so type $bin/nexusctl to run nexusctl." ;;
    esac
    echo
    echo "Open \"Nexus Open Settings\" from your app menu. If something is wrong, nexusctl doctor says what."
}

case "${1:-}" in
"") install_files ;;
--uninstall) uninstall ;;
-h | --help) sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//' ;;
*)
    echo "usage: $0 [--uninstall]" >&2
    exit 2
    ;;
esac
