# Corsair devices plan

Status: agreed 2026-11-15 (decisions below). Starts after the redesign's
Phase 5 ships the Nexus on Linux. Nothing here is built yet.

## Goal

Grow Nexus Open from a driver for one touch strip into an open alternative
to Corsair iCUE on Linux, then macOS and Windows: one native app that runs
cooling, lighting, LCD screens and keyboards from one set of signals, with
no root and no web UI.

Start with the devices on Matt's machine:

| USB id | Device | Protocol family |
|---|---|---|
| `1b1c:1b8e` | iCUE Nexus | iDisplay LCD (done) |
| `1b1c:0c39` | Elite Capellix LCD cap | iDisplay LCD |
| `1b1c:0c32` | iCUE Commander Core (Elite Capellix pump and fans) | Bragi |
| `1b1c:1b7c` | K100 RGB Optical | Bragi |

Then widen family by family, not device by device (see D6).

This changes the redesign plan's Goal, which listed other hardware as out of
scope. The redesign phases (4, 5, 6, 8) stay as they are; this plan adds
phases D1–D7 and its own exit tests.

## What would make it a 9/10

From the research (`docs/devices/research/scope.md`, section 5), what users
say the existing alternatives get wrong, in order:

1. **The pump is never at risk.** No state of the app, a crash included,
   leaves the pump stopped or slow.
2. **Hotplug, sleep and resume just work**, every time, with no restart.
3. **No root, no web page.** A user service and a native window.
4. **One app** for lighting, cooling, LCDs and keys, driven by one set of
   signals (the Nexus can show coolant temperature; the LCD can show the
   CPU; a fan curve can follow the GPU).
5. **Devices still work when the app is not running**: the keyboard types,
   G-keys and the dial do something sensible, the cooler follows a safe
   hardware curve.
6. **Every key is right** in every physical layout (ANSI, ISO, JIS).
7. **Per-app profiles on Wayland.**
8. **Light on CPU**: well under 1% of a core with every device attached.
9. **Settings survive upgrades.** The SQLite store already gives us
   migrations.

## Facts established on 2026-11-15

Verified first-hand on Matt's machine unless marked. Research notes, with
sources and a verified/inferred mark on each fact, are in
`docs/devices/research/`.

**Report sizes, from each interface's HID report descriptor** (read from
sysfs; no device was opened). The sources disagreed (64, 96 or 1024
bytes), so these are the ground truth for these units:

| Device | Interface | Reports |
|---|---|---|
| K100 `1b7c` fw 0.20 | 0 | boot keyboard, consumer, mouse (the typing interface) |
| | 1 | vendor page `ff42`: 1024-byte in and out (control) |
| | 2 | vendor page `ff42`: 64-byte in only (key and dial events) |
| Commander Core `0c32` fw 1.00, full speed | 0 | vendor page `ff42`: 64-byte in and out |
| | 1 | vendor page `ff42`: 64-byte in only |
| LCD cap `0c39` fw 2.00 | 0 | out report 2 (1023 B), in report 1 (511 B), feature reports 3–22 (31 B) |
| Nexus `1b8e` fw 2.00 | 0 | out report 2 (1023 B), in report 1 (511 B), feature reports 3–14 (31 B) |

- **The LCD cap uses the Nexus's display transport.** Same report layout.
  OpenLinkHub sends it 480×480 JPEG in 1024-byte chunks with an 8-byte
  header, where the Nexus takes raw 640×48 RGBA. [Source-verified in
  OpenLinkHub; not yet tried here.]
- **The Commander Core and K100 share one protocol ("Bragi"):** `08 <cmd>`
  framing, software mode `01 03 00 02`, hardware mode `01 03 00 01`, and
  endpoints opened, read, written and closed by handle. About 110 of
  OpenLinkHub's 187 supported device ids use it, including the iCUE LINK
  hub and wireless dongles. [Source-verified in OpenLinkHub, OpenRGB and
  ckb-next.]
- Only the Nexus nodes are open to the session user today. The other
  three are `crw------- root`. Each needs a `uaccess` rule sorted before 73,
  as `data/udev/70-nexus.rules` does.
