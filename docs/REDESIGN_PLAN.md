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
(Matt rejected web; the UI must be native), support for other display hardware.

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
- Update `CLAUDE.md` for the Rust workspace and remove the Go-only sections.
- **GATE**: archive the Go and Flutter code only after Matt says so.

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
