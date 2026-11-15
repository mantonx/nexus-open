# Nexus Open as an open iCUE alternative: scope research

Date: 2026-11-15. Research used the web and docs only. No device was touched and nothing was installed.
Target hardware: K100 RGB keyboard (PIDs 1b7c/1b7d/1bc5), Commander Core (0c1c / 0c32 "2022") with an Elite Capellix AIO, and the Elite LCD cap.

Legend: **[V]** verified against a primary source or the source code. **[S]** secondary source (forum, blog or retailer). **[U]** unverified, or my own inference. Test these before relying on them.

---

## 1. iCUE 5.x feature set and how central each feature is

Current version line: iCUE 5.51.42 (2026-09-16). Recent notes include 5.43 (Sensors List widget, Xeneon Edge), 5.46 (double-click install of `.icuewidget`) and 5.49 (Xeneon Edge mouse lock). [V] https://www.corsair.com/explorer/patch-notes/icue/ (Corsair release-notes index, via search results)
Corsair's own overview of the feature areas: [V] https://www.corsair.com/explorer/diy-builder/blogs/corsair-icue-everything-you-need-to-know/

"Centrality" is my judgement for a typical owner of these three devices. H = must-have, M = expected, L = niche.

| Feature | What iCUE does | Centrality | Notes and sources |
|---|---|---|---|
| Home and device dashboard | Home screen shows the active profile, Murals presets, fan/pump/temperature sensors and device tiles. A Dashboard shows sensors with filter and resize. | H | [V] overview URL above |
| Per-device lighting (software) | Presets and custom effects per lighting zone, with stacked lighting layers per device | H | The first thing users notice. The SDK talks about "layer priority" [V] https://corsairofficial.github.io/cue-sdk/ |
| Hardware lighting / Device Memory Mode | "Software Mode vs Device Memory Mode": settings stored onboard so the device behaves the same without iCUE. Covers hardware lighting for fans and coolers and hardware key assignments. | H | Matters for boot, login screen and a crashed daemon. [V] overview, plus release notes v5.x https://cwsmgmt.corsair.com/documents/CORSAIR_iCUE_Software_Release_Notes.pdf |
| Murals | Whole-system lighting scene. Effects from image, video, audio or screen mirror. Extends to Hue, Nanoleaf and Govee. | M (for RGB fans), L otherwise | [V] overview. [S] Murals marketing |
| Fan and pump curves, cooling presets | Fan/pump presets (Quiet/Balanced/Extreme), custom curves on any sensor, live graphs, alerts | H for the AIO | [V] overview ("Cooling & Alerts") |
| Sensor monitoring | CPU/GPU temperatures, coolant, RPM, and the Dashboard | H | [V] overview |
| LCD screens (Elite LCD, Nautilus, XC7, Xeneon Edge) | Preset sensor screens, Image/GIF (480×480, at most 30 fps, at most 30 MB), GIPHY browser, a "Hardware Screen" shown when iCUE is not running, and widgets | H for LCD owners | [V] https://www.corsair.com/explorer/diy-builder/cpu-coolers/how-to-make-a-gif-for-elite-lcd/ . Help article https://help.corsair.com/hc/en-us/articles/4412131516813 (403 to the fetcher, content via search snippet [S]). GIPHY: iCUE 5 notes [S] |
| iCUE widgets (new in 2025–26) | HTML/JS widgets in QtWebEngine, `manifest.json` plus `index.html`, packaged as `.icuewidget` with a CLI. Run on the Xeneon Edge, keyboard LCDs and pump LCDs. | M and rising | [S] https://docs.elgato.com/icue/widgets/ . https://www.corsair.com/explorer/diy-builder/accessories/how-to-create-a-custom-widget-for-the-xeneon-edge/ . Note: web tech is *inside the widget*, which conflicts with the "no web" rule if we copied it. |
| Key assignments: macros, remap, text, media, launch app, Stream Deck actions | Per-key "actions", recorded macros, remaps, application launch | H for K100 owners (6 G-keys) | [V] overview. K100 page: "six dedicated macro keys … complex macros and key remaps" [V] Corsair K100 product page |
| Profiles and automatic per-app switching | Profiles bundle lighting, actions and performance. "Linked App" switches the profile when an app gains focus. | H | [V] https://help.corsair.com/hc/en-us/articles/360058672092 (via search). Long-running forum complaints that auto-switching breaks [S] https://forum.corsair.com/forums/topic/146984-automatic-profile-switching/ |
| Onboard (hardware) profiles | K100 has 8 MB onboard memory holding "up to 200 profiles". HW profiles cannot be app-linked or default. | M | [V] K100 product page. [S] forum |
| iCUE control wheel (K100) | Brightness, app switch, zoom, track jog/select, custom modes, and the dial ring colour | M (K100-specific, visible) | [V] K100 product page. Wheel bugs fixed in 5.x notes [S] |
| Polling rate | K100: up to 8,000 Hz reports and 4,000 Hz scan (AXON). Set in Settings or Performance. | M (set once) | [V] K100 product page |
| Performance: Win-lock, debounce, actuation | Win-key lock and the disabled-key list. OpenLinkHub exposes K100 "Performance Mode" and debounce. | M | [V] OpenLinkHub k100.go (`cmdInitDebounce`, `cmdDebounceTime`) |
| Firmware updates | In-app, and since April 2025 also a **web (WebHID) Firmware Update Utility**, which has a Linux guide | M (rare but needed) | [V] https://www.corsair.com/newsroom/press-release/corsair-launches-new-web-based-firmware-update-utility-enabling-updates-without-additional-software . https://www.corsair.com/explorer/gamer/corsair-firmware-update-utility/how-to-use-the-corsair-firmware-update-utility-on-linux/ |
| Game integrations | Lighting reacts to game state, through the SDK and partner "event based SDKs" | L–M | [V] cue-sdk README ("event based SDKs on iCUE integrations for games") |
| Plugins and integrations | NVIDIA Broadcast, Hue, Nanoleaf, Govee, Stream Deck plugin | L | [V] overview |
| Notifications and alerts | Firmware notices, temperature alerts | M (temperature alerts matter for the AIO) | [V] overview |

