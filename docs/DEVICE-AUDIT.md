# Device audit: what we know, what we do not, and how to find out

Written 2026-10-12. The question (owner): "what haven't we discovered, implemented or researched;
what is missing for 100 % knowledge of this device and to get 100 % out of it?"

Sources: the official SDK DLLs in `ref/official/installed/sdk/` (every export disassembled with
`prototype/tools/dis.py` and a small scan of the bytes each export writes into its report buffer),
the official app's code (`App.vue`, `background.js`, store modules), `PROTOCOL.md`,
`crates/mh-protocol`. Nothing was sent to the hardware for this audit.

Legend: **verified** = tried on the owner's hardware and the effect seen; **sent** = sent without
a checked effect; **read only** = known from the code, never sent; **forbidden** = in
`mh-protocol/src/safety.rs`, refused by HubDeck, never to be sent.

## 1. Commands per SDK library

Which library drives which module: the official main process detects modules by USB product id
(`background.js`): `0x01F6` base (M0), `0x01FE` keys (M4315), `0x0200` faders, `0x0202` knobs,
`0x0204` rollers (all three M0), `0x0206` dial (M131E). Never sold, but detected: `0x9989` M3704,
`0x9998` M4300, `0x9993` M4600 ("Touch46"), `0x9992` M5000 ("Touch50"); the app also checks
`0x9991`, `0x9994`-`0x9997`, `0x9999` without naming them.

Report forms: output `02 cc ...` (`hid_write`), feature set `03 cc ...`, feature get = report id
`cc` (`hid_get_feature_report`).

### M0: base, faders, knobs, rollers (`CoolTEK_SDK_CM_M0_V02.05.00.dll`)

| Export | Bytes | Status | What it does / might do |
|---|---|---|---|
| Adjust_LED_Color | `02 2D group idx R G B save` | verified | LED color |
| Adjust_LED_Mode | `02 2C group idx mode save` | verified (modes 1-9) | LED effect |
| Adjust_LED_Speed | `02 30 0..100` | verified | effect speed, whole module |
| Adjust_LED_Brightness | `02 2E group idx 0..100` | **read only** | hardware LED brightness; would dim the rainbow too (our "Lights brightness" can only dim colors now) |
| Get_LED_Brightness / Get_LED_Mode | get `0x32` / `0x34` | verified | |
| Get_Slider_Value | get `0x33` | verified | fader positions on demand |
| Get_Device_Direction | get `0x35` | verified | base tilt switch |
| Get_FW_Infos(_by_type) | get `0x2D`, `0x2E`, `0x2F` | `0x2E` verified | firmware versions by part (application, boot loader?); `0x2D` / `0x2F` never read |
| Get_Serial_Number | get `0x30` | verified | |
| Get_Device_Infos | get `0x31` | **read only** | module info (kind, LED count?); safe to read on M0 |
| Get_Config_Info | get `0x36` | **read only** | stored configuration (saved LED mode/color?); safe to read on M0 |
| Get_Config_Data | `02 32` or `02 33` (one of two flags), answer on the input report | **read only** | asks the module to send its stored configuration as input reports; unknown tags would show it |
| Calibrate_Direction | `02 2F` | **read only, do not send** | calibrates the tilt sensor; likely stored in the module, no known way back |
| Update_FW / Update_FW_by_type | `02 2A` | **forbidden** | firmware update |
| Set_Serial_Number | `02 2B` | **forbidden** | |
| Save_Factory_Data | `02 31` | **forbidden** | |
| Test_FW_Recovery_Process | `02 49` | **forbidden** | |
| Test_Factory_Reset_Process | `02 4A` | **forbidden** | |

### M4315: 15 keys (`CoolTEK_SDK_CM_M4315_V03.02.00.dll`; the `_CM` exports are the same)

