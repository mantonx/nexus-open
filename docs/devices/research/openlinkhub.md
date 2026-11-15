# OpenLinkHub research notes (for Nexus Open device expansion)

Source: https://github.com/jurkovic-nikola/OpenLinkHub, shallow clone at commit
`4b59c5a2532ec2049e38857e5d860561775b6125` (2026-11-14), cloned to
`/tmp/claude-1000/-home-fictional-Projects-nexus-next/79883d82-05f8-4b3d-8732-30daaf6deb54/scratchpad/research/OpenLinkHub`.
I only read the source. I did not build or run it, and I did not touch any device.

Paths below are relative to the repo root. **[V]** = I read it in the source.
**[I]** = my inference or interpretation, not stated in the code.

---

## 1. Licence

- [V] `LICENSE` is the unmodified GNU GPL v3 text. Every Go source file has the header
  `// License: GPL-3.0 or later` (196 files, all identical; e.g. `src/devices/k100/k100.go:5`).
  **SPDX: `GPL-3.0-or-later`.**
- What that means for an MIT project [I, this is not legal advice]:
  - **Copying code is not OK** if Nexus Open stays MIT. That covers Go functions, tables, structs
    translated line by line, and the data files: `database/keyboard/*.json` layouts with
    packetIndex/keyData/keyHash tables, `database/external/lsh.json`, LED-count tables and so on.
    A literal translation is a derivative work, so the combined binary would have to be GPL-3.0+.
    Large data tables such as the 581-byte K100 packet-offset map are the risky part: copying
    the JSON wholesale is copying.
  - **Learning protocol facts is OK.** Copyright does not cover facts or interoperability
    information: command bytes, report sizes, sequences, which byte holds RPM. You can write an
    independent implementation from those facts. Good practice:
    (a) write a protocol spec in your own words (like these notes), then implement from the
    spec, not from the Go;
    (b) cite OpenLinkHub, liquidctl, OpenRGB and ckb-next as references in docs;
    (c) re-derive per-key tables yourself, for example by lighting LED index *n* and recording
    which key lights, instead of importing their JSON;
    (d) don't copy comments or identifiers verbatim.
  - liquidctl (GPL-3.0) and OpenRGB (GPL-2.0) are also copyleft, so the same rule applies to
    them. ckb-next is GPL-2.0.

## 2. Architecture

**Language and runtime [V].** A single Go binary. `main.go` traps SIGINT/SIGTERM and calls
`controller.Start()`. `src/controller` initialises everything in order: config, logger, display,
media, audio, dashboard, systeminfo, metrics, rgb, lcd, temperatures, keyboards, inputmanager,
applications, appmonitor, stats, macro, motherboards, **devices**, monitor, language, scheduler,
server.

**HID access [V].** `github.com/sstallion/go-hid v0.14.1` (`go.mod:8`), a cgo hidapi binding.
Legacy USB devices (H80i/H100i/H115i Hydro, H1xxi Platinum, Link TM dongle;
`legacyDevices` at `src/devices/devices.go:183`) go through raw usbfs ioctls in `src/usb/usb.go`,
which walks `/sys/bus/usb/devices`. DDR memory goes over SMBus (`src/smbus`), the motherboard
through `src/motherboards`.

**Discovery [V]** (`src/devices/devices.go:592-750`):
1. `hid.Enumerate(0x1b1c, any)` and then `hid.Enumerate(0x2e95 /*SCUF*/, any)`.
2. Each product ID is looked up in `deviceRegisterMap` (`devices.go:753-941`, 187 entries).
   Each entry is `{InterfaceId, UsagePage, Name, Init, InitEx}`. The device is kept only when
   `info.InterfaceNbr == InterfaceId`, or when `UsagePage` matches if one is set.
3. The key is the serial number (or the PID if there is no serial, or the path for interface
   1/3/4, or the devpath for PSUs).
4. `config.Exclude` is checked (`devices.go:735`, log "Product excluded via config.json").
5. `initializeDevice` starts one goroutine per device, calling `pkg.Init(vid,pid,serial,path)`.
   Dongles use `InitEx` with a callback so they can register their paired sub-devices.
6. Every device package returns a `*common.Device{Instance: d}`. The web/API layer calls
   methods by **reflection** (`CallDeviceMethod`, `devices.go:421`), using method names such as
   `"UpdateDeviceSpeed"` and `"StopDirty"`.

**Per-device packages [V].** `src/devices/<name>/<name>.go`, about 150 packages. Each one
re-implements its own `transfer()`, `read()`/`write()`, RGB loop, profile I/O and so on. There is
heavy copy-paste and no shared protocol layer. Suffixes: `WU` = wireless device plugged in by
USB cable, `W` = the same device reached through a dongle. Wireless sub-devices are created by
the dongle package (e.g. `slipstream.go` imports 20 `*W` packages).