Summary of what is central for our three devices: lighting (software plus a hardware fallback), fan/pump curves on sensors, sensor dashboard, LCD content (sensor screens and GIFs, plus a hardware screen), K100 macros/remaps/G-keys, profiles with per-app switching, wheel behaviour, and polling rate. Murals, the GIPHY browser, game integrations and third-party lights are secondary.

## 2. The official iCUE SDK

- Repo: https://github.com/CorsairOfficial/cue-sdk . Latest release **v4.0.84 (2023-11-21)**, last push 2024-06-12. The repo has no OSS licence (GitHub `license: null`). [V via gh api]
- API reference: https://corsairofficial.github.io/cue-sdk/ . **Windows 7/8/10/11 only (x86/x64)**. Requires iCUE 4.31 or later to be running: "iCUE SDK interacts with hardware through iCUE so it should be running". Does not work over RDP. [V]
- Exposes: device enumeration (type, id, serial, model, LED count, channels); LED colour set/get (immediate or buffered), LED positions, key-name→LED lookup; device properties (headset mic/sidetone/EQ, keyboard layout, battery, DIY channel info); connect/disconnect events; G/M/S macro-key events; shared vs exclusive control and layer priority. **No sensor, fan or pump, LCD or macro-authoring API.** [V]
- Licence: a proprietary EULA ("THE SOFTWARE IS A PROPRIETARY PRODUCT OF CORSAIR"), non-transferable, restricts reverse engineering "subject to applicable law", governed by California law. [V] Not usable on Linux, and it is only a client of iCUE anyway, so it is irrelevant to us except as a model for a "third-party apps can drive lighting" API.
- macOS: an old CUE SDK and the Python binding `cuesdk` listed macOS 10.13–11 [S] (https://github.com/CorsairOfficial/cue-sdk-python). The v4 docs list Windows only. [U] whether a current macOS SDK exists. iCUE itself does ship for macOS (5.x notes mention "iCUE macOS®") [S].
- **Linux:** I found **no official Corsair statement** on iCUE for Linux. [U for an absence] The closest Corsair-authored Linux material is the Firmware Update Utility Linux guide. It tells users to add a hidraw udev rule (`SUBSYSTEM=="hidraw", ATTRS{idVendor}=="…", ATTRS{idProduct}=="…", MODE="0666"`, or 0660 with GROUP plugdev) and says "Most mainstream distros like Ubuntu should work". The April 2025 press release says "support for Linux is being tested". [V] A French report says Corsair "ne certifie pas" Linux [S] https://www.cowcotland.com/news/97977/ . There are user open letters on the Corsair forum with no reply [S] https://forum.corsair.com/forums/topic/166022-corsairs-icue-on-ubuntulinux-os-a-open-letter-to-corsair/

## 3. Platform mechanics for an unprivileged user daemon

### Linux

**hidraw via udev `uaccess`**
- Pattern: `SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b7c", TAG+="uaccess"`. systemd-logind then grants an ACL to the user of the active local seat. Systemd ships the same pattern for AV controllers, lights and hardware wallets in `/usr/lib/udev/rules.d/70-uaccess.rules`. [V local file, systemd 262]
- **Ordering gotcha:** the file must sort *before* `73-seat-late.rules`, because the uaccess builtin only applies to devices already tagged. Nexus Open's own `/etc/udev/rules.d/70-nexus.rules` already documents this after a bug with `99-…`. Packages should ship `60-`/`70-` names in `/usr/lib/udev/rules.d/`. [V local]
- OpenRGB ships `60-openrgb.rules` into `/usr/lib/udev/rules.d/` from distro and AUR packages. Since 1.0 the file is generated by the binary (`--generate-udev-rules`). AUR commenters recommend `uaccess` over `plugdev`. [V] https://openrgb.org/udev . [S] https://aur.archlinux.org/packages/openrgb-git
- OpenLinkHub uses `MODE="0660", OWNER="openlinkhub"` with a dedicated system user and group, not uaccess. [V] https://github.com/jurkovic-nikola/OpenLinkHub (99-openlinkhub.rules)
- ckb-next runs a **root** daemon over usbfs. `--nonroot` "will almost certainly not work". [V] https://manpages.ubuntu.com/manpages/noble/man8/ckb-next-daemon.8.html . This is the model to avoid.
- Notes on ACLs. They are checked at `open()`, so an fd that is already open survives a VT switch or the session going inactive. A daemon over SSH-only, or without a seat session, gets no ACL. [U, standard logind semantics] Keystroke privacy: a uaccess tag on *every* hidraw node of the K100 also exposes the keyboard-input interface (the user can already read their own keys, but it is still wider than needed). Prefer matching the vendor interface with `ATTRS{bInterfaceNumber}=="01"` (plus `02` for the event interface). [U, test the rule]

**Does using the vendor interface interfere with typing?**
- The K100 is a composite device. The vendor collections are **interfaces 1 and 2 with usage page 0xFF42**. Normal keyboard reports come from a separate boot-keyboard interface handled by usbhid → input. [S] SignalRGB log https://forum.signalrgb.com/t/corsair-k100-rgb-signalrgb-disables-onboard-features-g-keys-wheel-profile-win-lock/9343 . K55 Core shows the same pattern (`U=0001 P=0xFF42 I=1`) [S] https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/4892
- Each HID interface gets its own hidraw node, and writing to it does not detach usbhid. There is no interference at the transport level. [V for the per-interface nodes] https://lore.kernel.org/all/CANq1E4Q=DC6OvtvgBtqDoYb4ZWVWSf9KXBo3=4jwS-AWzFhCiw@mail.gmail.com/T/
- **But there is a protocol-level effect.** OpenLinkHub's K100 driver sends `cmdSoftwareMode = 01 03 00 02` on start and `cmdHardwareMode = 01 03 00 01` on stop. It listens on interface 2 for key events. While software mode is on, the host must implement G-keys, wheel and remaps. OLH does this through its `inputmanager` package, which uses uinput. [V] OLH `src/devices/k100/k100.go` (lines ~128, 278, 356, 3129–3165). The SignalRGB thread reports that taking software control "disables onboard features (G-keys, wheel, profile, Win-lock)" [S]. OLH issue "K65 Wireless Plus: Cannot type * or | while OpenLinkHub is active" shows what goes wrong when the host re-implements keys [S]. **Implication:** if Nexus Open takes software mode, it owns macros, wheel and Win-lock and must survive crashes (fall back to hardware mode on exit). Lighting-only use might be possible without software mode via a "direct" lighting endpoint. [U, check OpenRGB CorsairPeripheralV2Controller]
- Avoid usbfs, `libusb_detach_kernel_driver` and nusb claim on a keyboard. Detaching the wrong interface kills typing (ckb-next docs warn "you'll have no input" without the daemon). [S] https://wiki.gentoo.org/wiki/Corsair_Strafe_RGB

**uinput for macros and remaps**
- `/dev/uinput` on Arch is `root:root 0660` by default. Access comes only from a rule like `KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"`, which `steam-devices` ships as `60-steam-input.rules`. [V local: `getfacl /dev/uinput` shows `user:<you>:rw-`] We should ship our own copy rather than rely on Steam.
- The `input` group grants `/dev/input/event*` (from `50-udev-default.rules: SUBSYSTEM=="input", GROUP="input"`), **not** uinput. Group-based fallbacks (`GROUP="input"`/`"uinput"`, `MODE="0660"`) are used by kanata and others for non-logind systems. No root is needed with the uaccess rule. [V local] [S] https://docs.rs/crate/kanata/1.11.0/source/docs/setup-linux.md
- Wayland: uinput devices sit below the compositor. libinput sees them as real keyboards, so injection works on every compositor (ydotool, kanata and xremap rely on this). [S] What Wayland breaks is **knowing the focused app** for per-app profiles. There is no standard protocol. Options are a KWin script plus D-Bus (Plasma 6), a GNOME Shell extension (e.g. window-calls), Hyprland/Sway IPC, `wlr-foreign-toplevel` on wlroots, or XWayland `WM_CLASS` only. [S] https://github.com/insidewhy/splash-damage . https://codeberg.org/impl-izanagi/evdev-remap . https://mail.kde.org/pipermail/kwin/2024-January/005308.html . This should be a per-compositor plugin layer.
- Rust crates: `evdev` 0.13.2 (Apache-2.0 OR MIT, has a uinput VirtualDevice builder), `input-linux` 0.7.1 (MIT), and `uinput` 0.1.3 (WTFPL, last release 2018, avoid). [V crates.io]

**Hotplug and sleep/resume**
- Hotplug: libudev monitor on `hidraw` and `usb` with the `udev` 0.9.3 (MIT) or `tokio-udev` 0.10 (Apache/MIT) crates. [V crates.io]
- Sleep: subscribe to logind `org.freedesktop.login1.Manager.PrepareForSleep(bool)` and take a *delay* inhibitor lock. Before sleep, switch devices to hardware mode or set a safe pump/fan state. On resume, re-enumerate, because devices often re-enumerate and old fds go dead. [U, standard logind API] Evidence that this is the usual pain point: OLH issues "Reset / Restart after system suspend" (18 comments) and "Not reinitialising after suspend" (11). ckb-next has "Keyboard lights no longer sleeping on idle". [V gh issue titles] There is also a Capellix pump not starting after wake (BIOS fan error) [S] https://forums.tomshardware.com/threads/cpu-fan-error-from-corsair-h100i-aio-capellix-pump-not-turning-on-after-pc-wakes-up-from-sleep.3656944/

### macOS
- Each HID interface/collection appears as its own `IOHIDDevice`. Opening a device with **keyboard usages** triggers the Input Monitoring (TCC `kTCCServiceListenEvent`) prompt on macOS 11+. SDL's hidapi patch: "Touching HID devices with keyboard usages will trigger a keyboard capture permission prompt on macOS 11+." [V] https://discourse.libsdl.org/t/sdl-hidapi-mac-only-enumerate-iohiddevices-that-are-likely-to-be-joysticks/43964 . So, match only the 0xFF42 usage page and never open the keyboard collection; even enumerating with a broad `IOHIDManagerOpen` can fail or prompt [S] (Wine bug).
- Apple DTS (2025): Input Monitoring is required for devices the system "directly" understands (keyboards and mice). Vendor-defined devices *may* avoid it "with the right device type". In that thread SetReport worked but input reports did not arrive. DTS recommends the new **CoreHID** framework. [V] https://developer.apple.com/forums/thread/804793 . **[U]** whether reading input reports from the K100's 0xFF42 collection needs Input Monitoring. Test on hardware.
- Seize (`kIOHIDOptionsTypeSeizeDevice`) is only needed to take exclusive ownership. It is not needed for output or feature reports to a vendor collection. hidapi on macOS opens non-exclusive by default (`hid_darwin_set_open_exclusive`). [U from memory of the hidapi source]
- Macros and remaps on macOS: injection needs CGEvent (Accessibility permission) or Karabiner's DriverKit VirtualHIDDevice (needs a system extension). [U]
- Sandboxing: an unsandboxed, notarized app is simplest. A sandboxed app needs `com.apple.security.device.usb`/HID entitlements. [U]

### Windows
- Windows opens keyboard (0x01/0x06) and mouse (0x01/0x02) top-level collections **exclusively** for the Raw Input Manager, so they cannot be opened by apps. Vendor-defined collections are *not* on that list and are opened with sharing through `CreateFile` and `HidD_*`/hidapi. [V] https://learn.microsoft.com/windows-hardware/drivers/hid/top-level-collections-opened-by-windows-for-system-use . hidapi author: "HIDAPI won't work with keyboards and mice on Windows" (meaning those collections) [S].
- iCUE coexistence: the **"Corsair Service" (Corsair.Service.exe) keeps exclusive control** even with the UI closed [S]. SignalRGB: "the last software opened takes control" [S] https://docs.signalrgb.com/troubleshooting/brand-specific/corsair/ . FanControl.CorsairLink: "will not function correctly if Corsair iCUE and the 'Corsair Service' service is running. These programs should be stopped" [V] https://github.com/EvanMulawski/FanControl.CorsairLink . Monitoring tools (HWiNFO, AIDA) polling the Commander Core flip it back to hardware mode [V SignalRGB docs, Corsair help]. **Uninstall is not strictly required. Stopping and disabling the service is enough, but updates can re-enable it** [S]. Recommend detecting a running iCUE/Corsair Service and refusing to drive the AIO while it runs.
- Macros and remaps: `SendInput` for injection. Low-level hooks or the Interception driver for true remap. With the K100, prefer firmware key assignments or vendor-interface key events, so no hook is needed. [U]

### Rust crate fit (crates.io, 2026-11-15) [V]
| Crate | Version / updated | Licence | Notes |
|---|---|---|---|
| `hidapi` (hidapi-rs) | 2.6.7 / 2026-08 | MIT (wraps the C hidapi, which is triple-licensed BSD/GPLv3/HIDAPI-orig [U from memory]) | Mature, 10.5 M downloads. Linux backends: hidraw (C) or `linux-native` (pure Rust hidraw) [U feature names]. Blocking API. Supports feature reports and usage-page enumeration on all three OSes. **Safest default.** |
| `async-hid` | 0.5.3 / 2026-06 | MIT | Pure Rust. hidraw / IOHIDManager / **WinRT** on Windows. [U] whether WinRT HID exposes feature reports and all vendor collections as well as Win32 HidD does. Small user base (66 k). |
| `nusb` | 0.2.9 / 2026-10 | Apache-2.0 OR MIT | Raw USB, not HID. Needs to claim the interface: on Linux that means detaching usbhid; on Windows a WinUSB driver; on macOS it competes with the IOKit HID driver. **Wrong tool for HID-class Corsair devices.** Fine for bulk-endpoint LCDs if any appear. [U for the macOS detail] |
| `hidra` | 0.0.4 / 2026-09 | MIT | HID over OS stack or nusb. Too young. |

Recommendation: use `hidapi` (linux-native backend) behind our own async actor per device. The existing nexusd probably already does something similar. [U, did not check repo]

## 4. Risks

**Bricking**
- Never implement firmware flashing. Corsair now offers a WebHID Firmware Update Utility with a Linux udev guide, so point users there. [V] Corsair-side update failures already happen (K100 FW 1.11→1.13→1.14 failures; K63/K90 bricks after updates) [S] https://appuals.com/k100-rgb-firmware-update-failed/ . https://forum.corsair.com/forums/topic/88696-k90-brick-after-fw-update/ . The K100 has an Esc-held-on-plug reset [S]. The Galleon has an official "B+L" bootloader recovery [S].
- Writing onboard memory (hardware profiles, hardware lighting, the LCD hardware screen GIF) is a flash write. Rate-limit it, write only on explicit user "save to device", and never in a loop. OpenRGB's 0.8/0.9 notes warn that corruption "remains possible" and recommend Direct mode only [S] https://www.softpedia.com/progChangelog/OpenRGB-Changelog-263197.html . Users report "lost" stored profiles and fan LEDs dark after OpenRGB [S] https://forum.endeavouros.com/t/openrgb-bricking-lighting-node/57126 (that user's hub later turned out fine).
- Commander Core firmware 2.6.201 broke Capellix setups (combined Core + XT + LCD) [S] https://forum.corsair.com/forums/topic/177043-26201-firmware-breaks-h150i-elite-capellix/

