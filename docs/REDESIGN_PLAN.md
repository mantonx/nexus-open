# Nexus redesign plan

Status: rebuild confirmed by Matt on 2026-11-14. Written for an agent (Claude
Opus) to execute with Matt. Phases 0 and 1 are skipped; work is in Phase 2
on branch `redesign`.

## How to use this plan

- Work one phase at a time, in order. Each phase has an exit test. Do not start
  the next phase until the exit test passes and you have reported the result.
- **Stop and ask Matt** at every point marked **GATE**. Those are decisions he
  owns or actions that touch his system.
- Where this plan gives a design, treat it as the starting point. If the code
  or a spike contradicts it, say so and propose the change instead of forcing
  the plan through.
- The project `CLAUDE.md` still applies: state the root cause in one sentence
  before any fix, and slow down when things go wrong.
- Report honestly. If an exit test fails or was not run, say that.

## Goal

Rebuild the app as a single Rust codebase with a native Slint UI. The product
changes from a polled stats dashboard into an event-driven touch surface:

- A stable layout of widgets, bound to named signals.
- Input that does things: tap actions, sliders, long-press.
- Temporary takeovers (media scrub, volume, notifications) over the stable
  layout. The layout does not rearrange itself based on context.
- Plugins in any language.
- Designed for Linux, macOS and Windows. Ships on Linux first, then macOS,
  then Windows. Each release must meet the 9/10 bar on real hardware on that
  OS. Matt can test on Linux and macOS; Windows needs a test machine first.

Out of scope: WASM plugins, a focused-window context engine, a web UI
(Matt rejected web; the UI must be native). Other Corsair devices are
planned separately in `docs/DEVICES_PLAN.md`.

## The 9/10 bar

A 9 means the planned scope works every time and feels finished. It does not
mean more features. Every item below is pass or fail with evidence, and
carries the phase that must deliver it (`[P2]` = Phase 2). A phase's exit
test includes its items. Build these in as you go; do not leave them for the
end.

**First run**

- `[P5]` Package install to a lit strip with no terminal, no replug, no
  reboot and no group membership.
- `[P4]` Onboarding appears only when something is wrong. It names the
  problem and offers the exact fix, using the same checks as `nexusctl doctor`.
- `[P3]` The default layout looks designed and is complete with zero
  configuration. A widget that needs setup (weather location) shows a clear
  "set this up" state, not an error.

**The strip**

- `[P2]` Text is crisp at every size. One type scale, one spacing rule and
  one baseline rule, defined in the theme and used by every widget.
- `[P2]` Swipes and transitions hold the target frame rate with no dropped
  frames. Measure the highest rate the device sustains and use it; 30 fps is
  the floor.
- `[P2]` Touch to visible feedback in under 50 ms, measured from HID report
  to frame sent.
- `[P2]` Gestures: rubber-band at the first and last page, a swipe can be
  reversed mid-gesture, velocity comes from a trailing window.
- `[P2]` A config reload does not flicker or reset the current page.
- `[P3]` Accidental touches do nothing. The strip sits above the keyboard, so
  a brush while typing must not fire an action. Set the thresholds from
  recorded touch traces and keep the traces as tests.
- `[P3]` Every source failure has a calm, designed state. No flashing
  "Stale" or "Error", and no repeating log lines.
- `[P3]` The strip follows the PC: it dims or blanks when the screen locks or
  the displays sleep, and is back within 2 s of resume. Brightness is a
  setting, with an optional night schedule. None of this exists today.

**Settings app**

- `[P4]` Window visible in under 1 s from a cold start.
- `[P4]` Every edit shows on the strip immediately. Undo and redo cover every
  edit. A disconnect loses nothing.
- `[P4]` Fully usable from the keyboard, with screen-reader labels.
- `[P4]` Follows system light and dark, scales correctly at fractional
  scaling, and remembers its window size.
- `[P4]` Starter layouts to choose from, and import and export of a layout as
  a file.
- `[P4]` Errors say what happened and what to do next. No test or debug
  widgets are visible.
- `[P4]` Locale-aware: 12 or 24 hour, °C or °F, strings ready to translate.

**Reliability**

- `[P2]` A bad config never blanks the strip. Keep the last good config and
  show the error in the UI and in `nexusctl`.
- `[P2]` Unplug and replug recovers within 2 s. Suspend and resume recover
  without a restart.
- `[P3]` A crashing or hanging source is isolated, restarted with backoff,
  and cannot stall rendering.
- `[P5]` 72-hour soak on hardware: no crash, RSS drift under 5 MB, no
  repeating log lines.
- `[P5]` Fuzz the config parser, the HID report parser and the plugin line
  protocol.
- `[P5]` The systemd unit restarts on failure, uses a watchdog, and its
  hardening is tested with the UI running.

**Performance budget** (on hardware, over 60 s)

- `[P2]` Daemon idle CPU under 2%, RSS under 40 MB, first frame within
  500 ms of the device connecting.
- `[P4]` A closed UI costs nothing: no preview frames are produced without a
  viewer.
- `[P5]` A simulator benchmark in CI fails the build on a render-cost
  regression.

**Supportability**

- `[P2]` `nexusctl doctor` reports: device found, access granted, who holds
  the device, config valid, versions. `nexusctl status` shows live state.
- `[P4]` A "copy diagnostics" button in the UI.
- `[P5]` A troubleshooting page keyed by `doctor` output.

**Safety**

- `[P2]` The control socket lives in `$XDG_RUNTIME_DIR` with mode 0600.
- `[P3]` Commands run only from the user's own config, and external sources
  must be listed there. Image decoding has size limits.

**Project quality**

- `[P2]` CI runs format, lint, tests, image tests with tolerance, and a
  dependency audit.
- `[P2]` Everything is testable on the simulator, without hardware.
- `[P2]` The config has a schema version and migrations.
- `[P5]` README with a short clip of the strip, a plugin spec with two
  examples, and a changelog.
- `[P5]` The release is verified by installing the built package on clean
  Arch, Debian and Fedora containers.

**Scoring.** Before release, go through this list with Matt. Each item is
pass, fail or waived by Matt, with the evidence noted. A 9 is every item
passing or explicitly waived.

