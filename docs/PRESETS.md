# Presets: what people use the MasterHUB for

A preset is a ready-made profile for one app or workflow that adapts to the modules attached
(keys, faders, knobs, rollers, dial). This catalog drives which presets and plugins we build first.

## What we learned (October 2026)

- **Cooler Master marketed** streaming (OBS, Streamlabs, Twitch), Adobe (Photoshop, Illustrator,
  Premiere Pro), gaming ("game settings without menus") and video editing ("scrub timelines").
  Kits for photo and video editing were announced at Computex 2023 but never shipped.
- **The official app shipped two templates** ("Default" and "Photoshop"), built-in actions per app
  (OBS, Streamlabs, Twitch, Discord, Adobe shortcuts, system actions; Voicemod appears only as UI
  text with no actions behind it), and profile import/export to a `.mhz` file. A promised
  marketplace never appeared. Full inventory: `docs/OFFICIAL-APP.md` (corrected 2026-10-11).
- **Reviewers built**: audio mix on faders, OBS scenes and screenshots on keys, volume on the dial,
  page scrolling and undo/redo on rollers and knobs, Steam/Discord launchers, CPU/GPU temperatures on
  the dial, game action bars (RPGs; Star Citizen, Flight Simulator and Helldivers 2 ideas).
- **Top complaints**: faders could only control 4 things (hub brightness, system volume, OBS and
  Streamlabs mixers); hotkeys had to be recorded from a physical keyboard (no F13–F24); LEDs only per
  module; the dial screen showed only clock, CPU/GPU or a picture; no presets, no marketplace;
  settings lost on startup; the app stopped being updated.
- **Competitors' pattern** (Stream Deck, Logitech MX Creative Console, Monogram, Razer, TourBox):
  ready-made profiles for Adobe, DaVinci Resolve, Lightroom and OBS; profiles switch automatically to
  the app in front, with a universal fallback; profiles are shared as files or through a marketplace.
  Paid profiles sell well for DaVinci Resolve and for flight-sim autopilot panels.

## What Stream Deck owners install (Elgato Marketplace, 2026-10-13)

Measured, not guessed: the marketplace's public product list sorted by downloads
(`mp-gateway.elgato.com/products?extension=plugins&ordering=-download_count`; 1530 plugins, 995
profiles). By theme, in the order the downloads put them:

1. **Volume per app and audio** (the most downloaded plugin of all): Volume Controller (1), Audio
   Switcher (10), Audio Mute (15), Discord Volume Mixer (22), Audio Mixer (35), VoiceMeeter (47), Wave
   Link, GoXLR. The MasterHUB's knobs and faders do this natively: our strongest first impression.
2. **Streaming**: OBS Studio (2), Twitch (7), YouTube (8), OBS Tools, Twitch Tools, Streamlabs (18,
   30), TikTok LIVE Studio, stream counters and timers, VTube Studio, Camera Hub.
3. **Voice and calls**: Discord (3), Voicemod (4), Zoom; profiles: Discord (2), Zoom (4), Teams (6).
4. **Music**: Spotify Integration (5), Spotify (9), Essentials for Spotify, YouTube Music, VLC.
5. **Windows tools and app shortcuts**: Control Center (6), Win Tools, SuperMacro, window movers,
   Advanced Launcher; profiles: Windows 11 Essentials (3), Excel, Word, Chrome, Mac shortcuts,
   PowerPoint, VS Code.
6. **Live information**: Clocks (13), Weather (17), Speed Test, System Vitals, GPU Metrics, Battery,
   World Time.
7. **Games**: the most downloaded profile is Helldivers 2 stratagems; ShadowPlay and Xbox Game Bar
   (clips), Star Citizen, World of Warcraft, Cyberpunk 2077, Arc Raiders profiles.
8. **Smart home and lights**: Home Assistant (28), Philips Hue (36), Govee, iCUE.
9. **Fun**: soundboards (Sound Deck, Soundpad, random sounds), an emoji keyboard, Tamagotchi
   profiles. AI prompt profiles exist but are niche (rank ~30 among profiles).