**Pump and fan safety**
- Commander Core boots in hardware mode and stays there until told otherwise. Software mode requires the host to keep talking: a "wake" is required after "a predefined number of seconds" of host silence, and "Sleep should be run when the device should return to hardware mode". [V] liquidctl protocol doc https://sources.debian.org/src/liquidctl/1.15.0-2/docs/developer/protocol/commander_core.md . FanControl.CorsairLink discussion #29 [S].
- [U] Exactly what the pump does after a host timeout (firmware default curve vs last duty) is not documented. Test with a temperature probe and a watchdog. Design: keep a sane hardware-mode pump/fan profile written to the device, return to hardware mode on stop, sleep and crash (systemd `ExecStopPost` helper), and run a software watchdog thread.
- FanControl.CorsairLink enforces **minimum pump duty 50%** on Commander Core and iCUE LINK "to prevent a 'pump failure' state and noise/resonance", with a default of 100%. [V] Adopt the same floor. liquidctl's man page lists the pump channel only for non-XT Commander Core [V] https://man.archlinux.org/man/liquidctl.8.en
- Exclusive access: HWiNFO, AIDA, OpenRGB, OLH, liquidctl and iCUE polling simultaneously cause "false pump failure" and mode flapping [V Corsair help article 4418796228877; S forum]. Detect other owners and warn.

