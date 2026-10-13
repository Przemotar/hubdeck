# MasterHUB: protocol and verification status

Base sources: `ref/cooltek-sdk-probe` (projectzero7, MIT) and `ref/cooltek-sdk` (MIT/Apache). The "Verified here" column says what we confirmed on our own hardware.

## Devices

Each module is a separate USB HID device (the base contains a Prolific USB hub `067B:2586`). Vendor `0x2516`, usage page `0x0C`.

| Module | Model | PID | Serial (our unit) | Firmware here | Latest |
|---|---|---|---|---|---|
| Base 4×6 | M0064 / MHBS01AA46 | `0x01F6` | MHBS01AA461251602207 | 02.00.04.00 | 2.0.4.0 |
| 15 keys IPS 4×3 | M4315 / MHKD15AA43 | `0x01FE` | MHKD15AA431251603100 | 02.00.01.00 | 2.0.1.0 |
| 5 faders 4×2 | M005S / MHFD05AA42 | `0x0200` | MHFD05AA421251701184 | 01.00.05.00 | 1.0.5.0 |
| 2 rollers 4×1 | M032LE / MHRR02AA41 | `0x0204` | MHRR02AA411251602398 | 01.00.03.01 | 1.0.3.1 |
| 3 knobs 2×2 | M003K / MHKB03AA22 | `0x0202` | MHKB03AA221252301845 | 01.00.05.00 | 1.0.5.0 |
| Dial IPS 2×2 | M131E / MHDD01AA22 | `0x0206` | MHDD01AA221252300567 | 01.00.04.00 | unknown |

Latest firmware versions: `https://dev.pms.masterctrl.coolermaster.com/api/devices/<uuid>/firmwares/stable` (UUIDs in `ref/cooltek-sdk-probe/docs/MasterHUB/modules/metadata.md`).

## Reports (HID descriptor read 2026-10-09)

- Input `0x01`: 63 bytes. Layout: `[1]` channel tag, `[2:4]` length (LE), `[4:]` payload = event id, length, values.
- Output `0x02`: 63 bytes (M0), **319 bytes** (keys).
- Feature `0x03` (set) and `0x2D..0x36` (M0) / `0x37..0x3E` (keys): 31 bytes each.

## Commands

| Module | What | Report | Verified here |
|---|---|---|---|
| M0 | firmware | get feature `0x2E` | yes |
| M0 | serial | get feature `0x30` | yes |
| M0 | brightness / LED mode | get feature `0x32` / `0x34` | yes (100, mode 9 = rainbow) |
| Faders | values 0–100 | get feature `0x33` | yes |
| Base | orientation (sensor) | input tag `0x2B`, payload `01` / `00`; sent on change and repeated while the base moves; read on demand: feature `0x35` → `35 00 vv` | yes (2026-10-09, rotating the base: `01` ↔ `00`) |
| M0 | LED color | output `02 2D target idx R G B save` | yes (base, faders; save=0), demo 2026-10-09 |
| M0 | LED mode | output `02 2C target idx mode save` | yes, mode 1 = static; reading `0x34` still shows the saved mode (9) with save=0 |
| M0 | LED brightness | output `02 2E target idx 0..100` | no |
| M0 | LED effects | `02 2C target FF mode 0` after a color `02 2D`, speed `02 30 50` | yes on knobs (2026-10-11, owner watching, violet, speed 50): 2 breathing = smooth fade in and out; 3 strobe = hard on/off; 4 "spring" = fast blinking; 5 cycling = colors jump in order; 6 random = colors jump; 7 wave = smooth color change; 8 "water" = shimmer, every other LED of the ring lit in turn (so a knob ring has several LEDs, addressed only as one); 9 rainbow = smooth color change. The base edge did not change in that test because it was sent index `FF` (see the next row). Base edge, index 0, step by step with the owner (2026-10-11): static red, breathing, wave (whole strip changes color smoothly) all work; 8 "water" runs a pattern along the strip (every second or third LED, with a visible direction) and changes colors meanwhile, so the edge has many LEDs, addressed only as one; index `FF` is ignored by the base |
| M0 | LED addressing limits | the official M0 SDK (`Adjust_LED_Color_CM_M0`) accepts target 0 = base with index 0 only (`FF` rejected), 1 = faders 0-4 or `FF`, 2 = knobs 0-2 or `FF`; the SDK has no other LED command (color, mode, brightness, speed only), so single LEDs inside a knob ring are not addressable through any known command | read from the SDK, 2026-10-11 |
| Faders | event | input tag `0x2A`, `02 05 v1..v5` | yes (441 events in the demo) |
| Rollers | event | input tag `0x2A`, `01 02 r1 r2` (0 up, 1 down, ff none) | yes (39 events in the demo) |
| Knobs | rotation / press | input tag `0x2A`, `00 03 ...` / `03 03 ...` | yes (335 rotations, 26 presses in the layout A test) |
| Keys | software mode | feature `03 3A 01` | yes, 2026-10-09 19:0x (before that the module was in a bad state, see below) |
| Keys | info | get feature `0x3B` → `14 03 05 0f e0 01 10 01 01 48 48` = 3 rows, 5 columns, 15 keys, 480×272, key 72×72 | yes (our decoding) |
| Keys | firmware / serial / brightness | get feature `0x38` / `0x3A` / `0x3C` (software mode only) | yes: 02.00.01.00, serial OK, 100 |
| Keys | backlight | feature `03 34 0..100` | yes: set 20 and 100, read back from `0x3C` (2026-10-10) |
| Keys | per-key image | output `02 36 chunk(LE16) len(LE16) idx last` + JPEG 72×72, in chunks of 312 B | yes: 15 keys in 0.1 s, digits on colored backgrounds, live change on press |
| Keys | screen / key color | feature `03 36 R G B` / `03 37 idx R G B` | sent |
| Keys | presses | input tag `0x34`, `02 0F k1..k15` | yes: 91 events in 60 s, indices 0–14, several keys at once work |

