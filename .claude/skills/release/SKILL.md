---
name: release
description: Publish a new version of Nexus Open. Runs make release VERSION=X.Y.Z — updates changelog, bumps packaging files, commits, tags, and pushes. CI builds packages and publishes the GitHub release automatically.
user-invocable: true
---

# /release — Publish a New Version

Runs `make release VERSION=X.Y.Z`, which is the **only correct way** to publish a release. Do not create the GitHub release manually — the AUR publish workflow fires on `release:published` and requires the tarball to already be attached when it does.

## What make release does

1. Validates VERSION is X.Y.Z and the working tree is clean
2. Updates CHANGELOG.md via git-cliff
3. Bumps `packaging/arch/PKGBUILD` pkgver + resets pkgrel=1
4. Bumps `packaging/rpm/nexus-open.spec` Version
5. Commits the release files (`release: vX.Y.Z`)
6. Creates an annotated git tag
7. Pushes the commit and tag to origin

CI takes over from there: builds all packages, creates the GitHub release with assets attached, then AUR publish fires automatically.

## Skill Instructions

### 1. Determine the version

If the user provided a version (e.g. `/release 0.3.0`), use it directly, stripping any leading `v`.

If no version was provided, check the last tag and recent commits to suggest the next version:

```bash
git tag --sort=-v:refname | head -1
git log $(git tag --sort=-v:refname | head -1)..HEAD --oneline
```

- Any `feat:` commits since the last tag → minor bump (0.2.0 → 0.3.0)
- Only `fix:`/`chore:` commits → patch bump (0.2.0 → 0.2.1)
- Any breaking changes → major bump

Ask the user to confirm the version before proceeding.

### 2. Verify the tree is clean

```bash
git status --porcelain
```

If there are uncommitted changes, stop and tell the user to commit or stash them first. Do not stash automatically.

### 3. Run make release

```bash
make release VERSION=<version>
```

The Makefile validates inputs and stops on any error. If it fails, report the exact error — do not attempt to run the steps manually.

### 4. Draft a release description

Before reporting completion, draft a 2–4 sentence human-readable description of what this release is *about*. This goes at the top of the GitHub release notes, above the changelog entries.

A good description answers: what's the headline feature or theme? Is anything a breaking change? Who should care?

Example:
> Adds the media plugin: now-playing display with album art from TMDb and Firefox MPRIS integration. Migrates all plugins to a flat `nexus-` naming scheme — breaking change for anyone with customised plugin paths.

Show the draft to the user and ask them to confirm or edit it. Once confirmed, save it to `/tmp/release-description.md`.

### 5. Report completion

Tell the user the tag has been pushed and link to the Actions page:
`https://github.com/mantonx/nexus-open/actions`

Remind them: CI takes 10–15 minutes. The GitHub release and AUR package will be live once it completes. Do not touch the release on GitHub while CI is running.

### 6. Update the GitHub release notes after CI

Once CI finishes and the GitHub release exists, prepend the confirmed description to the release notes:

```bash
PREV_TAG=$(git tag --sort=-v:refname | grep -v "v<VERSION>" | head -1)
git-cliff "${PREV_TAG}..v<VERSION>" --strip all > /tmp/changelog.md
cat /tmp/release-description.md <(echo) /tmp/changelog.md | GITHUB_TOKEN="" gh release edit "v<VERSION>" --notes-file -
```