What this means for the assistant's tour (built for what users usually do, not for one person's
setup): the stops follow this order, each one shown only when it fits the
person (a streaming stop only with OBS, Streamlabs or a streaming app installed; games only with
games found; Claude Code agents only with Claude Code), filled with their own apps.

## How presets work in masterhub

1. **Built in, free, editable**: every preset is an ordinary profile the user can change.
2. **Layout-aware**: a preset declares slots per module kind (keys, faders, knobs, rollers, dial)
   and fills whatever is attached; nothing breaks when a module is missing.
3. **Follows the app in front**: each preset lists the apps it belongs to; a universal profile is the
   fallback.
4. **Shareable**: export/import as one file; a community gallery (a GitHub repo first) later.
5. **Backed by plugins**: a preset only arranges actions; the actions come from plugins, so a new
   integration makes new presets possible.

## Catalog (priority order)

| # | Preset | Keys | Faders | Knobs / dial | Rollers | Needs |
|---|---|---|---|---|---|---|
| 1 | **Everyday desk** (universal) | app launcher, media, mic mute, screenshot, lock | per-app volume (browser, chat, music, game, system) | dial: system volume + now playing; knobs: monitor brightness, music volume | scroll vertical / horizontal | app audio sessions, media keys, DDC/CI |
| 2 | **OBS streaming** | scenes, record, stream, replay save, source toggles | OBS audio mixer | dial: mic gain with meter | scroll chat / scenes | obs-websocket |
| 3 | **Discord & meetings** | mute, deafen, push-to-talk, camera, leave | per-user / channel volume | dial: output volume | — | Discord RPC, Teams/Zoom hotkeys |
| 4 | **Photoshop** | tools, layers, blend modes | opacity, flow, hardness | dial: brush size; knobs: zoom, rotate, history | undo / redo, zoom | hotkeys (Adobe UXP plugin later) |
| 5 | **Lightroom** | flag, reject, rating, before/after, next photo | exposure, contrast, highlights, shadows, whites | knobs: temp, tint, vibrance; dial: selected slider | next / previous photo | Lightroom plugin (MIDI2LR-style) |
| 6 | **Video editing** (built as one panel per editor: Premiere Pro, DaVinci Resolve, VEGAS Pro, CapCut, Shotcut) | cut, ripple delete, markers, in/out, J K L | app volumes | dial: shuttle with acceleration, press = play; knobs: jump, edit points | frame by frame, timeline zoom | hotkeys; Resolve scripting later |
| 7 | **Music & DAW** | transport, record arm, mute/solo | channel faders | knobs: pan, sends; dial: master | scroll arrangement | MIDI output (any DAW) |
| 8 | **Flight & space sims** | autopilot modes, gear, flaps, lights | throttle, mixture, prop (absolute faders) | dial: heading / altitude / baro; knobs: speed, VS, course | trim | built as "Sim panel" through vJoy (any sim binds it); SimConnect (MSFS) later |
| 9 | **Game action bar** | ability / stratagem macros with icons | — | knobs: zoom, camera | weapon / item cycle | macros with key sequences and timing |
| 10 | **System monitor** | — | — | dial: CPU/GPU load and temps; full-panel graph on the keys | — | sensor source that needs no kernel driver |
| 11 | **Smart home** | scenes, devices | light brightness | knobs: color temperature | — | Home Assistant |
| 12 | **Developer & AI agents** | agent sessions waiting, approve/deny, run tests, build, git status | — | dial: details of the selected item | scroll logs | Claude Code hooks, local API, MCP |

## Fixes for the official app's complaints (built into the core, not presets)

- Faders and knobs can drive **any** numeric target: app volume, MIDI CC, plugin parameters.
- Hotkeys are picked from a list (including F13–F24 and media keys), recording optional.
- LEDs per control, and LED color can follow state (mute = red-orange, value = gradient).
- Dial and full-panel screens are drawn by plugins: any text, gauge, graph, image or animation.
- Settings are saved atomically and never lost on startup; profiles export to a readable file.