| Export | Bytes | Status | What it does / might do |
|---|---|---|---|
| Set_Sw_Mode | `03 3A 0/1` | verified | software mode |
| Display_Picture_On_Screen | `02 35` + JPEG 480x272 | verified | full panel |
| Display_Picture_On_Button | `02 36` + JPEG 72x72, key index | verified | one key |
| Display_Color_On_Screen | `03 36 R G B` | **sent**, effect not checked | fills the whole panel with a color: a cheap blank or flash |
| Display_Color_On_Button | `03 37 idx R G B` | **sent**, effect not checked | one key in a solid color without a JPEG: faster state changes |
| Adjust_Backlight / Get_Backlight | `03 34 0..100` / get `0x3C` | verified | |
| Get_FW_Infos | get `0x37`, `0x38`, `0x39` | `0x38` verified | software mode only |
| Get_Serial_Number | get `0x3A` | verified | software mode only |
| Get_Device_Infos | get `0x3B` | verified (rows, columns, sizes) | software mode only |
| Get_Animation_Infos | get `0x3D` | **read only** | the idle animation stored in the module (frame count, size?); software mode only |
| Get_Config_Infos | get `0x3E` | **read only** | stored configuration; software mode only |
| Get_Config_Data | `02 39` or `02 3A` | **read only** | like M0's: the module sends its configuration |
| Check_Rcv_Data | input tag `0x34` | verified | key states |
| Save_Screen_Picture | `02 38` (+ get `0x3D`) | **forbidden** (our choice) | writes pictures into the module's flash: the idle animation shown in hardware mode. Possible "own idle animation" feature, but it wears the flash and a failed write could leave a broken idle screen |
| Update_FW(_by_type) | `02 34` (+ `02 38`) | **forbidden** | firmware and animation update |
| Set_Serial_Number | `03 35` | **forbidden** | |
| Test_FW_Recovery_Process | `03 39` | **forbidden** | |
| Save_Factory_Data | `03 3C` | **forbidden** | |
| Test_Factory_Reset_Process | `03 3D` | **forbidden** | |
| Test_Animation_Recovery_Process | `03 3E` | **forbidden** | |

Note: the same numbers serve two directions. Feature *get* `0x3D` is "animation info", while
feature *set* `03 3D` is the factory reset. HubDeck must never send a feature *set* with these ids.

### M131E: dial (`CoolTEK_SDK_CM_M131E_V03.02.00.dll`)

| Export | Bytes | Status | What it does / might do |
|---|---|---|---|
| Set_Sw_Mode | `03 49 0/1` | verified | |
| Display_Picture_On_Screen | `02 45` + JPEG 240x240 | verified | |
| Display_Color_On_Screen | `03 46 R G B` | **read only** | fill the round screen with a color |
| Adjust_Backlight / Get_Backlight | `03 44` / get `0x4C` | verified | |
| Get_FW_Infos | get `0x47`, `0x48`, `0x49` | `0x48` verified | software mode only |
| Get_Serial_Number | get `0x4A` | verified | software mode only |
| Get_Device_Infos | get `0x4B` | verified (240x240) | software mode only |
| Get_Animation_Infos | get `0x4D` | **read only** | stored idle animation; software mode only |
| Get_Config_Infos | get `0x4E` | **read only** | software mode only |
| Get_Config_Data | `02 48` or `02 49` | **read only** | configuration as input reports |
| Check_Rcv_Data | input tag `0x44` | verified | turn `03 01 dir`, press `04 01 0/1` |
| Save_Screen_Picture | `02 47` | **forbidden** (our choice) | flash write of the idle picture / animation |
| Update_FW(_by_type) | `02 44` (+ `02 47`) | **forbidden** | |
| Set_Serial_Number `03 45`, Test_FW_Recovery `03 48`, Save_Factory_Data `03 4B`, Test_Factory_Reset `03 4C`, Test_Animation_Recovery `03 4D` | | **forbidden** | |

### Modules never sold

| Library | Commands found | What the module was (from the official app) |
|---|---|---|
| M3704 (`0x9989`) | picture on screen `02 09`, on a button `02 0A`, logo `02 0B` / show `03 0C`, colors `03 0A` / `03 0B`, backlight `03 08`; gets `0x0B`-`0x0F`; forbidden kinds: FW `02 08`, serial `03 09`, recovery `03 0D` | a key module: the app's key store draws on it like on M4315 (`Display_Picture_On_Button_CM_M3704`) |
| M3700 | screen `02 08`, logo `02 09` / `03 0A`, color `03 09`, backlight `03 07`; gets `0x0A`-`0x0E` | a screen without buttons (no button commands) |
| M4300 (`0x9998`) | (optimised build, read loosely) backlight `03 54`, button color `03 57`, screen color `03 56`, picture `02 55` / button `02 56`, logo `02 57` / `03 58`, gets `0x5B`, `0x5F` | an older key panel; it also checks for BMP pictures |
| M4600 "Touch46" (`0x9993`) | screen `02 26`, logo `02 27` / `03 28`, color `03 27`, backlight `03 25`, IO status (input); gets `0x28`-`0x2C` | a round touch screen with a color picker in the app's UI (title, color) |
| M5000 "Touch50" (`0x9992`) | screen `02 07`, logo `02 09` / `03 0A`, color `03 08`, backlight `03 06`; gets `0x09`-`0x0D` | a second touch screen, same UI as Touch46 |
| `CoolTEK_SDK_V01.00.00` | buttons and screen `02 22`-`02 24`, `03 23`-`03 25`; gets `0x24`-`0x28` | the first, generic key panel SDK |