**Sharing a device between features [V].** Each device struct owns its `*hid.Device` handles
(main plus optional `listener`, and an `lcd` for the Commander Core). Access is serialised by
two mutexes per device:
- `deviceLock` guards a full open/write/close endpoint transaction;
- `mutex` guards a single `transfer()` (write then read).

RGB animation runs in its own goroutine, and frames from OpenRGB or the cluster go into a
buffered `queue` (`chan []byte`, capacity 10, and frames are dropped when it is full). Speed
control has its own ticker goroutine, and so does the 1 s sensor poll. All of them contend on
`deviceLock`. Nothing arbitrates between processes: OLH holds the hidraw handles open the whole
time, so anything else that writes to the same device will interleave with it.

**Hot-plug, sleep and resume [V]** (`src/monitor/monitor.go`):
- A NETLINK_KOBJECT_UEVENT socket watches `usb_device` add/remove events. On add it sleeps
  7 s (+2 s for some PIDs) and then calls `devices.InitManual(pid, serial)`, which also checks
  `Exclude` (`devices.go:583`). On remove it calls `StopDirty`.
- D-Bus `org.freedesktop.login1.Manager.PrepareForSleep`: going to sleep calls
  `devices.Stop()` (which sends the hardware-mode commands). On resume it sleeps `resumeDelay`
  (15 s by default) and then calls **`os.Exit(1)`** so systemd restarts it (comment at around
  line 104).

**Daemon and service [V]:**
- `OpenLinkHub.service` is a **system** service with `User=openlinkhub`,
  `WorkingDirectory=/opt/OpenLinkHub`, `Restart=always`. Its `ExecStartPre=+udevadm` lines
  reload rules and trigger `idVendor=1b1c`.
- `install-user-space.sh` writes `~/.config/systemd/user/OpenLinkHub.service` (WorkingDirectory
  = the install dir) and rewrites the udev rules from `OWNER="openlinkhub"` to
  `GROUP="openlinkhub"`, adding the user to that group.
- `config.setSystemService()` (`src/config/config.go:~230`) treats uid<1000, or a home directory
  outside `/home/`, as running in system-service mode.
- `99-openlinkhub.rules`: 194 lines, one `SUBSYSTEMS=="usb", ATTRS{idVendor}=="1b1c",
  ATTRS{idProduct}=="xxxx", MODE="0660", OWNER="openlinkhub"` line per PID. It **includes
  1b7c (line 33) and 1b8e (line 86)** and sets
  `KERNEL=="uinput", MODE="0660", GROUP="input", OWNER="openlinkhub"` (line 194).
  **Hazard for us [I]:** if Matt installs these rules, the Nexus hidraw node becomes owned by
  `openlinkhub` with mode 0660. Unless his user is in the owner's group, Nexus Open loses
  access. Exclude 1b8e there too, or install the user-space (GROUP) variant.

**Web UI and API [V].** `src/server/server.go` (3236 lines) uses a `net/http` ServeMux with
around 200 `/api/...` routes: GET `/api/devices/`, `/api/temperatures/`, `/api/color/`,
`/api/keyboard/...`, `/api/macro/` and so on, plus POST/PUT/DELETE routes. HTML comes from Go
templates in `web/*.html` (one per device, e.g. `cc.html`, `k100.html`, `nexus.html`) with
`static/` assets. It listens on `127.0.0.1:27003` by default (`config.go` defaults). There is
optional TLS (a `certificates` dir) and `src/auth`. An OpenRGB SDK *server* is optional on port
6743 (`enableOpenRGBTargetServer`).

**Config and storage [V].** Everything is JSON files under the working dir
(`ConfigPath` = cwd):
- `config.json` (`config.go:95`). Fields are in `config.go:16-50`, including `listenPort`,
  `listenAddress`, `manual` (no fan control), `exclude []uint16`, `checkDevicePermission`,
  `graphProfiles`, `resumeDelay`, `enableOpenRGBTargetServer`, `enableGamepad`, `appMonitor`.
- `database/profiles/<serial>.json` holds per-device state: speed profile per channel, RGB
  profile per channel, LCD mode/rotation/brightness, labels, keyboard layouts and key
  assignments.
- `database/rgb/<serial>.json` holds RGB effect parameters.
- `database/temperatures/` holds fan curves.
- `database/keyboard/*.json` holds layouts with LED packet offsets, key-assignment indexes and
  key hashes.