### Keys: full-panel image

`02 35` + JPEG 480×272 in chunks (same header as the per-key image, index 0). Verified: one image across all 15 keys, ~11 KB per frame.

Key positions in the panel image (measured 2026-10-09 with a grid of labeled coordinates and a photo): key `i` shows a 72×72 square starting at (`(i % 5) * 102`, `(i / 5) * 100`), i.e. x = 0, 102, 204, 306, 408 and y = 0, 100, 200. The keys reach the panel edge; the gaps of 30 px horizontally and 28 px vertically are hidden by the frames. More precisely (calibration mode with double frames, the owner, 2026-10-09): key window **79×75 px**, column spacing 100, row spacing 99, start (0, 0); these are the app defaults. The windows are not perfectly even, so pixel-exact alignment on all 15 keys is hard. Speed (Windows, this unit): 15 separate key images (18 KB) 18 ms, one panel image (13.6 KB) 11 ms. USB is not the bottleneck; the app draws the keys as one panel image so they change at the same time.

### Dial (M131E), from disassembly of `CoolTEK_SDK_CM_M131E_V03.02.00.dll` and a hardware test

| What | Report | Verified here |
|---|---|---|
| software mode | feature `03 49 01` (`00` = hardware) | yes |
| brightness | feature `03 44 0..100`; read `0x4C` | yes: set 20 and 100, read back the same (2026-10-10, `examples/backlight.rs`) |
| screen color | feature `03 46 R G B` | no |
| image | output `02 45` + JPEG 240×240 in chunks (header as for the keys) | yes |
| firmware / serial / info | get feature `0x48` / `0x4A` / `0x4B` (`14 00 00 00 f0 00 f0 00 01 f0 f0` = 240×240) | yes |
| rotation | input tag `0x44`, `03 01 00` right, `03 01 01` left | yes |
| press | input tag `0x44`, `04 01 01` pressed, `04 01 00` released | yes |

We read the output report length from the HID descriptor (`_output_report_len`): keys 320 B, M0 modules 64 B. Data chunk = length − 8.

## Picture frame rate (measured 2026-10-10)

`cargo run --release -p mh-device --example framerate -- 4 <quality>` on the owner's hardware:
full key panel images (480x272) and dial images (240x240) both go in at about **32 frames per
second**, with JPEG quality 80 (~7 KB panel, ~4 KB dial) and 60 (~5 KB, ~3 KB) alike, and both
modules together each still at ~32 fps. The limit is on the device side, not the size of the
picture. Whether the screens show every frame was not checked by eye yet.