- No kernel driver binds any of these beyond `usbhid`, so there is nothing
  to unbind or race with.
- `/dev/uinput` is writable here only because the `steam-devices` package
  ships a rule. We cannot rely on that.

**Licences of the sources.** OpenLinkHub is GPL-3.0-or-later, liquidctl
GPL-3.0-or-later, OpenRGB GPL-2.0-or-later, ckb-next GPL-2.0-only, the
kernel hwmon drivers GPL-2.0-or-later. Nexus Open is MIT. No Rust crate
covers Bragi.

## Rules for this work

These add to the redesign plan's standing rules.

- **Clean room.** Learn protocol facts (bytes, offsets, sequences,
  timings) from other projects and cite them. Do not copy, translate line
  by line, or import their code or data tables (OpenLinkHub's
  `database/keyboard/*.json`, OpenRGB's LED maps). Key and LED maps are
  re-derived on hardware: light LED *n*, see which key lights. Our own
  protocol docs go in `docs/devices/`.
- **No firmware updates, ever.** Point to Corsair's own updater.
- **No writes to onboard memory** (profiles, stored lighting, stored
  curves) except on an explicit "Save to device", and never in a loop.
- **No polling-rate changes** until we know which ones re-enumerate the
  device.
- **Pump floor.** The app never asks for less than 50% pump duty, and the
  limit is in the device layer, not the UI.
- **One owner per device.** Before taking a device, check whether another
  process has its hidraw node open (`/proc/*/fd`), name it (OpenLinkHub,
  OpenRGB, liquidctl, ckb-next), and do not take the device. On Windows,
  take the mutex iCUE uses, `Global\CorsairLinkReadWriteGuardMutex`.
- **Give devices back.** On exit, on SIGTERM, before sleep, and from a
  watchdog if the daemon hangs, put each device back in hardware mode.
- **Trademarks.** "CORSAIR" and "iCUE" stay out of the app's name,
  binary and icon. "Works with the CORSAIR K100" plus a not-affiliated line
  in About is fine.

## Architecture

What exists: `nexus-device` has one `Surface` trait (send a 640×48 frame)
and a Linux hidraw backend for the Nexus. `nexusd` opens one device and
retries on a timer. Its engine renders widgets bound to named signals.

What changes:

1. **A device registry** in `nexusd`. Hotplug from udev (and IOKit /
   SetupAPI later), not a retry timer. Each device gets its own thread and
   message channel, so a hung cooler cannot stall the strip. Logind's
   sleep signal puts every device in a safe state first.
2. **Capabilities, not one trait per product.** A device offers some of:
   - `Display { width, height, format }`: the Nexus (640×48 RGBA) and the
     LCD cap (480×480 JPEG).
   - `Cooling`: channels with readings, duty, and a hardware fallback
     curve.
   - `Lighting`: a list of LEDs with positions, taking a colour per LED.
   - `Keys`: events from G-keys, the dial and remapped keys.
3. **Readings are signals.** Coolant temperature, pump and fan RPM become
   signals like `cooler.liquid.temp` and `cooler.fan1.rpm`. They show on
   the strip, in the inspector's signal picker and in `nexusctl signals`
   with no new UI.
4. **Commands are actions.** "Set the fan profile to Quiet" or "lighting
   off" join tap actions, so the strip can run the cooler.
5. **One renderer.** The widget engine draws at the display's size, so the
   LCD cap shows the same widgets as the strip at 480×480. Lighting effects
   are a separate small engine: a colour per LED per frame, sampled from
   effects placed in a shared space covering every LED, so one effect can
   sweep across the keyboard and the fans. That replaces nothing that
   exists, so it is not a parallel rendering path.
6. **The store** gains devices (by serial), their settings, fan curves,
   lighting scenes and key bindings, with undo like the layout.
7. **The settings window** gains a Devices section: each device's
   readings, and pages for cooling, lighting, screen and keys.

Crates: keep raw hidraw on Linux (it works and has no dependency). Move to
`hidapi` (MIT) when macOS and Windows start. `udev` for hotplug, `evdev`
for uinput, `image` or `jpeg-encoder` for the LCD.

## Phases

Each phase ends with a test on Matt's hardware. Phases D1–D4 are the
starting devices; D5 onwards widens.

### D1 — Registry and permissions

- Device registry with udev hotplug, per-device threads, sleep and resume
  via logind. The Nexus moves onto it with no change in behaviour.
- Owner check (another process holding the node).
- One udev rule file covering the supported ids and only the interfaces we
  use (the K100's interfaces 1 and 2, not its typing interface), plus
  uinput. **GATE: sudo** to install it.
- `nexusctl devices` and `nexusctl doctor` list every Corsair device, what
  is supported, and what is blocking each one.

Exit test: unplug and replug the Nexus and suspend and resume the machine
10 times each; the strip comes back every time, with no restart, and
`nexusctl devices` lists all four devices.

### D2 — The LCD cap

- `Display` backend for `0c39` over the Nexus transport: JPEG frames,
  brightness, rotation.
- The LCD gets layouts of its own in the store, drawn by the same engine
  at 480×480, with round-screen guides in the editor.
- Low risk: a display cannot hurt the cooler.

Exit test: the cap shows coolant and CPU temperature live for 24 hours;
GIF-like animation holds its frame rate; unplug and resume as in D1.

### D3 — The Commander Core (cooling)

Safety-critical. Read-only first.

1. **Read-only:** firmware, connected fans, pump and fan RPM, coolant
   temperature as signals, without leaving hardware mode, if the device
   allows reads in hardware mode. Find out.
2. **The pump test. GATE: Matt at the machine, every run.** It answers
   what the Commander Core does when it is in software mode and the host
   goes silent: falls back to its hardware curve, holds its last duty, or
   stops. The answer sets the watchdog design. See "The pump test" below
   for the procedure; nothing in it is shortened or skipped.
3. **Control:** fan and pump curves following any signal (coolant, CPU,
   GPU), presets (Quiet, Balanced, Extreme), the 50% pump floor, hardware
   fallback restored on exit.
4. **Commander Core lighting:** pump ring and fan LEDs.

Exit test: 72-hour soak with curves active; kill -9 the daemon, stop it,
suspend, and unplug the cooler's USB; each time the pump keeps running
and the device ends in a safe state.

### D4 — The K100

1. **Lighting:** per-key colour and effects in software mode, with the
   keepalive. Re-derive the LED map on hardware.
2. **Hardware lighting:** what the keyboard shows when the app is not
   running. Writing onboard memory: "Save to device" only.
3. **Keys:** G1–G6, the dial and its ring, the profile and macro keys.
   Remaps and macros through uinput. Nothing remapped by default, so the
   keyboard behaves as before until the user asks.
4. Typing must never stop working. If the daemon dies, the keyboard must
   be back to normal within seconds. Test this first.

Exit test: a day of normal typing and gaming with lighting and two macros
on; kill -9 the daemon mid-macro and the keyboard types normally.

### D5 — Profiles

- Profiles that switch lighting, cooling and keys together.
- Per-app switching. Agreed 2026-11-15, within a limit: the focused
  app's id is one signal, `app.focused`, and profiles may switch on it.
  The strip's layout does not rearrange on it; that stays out of scope.
  KDE first (a KWin script reporting over D-Bus), then GNOME (needs an
  extension), then wlroots (`wlr-foreign-toplevel`). Where none works the
  signal is absent and profiles switch by hand, from the strip or the
  window.

### D6 — Widen by family

In order of how many devices each covers (counts from OpenLinkHub's table;
see `docs/devices/research/openlinkhub.md` section 5):

1. Bragi wired peripherals (about 71 ids): other keyboards, mice, headsets.
2. Bragi wireless dongle (about 26): the same protocol behind endpoint
   `0x09`.
3. iCUE LINK hub (Bragi framing): LINK fans, AIOs, and their LCDs.
4. Older peripherals, "NXP" protocol (about 28).
5. Hydro Platinum and Elite AIOs (13) and Commander Pro / Lighting Node (6).
6. PSUs: read the kernel's `corsair-psu` hwmon sensors; no protocol work.
7. Memory over SMBus: needs root or a kernel module; probably never.

Each new family needs a device to test on. Without hardware it stays
unsupported; we do not ship untested protocols.

### D7 — macOS and Windows

Follows the redesign plan's Phase 6, on `hidapi`. Open questions there:
whether macOS asks for Input Monitoring when only the vendor interface is
opened, and stopping the Corsair Service on Windows.

## The pump test

Agreed with Matt on 2026-11-15: be careful. The procedure, in full.

**Before the first run**

- D3 step 1 is done: readings (pump and fan RPM, coolant temperature) have
  run read-only for at least three days and match what iCUE showed on
  Windows or what the hardware should read at idle (Matt to confirm the
  expected pump RPM).
- Every packet the test sends is written out in `docs/devices/` and
  matches at least two sources (OpenLinkHub and OpenRGB or liquidctl). Any
  byte the sources disagree on is not sent.
- The test is a separate dev binary with a fixed script, not the daemon.
  Its first run is a dry run that prints every packet and sends nothing;
  Matt reads it.
- Nothing else has the device open (the owner check), OpenLinkHub and
  liquidctl are not installed, and `nexusd` is not touching the cooler.
- The machine is idle: no game, build or render running. Coolant at its
  idle baseline, recorded.

**Independent readings.** The coolant reading comes from the device under
test, so it cannot be trusted while the device's state is unknown. During
every run the test also logs the CPU package temperature from the kernel
(`k10temp` or `coretemp` hwmon) every second, and Matt watches the pump
and fans (sound, the pump ring's light).

**Duty never goes down.** In software mode the test only ever sets the
pump to 100% and the fans to at least what they ran at in hardware mode.
So whichever answer we find, "it holds the last duty" is the safe one.

**The runs**, each starting and ending in hardware mode, with at least
5 minutes in hardware mode between runs:

1. Enter software mode, read speeds, return to hardware mode at once.
   Passes if the pump RPM never drops.
2. Enter software mode, set pump 100%, read speeds every second for 30 s,
   return to hardware mode. Passes if the pump reaches and holds full
   speed.
3. Silence: enter software mode, set pump 100%, then send nothing for
   5 s. The first packet after the silence reads speeds, which shows the
   state the device chose; then return to hardware mode.
4. The same with 15 s, 30 s, 60 s and 120 s of silence, one per run, only
   if the run before passed and showed no drop.

**Stop at once** (the script returns to hardware mode, and Matt is told
why) if any of these happen:

- CPU package temperature rises more than 10 °C over its idle baseline.
- The pump RPM read back is below the hardware-mode RPM recorded at the
  start, or zero.
- Any read fails or times out, or a reply does not match its command.
- Matt hears or sees the pump stop. He stops the run with Ctrl+C; the
  script's handler returns the device to hardware mode.

**If the device stops answering**: shut the machine down. The pump is
powered from SATA and returns to its hardware curve at power-on; a USB
reset alone has not been tested and is not relied on.

**Afterwards**: the results go in the decision log with the raw readings,
and the watchdog is designed from them: how often the daemon must speak,
and what it does if it cannot.

## Decision log

- 2026-11-15 — Matt asked to plan support for all supported Corsair
  devices, starting with his own, so the app can become an alternative to
  iCUE. Research from OpenLinkHub, liquidctl, OpenRGB, ckb-next, the kernel
  and Corsair's own documentation is in `docs/devices/research/`. Report
  sizes were settled from the devices' report descriptors. Found that the
  LCD cap shares the Nexus's display transport, and the Commander Core and
  K100 share Bragi, so three of the four starting devices need one new
  protocol.
- 2026-11-15 — Matt's answers. (1) The device work starts after the
  redesign's Phase 5 ships the Nexus. (2) Rename the app before the first
  package; the new name is Matt's pick. (3) Per-app profiles: Matt left it
  to Claude. Allowed as one signal, `app.focused`, that profiles may
  switch on; the strip's layout does not react to it (see D5). (4) The pump
  test is careful: the full procedure is in "The pump test".