- `database/lcd/` holds LCD images, `database/nexus/` holds Nexus assets,
  `database/macros/` holds macros, and `database/external/lsh.json` lists 35 iCUE LINK
  sub-device types.

---

## 3. Device protocols

### 3.0 Common "Bragi"-style framing, used by the Commander Core and the K100 [V]

Same shape in `cc.go:3976-4015` and `k100.go:2725-2751`:
```
OUT (hid_write, bufferSize+1 bytes):  [0]=0x00 (report ID 0), [1]=0x08, [2..]=command bytes, then payload
IN  (hid_read,  bufferSize bytes):    response; fields are read at fixed offsets (see below)
```
- Wireless sub-devices use `[1] = d.Endpoint` (0x09 for a device behind a dongle; e.g.
  `k100airW.go:3180`).
- Software mode is `01 03 00 02` and hardware mode is `01 03 00 01`. The firmware query is
  `02 13`, and the response holds major=`r[3]`, minor=`r[4]`, patch=`LE16 r[5:7]`.
- The "property get" form is `02 <prop>`. The "property set" form is `01 <prop> 00 <value...>`
  [I, inferred from the K100 brightness/polling/debounce commands].
- Endpoints/handles: open `0d <handle> <resource>`, close `05 01 <handle>` (+ resource), read
  `08 <handle>`, write `06 <handle> <payload>`, continuation `07 <handle> <payload>`.

### 3.1 iCUE Commander Core 1b1c:0c32 (PID 3122) — `src/devices/cc/cc.go`

**Registration [V].** `devices.go:756` maps `3122 -> {iface 0, "iCUE COMMANDER Core", cc.Init}`.
0c1c (3100) uses the same package. It is opened with `hid.Open(vid,pid,serial)` (`cc.go:351`),
so hidapi picks the first interface. The second HID interface is never used [V by absence; I
that it is unused for a reason].