## Base orientation and module placement (2026-10-09)

- **Orientation**: the base has a sensor. When the base is rotated, report `0x35` and event `0x2B` switch between `01` and `00`. The official app treats `1` as "horizontal" and everything else as "vertical". It tells only two states apart. Confirmed on the owner's hardware: base lying flat horizontally = `01` (horizontal), standing upright = `00` (vertical). 4-position test (2026-10-09 20:25–20:28, rotating 90° to the right each time): start `01`, 90° right `00`, 180° `01`, 270° `01`. It is a one-direction tilt switch, not an accelerometer: it only tells the "90° right" position (upright on a stand) apart from the rest. App: automatic horizontal/vertical from the sensor plus a manual rotation in 4 positions, remembered per base. The earlier "heartbeat every 0.5 s" is the same orientation reports. The switch can stick: on 2026-10-13, after the deck was carried to the Mac, the flat base read vertical until the owner lifted it slightly (mechanical, not software); the manual rotation covers it.
- **Module placement and rotation**: the base contains a tree of USB hubs (Prolific `067B:2586`). A module's port path relative to the base's top hub (e.g. `1-4-3`) identifies the slot, and through it the module's place and rotation. The official app maps it in CSS (`pos` + path, 99 rules for both orientations). Windows: `DEVPKEY_Device_LocationPaths`, e.g. `…#USB(6)#USB(1)#USB(4)#USB(3)`; macOS: `locationID`. On the owner's hardware, 2026-10-09: dial `1-3-4`, knobs `1-4-3`, keys `2-3`, rollers `3-3`, base MCU `1-4-4`.

### Slot table (2026-10-09)

`prototype/tools/gen_slots.py` converts the official app's board layout (88 module + slot combinations) into a cell and rotation in our 4×6 grid. Their board is rotated 180° relative to ours. The knob and roller drawings are off by another 180°. Verified with identify mode (numbers on the keys, an arrow on the dial, LED colors, roller preview) on the owner's hardware: keys@2-3, dial@1-3-4, knobs@1-4-3, rollers@3-3, faders@1-1-3 (faders placed by the table with no corrections). In the knobs module the order is: knob 2 on the left, 1 on the right, 3 below them. App: the user's manual layout > table > guessing.

## Blocked in the driver (`_FORBIDDEN` in `prototype/masterhub/driver.py`)

Firmware update, serial number change, factory data write, recovery tests and factory reset. Firmware is updated only with the official app.

## Hot unplugging (2026-10-09)

Removing a module from the base causes an HID read error (`read error`). `prototype/masterhub/hub.py` closes the handle, scans for devices every second and, when the module comes back, runs `initialize()` and `on_connect` (redraw). Measured: the keys came back 5 s after unplugging.

## Keys and dial firmware lockup (cause confirmed 2026-10-09)

**Reading any feature report from the keys or the dial in hardware mode locks up the module until a power reset.** From then on every feature read returns empty, and the USB strings (product, serial number) are garbage: they contain bytes from the last feature response, e.g. `02.00.01.00` read as UTF-16. Write commands are accepted without error but do nothing. Switching software mode does not fix it; only removing the module from the base and putting it back helps (or restarting the computer).

Test 2026-10-09 ~19:40: after a power reset the keys module responded correctly (`state_check.py`). One `get_feature_report(0x38)` in hardware mode, and the module was locked up. The dial, which we did not touch, kept working. The same had happened earlier with the dial (reading `0x4A` in hardware mode) and with the first run of the probe that day.

Unlocking without removing the module (test 2026-10-09 ~19:55, `prototype/tools/recover_test.py`, Windows 10):

| Method | Result |
|---|---|
| closing and reopening the HID handle + switching software mode | does not help |
| `IOCTL_USB_HUB_CYCLE_PORT` on the hub in the base (without administrator) | rejected, error 31 |
| `pnputil /restart-device "USB\VID_2516&PID_01FE\<serial>"` (administrator) | **unlocks** |
| removing the module from the base and putting it back | unlocks |

