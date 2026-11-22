# Release checklist

For the Rust app (`nexusd`, `nexusctl`). The release bar: someone installs
a package and the strip lights up with no terminal, no replug and no
guess. CI and containers cannot show that; step 3 on real hardware does.

The old Go app's release is `.github/workflows/release-go.yml`, now run by
hand only.

## 1. Choose the version

The packages are named `nexus-open`, like the Go app's, so they replace it.
Package managers only treat the new one as an upgrade if its version is
higher than the Go app's last, **0.3.8**: with 0.1.0, `dnf` refuses to
install it over the Go package, `apt` treats it as a downgrade, and AUR
helpers never offer it. Use 0.4.0 or higher (or an epoch on every format).

Set the same version in all four places; the release workflow fails if
they differ from the tag:

- `Cargo.toml`: `[workspace.package] version`
- `packaging/nexusd/arch/PKGBUILD`: `pkgver` (and `pkgrel=1`)
- `packaging/nexusd/rpm/nexus-open.spec`: `Version:` and a `%changelog` entry
- `packaging/desktop/io.nexus.open.metainfo.xml`: a new `<release>` on top

## 2. Before tagging

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Build the packages locally (no root). The deb, rpm and tarball should be
built from binaries made on an old glibc, as CI does on Ubuntu 24.04; a
binary built on Arch needs Arch's newer glibc.

```
MAKEPKG_FLAGS=--nodeps packaging/nexusd/arch/build-local.sh   # target/dist/*.pkg.tar.zst
packaging/nexusd/deb/build-deb.sh BINDIR                       # needs dpkg-deb
packaging/nexusd/rpm/build-rpm.sh BINDIR                       # needs rpmbuild
packaging/nexusd/tarball/build-tarball.sh BINDIR
```

Install each in a clean container (what CI does):

```
docker run --rm -v "$PWD/packaging/nexusd/test-install.sh:/t.sh:ro" \
  -v "$PWD/target/dist:/dist:ro" archlinux:latest bash /t.sh arch /dist/nexus-open-VERSION-1-x86_64.pkg.tar.zst
# likewise: debian:stable deb, fedora:latest rpm, debian:stable tarball
```

## 3. Clean-machine test, on the strip

Do this on Arch, Debian 13 and Fedora (and Ubuntu 24.04 if you can: it
restricts the user namespaces the service's sandbox uses). A fresh install
of each with a desktop, or a VM with the strip passed through over USB.
Write down every point where a terminal or a guess was needed: each is a
bug.

1. Make a new user. Check `id`: no `plugdev`, `input` or other device
   group.
2. Log in to the desktop as that user. **Plug in the strip before
   installing**, so the install has to grant access to a strip already
   there. It shows Corsair's boot screen or stays dark.
3. Download the package from the release page and install it **the way a
   non-technical user would**: open the file from the file manager, so
   GNOME Software or Discover installs it. Only if the desktop offers no
   way to, use the one command from `docs/INSTALL.md`; note which.
4. Within about 10 seconds of the install finishing, with no logout and no
   replug, **the strip lights** with the default layout. Time it.
5. Open **Nexus Open Settings** from the app menu (search "Nexus"). The
   icon shows; the header says "Strip connected". Change something (move a
   page); the strip follows at once; undo it.
6. Then, in a terminal, for the record:
   - `nexusctl doctor`: every line ok.
   - `getfacl /dev/hidraw*` for the strip's node: a `user:NAME:rw-` entry.
   - `systemctl --user status nexusd`: active, no sandbox errors in
     `journalctl --user -u nexusd -b`.
   - `systemctl --global is-enabled nexusd`: enabled.
7. Log out and back in: the strip lights again.
8. Reboot. At the login screen nothing draws on the strip (the login
   screen's user is skipped: `ps -eo user,comm | grep nexusd` shows only
   you after login). After login it lights.
9. Unplug and replug: back within 2 s.
10. A second user, by fast user switching: note what the strip does. Each
    logged-in user runs a daemon; whether the first one lets go when the
    second takes the seat has not been tested.
11. Remove the package (the software centre, or the command in
    `docs/INSTALL.md`): the service stops, the menu entry goes, and
    `nexusctl` is gone. Install it again: the strip lights again.

**Upgrade from the Go app**, on a machine with the old `nexus-open` 0.3.8
package installed, its service running and the strip lit by it:

1. Install the new package. The old service stops, and the strip comes back
   under nexusd **without a replug** (the package hands the strip back to
   its driver). If it needed a replug, that is a bug in
   `packaging/nexusd/package-hook`'s `rebind_strip`.
2. `nexusctl doctor`: no "old udev rule" line (on Arch the old app left
   `/usr/lib/udev/rules.d/99-corsair-nexus.rules` unowned; the new
   package's install script removes it).
3. `nexusctl import-legacy --print`, then `nexusctl import-legacy --activate`:
   the old layout comes across.

**Tarball**, on any of the above with no package installed:

1. Unpack, run `./install.sh`. It starts the service and prints one sudo
   command for the udev rule. Run that: the strip lights with no replug.
2. `./install.sh --uninstall` removes it and prints the command to remove
   the rule.

## 4. Tag

```
git tag -a vVERSION -m "Nexus Open VERSION"
git push origin vVERSION
```

The release workflow builds the four packages, installs each in clean
containers (Arch, Debian, Ubuntu 24.04, Fedora), and only then creates the
GitHub release with the packages and `SHA256SUMS`. Do not create the
release by hand before it finishes.

Then check the release itself, not the workflow's exit code: download the
files from the release page, `sha256sum -c SHA256SUMS`, install one in a
container with `test-install.sh`, and check that `nexusd --version` prints
the tag's version.

## 5. AUR

Not automated yet. The AUR package `nexus-open` is the Go app until this
is done:

```
cd packaging/nexusd/arch
# in PKGBUILD: the new pkgver, pkgrel=1, and sha256sums from:
makepkg -g
makepkg --printsrcinfo > .SRCINFO
```

Then commit `PKGBUILD`, `nexus-open.install` and `.SRCINFO` to
`ssh://aur@aur.archlinux.org/nexus-open.git`.

## 6. README clip

The README has a marked spot (`<!-- CLIP: ... -->`) for a short clip of
the strip: a swipe, a tap on a media control, and a volume takeover.
