#!/usr/bin/env bash
# Installs a built package in a clean container and checks it: the files
# land where they should, the binaries run, the unit and the udev rule
# pass systemd's own checks, the service is enabled for every user, and
# removing the package takes it all away again.
#
# Run as root inside the container (the release workflow does this in
# archlinux, debian:stable and fedora):
#   packaging/nexusd/test-install.sh arch    nexus-open-*.pkg.tar.zst
#   packaging/nexusd/test-install.sh deb     nexus-open_*.deb
#   packaging/nexusd/test-install.sh rpm     nexus-open-*.rpm
#   packaging/nexusd/test-install.sh tarball nexus-open-*-linux.tar.gz
#
# A container has no strip, no udev daemon and nobody logged in, so this
# cannot show the strip lighting up: docs/RELEASE_CHECKLIST.md has the
# steps for a real machine.
set -euo pipefail

kind="${1:?usage: test-install.sh arch|deb|rpm|tarball FILE}"
file="$(realpath "${2:?usage: test-install.sh arch|deb|rpm|tarball FILE}")"
failed=0

ok() { echo "  ok    $*"; }
fail() {
    echo "  FAIL  $*"
    failed=1
}
check() {
    local what="$1"
    shift
    if "$@" >/tmp/check.out 2>&1; then ok "$what"; else
        fail "$what"
        sed 's/^/        /' /tmp/check.out
    fi
}

files=(
    /usr/bin/nexusd
    /usr/bin/nexusctl
    /usr/lib/systemd/user/nexusd.service
    /usr/lib/systemd/user-preset/80-nexus-open.preset
    /usr/lib/udev/rules.d/70-nexus-open.rules
    /usr/share/applications/io.nexus.open.desktop
    /usr/share/metainfo/io.nexus.open.metainfo.xml
    /usr/share/icons/hicolor/256x256/apps/io.nexus.open.png
    /usr/lib/nexus-open/package-hook
)

run_checks() {
    echo "== files"
    for f in "${files[@]}"; do check "$f" test -e "$f"; done
    check "binaries are executable" test -x /usr/bin/nexusd -a -x /usr/bin/nexusctl
    check "a licence is installed" sh -c 'test -e /usr/share/licenses/nexus-open/LICENSE || test -e /usr/share/doc/nexus-open/copyright'

    echo "== binaries"
    check "nexusd --version" nexusd --version
    sed 's/^/        /' /tmp/check.out
    check "nexusctl --help" nexusctl --help

    echo "== systemd"
    check "systemd-analyze verify" systemd-analyze verify /usr/lib/systemd/user/nexusd.service
    check "enabled for every user" sh -c '[ "$(systemctl --global is-enabled nexusd.service)" = enabled ]'
    if udevadm verify --help >/dev/null 2>&1; then
        check "udevadm verify" udevadm verify /usr/lib/udev/rules.d/70-nexus-open.rules
    fi

    echo "== desktop"
    if command -v desktop-file-validate >/dev/null; then
        check "desktop-file-validate" desktop-file-validate /usr/share/applications/io.nexus.open.desktop
    fi
    if command -v appstreamcli >/dev/null; then
        check "appstreamcli validate" appstreamcli validate --no-net /usr/share/metainfo/io.nexus.open.metainfo.xml
    fi
}

removed_checks() {
    echo "== after removal"
    for f in "${files[@]}"; do check "$f is gone" test ! -e "$f"; done
    check "no longer enabled" sh -c '! [ -e /etc/systemd/user/default.target.wants/nexusd.service ]'
}

case "$kind" in
arch)
    pacman -Sy --noconfirm --needed desktop-file-utils appstream >/dev/null
    pacman -U --noconfirm "$file"
    run_checks
    pacman -R --noconfirm nexus-open
    removed_checks
    ;;
deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq desktop-file-utils appstream >/dev/null
    apt-get install -y "$file"
    run_checks
    apt-get remove -y nexus-open
    removed_checks
    ;;
rpm)
    dnf install -y -q desktop-file-utils appstream
    dnf install -y "$file"
    run_checks
    dnf remove -y nexus-open
    removed_checks
    ;;
tarball)
    # As an ordinary user, with no service manager (as over plain su):
    # install.sh must still put every file in place and say what to run.
    id tester >/dev/null 2>&1 || useradd -m tester
    work="$(mktemp -d)"
    tar -C "$work" -xzf "$file"
    chmod -R a+rX "$work"
    dir="$(echo "$work"/nexus-open-*)"
    home=/home/tester
    echo "== install.sh (as a user)"
    check "install.sh" su tester -c "$dir/install.sh"
    sed 's/^/        /' /tmp/check.out
    for f in .local/bin/nexusd .local/bin/nexusctl .config/systemd/user/nexusd.service \
        .local/share/applications/io.nexus.open.desktop \
        .local/share/icons/hicolor/256x256/apps/io.nexus.open.png \
        .local/share/metainfo/io.nexus.open.metainfo.xml \
        .local/share/nexusd/70-nexus-open.rules; do
        check "~/$f" test -e "$home/$f"
    done
    check "unit runs ~/.local/bin/nexusd" grep -q "^ExecStart=$home/.local/bin/nexusd run$" "$home/.config/systemd/user/nexusd.service"
    check "menu entry runs ~/.local/bin/nexusd" grep -q "^Exec=$home/.local/bin/nexusd ui$" "$home/.local/share/applications/io.nexus.open.desktop"
    check "install.sh prints the udev command" grep -q "sudo install -m 644 $home/.local/share/nexusd/70-nexus-open.rules /etc/udev/rules.d/" /tmp/check.out
    check "nexusd --version" su tester -c "$home/.local/bin/nexusd --version"
    check "nexusctl --help" su tester -c "$home/.local/bin/nexusctl --help"
    check "systemd-analyze verify" systemd-analyze verify "$home/.config/systemd/user/nexusd.service"
    echo "== install.sh --uninstall"
    check "install.sh --uninstall" su tester -c "$dir/install.sh --uninstall"
    check "nothing left in ~/.local/bin" test ! -e "$home/.local/bin/nexusd"
    check "no unit left" test ! -e "$home/.config/systemd/user/nexusd.service"
    ;;
*)
    echo "unknown package kind: $kind" >&2
    exit 2
    ;;
esac

if [[ $failed == 0 ]]; then echo "All checks passed ($kind)."; else echo "Some checks FAILED ($kind)."; fi
exit "$failed"