## Video editing panels (built 2026-10-12)

One preset per editor (the old "Video editing" preset mixed Premiere and Resolve keys); each
switches in with its editor and is what "Profile for an app" picks for it. Same layout everywhere:

- **Dial = shuttle**: turning sends the shuttle keys (L forward, J back; each press plays faster)
  with turn acceleration, so a fast spin plays fast and a slow detent plays at normal speed; the
  press plays / pauses. CapCut has no documented J/K/L, so its dial jumps (Shift+arrows) instead.
- **Roller 1** = frame by frame, **roller 2** = timeline zoom, **knob 1** = a bigger jump,
  **knob 2** = previous / next edit point, knob 3 = system volume.
- **Keys**: four edit tools (per editor), undo (hold = redo), mark in / out, marker, previous /
  next edit, reverse, stop, play, forward, save (hold = export where a default exists).
- Faders: system, the editor, Chrome, Spotify, Discord volumes.

Default Windows shortcuts, from the editors' own documentation:

| Editor (exe) | Tools (keys 1-4) | Play | Shuttle | Frame | Jump | Zoom out / in | Edit points | Export | Source |
|---|---|---|---|---|---|---|---|---|---|
| Premiere Pro (`Adobe Premiere Pro.exe`) | V select, C razor, Ctrl+K add edit, Shift+Delete ripple delete | Space | J K L | Left/Right | Shift+Left/Right (5 frames) | - / = | Up/Down | Ctrl+M | helpx.adobe.com/premiere-pro/using/default-keyboard-shortcuts.html |
| DaVinci Resolve (`Resolve.exe`) | A select, B blade, Ctrl+B split (Timeline > Razor), Delete ripple delete (Backspace leaves a gap) | Space | J K L | Left/Right | Shift+Left/Right (1 s) | Ctrl+- / Ctrl+= | Up/Down | none (Deliver page) | the manual installed with Resolve 20 (`Documents\DaVinci Resolve.pdf`; Mac "Forward Delete" = Windows Delete) |
| VEGAS Pro (`vegas2x0.exe`) | Ctrl+D edit tool, Ctrl+L auto ripple (no ripple-delete key: with it on, Delete ripples), S split, Delete | Enter (pause; Space stops and returns) | J K L | Alt+Left/Right | Page Up/Down (grid marks) | Down / Up | Ctrl+Alt+Left/Right | none by default (Ctrl+M renders to a new track) | VEGAS Pro 2026 shortcut list, cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/13-appendix/shortcutkeys.htm |
| CapCut (`CapCut.exe`) | A select, B split mode, Ctrl+B split, Delete | Space | none (the dial jumps) | Left/Right | Shift+Left/Right | Ctrl+- / Ctrl+= | Up/Down | Ctrl+E | official help pages give only I/O and Ctrl+B; the rest from third-party lists |
| Shotcut (`shotcut.exe`) | S split, Shift+S split all tracks, X ripple delete, Z lift | Space | J K L | Left/Right | Page Up/Down (1 s) | - / = | Alt+Left/Right | Ctrl+E | shotcut.org/howtos/keyboard-shortcuts |

Mark in / out are I / O and the marker is M in all five; undo Ctrl+Z, redo Ctrl+Shift+Z, save
Ctrl+S.

