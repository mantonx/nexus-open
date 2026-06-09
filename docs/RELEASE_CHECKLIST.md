# Release Checklist

## 1. Pre-release testing

```bash
make test-race        # full suite with race detector
make lint             # golangci-lint
make vet              # go vet
make coverage         # verify ≥60% coverage
```

Build and smoke-test locally:

```bash
make build-release
./bin/nexus-open --version
NEXUS_MOCK_DEVICE=1 timeout 5s ./bin/nexus-open || true
make doctor
```

Hardware test (if device available):

```bash
lsusb | grep 1b1c:1b8e
./bin/nexus-open   # verify display, metrics, swipe
```

## 2. Version bump

Update the version string passed via `-ldflags` in the Makefile (`main.version`). Check:

- `Makefile` — `VERSION` variable
- `packaging/arch/PKGBUILD` — `pkgver`
- `packaging/arch/.SRCINFO` — regenerate with `makepkg --printsrcinfo > .SRCINFO`
- `packaging/rpm/nexus-open.spec` — `Version:`
- `packaging/flatpak/com.github.nexusopen.NexusOpen.yaml` — release tag URL

## 3. Changelog

Update `CHANGELOG.md` — add a `[X.Y.Z] — YYYY-MM-DD` section above `[1.0.0]` with curated Added / Changed / Fixed entries. Do not paste raw `git log` output.

## 4. Commit and tag

```bash
git add -p
git commit -m "chore: prepare vX.Y.Z release"
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin main --tags
```

Pushing the tag triggers the `release.yml` CI workflow which builds all packages and creates the GitHub release draft automatically.

## 5. Build packages locally (optional pre-check)

```bash
make deb        # → dist/nexus-open_X.Y.Z_amd64.deb
make rpm        # → dist/nexus-open-X.Y.Z-1.x86_64.rpm
make appimage   # → dist/nexus-open-X.Y.Z-x86_64.AppImage
```

Verify DEB installs cleanly:

```bash
sudo dpkg -i dist/nexus-open_X.Y.Z_amd64.deb
make doctor
sudo dpkg -r nexus-open
```

## 6. GitHub release

After CI completes:

1. Open the draft release at `https://github.com/mantonx/nexus-open/releases`
2. Paste the relevant CHANGELOG section as the release description
3. Verify all artifacts are attached (`.deb`, `.rpm`, `.AppImage`, `SHA256SUMS`)
4. Publish

## 7. Distribution packaging

**AUR:**

```bash
cd packaging/arch/
# Update pkgver and sha256sums
makepkg --printsrcinfo > .SRCINFO
# Push to AUR repo (ssh://aur@aur.archlinux.org/nexus-open.git)
```

**Flathub:** open a PR against `https://github.com/flathub/flathub` with the updated `com.github.nexusopen.NexusOpen.yaml`.

**Snap:** `snapcraft remote-build`, then `snapcraft upload --release=stable`.

## 8. Post-release

- [ ] Announce in GitHub Discussions
- [ ] Monitor issues / AUR comments for regressions
- [ ] If a critical bug is found: hotfix on `main`, tag `vX.Y.Z+1`, re-run this checklist from step 4