To support any of them, HubDeck would need the module, its PID and the input report format
(touch coordinates for Touch46/50). Without hardware: not worth it.

## 2. Input we do not decode

| Module | Known | Not known / not used |
|---|---|---|
| Keys | tag `0x34`, `02 0F` + 15 states (press and release of every key) | any other event id; long-press timing is ours (gestures), the module has none |
| Dial | tag `0x44`: turn `03`, press `04` | event ids `00` / `01` are the "CES" prototype's (the app accepts both); long press is app-side (600 ms) |
| Knobs | tag `0x2A`: turn `00`, press `03` | press-and-turn arrives as two separate events (we could offer a "turn while pressed" gesture); acceleration is ours |
| Rollers | tag `0x2A`: `01 02 r1 r2` | whether a roller can be pressed (no event seen; to probe) |
| Faders | tag `0x2A`: `02 05 v1..v5` | none known |
| Base | tag `0x2B`: tilt switch | the "Get_Config_Data" answers (unknown tags) |
| All | unknown tags are dropped as `Input::Unknown` | they are not logged; a debug log line would catch anything new during normal use |

## 3. In the official app, not matched by HubDeck

- **Idle animation in the module's flash** (`Save_Screen_Picture`, animation updates): the picture
  the keys and dial show when no app runs. Possible "set your own idle animation", but it writes
  flash and is in the forbidden list by our choice (owner decision needed before any work).
- **Several bases**: the app keeps profiles per base serial and a name per base. HubDeck: one base
  (the owner: "we do not build for two bases").
- **Base name, module names**: names stored by the app only (no device command). Cosmetic.
- **Orientation calibration** (`Calibrate_Direction`): never used in the app's UI as far as the
  code shows; we do not need it (the user's turn setting covers it).
- **"Hub2" sensors** (temperature, humidity, light, CO2, case sensors): fields left in the store
  from another Cooler Master product; no MasterHUB module reports them.
- **Firmware updates**: out of scope (rule).

## 4. Safe hardware probe plan

Rules: the owner watches; one state per message ("look at key 1 now"); never a feature *read*
from the keys or the dial in hardware mode (switch software mode first and confirm with `0x3B` /
`0x4B`); nothing from the forbidden list; nothing that writes flash or calibration. HubDeck closed
(it owns the deck) and the Python tools used.

Ranked by value:

1. **Hardware LED brightness** `02 2E group FF 30` on the knobs while in rainbow, then `100`. If the
   rainbow dims, "Lights brightness" can dim every style, rainbow included.
2. **Solid key color** `03 37 0 FF 00 00` (software mode): key 1 turns red without a JPEG? Then
   `03 36 00 00 FF`: the whole panel blue? Faster state colors and an instant blank.
3. **Dial fill** `03 46 00 80 FF` (software mode): the round screen turns blue?
4. **Roller press**: run `listen.py`, press each roller wheel down: any event?
5. **M0 reads** (always safe on M0): get `0x31`, `0x36`, `0x2D`, `0x2F` on the base, knobs, faders
   and rollers; write the bytes into PROTOCOL.md.
6. **Config data**: `02 32` then `02 33` to the base with `listen.py` running: which input tags come
   back?
7. **Keys and dial info** (software mode only, after `0x3B` / `0x4B` answered): get `0x3D`, `0x3E`,
   `0x4D`, `0x4E`; record the bytes.
8. **Mixed knob effects**: knob 1 breathing, knob 2 static, knob 3 rainbow (per-index `02 2C`): do
   they really differ per knob? (HubDeck already sends this when the user mixes effects.)
9. **Fader LED effects** per fader, as 8.

Not to probe: `02 2F` (sensor calibration), `02 38` / `02 47` (flash pictures), anything in the
forbidden list.