## Why a rewrite, and why Rust

Recorded so the decision is not re-litigated without new evidence.

- The redesign replaces the plugin protocol, the sampler, the payload and
  render model, the state model and the UI. That is most of the Go daemon.
- Nobody depends on the current app: 0 stars, 1 fork, no human-filed issues.
  There is nothing to keep compatible.
- Rust is the option where the daemon and a native UI share one language.
- None of the measured problems below are caused by Go. Do not justify design
  choices with "Rust is faster".

## Facts established on 2026-11-14

Verified first-hand unless marked otherwise.

**Live system**

- The daemon cannot open the device. Log: `USB permission denied: open
  /dev/bus/usb/001/015`. The node is `crw------- root root` with no ACL.
- Root cause: the rule file is `99-corsair-nexus.rules`. Its `TAG+="uaccess"`
  is set after `73-seat-late.rules` has run the `uaccess` builtin, so no ACL
  is applied. `/dev/snd/controlC0` has the session ACL; the Nexus nodes have
  the tag but no ACL. Commit `fa9b530` changed `MODE` from `0660` +
  `GROUP=plugdev` to `0600`, leaving `uaccess` as the only path.
- The daemon uses 24–26% of a core while disconnected (measured twice over
  5 s from `/proc/<pid>/stat`). 95% is user time.
- Benchmarked cause: `UpdatePayload` runs `go m.preRenderAdjacentPages()` on
  every plugin sample (`internal/zone/manager_render.go:68`). On the live
  layout shape that costs 8.2 ms and 8 MB per call, about 11 times a second.
  The preview PNG path costs 2.4 ms per frame at 10 fps; a dirty render costs
  0.6 ms. This attribution is from benchmarks, not a live profile.
- Memory: 398 MB for the service cgroup, 13 processes. Each zone instance
  gets its own plugin process.

**Hardware**

- USB `1b1c:1b8e`. Interface 0 is HID class: interrupt IN `0x81` (512 bytes),
  interrupt OUT `0x02` (1024 bytes). Interface 1 is a boot keyboard.
- Frames: 640×48 RGBA, sent as 121 packets of 1024 bytes with an 8-byte header
  starting `0x02 0x05 0x40`. Full format in `docs/USB_PROTOCOL.md`.
- Quirk: EP `0x81` must be primed with a short read before the device accepts
  writes on EP `0x02`.
- Touch is one finger, X axis only, raw range 0–486, auto-calibrated upward.
  No Y, no pressure, no multi-touch.

**Current code worth carrying over (as knowledge, not structure)**

- `docs/USB_PROTOCOL.md`, `internal/device/usbfs.go`, `internal/device/nexus.go`
  for the protocol and reconnect behaviour.
- `internal/touch/reader.go` and `swipe_config.go` for gesture thresholds.
- `internal/zone/renderer.go` and `renderer_graphs.go` for the visual look.
- `testdata/golden/*.png` (11 images) and `docs/screenshots/display.png` as
  visual references.
- `plugins/*` for how each data source reads its data.
- `internal/fonts/assets/` for fonts; `design/tokens.json` for design tokens.

**Hazards**

- Never start a second copy of the Go daemon. `writePIDFile` in
  `cmd/nexus-open/main.go` runs unconditionally and sends SIGTERM to every
  process whose name is `nexus-open`, plus the UI and plugins.
- The new binary must not be named `nexus-open`, or the old daemon will kill
  it on start. Working names: `nexusd` (daemon and UI) and `nexusctl` (CLI).
- Only one process can hold the device, so the old service must be stopped
  before any hardware test. See Phase 1.
- Anything needing `sudo` is a **GATE**.
- Temporary benchmark files were used in `internal/app` and `internal/zone`
  during the investigation and removed. The tree was clean afterwards.

**Not verified**

- Whether the udev fix works; it has not been applied.
- Whether the device accepts frames through hidraw (Phase 2, milestone 2).
- How the hinted text of a Rust renderer looks on the real panel (Phase 2, milestone 1).
- Whether Slint runs cleanly on KDE Wayland with NVIDIA (Phase 2, milestone 3).
- Whether Slint exposes controls to screen readers on Linux (Phase 2, milestone 3).
- The device's highest sustainable frame rate. The Go daemon targets 30 fps.
- Which signals KDE and GNOME emit for screen lock and display sleep.
- Matt's Rust experience. Ask; it affects how much explanation to give.

## Phase 0 — Skipped

Matt chose to put all effort into the rebuild. Do not fix the Go or Flutter
code. The known problems there (idle CPU, the tray token 401, the committed
`plugins/media/music` binary) stay as they are until the code is archived.

One thing carries over: the udev ordering bug blocks the rebuild too, because
the new code needs the same device node. It is fixed as part of Phase 1.

## Phase 1 — Removed

Matt chose to build the real app directly instead of throwaway spikes. The
three risks the spikes covered are now the first milestones of Phase 2,
checked in real code. If one fails, fix it there or come back with a toolkit
change.

Device access is still needed before any hardware test:

1. The udev rule is `data/udev/70-nexus.rules`. It must sort before
   `73-seat-late.rules`, which applies ACLs only to devices already tagged.
2. **GATE** (sudo): install it to `/etc/udev/rules.d/`, remove
   `/usr/lib/udev/rules.d/99-corsair-nexus.rules`, reload udev, replug, and
   confirm with `getfacl` that the device node has a `user:` entry.
3. The old service is disabled (done 2026-11-14).

## Phase 2 — Core

A Cargo workspace at the repo root in `crates/`, on a `redesign` branch.
The Go and Flutter code stays untouched.

Milestones, in order. The first three replace the old spikes.

1. **Renderer.** The System page renders to a PNG through the real renderer
   (swash hinted glyphs, tiny-skia shapes). Matt judges text crispness on the
   strip once the device works. Fallback: FreeType bindings or a bitmap font.
2. **Device.** HID backend over hidraw: frames out, touch in, reconnect after
   replug. The report descriptor says output report 0x02 is the 1024-byte
   frame packet and input report 0x01 is the touch report. Fallback: `nusb`
   with kernel-driver detach.
