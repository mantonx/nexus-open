# Corsair protocol research: liquidctl, OpenRGB, ckb-next, Linux hwmon, crates.io

Scope: every source except OpenLinkHub. I only read source and docs. Nothing was built or run, and no device was opened.
Shallow clones (2026-11-15) are in `.../scratchpad/research/`:

| Repo | Commit | Commit date |
|---|---|---|
| liquidctl (github.com/liquidctl/liquidctl) | 292dc561 | 2026-06-16 |
| OpenRGB (gitlab.com/CalcProgrammer1/OpenRGB) | d09a493f | 2026-11-14 |
| ckb-next (github.com/ckb-next/ckb-next) | e22eaf2f | 2026-10-21 |
| linux (torvalds, sparse: drivers/hwmon, Documentation/hwmon) | aebb7274 | 2026-11-15 |

Labels used below:
- **[V]** I checked this in the source or docs, and the file:line is given.
- **[I]** This is my inference or synthesis. It needs checking on real hardware.

---

## 1. Licences

| Source | SPDX | Evidence |
|---|---|---|
| liquidctl (code and docs) | `GPL-3.0-or-later` | [V] `LICENSE.txt` (GPLv3), `setup.cfg:9 license = GPL-3.0-or-later`, `README.md:789-810`, `docs/developer/process.md:105-117` (each module carries `SPDX-License-Identifier: GPL-3.0-or-later`). `liquidctl/driver/commander_core.py:6-7` says "Copyright ParkerMc and contributors / GPL-3.0-or-later". |
| liquidctl protocol docs (`docs/developer/protocol/*.md`) | no separate licence, so the repo licence applies: GPL-3.0-or-later | [V] I grepped `docs/` for "license", "CC-BY" and "creative commons". The only hit is `process.md`, which is about code. No docs-specific licence exists. [I] The docs therefore fall under the repo-wide GPL-3.0-or-later. |
| OpenRGB | `GPL-2.0-or-later` for almost every file (a few are `GPL-2.0-only`) | [V] `LICENSE` is GPLv2. Controller headers say `GPL-2.0-or-later` (1460 files). 15 files say `GPL-2.0-only`. The Corsair controllers I read are all `GPL-2.0-or-later`, e.g. `CorsairCommanderCoreController.cpp:10` and `CorsairPeripheralV2Controller.h:9`. |
| ckb-next | `GPL-2.0` (README says "version 2") | [V] `LICENSE` is GPLv2. `README.md:7` says "licensed under the GNU General Public License (version 2)". Daemon `.c` files have no per-file SPDX headers. Some bundled libs are BSD-3-Clause or LGPL-2.0-or-later. [I] I would treat it as `GPL-2.0-only` because nothing says "or later". |
| Linux corsair-cpro / corsair-psu | `GPL-2.0-or-later` | [V] `drivers/hwmon/corsair-cpro.c:1`, `corsair-psu.c:1`, `Documentation/hwmon/corsair-cpro.rst:1`. |

### Practical guidance for an MIT project [I] (this is not legal advice)
- **Every source here is copyleft.** Nothing can be copied into an MIT codebase. That covers code, constant tables, struct layouts written as code, and keymap/LED-index tables lifted wholesale.
- **What you can learn from:** the functional facts. Command bytes, packet framing, field offsets, endianness, magic values, timing needs ("re-wake within N s"), quirks. Protocols are generally treated as unprotectable facts or interoperability information. Write your own description and your own implementation, and cite the source ("protocol as documented by liquidctl, docs/developer/protocol/commander_core.md").
- **The liquidctl protocol docs are the cleanest source**, because they are prose and tables of facts rather than code. Their licence is still GPL-3.0-or-later, so don't paste their tables or prose verbatim into the repo. Restate them in your own format and link back.
- **Treat OpenRGB and ckb-next C/C++ as reference material only.** Read them to confirm a fact, then close the file and write independently. Keep provenance notes (which fact, which source) in your protocol docs. Do not port per-device LED/key index tables mechanically. Those large tables (e.g. OpenRGB `CorsairPeripheralV2Devices.cpp`, ckb-next `keymap.c`) are the most "expressive" part and the riskiest to copy. Better to derive them from your own captures, or from the device itself where it reports them.
- Rust crates with permissive licences exist (section 6), but none covers these three devices.

---

## 2. Commander Core family (liquidctl docs and driver, cross-checked with OpenRGB)