**Report size [V].** `bufferSize=64`, writes are 65 bytes (`cc.go:60-61`). Only **0c1c** is
switched to 96/97 bytes with a max chunk of 93 (`cc.go:420-426`, comment: "There are 2 CCs. One
has a packet size of 64 and the other has 96"). **0c32 = 64-byte reports.**

**Command table [V]** (`cc.go:38-59`):

| name | bytes |
|---|---|
| open endpoint | `0d 01` + resource |
| open colour endpoint | `0d 00` + resource |
| close endpoint | `05 01 01` + resource |
| read | `08 01` |
| write | `06 01` |
| write colour | `06 00` |
| colour continuation | `07 00` |
| firmware | `02 13` |
| pump version | `02 57` → `r[3]` |
| radiator size | `02 58` → `LE16 r[3:5]` (240/280/360/420) |
| software mode | `01 03 00 02` |
| hardware mode | `01 03 00 01` |
| LED port config | resource `1e` |
| resource: LED counts | `20` |
| resource: speeds | `17` |
| resource: set speed | `18` |
| resource: temps | `21` |
| resource: fan presence | `1a` |
| resource: colour | `22` |
| data type set speed | `07 00` |
| data type set colour | `12 00` |

**Endpoint transaction [V]** (`read()` at `cc.go:3384`, `write()` at `cc.go:3563`):
`close(05 01 01 R)` → `open(0d 01 R)` → `read(08 01)` or `write(06 01 <payload>)` → `close(05 01 01 R)`.
Write payload = `[LE16 len][00 00][dataType 2 bytes][data...]`. `len = len(data)+2` when
`extra=true`, which is the case for speed writes.

**Init sequence [V]** (`cc.go:428-455`):
1. getDeviceLcd (open 0c39, see 3.3)
2. firmware
3. software mode
4. `initLedPorts`: `14 <port> 01` for port 0..6, then sleep 500 ms (`cc.go:3124-3134`)
5. pump version + radiator size, matched against the `aioList` table (`cc.go:113+`; "H150i
   ELITE CAPELLIX" = pump v1 or v2, 360 mm, no LCD; the "ELITE LCD" variants use pump v3 with
   LCD=true)
6. LED device scan
7. fan scan
8. RGB devices
9. `setColorEndpoint` (`close 05 01 01 22`, `open 0d 00 22`, `cc.go:2063`): **the colour handle
   0 stays open for the whole session**
10. `setDefaults`: **all channels including the pump set to 50 %** (`cc.go:2208`)
11. 1 s poll ticker
12. `resetLEDPorts`
13. speed loop
14. colours
15. LCD

**Reading sensors [V]** (`getDeviceData`, `cc.go:2099-2206`, polled every 1000 ms):
- Fan presence: `read(1a)` → count `r[5]`, status per channel at `r[6+i]`. `0x07` = connected
  and spinning, `0x02` = connected with no tach.
- RPM: `read(17)` → `LE16` per channel starting at `r[6]`, 2 bytes per channel. **Channel 0 is
  the pump.** Values ≤20 are ignored.
- Temperatures: `read(21)` → count `r[5]`, 3-byte entries from `r[6]`: `[status, LE16 temp×10]`.
  Status `0x00` = valid, and the value is kept only if 10–100 °C. **Entry 0 is the AIO liquid
  temperature.** The external probe is read as `r[9:]` in `getDevices` (`cc.go:1630`).

**Setting speed [V]** (`setSpeed`, `cc.go:1670-1717`):
- `write(resource 18, dataType 07 00, buf, extra=true)`.
- `buf = [N] + N × [channel, mode, value, 0x00]`, where mode=0 and value = duty % (1 byte).
  [I] The 4-byte stride suggests `value` is LE16 or has a reserved byte.
- Success check: `r[5]==0x07 || r[2]==0x00`, else retry up to 20× with 100 ms between tries.

**Curves [V].** The curves are **host-side only**: there is no on-device curve upload.
- `updateDeviceSpeed` (`cc.go:1780-2010`) ticks every 3000 ms, reads the profile sensor (CPU,
  GPU, liquid, hwmon, external executable…), interpolates `Points[0]` (pump) and `Points[1]`
  (fans), and sends `setSpeed` only when the result changes.
- Clamps: fans ≥20 % unless ZeroRpm. Pump below 50 is forced to **70** in graph mode
  (`cc.go:1938-1941`); pump is clamped to 50–100 in table mode; manual `UpdateDeviceSpeed`
  forces pump ≥50 (`cc.go:2245-2269`).
- If every sensor fails, the temperature defaults to 50 °C.

**RGB [V]:**
- LED counts come from `read(20)`, with 4 bytes per port from `r[6]`: `[status(2=connected), ?,
  LE16 numLEDs]`, 7 ports (0 = pump, 1–6 = fan headers).
- LED count → device-type command (`cc.go:893-937`):

  | LEDs | command | device |
  |---|---|---|
  | 4 | 03 | ML PRO |
  | 8 | 05 | 8-LED |
  | 10 | 01 | strip |
  | 12 | 04 | HD |
  | 16 | 02 | LL |
  | 21/24/29 | 08 | pump |
  | 34 | 06 | QL |

- Port config: `write(resource 1e, payload 0d 00 07 + per port [01 cmd] or [00])` (`cc.go:3136`).
- Colour frame (`writeColor`, `cc.go:3422`): payload = `[LE16 len(data)+2][00 00][12 00][R G B …]`,
  split into 61-byte chunks. First chunk via `06 00`, then `07 00` (no open/close; handle 0
  stays open).
- LED order: port 0 (pump) LEDs first, then ports 1..6 in order, each port's LEDs consecutive,
  bytes R,G,B (`rgb.SetColor`, `src/rgb/rgb.go:368`).
- Animated effects run about every 20 ms (`cc.go:1422`); static colours are sent once.

**Hardware mode and closing [V].** `Stop()` (`cc.go:508-573`) sends the LCD feature reports
(see 3.3), then `setHardwareMode` (`01 03 00 01`), then closes the device. `StopDirty`
(unplug) skips hardware mode.

**Safety [V + I]:**
- [V] There is **no explicit keepalive** for the Commander Core. The 1 s sensor poll is the only
  regular traffic.
- [V] Pump minimum is 50 % in software.
- [I] If the process is SIGKILLed, crashes, or the resume `os.Exit(1)` path runs before Stop,
  the device stays in software mode at the last duty with no host curve. With a pump at ≥50 %
  this probably isn't dangerous, but fans may sit at a fixed duty. I did **not** find anything
  in this source about a firmware watchdog falling back to hardware mode; liquidctl's notes
  would be the next place to check.
- Recommendations for our implementation [I]: always restore hardware mode on exit and on
  suspend; never send pump <50 %; keep the fan floor; treat 0x02 status as "present".
- [V] No firmware-write or DFU commands exist in this package. Only `14 xx 01`, the endpoint
  ops and mode switches are used.

### 3.2 (same device) AIO pump/fans on the Elite Capellix

These are all handled through the Commander Core above. "AIO" = channel 0 (pump RPM + liquid
temperature + pump LEDs). Fans are channels 1..6.

### 3.3 LCD Cap for Elite Capellix 1b1c:0c39 (PID 3129)

**Not a standalone device [V].** It is not in `deviceRegisterMap`. `cc.getDeviceLcd()`
(`cc.go:788-819`) enumerates PIDs 3129 and 3123 and opens the one whose interface-0 serial
matches with `hid.Open`, setting `HasLCD=true`. `lcd.checkForLcd()` (`src/devices/lcd/lcd.go:1623`)
only uses 3129 to decide whether to load fonts and images.
- [I] Excluding 0c32 therefore also disables the LCD. The LCD is tied to whichever Commander
  Core enumerates first; there is no pairing by topology.

**Image format [V].**
- **480×480** (`lcd.go:79-80`). Sensor screens are **JPEG** at Go default quality (75)
  (`jpeg.Encode(...,nil)`, `lcd.go:1307`); animations use quality 90 (`lcd.go:868`).
- Uploaded images are resized to 480×480 and re-encoded to JPEG, and each GIF frame becomes a
  JPEG (`lcd.go:1432-1540`).

**Transfer [V]** (`cc.transferToLcd`, `cc.go:3947-3974`):
- Output reports are **1024 bytes** with header (8 bytes):
  `[0]=0x02 (report ID 2) [1]=0x05 [2]=0x00 [3]=last?0x01:0x00 [4]=chunkIndex (byte) [5]=0x00
  [6:8]=LE16 chunkLen`, followed by up to 1016 bytes of JPEG.
- "The last packet needs to end with 0x01 in order for display to render data."
- [I] Edge case: the "last" flag is set only when the chunk is shorter than 1016 bytes, so a JPEG
  that is an exact multiple of 1016 never gets flagged.
- [I] The chunk index is a single byte, so it wraps if a frame exceeds 255 chunks (≈259 KB).
  That isn't reached at 480² JPEG.
- [I] Byte [2] looks like a format/target selector: 0x00 here (also XC7 and Nautilus), 0x01 for
  iCUE LINK LCD (`lsh.go:6011`), 0x40 for the Nexus raw RGBA (`nexus.go:1770`).

**Frame rate [V].** Sensor modes refresh every 1000 ms (`lcdRefreshInterval`). Images/GIFs use
the per-frame GIF delay, or 100 ms when it is 0 (`cc.go:3894-3945`).

**Feature reports (ID 3, 4 bytes) [V]:**

| purpose | bytes | values / source |
|---|---|---|
| brightness | `03 0b <b> 01` | b ∈ {1=off, 4=33%, 16=66%, 64=100%}; `cc.go:50,397-401,2377` |
| rotation | `03 0c <r> 01` | r ∈ 0..3 = 0/90/180/270°; `cc.go:2737` |
| on Stop | `03 1e 01 01`, then `03 1d 00 01`, then `03 0b 40 01` | `cc.go:546-556` |
| on StopDirty | the first two only | |

[I] `1e`/`1d` probably return the cap to its built-in (hardware) screen.

**Safety [I].** The LCD is harmless. Leaving it in host mode just shows the last frame.

### 3.4 K100 RGB 1b1c:1b7c (PID 7036; also 7109, 7037) — `src/devices/k100/k100.go`

**Interfaces [V].** `devices.go:848` maps `7036 -> {iface 1, "K100", k100.Init}`. It is opened
by **path** on interface 1 (`k100.go:195`). **Interface 2** is opened as the event `listener`
(`backendListener`, `k100.go:3129-3146`). Interface 0 (the boot keyboard) is left to the kernel.

**Report size [V].** `bufferSize=1024`, writes are 1025 bytes, max chunk 1021
(`k100.go:157-161`). Framing is as in 3.0: `[00][08][cmd…]`.

**Command table [V]** (`k100.go:127-153`):

| name | bytes |
|---|---|
| software mode | `01 03 00 02` |
| hardware mode | `01 03 00 01` |
| activate LED / colour endpoint | `0d 01 22` (sleep 500 ms after) |
| brightness | `01 02 00 <LE16>` and `01 6d 00 <LE16>` (0–1000), both sent (`k100.go:1908`) |
| polling rate | `01 01 00 <code>` (1=125 Hz … 4=1000 Hz, 5=2 kHz, 6=4 kHz, 7=8 kHz); OLH sets `Exit=true` first (`k100.go:1012-1043`) |
| performance lock bitmask | `01 4a 00 <bits>` (1=WinKey, 2=Alt-Tab, 4=Alt-F4, 8=Shift-Tab), then `01 45 00 <0/1>` (`k100.go:538-599`) |
| debounce init | `01 09 00 01` and `01 39 00 01` |
| debounce value | `01 0a 00 <30+ms>` and `01 38 00 <30+ms>`, ms 1..9 (`k100.go:1945-1982`) |
| key assignment | open `0d 02 02`, write `06 02 <payload>`, close `05 01 02` |
| keepalive | `12` every **20 000 ms** (`k100.go:153,155,895-923`) |

[I] Because OLH sets `Exit=true` before changing the polling rate, the keyboard probably
re-enumerates afterwards.

**Init [V]** (`k100.go:269-290`): software mode → `0d 01 22` → firmware → profiles → keepalive
timer → colours → brightness → listener → performance and key assignment → debounce → queue.

**LED frame [V]:**
- The colour buffer is **581 bytes** (`colorPacketLength`, `k100.go:162`), RGB triplets
  addressed by byte offset `packetIndex` taken from the layout JSON
  (`database/keyboard/k100.json`, version 4, 8 rows, 165 entries, offsets 12..576). `LEDChannels:
  194` is set in Init.
- Examples (US layout): `ESC=123`, `F1=174`, `A=12`, `B=15`, `C=18`, `Q=60`, `Space=132`,
  `G1..G6=393..408`, `PF/profile=384`, `LOCK=342`, `DIAL ring=411`, `LOGO=570,573,576`, MUTE=306.
  The row-0 light bar is 414..477 and the side bars are 480..543.
- [I] LED index = offset/3, so offsets 0..11 are four LEDs not in the layout. `data[0..2]` are
  forced white in performance mode (probably the indicator).
- `lockLedIndex=342` turns red when performance mode is on (`k100.go:2394-2401`).
- **Control-wheel ring** = 7 LEDs at bytes 546..566 (`copy(data[546:567], …)`,
  `k100.go:2403`), coloured per wheel mode.
- Send (`writeColorBuffer`, `k100.go:2392-2425`): payload =
  `[LE16 len(data)][00 00][12 00][581 bytes]` (≈587 bytes). It fits in one chunk, sent with
  `06 01`; continuations would use `07 00`. Animated frames go every 20 ms (`k100.go:2273`).
- Copyright note [I]: the JSON layout tables (offsets, keyData, keyHash) are the GPL'd data you
  would most want to copy. Re-derive them instead.

**Key remap and macros [V]. Yes, it uses uinput.**
1. Key assignment array: 138 bytes (`KeyAssignment`, `k100.go:164`), one byte per `keyData`
   index. `0x01` = the firmware sends the normal HID key, `0x00` = the key is intercepted and
   reported only on the vendor interface (`setupKeyAssignment`, `k100.go:395-425`). Written with
   `[LE16 138][00 00][138 bytes]` through `0d 02 02` / `06 02` / `05 01 02` (`k100.go:3090-3112`).
   - [I] `keyData` values match HID usage IDs for normal keys (A=4, Esc=41, F1=58). G1–G6 =
     131–136, MUTE=102, LOCK=114, PF=128.
2. Events on interface 2 (read with 100 ms timeout):
   - `data[1]==0x02` is a key-state report. `data[2:22]` (20 bytes = a 160-bit bitmap,
     little-endian) has bit *n* set = key with keyData *n* pressed. Bits `0x04`/`0x40` in byte
     13 are masked off. The value is turned into a big integer that is the `keyHash` in the
     layout JSON (G1 hash `2722258935367507707706996859454145691648` = 2^131)
     (`k100.go:2825-2840`).
   - `data[1]==0x05` is the **control wheel**: `data[4]` = `0x01` clockwise, `0xFF`
     counter-clockwise (`k100.go:3173-3256`).
3. Actions are injected through `src/inputmanager` using `/dev/uinput` (virtual keyboard
   `keyboard.go:32`, mouse `mouse.go:43`, gamepad `gamepad.go:239`). Supported actions: media,
   keyboard key, macro (`src/macro`, with delays and repeats), DPI, sniper, scroll, zoom,
   profile switch.

**iCUE control wheel [V].** Seven modes (`k100.go:213-221`): 1 volume, 2 keyboard brightness
(±100 of 1000), 3 vertical scroll, 4 zoom, 5 screen brightness, 6 media next/prev,
7 horizontal scroll. A key flagged `IsDialChange` (the dial button) cycles modes. The ring colour
shows the mode.

**Onboard profiles [V by absence].** There is no code that writes hardware/onboard lighting or
profiles to K100 memory. Profiles are host-side JSON. "Hardware mode" just hands control back to
whatever the firmware already holds (iCUE-saved profiles).

**Safety [I]:**
- If the keepalive stops (process killed), the firmware probably drops back to hardware mode
  by itself after a timeout. That is the reason for the keepalive.
- Keys set to 0x00 (intercepted) stop typing while OLH isn't reading interface 2. That is a
  usability hazard (G-keys / remapped keys dead), not a hardware one.
- Avoid the polling-rate command unless you deliberately want it; it re-enumerates the
  keyboard.
- No firmware-update commands exist in this repo (grep for bootloader/DFU/firmware update found
  nothing).

### 3.5 iCUE Nexus 1b1c:1b8e (PID 7054) — `src/devices/nexus/nexus.go`

See section 4.

---

## 4. Nexus handling in OpenLinkHub and how to exclude it

**What it does [V]:**
- `devices.go:802` maps `7054 -> {iface 0, "iCUE NEXUS", nexus.Init}`, so **it is claimed by
  default**. `hid.Open(vid,pid,serial)` (`nexus.go:183`). A second handle on interface 0 is
  opened by path as the touch listener (`nexus.go:1661-1685`).
- Firmware: feature report ID 5, 33-byte buffer; ASCII digits at `[6]`, `[8]`, `[10]`,
  `[12:14]` (`nexus.go:352-365`).
- Display: **640×48 raw RGBA** (4 bytes/pixel = 122 880 bytes; `renderImageToBytes`,
  `nexus.go:953-970`), sent in 1024-byte output reports
  `[02][05][40][last?01][chunkIdx][00][LE16 len][≤1016 bytes]` (`nexus.go:1758-1786`). Refresh
  every 1000 ms (`nexus.go:166,1482`).
- UI: a tiled "screens" system (tiles = sensor value or button) with swipe between screens.
- Touch input: input reports with `data[0]==1 && data[1]==2 && data[2]==0x21`, state `data[5]`
  (1 = down/move, 0 = up), x = `LE16 data[6:8]`. Taps are mapped to macro, media key (uinput),
  app profile, or screen change (`nexus.go:1588-1735`).
- Stop: feature reports `03 0d 01 01` and `03 01 64 01` ("Switch LCD back to hardware mode";
  [I] 0x64 = 100 % brightness), then close (`nexus.go:247-283`).

**How to exclude it [V]:**
- **`"exclude": [7054]`** in **`config.json`** in OLH's working directory (`/opt/OpenLinkHub/
  config.json` for the system install, or the user-space install dir). The field is
  `Exclude []uint16 \`json:"exclude"\`` (`src/config/config.go:27`). It takes **decimal USB
  PIDs** (0x1b8e = 7054).
- It is checked at startup enumeration (`devices.go:735`) and on hot-plug (`InitManual`,
  `devices.go:583`).
- The web UI's "supported devices" toggles write the same list (`config.UpdateSupportedDevices`,
  `config.go:116`; `GetSupportedDevices` reports `Enabled=false`, `devices.go:247-270`).