3. **UI shell.** A minimal Slint window showing the live frame at an integer
   scale with no smoothing, on native Wayland (KDE, NVIDIA). Also check
   keyboard operation and whether Orca sees its controls. Fallback: Qt 6/QML.
4. **Core.** The single state owner, signals, layout, config with reload,
   gestures, the control socket, `nexusctl`.

Design:

- **One owner.** A single task owns all display state. Inputs arrive as
  messages: signal updates, touch events, control commands, timers. No shared
  mutable state and no mutexes around frame data.
- **Render on change.** A frame is produced only when something changed. An
  animation tick runs only while something is animating. There is no idle
  30 fps loop.
- **Surface interface.** `size`, `send_frame`, a touch event stream,
  `set_brightness`. Two implementations: the Nexus, and an in-memory
  simulator the UI and tests use.
- **Signals.** Named, typed values: number with unit and range, text, series,
  image, state. Sources publish them; widgets subscribe.
- **Layout.** Pages are rows of widgets. A widget has a type, a size rule and
  signal bindings. Presentation belongs to the widget and theme, never to the
  source.
- **Config.** One human-editable text file, validated on load, reloaded live.
  No SQLite.
- **Control.** A Unix socket with a line-based protocol, plus `nexusctl`.
  No HTTP, no token.

Illustrative config, to be refined:

```toml
[[page]]
name = "System"

[[page.widget]]
type = "clock"
width = 120

[[page.widget]]
type = "value"
signal = "cpu.temp"
graph = { signal = "cpu.temp.history", style = "area" }
warn = 75
crit = 90
on_tap = { action = "page", target = "Performance" }
```

**GATE**: get Matt's sign-off on the config schema and the signal naming
before Phase 4 builds a UI on top of them.

Port the gesture thresholds from `internal/touch`; do not re-derive them.

Exit test:

- The current default layout, driven by a fake signal source, renders on the
  simulator and on hardware.
- Image tests compare against references with a per-pixel tolerance, not
  exact bytes.
- Idle CPU under 2% and RSS under 40 MB, measured over 60 s on hardware.
- Swipe between pages feels right to Matt on hardware.
- `cargo test` and `cargo clippy -- -D warnings` pass.
- Every `[P2]` item in the 9/10 bar passes.

## Phase 3 — Sources and interaction

- **Built-in sources, in-process:** clock, CPU temperature and load, GPU
  temperature and load, network, memory, disk, weather, media. Read
  `plugins/*` for how each one gets its data. Media must be event-driven
  (MPRIS signals), not polled through `playerctl`.
- **External sources:** any program that prints one JSON object per line on
  stdout. `nexusctl publish <signal> <value>` covers one-off pushes. Write the
  protocol down as a spec with a shell example and a Python example.
- **Actions:** switch page, launch a command, open a URL, send a key, media
  control, publish a signal.
- **Slider widget:** continuous X drives an action (volume, seek,
  brightness).
- **Takeovers:** a rule shows an overlay while a condition holds or for a set
  time. Build two first: media change with scrub, and volume change.

**GATE**: Matt uses it as his daily driver for two weeks. Takeovers are a
product bet; if they annoy him, cut or change them before building their
editor.

Exit test: every widget on Matt's live layout (System, Performance, Load,
Clock) has a working equivalent, the two takeovers work on hardware, and
every `[P3]` item in the 9/10 bar passes.

## Phase 4 — UI

One Slint app, launched as a separate process of the same binary, talking
to the daemon over the control socket. The strip must keep working with the
window closed.

- **The preview is the editor.** Show the strip enlarged. Click a widget to
  select it, drag to reorder, drag an edge to resize. Do not build a second
  row of zone cards under the preview, as the Flutter app does.
- **Inspector** for the selected widget, generated from its schema.
- **Signal browser** showing every signal and its live value.
- **Takeover rules** editor.
- **Onboarding** that diagnoses device access: rule installed, ACL present,
  device claimed by another process.
- **Tray icon** with show, hide and quit.
- Minimum window size no larger than 1024×640.
- A disconnect shows a banner and keeps unsaved edits.
- The preview doubles as the simulator: touch on it reaches the daemon.

Exit test: Matt can rebuild his live layout from an empty config using only
the UI, on native Wayland, and every `[P4]` item in the 9/10 bar passes.

## Phase 5 — Harden and ship

Hardening comes first. Do not package until it passes.

- Run the 72-hour soak on hardware and record CPU, RSS and log volume.
- Add the fuzz targets and the CI render-cost benchmark.
- Test the systemd unit's hardening with the UI running.
- Do a first-run test as a fresh user account: install the package, plug in,
  and note every point where a terminal or a guess was needed. Each one is a
  bug.
- Go through the whole 9/10 bar with Matt and record pass, fail or waived.

Then ship:

- Packaging for Arch, deb, rpm and a tarball, with the udev rule numbered
  below 73.
- A converter from the old layout YAML, or a documented manual migration.
  **GATE**: ask which.
- Docs: install, config reference, plugin protocol.
- Retiring the Go and Flutter code is Phase 8, after this release.

Exit test: a clean machine install from a package brings up the strip, the
release artifact is verified by installing it, not by CI exit codes, and
every item in the 9/10 bar is passing or waived by Matt.

## Phase 6 — macOS, then Windows

The Linux release ships first. Then, per OS:

- Device backend behind the `Surface` trait (IOKit HID on macOS, the HID
  API on Windows; neither needs a driver). Check whether macOS asks for the
  Input Monitoring permission, since the Nexus also presents a keyboard.
- Built-in sources: CPU and GPU temperature and load, network, memory,
  disk, media. Windows has no simple CPU temperature API; decide how to
  handle that before starting.
- Startup (LaunchAgent, Windows autostart), the control channel (Unix socket
  or named pipe), and an installer.
- Detect iCUE holding the device and explain the conflict, since iCUE runs
  on both.
- **GATE**: Windows work starts only once there is a Windows machine to test
  on with the Nexus.

## Phase 8 — Retire the legacy app; a first-class repo

Matt, 2026-11-14: "At the end of this entire tract of work, we should retire
the legacy code and cleanup the repo to be a first-class app." It runs once
the Rust Linux release has replaced the Go package. It does not wait for
macOS or Windows (Phase 6), since the Go app is Linux-only and nothing
depends on it after that.

