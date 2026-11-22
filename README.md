# Nexus Open

Linux software for the Corsair iCUE Nexus, the 640×48 touch strip that sits
above a keyboard. Corsair makes no Linux software for it; Nexus Open is an
open source (MIT) alternative. It is not made or endorsed by Corsair.

<!-- CLIP: a short clip of the strip on a desk goes here (Matt records it):
     a swipe between pages, a tap on a media control, and a takeover when
     the volume changes. Keep it under 10 seconds and a few MB, e.g.
     docs/media/strip.webm, embedded as a link to the file. -->

[![Release](https://img.shields.io/github/v/release/mantonx/nexus-open)](https://github.com/mantonx/nexus-open/releases/latest)
[![Rust](https://github.com/mantonx/nexus-open/actions/workflows/rust.yml/badge.svg)](https://github.com/mantonx/nexus-open/actions/workflows/rust.yml)
![License](https://img.shields.io/badge/license-MIT-blue)

## What it is

The strip becomes a glance display and a control surface: the time,
temperatures and load, network, the weather and what is playing, at a
glance; media, volume and launchers under a finger. You arrange it as pages
of widgets and swipe between them. When something changes, such as the
track or the volume, a takeover covers the page for a moment.

You set it up in a native settings window with a live preview. Every edit
shows on the strip at once and can be undone. There is no account, no cloud
service, and no root once it is installed.

## How it is built

A small daemon, `nexusd`, runs as a systemd user service. It reads the
machine's sensors, the media player (MPRIS) and the weather, draws the strip
with its own renderer, and talks to the strip over hidraw; a udev rule gives
the user at the computer access. Its layouts live in one SQLite database,
and the daemon is the only program that writes to it. The settings window
(`nexusd ui`, built with Slint) and the command line (`nexusctl`) send it
edits over a socket in your runtime folder. Your own programs can feed the
strip too, by printing lines the daemon reads.

## Install

Packages for Arch, Debian and Ubuntu, and Fedora, and a tarball for any
other distribution, are on the
[releases page](https://github.com/mantonx/nexus-open/releases/latest).
Install one and the strip lights up a few seconds later; open **Nexus Open
Settings** from your app menu to set it up.

[docs/INSTALL.md](docs/INSTALL.md) has the steps for each distribution,
what the package does, and how to uninstall.

**Coming from the old Nexus Open** (the Go app): the new package replaces
it. To bring your old layout across, run `nexusctl import-legacy`.

## Set it up

- [docs/LAYOUT.md](docs/LAYOUT.md): every setting of a layout, for editing
  it as text with `nexusctl edit`, or sharing it with `export` and `import`.
- [docs/EXTERNAL_SOURCES.md](docs/EXTERNAL_SOURCES.md): show readings from
  your own programs and scripts on the strip.

## When something is wrong

`nexusctl doctor` checks everything the strip needs and says how to fix
what is wrong. The settings window runs the same checks and shows a
problem in its header. [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)
explains each check.

## Build from source

Rust 1.100 (the version is pinned in `rust-toolchain.toml`) and, on Linux,
the fontconfig headers (`libfontconfig-dev` or `fontconfig-devel`).

```
cargo build --release --locked -p nexusd -p nexusctl
scripts/nexusd-dev-install.sh   # installs under ~/.local and runs the service
```

`nexusd run --no-device --demo` runs without the strip, with made-up
readings; the settings window's preview shows what it would draw.
Packaging lives in `packaging/nexusd/`.

## License

MIT. See [LICENSE](LICENSE). The fonts compiled into the app are under the
OFL, MIT and Bitstream Vera licences; see
`crates/nexus-core/assets/fonts/licenses/`.