### 2.0 PID naming discrepancy (important)
- [V] liquidctl calls **0x0c32 "Commander ST"** (`docs/developer/protocol/commander_core.md:9`, `commander_core.py:64`). It lists 0x0c1c as Commander Core and 0x0c2a as Commander Core XT. The doc says newer Elite Capellix AIOs "may instead come with the Commander ST" (`commander_core.md:13-16`).
- [V] OpenRGB calls 0x0c32 `CORSAIR_COMMANDER_CORE2_PID` and names it "Corsair Commander Core" (`CorsairCommanderCoreController.h:36`, `...Detect.cpp:41`). It also knows 0x0c1d, 0x0c3c, 0x0c3d and 0x0c3e as Commander Core variants (`.h:35-41`).
- [I] Matt's "iCUE Commander Core 0c32" is the device liquidctl calls Commander ST: the newer Capellix-bundled controller, with a pump and 6 fans.

### 2.1 Packet framing [V] `commander_core.md:20-45`
- **Host→device:** report length is **96 bytes on firmware v2.x** and **1024 bytes on firmware v1.x**. Byte 0 = `0x08`, byte 1 = command, byte 2 = "channel" (really a handle or sub-argument), byte 3 onwards = data. Little-endian throughout.
- **Device→host:** byte 0 = `0x00`, byte 1 = echo of the command, byte 2 = `0x00` (status), then data.
- **liquidctl implementation** (`commander_core.py:271-295`): a 97-byte buffer with buf[0] = report ID 0, buf[1] = 0x08. It flushes queued reports, writes, then reads repeatedly until res[0]==0x00. It asserts that `res[1] == command[0]`.
- **Packet size conflict for 0x0c32:**
  - [V] liquidctl uses 96 for every PID (`commander_core.py:20-21`).
  - [V] OpenRGB uses **65 bytes (64 plus the report ID)** for 0x0c32 (`CORSAIR_COMMANDER_CORE_PACKET_SIZE_V3`, `CorsairCommanderCoreController.h:26`, `.cpp:33-37`). It uses 97 for 0x0c1c on v2 firmware, 1025 for 0x0c1c on v1 firmware (`.cpp:95-99`), and 385 for the Core XT (`.h:27`).
  - [I] Don't hard-code the size. Read the report length from the HID report descriptor (sysfs `report_descriptor`, or `HIDIOCGRDESC`).
  - [I] liquidctl's issue #705 ("Commander ST: fan curve still not working", IndexError) fits a size mismatch, though I haven't confirmed that.
- **Interface:** [V] liquidctl accepts only HID interface 0 (`commander_core.py:23,224-230`). [V] OpenRGB matches interface 0, usage page 0xFF42, usage 0x01 (`...Detect.cpp:40-46`).

### 2.2 Commands [V] `commander_core.md:47-210`, `commander_core.py:25-36`
| Bytes after 0x08 | Meaning |
|---|---|
| `01 03 00 02` | **Wake**: enter software mode |
| `01 03 00 01` | **Sleep**: return to hardware mode |
| `02 13` | Get firmware. Response bytes [3..5] = major, minor, patch. The doc's table labels these indices 2-4. liquidctl reads `res[3],res[4],res[5]` including the report-free offset (`commander_core.py:76-77`). [V] OpenRGB uses `res = &buf[3]` and then `[0..2]` (`.cpp:89-92,223`). |
| `0d 00 <mode…>` | Open endpoint. liquidctl sends `0d 00` followed by mode bytes, e.g. `0d 00 17` or `0d 00 60 6d` (`.py:31,261`). |
| `05 01 00` | Close endpoint. The doc writes `05 01 <channel>` (`.md:111-122`). liquidctl sends `05 01 00` (`.py:30`). |
| `06 00 <len16> 00 00 <type16> <data>` | Write. Length = data length + 2 (it includes the data-type bytes) (`.py:321-331`). |
| `07 00 <data>` | Write more (continuation) |
| `08 00 01` / `08 00 02` / `08 00 03` | Read initial / more / final. The initial response carries the data type at [3..4] and data from [5]. Later responses carry data from [3] (`.md:162-199`, `.py:260-269`). liquidctl always does all three reads, then closes, then checks the data type. |

### 2.3 Cross-identification with Bragi [I] (strong)
The Commander Core command set is the ckb-next "Bragi" protocol byte for byte (`ckb-next/src/daemon/bragi_proto.h:4-14`):

| Bragi | Value | Commander Core use |
|---|---|---|
| MAGIC | 0x08 | first byte |
| SET | 0x01 | `08 01 03 00 02` = SET property 0x03 (MODE) to 2 = SOFTWARE (`bragi_proto.h:19,47-48`). liquidctl's "wake/sleep" is a mode switch. |
| GET | 0x02 | `08 02 13` = GET property 0x13 (APP_VER, `bragi_proto.h:21`) |
| CLOSE_HANDLE | 0x05 | `05 01 <handle>` |
| WRITE_DATA | 0x06 | |
| CONTINUE_WRITE | 0x07 | |
| READ_DATA | 0x08 | |
| OPEN_HANDLE | 0x0d | `0d <handle> <resource16>`. liquidctl's "modes/endpoints" are Bragi *resources*, and handle 0 is used. |