- **Keep history, not code:** tag the last commit with the Go app as
  `legacy-go-final` and push the tag. Everything below is then a deletion
  from the tree, never from history.
- **Remove the Go and Flutter app:** `cmd/`, `internal/`, `pkg/`, `api/`,
  `plugins/`, `ui/`, `configs/`, `data/`, `test/`, `testdata/`, `go.mod`,
  `go.sum`, `.golangci.yml`, `.air.toml`, `Procfile`, `sqlc.yaml`,
  `build-package.sh`, `nexus-open-dev.desktop`, and the Go parts of
  `scripts/`. Check `design/` and `docs/` file by file: keep what still
  describes the product (`docs/USB_PROTOCOL.md`), drop what describes the
  old app.
- **Clean the working tree:** build output (`coverage.out`, the
  `nexus-open` binary, `swipe-sim`, `bin/`, `build/`, `dist/`,
  `plugins-dist/`, `test-screenshots/`) deleted and covered by
  `.gitignore`.
- **One build system:** Cargo only. Replace the Makefile with a short one
  (or none) over cargo; `scripts/nexusd-dev-install.sh` stays for
  development.
- **Packaging:** `packaging/` holds only the Rust app's files. The AUR
  package keeps the name `nexus-open`, so existing users upgrade in place.
  The udev rule is renumbered below 73.
- **CI:** one Rust workflow and one release workflow; the Go `ci.yml` and
  `release.yml` go. The pre-push hook runs `cargo fmt --check`, clippy and
  tests, which ends the `--no-verify` pushes.
- **Docs for a newcomer:** README (what it is, install, first run,
  screenshot), DEVELOPMENT, CONTRIBUTING, SECURITY, DEVICE_SETUP, the config
  reference and the external-source protocol, all describing the Rust app
  only. CHANGELOG continues.
- **Agent instructions:** rewrite `CLAUDE.md` for the Rust workspace (drop
  overmind, air, Flutter hot reload and the zone lock ordering), and update
  the memory notes that describe the Go development setup.
- **Branch:** merge `redesign` into `main`. **GATE**: Matt approves the
  merge and the version number.
- **Matt's machine:** remove the old Go package and its leftover files
  (`config.yaml`, `zone-configs.yaml`, `nexus.db`, the stale autostart
  override). **GATE**: ask before deleting any of his files.

Exit test: a fresh clone contains only the Rust app, its packaging and its
docs; `cargo test` and CI are the only checks; the README gets a new user
from clone or package to a lit strip; and `git grep -i -e flutter -e
golang` finds nothing outside the changelog.

## Phase 7 — Later, unscheduled

- Focused-window signals per compositor.
- Other display hardware behind the surface interface.

## Standing rules

- Do not add WASM, a web UI, or a context engine. If one seems necessary,
  stop and ask.
- Do not build parallel rendering paths. The UI displays frames the core
  rendered.
- Keep every crate building on Linux, macOS and Windows. OS-specific code
  lives in `cfg(target_os = ...)` modules behind a shared trait, never
  scattered through shared code. CI (`.github/workflows/rust.yml`) checks
  all three on every push.
- Prefer deleting scope to adding it. If a phase is growing, say so.
- Commit and push to the `redesign` branch as work progresses (Matt asked
  for this on 2026-11-14). Never push to `main`.
- Keep a short log of decisions and spike results at the bottom of this file.

## Decision log

- 2026-11-14 — Plan written. No decisions confirmed yet beyond: native UI,
  not web.
- 2026-11-14 — Matt asked for the improvements needed for a solid 9/10 app
  and experience. Added "The 9/10 bar" and made Phase 5 "Harden and ship".
- 2026-11-14 — Matt: focus on the rebuild, not on fixing the current app.
  Phase 0 skipped. Rust + Slint proceeds to the spikes; the go/no-go after
  Phase 1 still stands.
- 2026-11-14 — Phase 1 started on branch `redesign`. Old service disabled by
  Matt. Matt asked to commit and push as work progresses.
- 2026-11-14 — HID report descriptor of interface 0 (`hidraw15`): output
  report ID 0x02 with 1023 bytes (= the 1024-byte frame packet including the
  ID), input report ID 0x01 with 511 bytes (= the touch report), feature
  reports 0x03–0x0e with 31 bytes each. The hidraw path for Spike 3 looks
  viable without detaching the kernel driver.
- 2026-11-14 — Matt: no spikes; build the real app. Phase 1 removed. Its three
  risks (text crispness, hidraw device access, Slint on Wayland) became the
  first milestones of Phase 2. The udev rule moved to `data/udev/`.
- 2026-11-14 — Matt has not written Rust before. Explain Rust choices as the
  code is written.
- 2026-11-14 — Font is a user setting with five bundled, hinted choices:
  IBM Plex Sans (default), Atkinson Hyperlegible, Roboto, Noto Sans, Lato.
  Matt dropped Go ("we moved on"). DejaVu Sans is the fixed fallback for
  missing characters; Font Awesome Solid supplies icons. See
  `crates/nexus-core/src/fonts.rs` and `crates/nexus-core/assets/fonts/`.
- 2026-11-14 — Platforms: design for Linux, macOS and Windows; ship Linux
  first, then macOS, then Windows. Matt can test on Linux and macOS only, so
  Windows waits for a test machine. Linux-only code is now `cfg`-gated and
  CI builds and tests the Rust workspace on all three.
- 2026-11-14 — Milestone 2 (device) partly verified: frames reach the strip
  over hidraw (`/dev/hidraw15`) in about 15 ms each. Still to check: Matt
  confirming the image on the panel, touch (blocked on `parse_touch`), and
  replug recovery under 2 s.

- 2026-11-14 — Rust toolchain pinned to 1.100.0 in `rust-toolchain.toml`.
  The first CI run failed because the runners' newer clippy had a lint the
  local 1.93 lacked; with the pin, local and CI run the same compiler and
  lints. Rust CI is green on Linux, macOS and Windows (run 43603527771).