**Warranty and ToS**
- The Corsair limited warranty covers hardware operating "in accordance with published technical specifications". I found **no Corsair statement** that third-party software voids it. [U for the absence] https://www.corsair.com/explorer/general/general/warranty-guide/ . In the US, Magnuson-Moss generally bars conditioning a warranty on using the maker's own products unless they are free or a waiver is given. Damage actually caused by our software is still on us. [U, not legal advice]
- iCUE and SDK EULA restrict reverse engineering "subject to applicable law" [V]. The EU Software Directive art. 6 (interoperability decompilation) and DMCA §1201(f) give interoperability carve-outs. Safest practice: clean-room work from USB captures of *your own* hardware, and cite liquidctl, OpenRGB and OLH protocol docs. Do not ship iCUE code or assets (GIFs, icons, fonts). [U, not legal advice]

**Trademarks**
- "ICUE" is a registered mark of Corsair Memory, Inc. (US Reg. 6002996) [S] https://markinton.com/trademark/icue-87618234 . CORSAIR is obviously registered too.
- The US nominative fair use test (New Kids on the Block, 9th Cir. 1992) has three parts: the product cannot be readily identified without the mark; only as much of the mark as necessary is used; and nothing suggests sponsorship. [V] https://en.wikipedia.org/wiki/Nominative_use . The EU allows referential use to "indicate the intended purpose" (EUTMR art. 14(1)(c)) [U, from memory].
- Practical rule: **do not put "Corsair" or "iCUE" in the project, binary or app name or the icon.** Text like "Nexus Open supports CORSAIR K100 RGB…" in docs and the device list is fine, as are model names in the UI, a "not affiliated with or endorsed by Corsair" line, and no logos. The current name "Nexus Open" uses "iCUE NEXUS"'s product word "Nexus", a generic English word. [U] Lower risk, but keep the tagline from implying it is official. Precedent: ckb-next and OpenLinkHub avoid "Corsair" in their names. OLH's name echoes "iCUE LINK" and it carries a "not an official Corsair product" disclaimer [V README].