- The iCUE LINK System Hub uses the same framing: [V] OpenRGB `CorsairICueLinkProtocol.h:68-78` (`0d 01`, `05 01 01`, `02 13`, `01 03 00 02/01`, `06`, `07`, `08`).
- So **one Bragi transport implementation** (property get/set, handle open/close/read/write with chunking) covers K100, Commander Core/ST/XT and the iCUE LINK hub. Per-device code reduces to resource IDs and payload schemas.

### 2.4 Resources ("modes") and payloads [V] `commander_core.md:210-end`, `.py:38-55`
| Resource | Data type | Content |
|---|---|---|
| `17` | `06 00` | Speeds. [0] = count, then u16 rpm each. On Core/ST, index 0 = pump (AIO/EXT), 1..6 = fans. On XT, fans start at 0. |
| `1a` | `09 00` | Connected fan ports. [0] = count, then per port 0x07 = connected, 0x01 = not. |
| `20` | `0f 00` | LED ports. [0] = count, then per port u16 mode (0x02 = connected, 0x03 = disconnected) and u16 LED count. EXT/AIO first, then RGB ports 1..6. |
| `21` | `10 00` | Temps. [0] = count, then per sensor 1 byte (0x00 = connected, 0x01 = not) plus u16 deci-°C (0x013b = 31.5 °C). Sensor 0 = water temperature. |
| `60 6d` | `03 00` | Hardware speed mode per port: 0x00 = fixed %, 0x02 = curve (the doc calls the list incomplete). |
| `61 6d` | `04 00` | Hardware fixed % per port: [0] = count, then u16 % each. |
| `62 6d` | `05 00` | Hardware curves per port. Each curve: [0] = sensor (0x00 = water, 0x01 = probe), [1] = point count (2..7), then points of u16 deci-°C and u16 %. Each record is 2 + 4·n bytes. liquidctl walks variable-length records (`.py:166-177`). |

- **Fixed speed procedure** (`.py:200-221`): wake. Read 60 6d, set the port bytes to 0x00, write it back. Read 61 6d, patch the u16 % for each port, write it back. Sleep.
  - `_write_data` always does a read first to confirm the data type (`.py:305-306`).
- **Curve procedure** (`.py:147-198`): same pattern using 0x02 and 62 6d. liquidctl always uses sensor 0x00 (water).
  - [V] Bug: the 2–7 point checks build a `ValueError(...)` but never `raise` it (`.py:150-153`).
- **Channels:** `pump` = index 0, `fan1..6`. The XT has no pump and fans are offset down by one (`.py:344-363`).
- **What liquidctl supports** [V] (`docs/corsair-commander-core-guide.md`): status (pump, fan rpm, water and probe temps), fixed duty (since 1.9.0) and curves (since 1.14.0).
  - Every PID is labelled "(broken)" in `_MATCHES` (`.py:60-65`), with a pointer to issues #520, #583, #598, #623 and #705.
  - Issue states via `gh api`: #448 closed (lights flash), #520 closed (incorrect data type), #583 closed (XT incorrect data type), #598 open (XT fan control), #623 closed, #705 open (ST curve IndexError).
- **LEDs: not supported in liquidctl.** `set_color` raises NotSupportedByDriver (`.py:144-145`).

### 2.5 LEDs, from OpenRGB [V] `CorsairCommanderCoreController.cpp`
- **Direct colour:** write resource `22 00` with data type `12 00`. The payload is RGB triplets.
  - Ports after the first are padded to 34 LEDs each: `packet_offset += 3*(34 - leds_count)` (`.cpp:352-380`).
  - Buffer sizes: 699 bytes on Core/ST, 1224 on XT (`.h:29-30`).
  - Writes are rate-limited to 33 ms or more apart (`.cpp:337`).
- **Port configuration** happens at init. Write resource `1e 00` with data type `0d 00`, 15 bytes (`SetFanMode`, `.cpp:386-460`). The payload is `07`, then `01 08` for AIO mode on Core/ST, then `01 06` per port ("6 QL fan mode" to expose 34 LEDs per port). Wake again afterwards.
- **LED count per port:** write resource `1d 00` with type `0c 00`, payload `07 <n>` (`.cpp:462-481`).
- **Keepalive:** every 1 s, OpenRGB checks whether more than 10 s has passed since the last commit. If so, it resends the last colours or `02 13` (`.cpp:142-179`).
  - [I] That implies the device drops back from software mode after a timeout of roughly 10–20 s. The liquidctl doc says only "a predefined number of seconds" (`commander_core.md:53-56`).
