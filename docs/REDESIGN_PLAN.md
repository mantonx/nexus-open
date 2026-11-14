# Nexus redesign plan

Status: rebuild confirmed by Matt on 2026-11-14. Written for an agent (Claude
Opus) to execute with Matt. Phase 0 is skipped; start at Phase 1. Nothing has
been started.

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
- Linux first. Windows and macOS stay possible but are not built in this plan.

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
  before any hardware test. See "Before the spikes" in Phase 1.
- Anything needing `sudo` is a **GATE**.
- Temporary benchmark files were used in `internal/app` and `internal/zone`
  during the investigation and removed. The tree was clean afterwards.

**Not verified**

- Whether the udev fix works; it has not been applied.
- Whether the device accepts frames through hidraw (Spike 3).
- How the hinted text of a Rust renderer looks on the real panel (Spike 1).
- Whether Slint runs cleanly on KDE Wayland with NVIDIA (Spike 2).
- Whether Slint exposes controls to screen readers on Linux (Spike 2).
- The device's highest sustainable frame rate. The Go daemon targets 30 fps.
- Which signals KDE and GNOME emit for screen lock and display sleep.
- Matt's Rust experience. Ask; it affects how much explanation to give.

## Phase 0 — Skipped

Matt chose to put all effort into the rebuild. Do not fix the Go or Flutter
code. The known problems there (idle CPU, the tray token 401, the committed
`plugins/media/music` binary) stay as they are until the code is archived.

One thing carries over: the udev ordering bug blocks the rebuild too, because
the new code needs the same device node. It is fixed as part of Phase 1.

## Phase 1 — Spikes

Three throwaway experiments. Keep them outside the product crates. Each
produces a short written result: pass or fail, evidence, and what it means
for the plan.

**Before the spikes — device access.** Spikes 1 and 3 need the hardware.

1. Write the new udev rule as `70-nexus.rules` (any number below 73), with
   `TAG+="uaccess"` for the usb and hidraw nodes of `1b1c:1b8e`. Leave out
   the `usbreset` step unless a spike shows it is needed.
2. **GATE** (sudo): install it, remove the old
   `/usr/lib/udev/rules.d/99-corsair-nexus.rules`, reload udev, and have Matt
   replug the device. Confirm with `getfacl` that the device node has a
   `user:` entry.
3. **GATE**: the old service must not be running. Once access works, the Go
   daemon's watcher connects within 3 s and holds the device. Ask Matt to
   stop and disable it for the duration of the rebuild
   (`systemctl --user disable --now nexus-open`). It is currently using about
   25% of a core and 400 MB while showing nothing.

**Spike 1 — text on the panel.** Render the System page (clock, weather,
CPU value with sparkline) to a 640×48 image using a Rust rasterizer with
hinted glyphs (swash's scaler, directly or through cosmic-text). Compare
with the goldens and `docs/screenshots/display.png`, then show it on the
real strip.
Pass: Matt judges the 10 px labels as crisp as today.
Fallback: FreeType bindings, or a bitmap font for small sizes.

**Spike 2 — Slint on Matt's desktop.** A window that:
- runs natively on Wayland (KDE, NVIDIA), without XWayland;
- shows a 640×48 image at an integer scale with no smoothing, updating at
  30 fps from a channel;
- maps pointer down, move and up to strip X coordinates;
- lets you drag to reorder up to 6 zone rectangles;
- builds one form from a schema with string, enum, int, bool and colour.
Pass: all five work with no blur and no crash.
Also report, separately: is the window fully keyboard-operable, and does a
screen reader (Orca) see its controls? This decides whether the 9/10 bar's
accessibility items are reachable in Slint.
Fallback: Qt 6/QML (6.12 is installed with declarative and websockets).

**Spike 3 — device access.** Send a test frame and read touch X from Rust.
Try the HID path first (`hidapi`, hidraw backend); the frame header's first
byte `0x02` looks like a HID report ID. If that fails, use `nusb` with
kernel-driver detach, mirroring `usbfs.go`. Remember the EP `0x81` priming
read.
Pass: frame visible, touch X read, reconnect after replug.
Record which path worked. Only the HID path avoids a driver install on
Windows and macOS.

**GATE**: report all three results. Matt decides go, change toolkit, or stop.
Also ask which Slint licence to use: GPLv3 (binary is GPL, source files can
stay MIT) or the royalty-free licence (attribution required).

## Phase 2 — Core

A Cargo workspace at the repo root in `crates/`, on a `redesign` branch.
The Go and Flutter code stays untouched.

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

## Phase 6 — Later, unscheduled

- Windows and macOS device backends and per-OS sources. iCUE already supports
  both, so this is an alternative there, not the only option.
- Focused-window signals per compositor.
- Other display hardware behind the surface interface.

## Standing rules

- Do not add WASM, a web UI, or a context engine. If one seems necessary,
  stop and ask.
- Do not build parallel rendering paths. The UI displays frames the core
  rendered.
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