- 2026-11-14 — Milestone 3 (UI shell) built: `nexusd ui` opens a Slint
  1.18.1 window (`crates/nexus-ui`, winit backend, femtovg/OpenGL) on native
  KDE Wayland with the NVIDIA driver. No runtime warnings. The preview is
  pixel-identical to the renderer's output at 1× and 2×, including under
  fractional desktop scaling (`SLINT_SCALE_FACTOR=1.25`), because its size
  is counted in physical pixels (`phx`). Over AT-SPI (what Orca reads) the
  window exposes a frame "Nexus Open", the preview as an image with a spoken
  description, and a focusable combo box named "Font". Still to check by
  hand: Tab and arrow-key operation, and Orca actually speaking the
  controls. The Qt 6/QML fallback is not needed so far.
- 2026-11-14 — Open: Slint licensing. The repo is MIT; Slint is GPL-3.0 or
  its Royalty-free 2.0 license, which allows a free desktop app but requires
  attribution (the `AboutSlint` widget in an About screen, or the "Made with
  Slint" badge on the project page). Proposed: keep MIT and add an About
  screen with `AboutSlint` in Phase 4. Needs Matt's confirmation.
- 2026-11-14 — `nexus-ui` sets `unsafe_code = "deny"` instead of the
  workspace's `forbid`, because Slint's generated code carries
  `#[allow(unsafe_code)]`, which `forbid` rejects. Hand-written unsafe code
  is still an error.
- 2026-11-14 — Milestone 4 (core) built: signals, TOML config with live
  reload and last-good fallback, the single-owner engine (render on change),
  gestures ported from `internal/touch`, the control socket, `nexusctl`
  (status, doctor, signals, page, reload, publish), and the settings window
  streaming its preview from the daemon. Measured on hardware with demo data
  redrawing every second: 0.13% CPU, 10 MB RSS. Page render 0.035 ms, drag
  frame 0.006 ms; the ~15 ms USB transfer limits the frame rate to ~60 fps.
  Not yet checked on hardware: swiping (needs Matt), replug recovery under
  2 s, first frame within 500 ms, idle CPU over 60 s without demo data.
- 2026-11-14 — Deviations from the illustrative config: graph history is
  kept by the signal store for every numeric signal, so a widget says
  `graph = "area"` instead of naming a `.history` signal; thresholds are in
  the signal's base unit (°C), so they hold whatever display unit is chosen.
  **GATE** still open: Matt signs off the schema (`crates/nexus-core/assets/
  default.toml`) and signal names before Phase 4 builds on them.
- 2026-11-14 — `parse_touch` written by Claude rather than left as Matt's
  exercise, because gestures could not run on hardware without it.
- 2026-11-14 — Gesture addition: a release with the finger moving back
  against the swipe at 300 px/s or more cancels it (reversible swipes). All
  other thresholds are the Go app's.
- 2026-11-14 — Milestone 4 checks run by Claude on hardware. First frame
  18 ms after `nexusd run` starts (socket 2 ms; target 500 ms). Idle over
  60 s without demo data, System page (clock only): 0.05% CPU, 10 MB RSS.
  Settings window idle: 0.03% CPU, 127 MB RSS, of which 92 MB is shared
  libraries and the GL driver, 21 MB its own. Over AT-SPI the window exposes
  both labels, the preview image named with the current page, and two
  focusable buttons whose `click` action changes the daemon's page both
  ways. First run wrote the default config; `nexusctl doctor` passed every
  check. Raw hidraw reports were silent while the strip was untouched (no
  phantom touches). Still Matt's: swipe feel, replug under 2 s, Tab/Space
  in the window (keys cannot be injected into a native Wayland window
  without root), and Orca speaking.
- 2026-11-14 — **GATE passed:** Matt signed off the config schema and
  signal names ("carry on" after the Phase 3 proposal). Additions since are
  additive only: an optional `[weather]` section (`location`, or `latitude`
  and `longitude`).
- 2026-11-14 — Phase 3 step 1: built-in sources in `nexus-sources`.
  `sysinfo` for CPU load, memory, network and CPU temperature; NVML for
  NVIDIA, sysfs for AMD on Linux; Open-Meteo (forecast and geocoding) over
  `ureq`/rustls for weather, refreshed every 15 min, retried every minute
  while failing. Network counts only interfaces with a backing device
  (skips loopback, Docker, Tailscale). Verified against `nvidia-smi` and
  `free` on Matt's machine.
- 2026-11-14 — Found: the Go cpu-temp plugin read `thermal_zone0`, which on
  Matt's machine is `acpitz` (motherboard, ~28 °C), not the CPU
  (`coretemp Package id 0`). The new sensor choice is `cpu_sensor::pick`,
  left as Matt's contribution; until written, `cpu.temp` shows "—".
- 2026-11-14 — Memory: NVML adds ~21 MB of the NVIDIA driver's own
  allocations to the daemon (measured in isolation: sysinfo +0.3 MB, an
  HTTPS fetch +0.7 MB). Daemon with real sources: 0.47% CPU, 41 MB RSS,
  over the 40 MB target. The alternative, `nvidia-smi` every second, is
  the process churn the Go daemon paid for in CPU. Needs Matt's call on the
  target.
- 2026-11-14 — Matt: temperatures in °F on his strip (`temperature =
  "fahrenheit"`, already a setting). Memory target amended: 40 MB plus the
  GPU driver's own allocations (NVML), as proposed.
- 2026-11-14 — `cpu_sensor::pick` written by Claude after Matt saw the dash
  on the strip; prefers Intel package, AMD Tdie/Tctl, ARM cpu_thermal, and
  never a motherboard zone.
- 2026-11-14 — Phase 3 step 2: media. A `media` widget (artist, title with
  ellipsis, progress bar; title dimmed when paused) restored to the System
  page as in the Go layout (6 zones, 107/106 px). MPRIS source over zbus
  5.19's blocking API (zbus 6 dropped it; its connections only progress
  inside `block_on`). Event-driven: player start/quit, property changes and
  seeks wake it; while playing it advances the position itself each second.
  Skips `playerctld`, which mirrors another player. Known gap: VLC sends no
  `Seeked` for a seek while paused, so that seek shows when play resumes.
  Verified with VLC (play, pause, seek, quit) and Matt's browser.