- **Shutdown:** the destructor sends hardware mode `01 03 00 01` (`.cpp:58-65`).
- **Firmware reads:** OpenRGB reads only the first packet of a resource and says "It is not currently know how to read more" (`.cpp:302-308`). liquidctl does three reads.
- **Windows:** OpenRGB takes a named mutex **`Global\CorsairLinkReadWriteGuardMutex`** around every transaction (`CorsairController/CorsairDeviceGuard.cpp:15-61`, Windows only). [I] This is the iCUE inter-process lock. A Windows build of Nexus Open should take it too, so it can coexist with iCUE services.

### 2.6 Hazards
- **Mode thrash and flashing.** [V] Every liquidctl operation does wake…sleep (`.py:297-303`). Issue #448 ("lights flash with every update") is caused by this. A maintainer's comment there: waking "puts it in software mode and somewhere in that it turns off the leds that get turned back on when it falls back to hardware mode". Another comment: "There is currently no path to fixing this as liquidctl only supports hardware mode."
  - [I] A daemon should stay in software mode for its whole session, keep it alive, and write hardware-mode profiles (60/61/62 6d) as the fallback.
- **Software-mode timeout.** [I] If a daemon dies while in software mode, the device falls back to hardware mode after the timeout, and the pump follows the stored hardware profile. Make sure a sane hardware curve is stored before relying on software control. Never leave a hardware profile with a 0 % pump.
- **Pump and fan minimums.** [V] The guide's Notes say "The pump and some fans have a limit to how slow they can go and will not stop when set to zero. This is a hardware limitation." liquidctl clamps only to 0–100 (`.py:192,217`).
  - [I] Enforce a floor for the pump channel anyway, e.g. 50 %+. iCUE's presets map to fixed percentages: Quiet 75 %, Balanced 85 %, Extreme 100 % (guide, "In iCUE the pump can be set…").