macOS (2026-10-13): 200 ms between software mode on and the first read was not always enough when
the module had just been handed back by another app; the read then wedged it. 600 ms and a second
software-mode command before the read: no wedge in twelve restarts.

macOS: to be checked (candidate: `USBDeviceReEnumerate` from IOKit).

Consequences:
- `mh-device` refuses to read from the keys and the dial outside software mode (`Error::HardwareMode`).
- We identify modules by device path, not by USB string. We take the serial number from the report (`0x3A`/`0x4A`) after switching to software mode.
- When a module does not respond after switching to software mode, the hub sends `Event::Unresponsive`. The app offers "Restart module" (Windows: `pnputil /restart-device` via UAC) and, as a fallback, re-seating the module.
- M0 modules (base, faders, knobs, rollers) can always be read.

## Official app (MasterHUB 1.0.0.113)

How to get at the knowledge (all in `ref/official/`, gitignored, do not redistribute):
- Installer: `ref/official/MasterHUB-Setup_1.0.0.113.exe` (signed by Cooler Master, 2026-05-08; link `https://linkto.cm/dl-masterhub`).
- The app is Electron: main process `background.js` and a Vue UI in `app.asar`. **The source maps contain the original `.vue`/`.js` files** (`js/app.c762092d.js.map`, field `sourcesContent`): `components/Baseboard.vue` (layout and slots), `App.vue` (input events), `store/module/*.js` (module logic), `lang/pl.js` etc.
- Unpacking: `python -I prototype/tools/asar.py list|extract <app.asar> <out> [filter]` (a copy of `app.asar` is in `ref/official/installed/`, extracted into `ref/official/extracted/`).
- Module SDK libraries: `ref/official/installed/sdk/CoolTEK_SDK_CM_*.dll`. Command bytes: `python -I prototype/tools/dis.py <dll> <export>` (e.g. `Display_Picture_On_Screen_CM`, `Set_Sw_Mode_CM`).
- Firmware: `GET https://dev.pms.masterctrl.coolermaster.com/api/devices/<uuid>/firmwares/stable` (no login). UUIDs: keys `c9ffd9c7-9679-549a-b2be-a9530de33ebd`, **dial `93e23f53-b0b2-509c-8777-09f51522dab7`** (found in the app code), the rest in `ref/cooltek-sdk-probe/docs/MasterHUB/modules/metadata.md`. Status 2026-10-09: keys 2.0.1.0, dial 1.0.4.0 (the owner's units have the same).
- Slots in the official app: `portNumber` = USB port path (node library `usb`, `portNumbers`) with the base prefix cut off, joined without separators (`143`); CSS classes `.<widget>.pos<path>` in `Baseboard.vue`. Widgets: `control-keypad` (keys), `roller`, `knob3` (knobs), `knobx1` (dial), `control-sliders5` (faders). Our `prototype/tools/gen_slots.py` converts this into `app/src/slots.generated.json`.
- It drew the keys one by one (72×72, JPEG quality 60) and did not use the full panel for the keys. It exported profiles to `.mhz`. Bundled profiles: only Default (plus Photoshop in the resources). It took the base orientation from the base input report (`data[0] == 1` → horizontal) and offered a manual switch.

Copy of the code and SDK: `ref/official/installed/` (`app.asar`, `sdk/CoolTEK_SDK_CM_*.dll`, `worker/*.js`). Main process: `background.js` in `app.asar`.
- On exit, on computer sleep and on settings changes the app calls `Set_Sw_Mode_CM_M4315(false)` and `Set_Sw_Mode_CM_M131E(false)`. Our driver should do the same.
- After `Set_Sw_Mode(true)` the app waits (`200 * i` ms) before reading the firmware.
- `MHDaemon` (a separate process) reads CPU/GPU sensors every second through the CPUID SDK (kernel driver). Likely cause of the computer freeze on 2026-10-09, in conflict with MSI Mystic Light/Afterburner (a hypothesis with no proof, because the logs were lost in the hard reset).
- Modules supported in the SDK: M0 (base, faders, knobs, rollers), M131E (dial), M4315 (keys), M3700, M3704, M4300, M4600, M5000.

## Side note

The keys module returns corrupted USB string descriptors (shifted by one UTF-16 byte), but `hid.enumerate` shows the correct names. Harmless for now.