- **Also** remove or adjust line 86 (`1b8e`) of `99-openlinkhub.rules` so the hidraw node isn't
  re-owned by the `openlinkhub` user [I recommendation].
- [V] `monitor.go:33` has a separate hard-coded `exclude` list (headset PIDs) for the hot-plug
  cache only. It is not user-configurable.

---

## 5. Protocol families across the whole catalogue

There are 187 PIDs in `deviceRegisterMap` (plus LCD PIDs 3123/3129/3139/3150 opened indirectly,
21+ dongle sub-device types, and 35 iCUE LINK sub-device types in `database/external/lsh.json`).
I assigned families by fingerprinting each package's command constants [V for the constants;
the grouping is mine, I]:

| # | Family | Signature | Packages → PIDs |
|---|---|---|---|
| 1 | **Bragi cooling hubs** | `[00 08 …]`, `01 03 00 02` sw mode, endpoint open/read/write/close, resources 17/18/1a/1e/20/21/22 | cc, ccxt (384-byte), cduo, lsh/iCUE LINK System Hub (512-byte, plus 35 LINK sub-device types and LINK LCDs) → **5 PIDs** |
| 2 | **Bragi wired peripherals** (kbd/mouse/mousepad/SCUF controller over USB) | same framing, 64/128/256/512/1024-byte reports; colour `0d 01 22` + `06 01`/`07 00`; keepalive `12` | k100, k70 pro/max/core/tkl, k65 pm/mini/plus, k55 core/pro/xt, k60, k95 platinum XT, vanguard 96/99, galleon, clipper, makr75, k100 air, katar pro/xt, m55, m65 ultra, m75, scimitar elite (7139), sabre rgb pro/cs, ironclaw W/SE, darkstar, dark core pro, harpoon wireless, nightsabre, nightsword v2, mm700, SCUF Envision → **71 PIDs / 55 pkgs** |
| 3 | **Bragi headsets (USB)** | Bragi sw mode | virtuoso, virtuoso SE, virtuoso XT, HS80 RGB wireless (USB) → **6 PIDs** |
| 4 | **Bragi wireless dongles** (Slipstream V1/V2, headset/SCUF dongles) | Bragi framing; paired device reached with `[1]=0x09` sub-endpoint (`d.Endpoint`); dongle creates `*W` sub-devices | slipstream (6), slipstreamV2, k65plus dongle, katar pro W, headset dongle (7), HS80 MAX, Virtuoso MAX, VOID V2, SCUF dongles → **26 PIDs** (+21 sub-device types via slipstream) |
| 5 | **Sabre V2 Pro** (newer mouse protocol, not Bragi) | `[1]=0x08` but cmds `07`/`04`, DPI `04 02` | sabrev2proWU, sabrev2prodongle → **6 PIDs** |
| 6 | **Legacy CUE-1 / NXP keyboards and mice** | sw mode `04 02`, hw `04 01`; `07` write / `0e` read; keyboards colour-stream `7f`; mice `22 03 01`, DPI `13 02 00`; key remap `40 1e 00` | k55, k65 rgb/RF, k68, k70 lux/lux rgb/RF/mk2, k95, k95 platinum, strafe/mk2, glaive/pro, m65 pro/elite, scimitar rgb/pro/elite(7051), harpoon rgb pro, ironclaw, nightsword, dark core se (+dongle), mm800, st100 → **28 PIDs / 26 pkgs** |
| 7 | **Commander Pro / Lighting Node** | single-byte cmds: `02` fw, `32` colour, `33` save, `34` start, `35` LED cfg, `37` reset, `38` port state; cpro adds `10/11/12/20/21` fans/temps/volts | cpro, lnpro, lncore, lt100 → **6 PIDs** |
| 8 | **HID AIO "Hydro Platinum-style"** (iCUE H1xxi ELITE RGB, RGB PRO XT, RGB PLATINUM, Corsair ONE) | 64-byte, CRC-8-CCITT checksum (`elite.go:162`), state `ff 00`, colour `04` | elite (12), cone (1) → **13 PIDs** |
| 9 | **Legacy raw-usbfs** (Asetek Hydro H80i/H100i/H115i, H1xxi PLATINUM, Link TM dongle) | `src/usb` ioctls; hydro `10/11/13/20`, platinum `aa/a9/30/31/41/42/61/62` | hydro, platinum, psudongle → **7 PIDs** |
| 10 | **Standalone HID LCDs** | output report `02 05 xx last idx 00 len…` (1024 B), feature report `03 …` | xc7, nautilusLcd (2), nexus → **4 PIDs** (+ 0c39/0c33 Elite LCD caps via cc, + iCUE LINK LCDs via lsh) |
| 11 | **PSU HID** (HXi/RMi) | `fe` init, `02` write / `03` read, PMBus-ish registers | psuhid → **10 PIDs** |
| 12 | **Legacy headsets** (VOID Elite, HS80 RGB USB) | `c8/c9/cb`, 16–32-byte reports | voidelitedongle (2), hs80rgb → **3 PIDs** |
| 13 | **Xeneon Edge monitor** | `02 00/01/02` backlight/contrast/brightness | xeneonedge → **1 PID** |
| 14 | **SMBus DDR memory** (+ motherboard RGB) | i2c | memory, motherboard → 1 (+mobo) |