- **Data-type mismatch.** [V] If another process interleaves on the same handle, liquidctl raises `ExpectationNotMet('device returned incorrect data type')` (#520, #583). [I] That is the symptom of two tools sharing the device, or of a wrong packet size.
- **Firmware variants.** [V] v1.x firmware uses 1024-byte packets and v2.x uses 96 (doc). [V] liquidctl changelog: "support 2.10.219 firmware (PR #501)".
- **Elite Capellix LCD cap 0x0c39.** [V] liquidctl, OpenRGB and ckb-next have no matches for 0c39. OpenRGB's only Corsair LCD handling is iCUE LINK endpoint type 0x06, which it skips (`RGBController_CorsairICueLink.cpp:73-96`). Nothing in these sources documents it.

---

## 3. K100 RGB Optical (1b1c:1b7c): Bragi

Both projects list three PIDs, and both class them as Bragi:

| PID | Model |
|---|---|
| 1b7c | K100 Optical (V1) |
| 1bc5 | K100 Optical V2 ("OPTICAL_VARIANT" in ckb-next) |
| 1b7d | K100 MX Red / mechanical |

[V] ckb-next `usb.h:116-118,313`. [V] OpenRGB `CorsairPeripheralV2Devices.h:89-91`.

### 3.1 Transport
- **ckb-next does not use hidraw.** [V] It opens usbfs. It calls `USBDEVFS_DISCONNECT` on every interface to detach usbhid, then `USBDEVFS_CLAIMINTERFACE` (`usb_linux.c:318-330`). It reconnects the kernel driver on close (`usb_linux.c:253-279`).
  - For the K100 it talks on interrupt **OUT EP 0x01 / IN EP 0x82** (`usb_bragi.c:35-44`). The default for other devices is 0x04/0x84.
  - **Packet size 1024 ("jumbo")** for K100, K65 Mini, K70 TKL and K70 Pro (`usb.h:315-316`, `usb.c:375-376`, `structures.h:244`). Other devices use 64 (or 128 for the Scimitar Elite Bragi).
- **OpenRGB uses hidapi/hidraw.** [V] It matches interface 1, usage page 0xFF42 (`CorsairPeripheralV2ControllerDetect.cpp:108-110`).
  - Report size for the K100 is **65** (64 plus report ID) (`CorsairPeripheralV2Devices.cpp:1438-1440`, device struct field `pkt_size`).
  - Byte layout: buf[1] = `0x08` (wired; `0x09` = wireless via Slipstream), buf[2] = command (`CorsairPeripheralV2Controller.h:27-29,54-62`).
- [I] The two disagree on size (1024 via usbfs versus 64 via hidraw). Possibly the vendor interface's interrupt endpoint has wMaxPacketSize 1024 while the HID report is 64. **For a hidraw design, follow OpenRGB's 64-byte reports**, and verify by reading the report descriptor.
  - ckb-next reads `wMaxPacketSize` from sysfs (`usb_linux.c:146-164`).

### 3.2 Commands [V] ckb-next `bragi_proto.h`, `bragi_common.c`
- **Get property:** `08 02 <prop> 00`. Reply: [2] = status (0 = ok, 0x05 = not supported), value at [3..5] little-endian (`bragi_common.c:6-19`).
  - OpenRGB's error strings: 1 = invalid value, 3 = failed, 5 = unsupported (`CorsairPeripheralV2Controller.cpp:174-187`).
- **Set property:** `08 01 <prop> 00 <u16 LE>` (`bragi_common.c:23-36`).
- **Property IDs** (`bragi_proto.h:17-43`):

| ID | Property |
|---|---|
| 0x01 | poll rate |
| 0x02 | brightness (0–1000) |
| 0x03 | mode |
| 0x07 | angle snap |
| 0x0f | battery level |
| 0x10 | battery status |
| 0x11 | VID |
| 0x12 | PID |
| 0x13 | app firmware version |
| 0x14 | bootloader version |
| 0x15 / 0x16 | radio app / bootloader versions |
| 0x1e / 0x1f | DPI index / mask |
| 0x21 / 0x22 | DPI X / Y |
| 0x2f..0x31 | DPI colours |
| 0x36 | dongle subdevice bitfield |
| 0x41 | hardware layout (1 = ANSI, 2 = ISO, 3 = ABNT, 4 = JIS, per OpenRGB `Devices.h:33-39`) |
| 0x44 | coarse brightness |
| 0x96 | max poll rate (comment: "Untested. Only known because CUE complained about it in its logs") |

- **Mode values:** 1 = HARDWARE, 2 = SOFTWARE (`bragi_proto.h:47-48`). OpenRGB: `buf[3]=0x03, buf[5]=mode` (`.cpp:212-232`).
- **Poll rate values:** 1 = 8 ms, 2 = 4 ms, 3 = 2 ms, 4 = 1 ms (`bragi_proto.h:51-54`). The ckb enum extends to 0.5, 0.25 and 0.125 ms (`structures.h:227-235`), and ckb-next sets `prop = rate+1` (`device_bragi.c:290-296`). [I] The K100's 4000/8000 Hz modes would be values 5/6/7. Unverified.
- **Handles:**
  - Open: `08 0d <handle> <res lo> <res hi>`. A 0x03 reply means the handle is already open, so close it and retry (`bragi_common.c:179-195`).
  - Close: `08 05 01 <handle>` (`:169-177`).
  - Probe size: `08 09 <handle>`, which returns u32 at [5..8] (`:109-122`).
  - Read: `08 08 <handle>`, payload from [3] (`:124-167`).
  - Write: `08 06 <handle> <u32 LE length> <data from byte 7>`. Continuation: `08 07 <handle> <data from 3>` (`:60-102`).
  - Every packet gets a response, so there are no fixed delays (`device_bragi.c:327-331`).
- **OpenRGB's variant** (`CorsairPeripheralV2Controller.cpp:320-460`):
  - Start transaction: `0d <opt> <light_ctrl>`.
  - First block: `06 01 <len16 LE>` with data at offset 8 of the 65-byte buffer, i.e. byte 7 of the report.
  - Continuation: `07` with data at offset 4.
  - Stop: `05 01 <opt>`.
  - OpenRGB says the K100 "drop[s] packets that arrive back to back", so it waits for the response, or 2 ms, between packets (`:416-427,449-454`).
- **Resources** (`bragi_proto.h:60-67`):

| Resource | Meaning |
|---|---|
| 0x01 | lighting, planar RRR…GGG…BBB… |
| 0x10 | monochrome lighting |
| 0x22 | "alt" lighting, RGBRGB triplets |
| 0x2e | extra/secondary lighting |
| 0x05 | pairing ID |
| 0x06 | encryption key |

### 3.3 LED buffer for the K100 [V]
- **ckb-next:**
  - K100: **193 LEDs** (`led_bragi.c:44-46`).
  - It first tries resource 0x01. "Some K100s have been observed to return either 0x01 or 0x06", and in that case it opens **resource 0x22 (alt)** plus handle 2 on resource 0x2e (`device_bragi.c:82-134`).
  - Alt payload: `12 00` header, then R,G,B per LED, with write length `3*n+2` (`led_bragi.c:128-166`).
  - A commented-out K100 secondary-handle packet is noted as having broken the '3' key on fw 0.32.6, bld 0.10.45 (`device_bragi.c:116-132`).
  - Turning everything black also sets brightness to 0 (`led_bragi.c:107-112`).
- **OpenRGB:** the K100 is pinned to `CORSAIR_V2_LIGHT_CTRL2 = 0x22`, because "the K100 answers both endpoints, but its software direct lighting only renders through endpoint 2" (`CorsairPeripheralV2Controller.cpp:110-123`).
  - Payload: `12 00` (CORSAIR_V2_MODE_DIRECT = 0x0012), then RGB triplets (`CorsairPeripheralV2HardwareController.cpp:66-88`, `DATA_OFFSET 2`).
  - Init: SetRenderMode(SW), then `08 02 5F` (light_ctrl_reg 0x5F) (`...HardwareController.cpp:15-19`).
  - Matrix: 12×24, using `corsair_k100_layout` (`Devices.cpp:294-340,1409-1441`).
  - Includes media keys, Profile (128), iCUE (137), Lock (114), Logo L/M/R (190-192), **G1–G6 = LED indices 131–136**, and 44 underglow LEDs from 138.
- **Hardware effects:** OpenRGB enumerates hardware effect IDs (static 0x207E, rainbow 0xB94C, etc., `CorsairPeripheralV2Controller.h:64-78`). Its `UpdateHWMode` writing code is **commented out** (`.cpp:475-509`). Hardware-effect programming is not working there.

### 3.4 Keymap, input, control wheel, G-keys [V] ckb-next
- **Hardware mode:** the keyboard sends standard NKRO HID reports (0x01 keys, 0x02 media).
- **Software mode:** it sends Bragi input on the command endpoint: `xx 02 <bitfield>` for keys and `xx 05 60 <i32 delta>` for the dial (`keymap.c:900-934`). ckb-next ignores NKRO while active (`keymap.c:555-557,927-929`). This is the "software mode = Corsair input" switch.
  - The bitfield index for each key is in ckb-next's bragi keymap (`keymap.c` `keymap_bragi`, `keymap_patch.c:83,153-155`).
  - Examples: g1..g6 = 131–136, profswitch 128, ctrlwheelb 137, ctrlwheel1–8 = 182–189 (dial ring LEDs), "light" 113, "lock" 1 (`keymap.c:445-518`).
  - Notifications arrive as `xx 01 <id>` (`bragi_proto.h:75-81`). 0x36 is a dongle connection status (`bragi_notification.c`).
- **iCUE control wheel:** `DEV_HAS_CTRLWHEEL` includes all K100 PIDs (`usb.h:300`).
  - Rotation is a sign-of-i32 delta, mapped to virtual keys 116 (cw) and 130 (ccw). Key-up is synthesised by clearing the bits on each packet (`keymap.c:901-924`, `input.c:13-15`).
  - The centre button is 137 (ctrlwheelb).
- **Macros and remapping on Linux:** done entirely in the daemon.
  - It creates **uinput** keyboard and mouse virtual devices (`/dev/uinput`, `modprobe uinput` fallback; `input_linux.c:10-120`).
  - The physical device is detached from usbhid. Every key goes through ckb's binding table (`cmd_bind`/`cmd_macro`, `input.c:565-640`) and is re-emitted.
  - The udev rule only tags the ckb virtual inputs as non-joystick (`linux/udev/99-ckb-next-daemon.rules`).
  - The daemon runs as a root system service (`linux/systemd/ckb-next-daemon.service`).
- **Onboard memory and profiles:** **not supported on Bragi** in ckb-next. Every Bragi vtable has `hwload = hwsave = cmd_io_none` (`device_vtable.c:500-501`), and `start_bragi_common` clears `FEAT_HWLOAD` (`device_bragi.c:193-194`). Firmware update is disabled for all Bragi devices (`usb.c:449-450`). OpenRGB has nothing for profiles either.
- **Keepalive:** ckb-next sends a poll every **50 s** (`08 12`, i.e. GET PID), with the comment "CUE polls devices every 52 seconds" (`device_bragi.c:14-36`).
  - OpenRGB's V2 HW controller resends LEDs if nothing has been sent for 30 s, checking every 12.5 s (`CorsairPeripheralV2Controller.h:34-35`, `RGBController_CorsairV2Hardware.cpp:60-71,238-250`). The K55 is cited as reverting after 1 minute.
- **Init sequence (ckb-next):** get mode. If HW, set SW. Read versions, poll rate, max poll rate, brightness support and layout 0x41. **Set HW again.** Then set SW when activated (`device_bragi.c:147-238,250-267`).
- **Release:** OpenRGB's destructor sets HW mode, because some devices "stop generating onboard input events (e.g. the volume knob) while left in software render mode, and the mode persists until the next USB enumeration" (`CorsairPeripheralV2Controller.cpp:139-152`).

---

## 4. Cross-project pitfalls

1. **Mode ownership is global per device.** Software/hardware mode is a single device property. Any tool that sets HW mode kicks every other tool out:
   - liquidctl sleeps after each operation.
   - The OpenRGB destructor sets HW mode.
   - ckb-next `idle` sets HW mode.
   - [I] Two tools active at once will fight: flicker, lost LEDs, and data-type mismatches (liquidctl #520/#583).
2. **Shared handles.** Bragi handles are device-global. ckb-next reopens on reply 0x03 by closing first (`bragi_common.c:185-191`), which would close another tool's handle. [I] Nexus Open must detect other owners and refuse or warn.
   - On Linux, check `/proc/*/fd` for the hidraw node, or check whether usbhid is unbound (which means ckb-next is running).
   - On Windows, use `Global\CorsairLinkReadWriteGuardMutex`.
3. **ckb-next makes hidraw disappear.** [V] usbfs DISCONNECT detaches usbhid from every interface (`usb_linux.c:318-330`), so the K100's hidraw nodes vanish while ckb-next runs. The two cannot coexist.
4. **Kernel hwmon drivers versus hidraw.**
   - [V] corsair-cpro and corsair-psu both call `hid_hw_start(hdev, HID_CONNECT_HIDRAW)` plus `hid_hw_open`, so hidraw stays available (`corsair-cpro.c:609-613`, `corsair-psu.c:791-795`).
   - corsair-cpro's header warns: "The device does not use report ids. When using hidraw and this driver simultaniously, reports could be switched." (`corsair-cpro.c:6-8`).
   - [V] liquidctl's policy is to detect a bound hwmon driver and delegate to it, with a `--direct-access` override (`docs/developer/hwmon.md`, `corsair_hid_psu.py:164-178`: values "are corrupted by the race").
   - [V] There is **no in-tree driver for Commander Core/ST/XT, K100, or iCUE LINK**. `drivers/hwmon` has only corsair-cpro (0x0c10, 1000D) and corsair-psu (`corsair-psu.c:875-889`). I found no out-of-tree Commander Core hwmon module in these sources or in a web search.
   - [V] A liquidctl maintainer said in #448 that a hwmon driver for the Commander Core "could be a solution… keep device in software mode".
   - [I] If such a driver ever lands, it would own software mode, and Nexus Open should read hwmon instead.
5. **udev rules and permissions.**
   - [V] liquidctl ships `extra/linux/71-liquidctl.rules`, with `TAG+="uaccess"` per USB VID:PID, including 0c1c, 0c2a and 0c32. It is generated by `generate-uaccess-udev-rules.py`. The 71 prefix sorts before 73-seat-late so uaccess applies.
   - [V] OpenRGB expects `60-openrgb.rules` (built from its detector list) and warns if it's missing or duplicated in /etc and /usr/lib (`DetectionManager.cpp:72-80,907-910`).
   - [V] ckb-next needs root and ships no hidraw rule.
   - [I] Nexus Open should ship one `7x-nexus-open.rules`. Match `SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="…", TAG+="uaccess"`, scoped to supported PIDs rather than a blanket 1b1c match.
6. **Suspend and resume.**
   - [V] ckb-next waits up to 5 s after resume for the kernel to reattach drivers, then reclaims interfaces (`usb_linux.c:635-720`, a "kernel bug" race).
   - [V] corsair-psu has `.resume`/`.reset_resume` handlers (`corsair-psu.c:901-902`).
   - [I] After resume, devices come back in HW mode, so redo SW mode, reopen handles and resend LEDs. Treat the hidraw fd as possibly stale (ENODEV, then reopen).
7. **Re-enumeration and reconnect.**
   - [V] OpenRGB retries the PID query 8× at 120 ms because "A device that has just enumerated does not answer the PID query straight away" (`CorsairPeripheralV2Controller.h:41-46`, `.cpp:269-289`).
   - [V] ckb-next uses a udev netlink monitor for hotplug (`usb_linux.c:820-872`).
8. **Firmware variance.**
   - [V] K100 lighting resource 0x01 versus 0x22 depends on the unit or firmware (§3.3).
   - [V] Commander Core v1 versus v2 packet size (§2.1).
   - [V] liquidctl's Core 2.10.219 firmware fix.
   - [I] Read firmware versions and branch on them. Log them in bug reports.
9. **K100 drops back-to-back packets.** [V] OpenRGB `.cpp:416-427`. Wait for each response before sending the next packet.

---

## 5. Corsair catalogue by protocol family (as these projects see it)

Rough counts are PIDs or entries in each project, not unique products.

| Family | Framing | Examples | Counts |
|---|---|---|---|
| **Bragi** (2019+ peripherals and newer controllers) | `08 <cmd>` property and handle protocol | K100, K70 Pro/TKL/Core, K65 Mini, K60/K55 Pro, M65 Ultra, M75, Katar, Dark Core RGB Pro, Ironclaw WL, Scimitar Elite Bragi, MM700, Slipstream dongles; [I] Commander Core/ST/XT and iCUE LINK hub | ckb-next 35 Bragi PIDs (`usb.h:313`). OpenRGB CorsairPeripheralV2 ~39 registrations. Plus Commander Core 7 PIDs and iCUE LINK hub 1 in OpenRGB. |
| **NXP / "V1" peripherals** (2015–2019) | 64-byte, `07` write / `0e` read / `7f` stream | K70/K95 RGB, MK.2, Strafe, Glaive, Scimitar, Polaris, ST100 | ckb-next ~49 PIDs (89 total, minus 35 Bragi, minus 5 legacy). OpenRGB CorsairPeripheralController 38 entries (`CorsairPeripheralController.h:23-38`). |
| **Legacy pre-RGB** | HID | K65/K70/K90/K95 legacy, M95 | ckb-next 5 (`usb.h:231`) |
| **Hydro Platinum / Pro XT / Elite RGB AIOs** | own protocol with CRC | H100i–H150i Platinum, Pro XT, Elite RGB | liquidctl 12–13. OpenRGB 13. |
| **Asetek-based Hydro** | USB bulk | H80i GT, H100i GTX/v2, H110i GTX, H115i, Pro series | liquidctl asetek 6 plus asetek_pro 3. OpenRGB Hydro/Hydro2 2. |
| **CoolIT** | — | H110i GT | liquidctl 1 |
| **Commander Pro / Lighting Node** | HID, no report IDs | Commander Pro, 1000D, Lighting Node Pro/Core | liquidctl 4. OpenRGB 7. Kernel corsair-cpro 2. |
| **HID PSUs** | — | HXi/RMi | kernel corsair-psu 15. liquidctl ~12. |
| **SMBus DRAM** | i2c, not USB | Vengeance RGB / Dominator | OpenRGB CorsairDRAM and Vengeance. liquidctl ddr4. |
| **Headsets** | — | Void | OpenRGB 4 |
| **Elite Capellix LCD cap (0c39), iCUE LINK LCDs, Nexus** | — | — | not documented in any of these sources |

**Estimate** [I]: about 4–5 transport families cover most of the current catalogue:
1. Bragi covers peripherals, Commander Core and iCUE LINK. This is clearly the strategic one: one transport plus per-device resource schemas.
2. NXP covers older peripherals.
3. Hydro Platinum covers the 2019–2022 AIOs.
4. Commander Pro / Lighting Node, partly in-kernel.
5. HID PSU, in-kernel.

Plus SMBus DRAM, which needs root/i2c and has a different permission model. Starting with K100 and Commander Core means building Bragi first, which is the right first investment.

---

## 6. Rust crates on crates.io (API query 2026-11-15, q=corsair, 16 hits)

| Crate | Version | Licence | Scope |
|---|---|---|---|
| `corsairmi` | 2.0.1+deprecated | MIT | RMi/HXi PSU. Deprecated in favour of the kernel driver. |
| `liquidrust` | 0.2.2 | MIT | H115i RGB Pro XT (Hydro Platinum family) |
| `corsair-void-cli` | 0.1.3 | BSD-2-Clause | Void headsets |
| `sabre_v2_pro` | 0.1.0 | MIT | Sabre v2 Pro battery |
| `galdeck` | 0.3.1 | MIT | Stream Deck module in the Galleon 100 SD keyboard |
| `cue-sdk-sys` / `cue-sdk` | 0.1.0 / 0.1.1 | MIT | FFI to Corsair's Windows iCUE SDK. Not a device protocol. |
| `rigbat` | 0.5.0 | (MIT OR Apache-2.0) AND OFL-1.1 | battery tray for peripherals |
| `openrgb` / `openrgb2` | — | GPL-2.0 | OpenRGB network SDK clients |

**No crate implements Bragi, Commander Core or the K100.** Searches for "bragi", "commander core" and "k100" found nothing relevant. The `bragi` crate is unrelated (a code generator). Nexus Open would be first, and an MIT `bragi`-style transport crate would be a reusable contribution.

---

## URLs
- https://github.com/liquidctl/liquidctl/blob/main/docs/developer/protocol/commander_core.md
- https://github.com/liquidctl/liquidctl/blob/main/docs/corsair-commander-core-guide.md
- https://github.com/liquidctl/liquidctl/blob/main/liquidctl/driver/commander_core.py
- https://github.com/liquidctl/liquidctl/blob/main/docs/developer/hwmon.md
- https://github.com/liquidctl/liquidctl/issues/448 (flashing; mode explanation), /520, /583, /598, /623, /705
- https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/CorsairCommanderCoreController
- https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/CorsairPeripheralV2Controller
- https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/CorsairController/CorsairDeviceGuard.cpp
- https://github.com/ckb-next/ckb-next/tree/master/src/daemon (bragi_proto.h, bragi_common.c, device_bragi.c, led_bragi.c, keymap.c, usb_linux.c, input_linux.c)
- https://github.com/torvalds/linux/blob/master/drivers/hwmon/corsair-cpro.c ; corsair-psu.c ; Documentation/hwmon/corsair-cpro.rst
- https://crates.io/api/v1/crates?q=corsair
