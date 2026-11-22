#!/usr/bin/env bash
# Builds the .deb from binaries already built, with dpkg-deb (no root: file
# owners are set to root in the archive).
#
# Usage: packaging/nexusd/deb/build-deb.sh [BINDIR] [OUTDIR]
#   BINDIR  the built nexusd and nexusctl (default target/release). Build them
#           on the oldest system the package should run on: the release
#           workflow uses Ubuntu 24.04, which runs on Debian 13 and newer.
#   OUTDIR  default target/dist
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin="$(cd "${1:-$repo/target/release}" && pwd)"
out="$(mkdir -p "${2:-$repo/target/dist}" && cd "${2:-$repo/target/dist}" && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -1)"
arch="$(dpkg --print-architecture)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/root"
"$repo/packaging/nexusd/stage.sh" "$root" "$bin"

# Debian keeps licences in /usr/share/doc/<package>/copyright, not
# /usr/share/licenses.
doc="$root/usr/share/doc/nexus-open"
mkdir -p "$doc"
{
    echo "Nexus Open, from https://github.com/mantonx/nexus-open"
    echo
    echo "Nexus Open is under the MIT licence:"
    echo
    cat "$repo/LICENSE"
    echo
    echo "The fonts compiled into nexusd come with these licences."
    for f in "$root"/usr/share/licenses/nexus-open/fonts/*; do
        echo
        echo "==> $(basename "$f") <=="
        echo
        cat "$f"
    done
} >"$doc/copyright"
rm -r "$root/usr/share/licenses"
# Rust links its crates in statically. lintian takes unsafe-libyaml (the
# Rust translation of libyaml, behind serde_yaml_ng, which reads the old
# app's layouts) for a copy of the C library.
mkdir -p "$root/usr/share/lintian/overrides"
printf '%s\n' "# unsafe-libyaml, a Rust crate, not the C libyaml; Rust links crates statically." \
    "nexus-open: embedded-library libyaml [usr/bin/nexusctl]" \
    >"$root/usr/share/lintian/overrides/nexus-open"
printf '%s\n' \
    "nexus-open ($version) unstable; urgency=medium" "" \
    "  * Release $version. See https://github.com/mantonx/nexus-open/releases" "" \
    " -- Matt <matthew.panton@gmail.com>  $(date -R -u -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}")" |
    gzip -9n >"$doc/changelog.gz"
chmod 644 "$doc/copyright" "$doc/changelog.gz"

# Libraries the binaries link: from dpkg-shlibdeps where it is installed
# (it needs a debian/control to run, and the linked libraries installed:
# libfontconfig1 and libfreetype6), else the known list.
depends="libc6, libgcc-s1, libfontconfig1, libfreetype6"
if command -v dpkg-shlibdeps >/dev/null; then
    mkdir -p "$work/shlibs/debian"
    printf 'Source: nexus-open\n\nPackage: nexus-open\nArchitecture: any\n' >"$work/shlibs/debian/control"
    depends="$(cd "$work/shlibs" && dpkg-shlibdeps -O "$root/usr/bin/nexusd" "$root/usr/bin/nexusctl" |
        sed -n 's/^shlibs:Depends=//p')"
fi

mkdir -p "$root/DEBIAN"
install -m755 "$here/postinst" "$here/prerm" "$here/postrm" "$root/DEBIAN/"
cat >"$root/DEBIAN/control" <<EOF
Package: nexus-open
Version: $version
Architecture: $arch
Maintainer: Matt <matthew.panton@gmail.com>
Installed-Size: $(du -sk --exclude=DEBIAN "$root" | cut -f1)
Depends: $depends, libxkbcommon0, systemd, udev
Recommends: libwayland-client0, libwayland-cursor0, libwayland-egl1, libegl1, libgl1, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrandr2, libxkbcommon-x11-0
Section: utils
Priority: optional
Homepage: https://github.com/mantonx/nexus-open
Description: drives the Corsair iCUE Nexus touch strip
 Nexus Open shows the time, readings from the computer, the weather and
 what is playing on the Corsair iCUE Nexus touch strip, and puts media,
 volume and launchers under a finger. A user service draws the strip from
 login; the settings window (nexusd ui) edits it with a live preview.
 .
 Not made or endorsed by Corsair.
EOF

deb="$out/nexus-open_${version}_${arch}.deb"
dpkg-deb --root-owner-group -Zxz --build "$root" "$deb"
echo "$deb"