## 5. Other open "iCUE alternatives" and their gaps

| Project | Licence / activity | Scope | What users complain about |
|---|---|---|---|
| **OpenLinkHub** (Go) https://github.com/jurkovic-nikola/OpenLinkHub | GPL-3.0, 1.26 k★, very active (push 2026-11-14) [V] | Closest competitor and **already supports all three target devices plus iCUE NEXUS**: Commander Core 0c1c/0c32 with Elite Capellix and Elite LCD models, K100 1b7c/1b7d/1bc5 (RGB, key assignments, macros, performance), LCD over HID (PIDs 0c33/0c39/0c42/0c4e/0c55/0c57…) [V supported-devices.md, lcd.go] | **Web UI** on 127.0.0.1:27003. System service with a dedicated user and `/opt` config. Top issues [V gh]: suspend/resume reinit, high CPU, config.json breakage, crashes, K100 missing keys, keys untypable with the K65, RGB not staying applied, LCD can't change display. Effectively one maintainer. |
| **OpenRGB** (C++/Qt) https://gitlab.com/CalcProgrammer1/OpenRGB | GPL-2.0+, active [V] | Multi-vendor lighting only | No macros or DPI, and no fan/pump curves [S] https://lemmy.kde.social/post/8664 . Can clobber onboard profiles [S]. Corsair K100 detection flaky [S] https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/2389 . "Never had luck with openrgb" style sentiment [S]. |
| **ckb-next** (C/Qt) https://github.com/ckb-next/ckb-next | GPL-2.0, last release v0.6.2 (2025-03) [V] | Keyboards and mice | Root daemon. K100 issues open for years (wheel binding, FN, 1bc5 support) [V gh]. Wayland GUI crashes [V gh titles]. Slow releases. |
| **liquidctl** (Python) https://github.com/liquidctl/liquidctl | GPL-3.0 [V] | CLI for coolers | One-shot CLI, hardware mode only, no daemon or curves [S] issue #697. Needs CoolerControl on top. |
| **CoolerControl** https://gitlab.com/coolercontrol/coolercontrol | GPL-3.0+, active [V] | Fan/pump curves GUI and daemon over liquidctl and hwmon | Not lighting, keyboards or LCD-centric. Root daemon. [U] |
| **FanControl.CorsairLink** (Windows) | Active 2026-05 [V] | Fan Control plugin | Windows only, software mode only. Needs iCUE stopped. |
| SignalRGB, Artemis | Proprietary freemium / GPL-ish | Lighting | SignalRGB disables K100 onboard features [S]. |