- 2026-11-14 — Phase 3 step 3: `nexusd` is Matt's daily driver, as a
  sandboxed user service (`packaging/systemd/nexusd.service`, exposure 4.3
  "OK"; device access limited to hidraw and NVIDIA's control devices).
  `scripts/nexusd-dev-install.sh` builds, installs to `~/.local/bin`, and
  restarts it; rerun after each change. **GATE clock started:** the
  two-week daily-driver trial runs to 2026-11-28.
- 2026-11-14 — Matt asked for the legacy code to be retired and the repo
  cleaned up into a first-class app at the end of the rebuild. Recorded as
  Phase 8, replacing the archive gate in Phase 5; it follows the Linux
  release rather than macOS/Windows.
- 2026-11-14 — Phase 3 step 4: tap and hold actions. Any widget takes
  `tap`/`hold` = `page`, `run` (argv, no shell), `open`, `media` or
  `publish`; checked at load (unknown page names are errors). A zone with an
  action lights the moment a finger lands and goes dark when the touch
  becomes a swipe. Under systemd, `run`/`open` go through `systemd-run
  --user` so launched apps run outside the daemon's sandbox (verified: a
  launched program wrote outside the service's private /tmp and saw
  WAYLAND_DISPLAY). `nexusctl tap X` / `hold X` inject a touch through the
  same gesture path (also the Phase 4 simulator's hook). Verified on the
  service: page taps, play-pause and next on VLC.
- 2026-11-14 — Brush guard: presses under 30 ms are ignored. PROVISIONAL.
  `nexusd trace FILE` records raw touches alongside the daemon; traces
  dropped in `crates/nexus-core/tests/traces/` (tap-, hold-, swipe-,
  brush-) are replayed by a test. Needs Matt's recordings to set the real
  threshold. "Send a key" is not built: on Wayland it needs root (uinput)
  or a portal prompt; `run` with a tool like `wpctl` covers volume.
- 2026-11-14 — First touch recording from Matt (53 touches): 27 taps
  (46–198 ms, still), 20 swipes, 6 single-report lift bounces (8–12 ms).
  All read correctly with the 30 ms brush guard; without it the bounces
  fire taps (checked). The strip reports every ~9.6 ms (~100 Hz), not the
  500 Hz the unit tests assume. Saved as `tests/traces/{tap-matt,
  swipe-matt,brush-lift-bounce}.trace`; `cargo run -p nexus-core --example
  replay -- FILE` prints how each touch is read. Still wanted: a
  `brush-typing` trace.
- 2026-11-14 — Phase 3 step 5: the slider widget. `controls = "volume"`
  (live, via `pactl`; works for PipeWire), `"seek"` (MPRIS SetPosition on
  release) or `{ run = [..., "{value}"] }` (0–100, for brightness via
  ddcutil and the like). A touch landing on a slider belongs to it, so
  pages cannot be swiped from a slider; a page of only sliders is rejected.
  The bar follows the finger from the first sample; values are set only
  after the 30 ms brush guard. Slow setters never queue: a worker applies
  the newest value only. New `volume.level`/`volume.muted` signals from an
  event-driven `pactl subscribe` source. `nexusctl drag A B` simulates a
  drag at the strip's 100 Hz. A "Sound" page (media, position, volume) is
  added to the default and to Matt's config. Verified on the service:
  volume 100→91→100 % agreeing with pactl, seek both ways on VLC.
- 2026-11-14 — Phase 3 step 6: takeovers. `[[takeover]]` with `name`,
  `on_change` (signals), `seconds` (0.5–60) and widgets written as on a
  page, through the same binding and renderer. Shown when a signal changes
  (not on its first reading, not to blank text) unless the page on the
  strip already shows that signal, so the Sound page never pops a volume
  takeover. A touch restarts its time; a swipe dismisses it without moving
  the page underneath; it never ends under a finger; a different takeover
  never replaces one mid-touch. Defaults: "Now playing" (media + seek, 5 s)
  and "Volume" (full-width slider, 2 s). A full-width slider cannot be
  swiped away; it times out. Verified on the service with a published
  `volume.level` (signal only; the real volume was untouched): covered at
  once, page back after 2 s. Instant cut for now; a transition is Phase 4
  polish. Remaining Phase 3 item: external sources.
- 2026-11-14 — Phase 3 step 7: external sources. `[[source]]` with `name`
  and `run` (argv). Protocol (docs/EXTERNAL_SOURCES.md): one JSON object
  per line, signal name → number, text or `{value, unit}`; a mistake drops
  the whole line; lines over 64 KB are dropped unread. The daemon restarts
  an exited source after 2 s doubling to 60 s (reset after a healthy
  minute), logs its stderr, and applies config changes by diff. Programs
  run in the config folder. Under systemd each runs as its own user unit
  `nexus-source-NAME` outside the sandbox (`systemd-run --pipe`) with
  `BindsTo=` the daemon's unit. Found: killing `systemd-run --pipe` leaves
  its unit running, so sources are stopped by unit name. Examples in
  `examples/sources/` (POSIX shell disk use; Python GitHub stars). Verified
  on the service: signals published, working directory, stderr and bad
  lines logged, exit status 3 seen through systemd-run, backoff 2 → 4 s, a
  changed `run` restarts it, removing it stops it, and SIGKILL of the
  daemon stops it (BindsTo) and it returns with the daemon. Phase 3 is
  built; its gate is the daily-driver trial to 2026-11-28.
- 2026-11-14 — Matt chose stale-signal clearing for sources: a source down
  for 60 s (exited, not starting, or restarting without a reading) has its
  signals forgotten (`Input::Forget`; widgets show "—" and a return counts
  as a first reading, so no takeover). Removing a source or changing its
  `run` forgets them at once. Verified on the service: forgotten 60.000 s
  after the first exit, mid-backoff; removal cleared at once.
- 2026-11-14 — Matt: SQLite is to be the source of truth for everything the
  app is configured with, replacing `nexus.toml`. Design proposed in
  docs/STORAGE.md (daemon as sole writer, tables plus per-type JSON fields,
  page actions by ID, snapshot undo, TOML kept as the import/export format).
  Phase 4's editor waits on it.
- 2026-11-14 — Storage design agreed (docs/STORAGE.md). Matt: hand-editing
  must beat a lossy export/import, so `nexusctl edit` opens the state as
  TOML in $EDITOR and comments directly above a table are stored as that
  item's note; several layouts; undo history in the database. Step 1 built:
  `nexus-store` (rusqlite 0.42, bundled SQLite) with schema v1, migrations
  by `user_version` (a newer database is refused), import of a `Config` and
  its notes into rows with page actions by ID, load back to `Config`, and
  TOML text with notes in both directions. Matt's live file round-trips
  exactly (5 pages, 2 takeovers); all its comments were copies of the
  bundled help, which a legacy import now drops (`without_lines_in`). The
  bundled default's help is now `##` lines, never kept as notes.
- 2026-11-14 — Step 2 and 3 of the store built and deployed. Typed edits
  (`Edit`, JSON over the socket) each run in one transaction, are validated
  by loading every layout before commit, and record whole-state snapshots
  for undo and redo (200 kept; a new edit drops the redo branch). The daemon
  now reads only the store (`nexusd run --db`; default
  `$XDG_STATE_HOME/nexusd/nexusd.db`); the file watcher is gone. On first
  start it imports the legacy `nexus.toml` as layout "Main" and renames the
  file `nexus.toml.imported`. A store it cannot open leaves the default
  layout on the strip and refuses edits, so nothing is lost by writing over
  it. A daily `VACUUM INTO` backup keeps 7. `nexusctl` gains `edit` (with a
  conflict check against the revision the text was made from), `export`,
  `import`, `undo` and `redo`. Found: `StateDirectory=nexus-open` made
  systemd symlink `~/.local/state/nexus-open` to the existing
  `~/.config/nexus-open`, so the unit uses `StateDirectory=nexusd`.
  Verified on the service: Matt's import exports with an empty diff; an
  edit moved two widgets on the strip, undo, redo and undo again returned
  it to the original text exactly; a stray trailing comment was warned
  about; the day's backup was written.
- 2026-11-15 — Phase 4 step 1: the preview is the editor. The store gives
  out the IDs behind a layout (`Store::ids`, in `Config` order) and the
  daemon answers a new `layout` request with the layout, those IDs, the
  revision and what undo and redo would do; `status` gains the covering
  takeover and the revision, which the window polls every 250 ms to follow
  swipes and outside edits. Over the live frame the window outlines the
  page's widgets: click selects, dragging moves (a widget passes a
  neighbour when its leading edge crosses that neighbour's middle),
  dragging a right edge resizes (an inner edge trades width with the
  neighbour; the last edge uses the empty strip; never below the minimum
  width). Keyboard: arrows select, Ctrl+arrows move, Shift+arrows resize by
  4 px (a run of presses settles into one edit), Ctrl+Z and Ctrl+Shift+Z or
  Ctrl+Y undo and redo, also as buttons. Each gesture is one edit made
  against the revision shown; nothing is sent while one is in flight. A
  takeover over the page or an unwritable store hides the outlines and the
  caption says why. Verified against the service from a headless KWin
  session (xdotool): select, drag-move, Ctrl+Z, keyboard select and move,
  edge resize with live widths, five Shift+→ as one edit, and back to the
  original layout exactly. Found: AccessKit reports the widget items as
  siblings of their list, not children; for Matt's Orca pass.
- 2026-11-15 — Phase 4 step 2: the inspector. Under the preview, the
  selected widget's settings, generated from one description in
  `nexus-ui/src/inspector.rs`: a widget is flattened into named values in
  the window's words and rebuilt from them, so every type shares one field
  list and one rebuild. Fields: type (a change of type keeps width,
  actions and note, and gives the new type working defaults), width (the
  right neighbour gives or takes the difference), label, signals with their
  live value ("Now 109 °F", or "nothing has published this yet"), graph,
  warn and critical thresholds typed in the display unit (°F here, stored
  in °C), slider control and command, tap and long press (page, next,
  previous, run, open, media, set a signal) and the note. Commands are
  shown and typed as shell words (shlex), stored as argv. A change that
  cannot be built yet, such as "Run a command" with no command, stays a
  local draft with an amber "Not saved yet" line, so the strip never gets a
  half-written action. Text is sent on Enter or when focus leaves; the row
  just typed in is updated in place so focus stays. Add widget (after the
  selection, in the empty strip, else by splitting the selection) and
  Delete widget, both undoable. The daemon's `layout` now carries notes and
  `signals` carries numbers and units. Verified against the service from a
  headless KWin session: label, a °F threshold stored as 85 °C, a draft
  tap finished into a quoted command, a note shown in `nexusctl export`,
  Reading → Clock, width, add by splitting, delete; eight undos returned
  the exact original. Each control is labelled for screen readers.
- 2026-11-15 — Phase 4 step 3: the signal browser. A Signals panel
  beside the widget settings lists every signal and its live value in the
  display units (°F here), updated each second in place so the list keeps
  its scroll position, with a filter by name. Every signal field (a
  reading's signal, a rate's download and upload, a slider's, a tap that
  sets one) gets "Choose…": a click on a signal fills it, or type part of
  the name in the filter and press Enter; Escape or Cancel stops. The hint
  sits under the list so starting to choose moves no row under the
  pointer (found in testing: it was above the list and a click landed one
  row low). Found and fixed: changing a reading's signal between a
  temperature and anything else stored the thresholds as the °F numbers
  shown, 75 °C becoming 167. The stored values now carry over; a test
  covers it. Verified against the service from a headless KWin session:
  pick by click and by keyboard, Escape, the threshold fix, each undone
  back to the exact snapshot. AT-SPI reads each row as name and value.
- 2026-11-15 — Phase 4 step 4: pages. A tab per page replaces Previous
  and Next; a click shows that page on the strip, and Add page puts an
  empty "Page N" after the current one and shows it. With no widget
  selected, the settings area shows the page's: its name (refused if empty
  or taken, and the field goes back to the real name), Move earlier, Move
  later, Delete page (not the last one). The store already kept actions
  pointing at pages by ID, so a rename never breaks a tap, and refuses to
  delete a page a tap opens; its reason now reads "The tap on the clock
  widget on page "System" opens page "Clock". Change that tap first."
  Problems now show in the caption line under the preview instead of a
  banner above it: in testing the banner pushed the controls down and a
  click landed in empty space. Found too: a 760 px cap on the page
  settings became the whole window's maximum width (a spacer absorbs it),
  and a problem outlived the next change (cleared whenever an edit is
  sent). Verified against the service from a headless KWin session: add,
  rename, a taken name, move, add a widget to the empty page, delete, the
  refused delete; five undos returned the exact snapshot.