**To check** (not in an official list): Premiere's V / C tools (long-standing defaults, but not in
the table read); VEGAS executable names (`vegas230.exe`, `vegas220.exe`, `vegas210.exe`,
`vegas200.exe` follow the install folders' version numbers) and whether a newer VEGAS keeps the
2026 list; **all CapCut keys except I, O and Ctrl+B** (CapCut > Menu > Keyboard shortcuts). Where an
editor differs, change the key in the inspector.

## Sim panel (built 2026-10-12)

The deck as one virtual joystick through vJoy (Windows; the user installs vJoy, HubDeck never
bundles a driver). Keys 1-15 hold buttons 1-15; faders set the throttle (Z), mixture (Rx), prop
(Ry), flaps (Slider) and trim (Dial) axes; the dial fine-tunes the same trim axis; knobs move X, Y
and Rz; the rollers tap buttons 16/17 and 18/19. The sim binds them in its own control settings,
so it works with MSFS, DCS, X-Plane, iRacing, Star Citizen and anything that reads a joystick.
Switches in with `FlightSimulator.exe`, `FlightSimulator2024.exe`, `DCS.exe`, `X-Plane.exe`,
`iRacingSim64DX11.exe`, `StarCitizen.exe`. Setup and binding per sim: `docs/SIM-PANEL.md`.

## Game presets (built 2026-10-12)

Each game preset puts the game's **default PC key bindings** on the 15 keys, sent in the hotkey's
"For games" mode (hardware scan codes held 30 ms, so games that read the key state see them), with
game icons from game-icons.net and the glass key style. It switches in by itself while the game is
in front (its executable is in the profile's apps). The dial, knobs and faders carry the same
volumes in every game preset: dial = the game; knobs = Discord, music (Spotify), system; faders =
system, game, Discord, music, browser; roller 1 = mouse wheel. Keys a game reads only while held
(push-to-talk, walk, crouch) are left out: a key press is a tap.

Bindings come from community references (no publisher publishes a default key list); the ones
marked "check" disagree between sources, so check them in the game's key-binding screen (or reset
the game's bindings to defaults):

| Preset | Apps (auto-switch) | Keys (default bindings) | Check | Source |
|---|---|---|---|---|
| Diablo IV | `Diablo IV.exe` | skills 1-4, potion Q, evade Space, mount Z, town portal T, emote E, map Tab, inventory I, skills A, character C, journal J, world map M | map Tab/M, skills A | progametalk.com/diablo-4/pc-controller-key-bindings |
| Baldur's Gate 3 | `bg3.exe`, `bg3_dx11.exe` | end turn Space, turn-based Shift+Space, shove V, throw X, jump Z, sneak C, highlight Alt, inventory I, character N, spellbook K, journal J, map M, alchemy H, quicksave F5, quickload F8 | | bg3.wiki/wiki/Controls |
| Counter-Strike 2 | `cs2.exe` | slots 1-5 (primary, pistol, knife, grenades, bomb), reload R, inspect F, drop G, use E, buy B, scores Tab, radio Z, team chat U, all chat Y, spray T | | bisecthosting.com CS2 controls; developer.valvesoftware.com/wiki/VCFG |
| Valorant | `VALORANT-Win64-Shipping.exe` | abilities C Q E, ultimate X, slots 1-4 (primary, pistol, knife, spike), reload R, use F, ping Z, map M, scores Tab, chat Enter, jump Space | chat Enter | shacknews.com Valorant PC controls |
| Fortnite | `FortniteClient-Win64-Shipping.exe` | wall F1, floor F2, stairs F3, roof F4, trap F5, edit G, slots 1-5, map M, inventory Tab, emote B, repair H | building keys (newer installs may use Z/X/C/V) | keymap.io/controls/fortnite; onlinegamecommands.com |
| Minecraft | `javaw.exe: Minecraft` (Java, by window title), `Minecraft.Windows.exe` | hotbar 1-4, inventory E, drop Q, swap hands F, chat T, players Tab, advancements L, camera F5, screenshot F2, debug F3, fullscreen F11, jump Space | | minecraft.wiki/w/Controls |
| League of Legends | `League of Legends.exe` | Q W E R, summoners D F, items 1-3, trinket 4, recall B, shop P, scores Tab, camera lock Y, center Space | recall, shop, scores, center | wiki.leagueoflegends.com/en-us/Hotkeys |

A player's own bindings: change the key in the inspector (Record), or start from the preset and
adjust; `docs/GAMES-SETUP.md` walks through it.