**What a 9/10 alternative must get right** (synthesised from the gaps above and iCUE's own complaint history: disconnects, profiles not loading, auto-switch failing [S] https://tech4gamers.com/icue-still-needs-lots-of-attention-from-corsair/):
1. **Safety first for cooling.** Pump floor, hardware-mode fallback on stop, sleep and crash, a watchdog, and detection of conflicting tools.
2. **Rock-solid lifecycle.** Hotplug, suspend/resume and re-enumeration with no restart needed. This is the number one OLH complaint.
3. **No root and no web UI.** uaccess udev rules shipped in the package, a user daemon, and a native UI. This is a differentiator against OLH (web, system service), ckb-next (root) and CoolerControl (root daemon).
4. **One app for all three device classes**: lighting, cooling curves on any sensor, LCD, keyboard macros and remaps, with *shared sensors* (the existing signals system) driving LCD screens, curves and lighting.
5. **Respect onboard memory.** Never silently overwrite hardware profiles. Provide an explicit "save to device", and make sure the keyboard works when the daemon is not running.
6. **Keyboard correctness.** If software mode is taken, every key, the wheel, Win-lock and G-keys must work, including layouts (OLH has German-layout and missing-key issues). Recover input on crash.
7. **Per-app profiles that actually work on Wayland**, through compositor backends (KWin, GNOME extension, Hyprland/Sway, wlr).
8. **Low CPU.** Event-driven, not polling loops (an OLH complaint).
9. **Stable config with migrations.** We already have SQLite with schema versions, which suits this; OLH's config.json breakage is a recurring issue.
10. Firmware: don't implement it. Link to Corsair's web updater.

## Unverified / open items to test on hardware
- Whether K100 lighting can be driven without entering software mode (so onboard G-keys and wheel keep working).
- What the Commander Core does with the pump after a software-mode host timeout or a crash.
- The exact Elite LCD PID on Matt's unit (likely 0c39 or 0c33) and its transport. The OLH LCD driver uses HID, with PIDs 3150/3139/3129/3123/3159/3157/3138 decimal.
- macOS: whether reading input reports from the K100 0xFF42 collection needs Input Monitoring.
- Windows: whether async-hid's WinRT backend reaches feature reports and vendor collections as Win32 hidapi does.
- Whether Corsair has any newer public Linux position than the firmware-utility guide.