**Bottom line [I]:** about 110 of 187 PIDs (families 1–4) share **one transport**: Bragi framing
with endpoint/property commands. The rest splits into about 10 small legacy protocols. Three of
Matt's four devices sit on two transports:
- Commander Core and K100 are Bragi (64-byte vs 1024-byte reports; property/endpoint semantics
  are the same).
- LCD cap and Nexus are the "HID LCD" family (report 0x02/0x05 chunks + report 0x03 features).
  Nexus Open already implements the latter.

A Rust design with a `BragiTransport` (report size, endpoint byte, open/read/write/close,
property get/set, mode switch, keepalive) plus per-device "personalities" would cover most of the
catalogue.

---

## Appendix: verification status summary

- **Verified in source:** licence header; discovery flow; Exclude option and its checks;
  udev/service files; all command bytes and offsets quoted above; report sizes; LCD 480×480
  JPEG and 1024-byte chunk header; Nexus 640×48 RGBA; K100 581-byte colour buffer, 138-byte
  key-assignment array, interface-2 event format, 20 s keepalive, uinput injection; pump ≥50 %
  clamps; no keepalive on the CC; hardware mode on Stop; no firmware-write code found.
- **Inferred:**
  - meaning of `03 1e/1d` LCD reports and of header byte [2];
  - keyData ≈ HID usage IDs;
  - firmware watchdog behaviour when software mode is abandoned;
  - polling-rate change causing re-enumeration;
  - 4-byte speed stride meaning;
  - the family grouping (based on fingerprints of constants, not a full read of each package);
  - all legal conclusions.