- 2026-11-15 — Phase 4 step 5: takeovers. Each takeover has a tab after
  the pages; choosing one shows it on the strip so the preview edits its
  widgets like a page's. The daemon gained `show-takeover <seconds> <name>`
  and `end-takeover` (also `nexusctl takeover`): the window asks for 15 s
  and asks again every 5 s, so a closed or crashed window leaves it up at
  most 15 s. A config reload now keeps a takeover that still exists by
  name; it used to drop it, which would have closed the takeover after
  every edit. With no widget selected the settings show its name, the
  signals whose change shows it (Choose… adds to the list) and seconds,
  from `nexus-ui/src/takeovers.rs`. Add takeover is a draft until it has
  signals, then starts with a clock across the strip. Undo and Redo moved
  to the header to make room. The "Not saved yet" hint now has a line of
  its own: appearing above the fields it pushed them down and a click
  landed in the wrong field, the third such shift found by testing, so
  every message that comes and goes now has fixed room. Verified against
  the service from a headless KWin session: edit Volume's seconds and
  signals, select its slider, add a takeover by its signal, rename it
  (the lease follows), delete it; undo returned the exact snapshot.
- 2026-11-15 — Phase 4 step 6: settings. A Settings button in the header
  opens the strip's own settings in the settings area, as fields from
  `nexus-ui/src/settings.rs`: temperatures °F or °C, 24- or 12-hour
  clock, font, and where the weather is for (a place, or "latitude,
  longitude"; empty for none). One change is one undoable edit. Found: the
  window's form kind was set before the form was worked out, so the
  settings first opened under the page settings. Verified against the
  service: the 12-hour clock showed on the strip at once, coordinates out
  of range were refused with nothing sent, and undo restored the exact
  snapshot.
- 2026-11-15 — Phase 4 step 7: onboarding and diagnostics. The checks
  behind `nexusctl doctor` moved into a `nexus-doctor` crate that returns
  them as data; the CLI prints them as before, and the window runs them on
  a thread (at start, every 5 s while one fails, every 30 s otherwise, and
  at once when the daemon connection comes or goes). A banner appears only
  when a check fails, naming it and its fix as selectable text, with
  "Check again" and "Copy diagnostics"; Strip settings has the copy button
  too. The report gives versions, the OS and desktop session and every
  check, with the home directory shown as ~ and nothing from the layout
  (it can hold where the user lives). Copying goes through a hidden text
  input's own clipboard support, so no clipboard crate. With a service
  installed, "nexusd is not running" now says to start the service and
  where its log is, not `nexusd run`. Verified from the headless session:
  a window given an empty runtime dir showed the banner; a fix line could
  be selected and copied; linking the real socket in cleared the banner
  within 3 s. Found while testing: a runtime dir path over about 90
  characters makes the socket path exceed the 108-byte limit, and that
  reads as "not running".
- 2026-11-15 — Phase 4 step 8: layouts. Strip settings lists every
  layout with Show and Delete, the name of the one on the strip, and Add
  layout from a copy of it, an empty page, or a starter: Everything (the
  default), Monitor, Music and Minimal, bundled beside `default.toml` and
  tested to fill every page. Each add is one `import` request that also
  puts it on the strip, since the window sends one request at a time;
  import keeps the user's settings and sources, and its undo label is now
  "Add layout X". The `layout` answer gained the list of layouts. With
  more than one layout the header names the one shown. Found: after the
  daemon restarts (an upgrade) the store revision is unchanged, so an
  open window never fetched the new answer and showed no layouts; it now
  fetches the layout whenever the connection comes back. Verified against
  the service: add Minimal (on the strip at once), Show Main, Delete
  Minimal, an empty name refused, rename and undo; four undos gave back
  the exact snapshot. Import and export as files come next, with file
  dialogs.
- 2026-11-15 — Phase 4 step 9: layout files. Strip settings has
  Import from file… and Export this layout…, through the native file
  dialog (`rfd`: the XDG portal on Linux, so the desktop's own picker;
  the system dialogs on macOS and Windows), opened as children of the
  window. Export leaves out where the weather is for and the sources: a
  layout file is made to be shared, and those say where the user lives
  and what is in their home directory (import ignores them anyway).
  Import checks the file is a layout first, names it after the file and
  puts it on the strip in one request. Verified with the KDE dialog in
  the headless session (a private D-Bus, so the dialogs could not open on
  the real desktop): export wrote the pages with no location in it;
  importing it as Desk.toml added "Desk" on the strip with the same pages;
  two undos gave back the exact snapshot. This completes the starter
  layouts and import/export item of the bar.
- 2026-11-15 — Phase 4 step 10: window size. The fixed 720 px minimum
  was smaller than the layout needed, so at that width the signal browser
  was cut off; the minimum now comes from the layout itself (753×485 with
  the default layout, inside the 1024×640 bar). The page and takeover tabs
  scroll sideways instead of adding to the minimum, which also fixes many
  tabs pushing the panels off screen. Undo and redo labels are cut to 26
  characters for the same reason. The window's size is saved in logical
  pixels when it is closed and restored at start (state dir,
  `window-size`). Verified headless: resized to 1100×700, closed with a
  WM_DELETE_WINDOW request, and the next window opened at 1100×700.
- 2026-11-15 — Matt asked to plan for all supported Corsair devices,
  starting with his K100, Commander Core and Elite LCD cap, so the app can
  become an alternative to iCUE. The plan is `docs/DEVICES_PLAN.md`; it
  lifts "other hardware" out of the out-of-scope list and adds phases
  D1–D7 after this plan's Phase 5 (Matt confirmed the timing).
