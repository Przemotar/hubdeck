# The official MasterHUB app vs HubDeck

What the official Cooler Master MasterHUB software offers, what Cooler Master promised and never
delivered, and for each item whether HubDeck has it. Written 2026-10-11.

Sources:

- The official app, version 1.0.0.113 (installer of 2026-05-27), read as data in `ref/official/`
  (gitignored): the UI source recovered from its source map (Vue components, the action catalog in
  its store, locale files), the main process bundle and the background daemon. Nothing from it is
  copied here; features are described in our own words.
- The web: the Kickstarter campaign, Cooler Master's pages, press reviews (links in the sections).
- HubDeck: `README.md`, `STATE.md`, `docs/ROADMAP.md`, `docs/PAIN-POINTS.md`, `docs/PRESETS.md`,
  `app/src-tauri/src/engine/actions.rs`.

HubDeck column: **has**, **partial** or **missing**, with one line on why. "Built" items marked
`[~]` in the roadmap (waiting for the owner's test) count as "has".

## What the official app does

The official app is an Electron + Vue app with a second background process ("daemon") for CPU and
GPU sensors. Its action catalog has 11 groups and about 170 actions; most of them are fixed hotkeys
for Adobe apps. Each action declares which module kinds it works on (keys, dial, knobs, rollers,
faders).

### Actions on keys (and where else they work)

| Feature | Official app | HubDeck |
|---|---|---|
| Multi-action | "Multi-task": a list of actions with a delay step between them; keys only | **has**: Multi-action with delays, plus Toggle and "only if" conditions |
| Folders and pages | New folder, create page, previous / next page (keys only) | **has**: pages per profile, "Go to page", page bar |
| Switch profile | Next / previous profile on keys, the dial and knobs | **has** (2026-10-11): a named, the next or the previous profile; turning steps through them |
| Deck brightness | Brighter / darker on keys, faders, dial, knobs, rollers | **has** (2026-10-11): "Deck brightness" on keys, knobs, the dial and faders (key panel and dial backlight) |
| Lighting effect | A key that sets an effect for the whole kit, or goes to the next / previous effect | **has** (2026-10-11): "Lighting effect" on the knobs, faders, base or all, until the profile changes; no next / previous |
| Timer | Countdown on a key or the dial with a sound at the end; hold 2 s to start or stop; up to ~100 h | **has** (2026-10-11): Timer action, press starts / pauses, turning sets minutes, sound at the end |
| Clock | On a key: date or analog face; on the dial as a display mode | **has** (2026-10-11): Clock action on a key or the dial (time, date or both; no analog face), plus the full-panel clock |
| Undo / redo, switch virtual desktop, screen capture, zoom (browser and app), scroll page up / down | Fixed shortcuts, also on knobs, rollers and the dial | **has**: Hotkey per direction; Scroll sends real wheel steps, also horizontal |
| Media track control | Play, pause, play/pause, stop, next, previous | **has**: Media control, Now playing with cover art |
| Media fast forward / back | Picks the right shortcut for 10 named players (VLC, Windows Media Player, KMPlayer, PotPlayer, MPC-BE, GOM, DivX, WinDVD, PowerDVD) | **has** (2026-10-11): "Seek in a player" with 12 players' shortcuts (VLC, Windows Media Player old and new, MPC, PotPlayer, KMPlayer, GOM, YouTube, Spotify, Netflix, arrows) |
| Volume | System or one app (picked from running apps): up, down, mute | **has**: system, one app, app in front, microphone; Windows popup option |
| Play audio file | Plays a chosen sound file: play/pause or play/stop (a small soundboard) | **has** (2026-10-11): Play sound (WAV, MP3, FLAC, OGG), plays / stops, starts over or overlaps, volume, output device, "stop all sounds"; no pause |
| Open URL / app / file | Yes | **has**, plus the app's own icon and run scripts with arguments |
| Type text | Yes, optional Enter after the text | **has** (Unicode, any layout; multiline) |
| Hotkey | Recorded from a physical keyboard only | **has**: recorded or picked from a list, F13-F24, media keys, "for games" scan codes |
| Recorded macro | Records a key sequence with its timing into an editable timeline | **partial** (2026-10-11): the recorder is built in the engine (`macro_record_start` / `macro_record_stop` give Hotkey steps with the pauses); the button in the editor is still to come |
| Copy / paste a control | Yes | **has**, plus undo |

### Dial, knobs, rollers, faders

| Feature | Official app | HubDeck |
|---|---|---|
| Several functions on the dial | "Function array": a list of functions, a press goes to the next, sortable | **has**: Modes (also on knobs) |
| Several functions on a knob | A function list per knob (the "dial function array" item for knobs is hidden in the catalog) | **has**: Modes on a knob |
| Turn left / right | Each direction picks an option or its own hotkey | **has**, plus turn acceleration and merged fast turns |
| Knobs and rollers | Same actions as the dial except timer and clock | **has** |
| Faders | Deck brightness, volume (system or app), OBS mixer, Streamlabs mixer, Discord user / input / output volume | **partial**: volume, microphone, OBS source volume, monitor brightness, Home Assistant lights, MIDI CC; deck brightness, Streamlabs mixer volume; no Discord |
| Fader calibration | None | **has**: ends snap, jitter filtered |
| Push-to-talk / push-to-mute | On OBS and Streamlabs mixer sources and Discord | **has** (2026-10-11) for the microphone and OBS sources: a "Press" option on the mute actions |

### Dial screen

| Feature | Official app | HubDeck |
|---|---|---|
| Name of the current function | Default mode | **has** (mode name, value for 1.5 s after a change) |
| Picture | JPG / PNG / GIF up to 800 KB | **has**: PNG, JPG, GIF, WebP, SVG, animated up to 15 fps |
| CPU and GPU load and temperature | Four modes, data from the daemon (CPUID library with a kernel driver) | **has** (2026-10-11): CPU load, memory, NVIDIA GPU through NVML; CPU temperature and AMD / Intel GPUs from LibreHardwareMonitor's WMI when the user runs it (a dash otherwise); no driver shipped |
| Clock | A display mode | **has** (Clock action) |
| Timer | Counts down on the dial | **has** (Timer action, with a ring of the time left) |
| Now playing | None | **has** (cover, title, artist) |

### Integrations

| Feature | Official app | HubDeck |
|---|---|---|
| OBS Studio (obs-websocket, IP / port / password) | 17 actions: record start/stop, record pause, stream, virtual camera, replay buffer on/off and save, scene collection, scene, source visibility, OBS profile, audio mixer (set, adjust, mute, push-to-talk, range), media source (play/pause/stop/restart, elapsed or remaining time on the key), studio mode, transition, filter on/off, screenshot, transition duration | **partial**: scene, record and pause, stream, replay buffer on/off and save, virtual camera, studio mode, transition, scene collection, OBS profile, filter on/off, screenshot, mute, source volume, show/hide source, with live state (added 2026-10-11); missing media source control and push-to-talk |
| Streamlabs Desktop (remote control API, IP / port / token) | 7 actions: record, stream, scene, source visibility, audio mixer, studio mode, transition | **has** (2026-10-11, not yet tried with Streamlabs): those plus save replay and replay buffer on/off, with live state |
| Twitch (OAuth sign-in) | 12 actions: chat message, clear chat, followers-only / sub-only / emote-only / slow chat, Shield Mode, run an ad, stream title and game, stream marker, create clip, open last clip | **has** (2026-10-11): the same 12 (chat modes as one action); sign-in waits for the HubDeck Twitch app id |
| Discord (RPC with Cooler Master's approved app) | 9 actions: mute, deafen, join voice channel, open text channel, set input / output device, voice activity or push-to-talk, per-user volume, input and output volume | **missing**: Discord does not approve RPC voice scopes for open-source apps; HubDeck uses Discord's own hotkeys |
| Photoshop | 39 actions: file, edit, layers, blend mode, brush size / hardness / mode, 25 tools; all fixed Windows shortcuts (Hue / Saturation / Lightness are hidden) | **partial**: Photoshop preset with default shortcuts; any shortcut by Hotkey |
| Illustrator | 42 actions, fixed shortcuts (tools, paste in front / back / in place, group) | **has** (2026-10-11): Illustrator preset with default shortcuts |
| Premiere Pro | 20 actions, fixed shortcuts (in / out, markers, razor, ripple delete, zoom, export) | **partial**: Video editing preset (Premiere / Resolve) |
| MasterCTRL (Cooler Master's PC-control app) | Switch its scenes, lighting and cooling profiles when `CMAgent.exe` runs | **missing**, not planned (Cooler Master hardware only) |
| Voicemod | Strings for voice changer, hear myself, background effects, mic mute, soundboard, censor beep, stop sounds; **no actions in the catalog** | **missing** (not shipped by the official app either) |
| Spotify, Home Assistant, MIDI, monitor brightness, audio device switch, run scripts, plugins, local API | None | **has** |

### Key look

| Feature | Official app | HubDeck |
|---|---|---|
| Title on a key | Yes, on / off per action | **has** |
| Icon library and recently used icons | Yes | **has** (Lucide icons); no "recently used" |
| Text color, icon color, background color | Yes, with favorite colors | **partial**: background and foreground color; one color for text and icon |
| Font style, font size (4 sizes), text alignment | Yes | **missing** |
| Own picture or animated GIF | JPG / PNG / GIF up to 800 KB | **has**, plus a picture, clock or live graph across all 15 keys |
| State on the key | OBS states, media time count | **has**: second look for mute, OBS, Spotify, Home Assistant, audio device |
| App icon for launchers | No | **has** |

### Lighting

| Feature | Official app | HubDeck |
|---|---|---|
| Effects | Off, static, breathing, strobe, cycling, random, wave, water, rainbow ("spring" is hidden) with color, brightness and speed | **has** (2026-10-11): per knob, fader and the base edge: static, breathing, strobe, fast blink, color cycle, random, wave, shimmer, rainbow, with speed (one per module); the official "spring" is the fast blink |
| Scope | One module, or the whole kit at once | **has** and finer: a color per knob and fader, base edge per profile |
| LEDs that show a value or state | No | **has** (brightness follows the value) |
| Favorite colors | Yes | **missing** (minor) |

### Profiles

| Feature | Official app | HubDeck |
|---|---|---|
| Create, rename, duplicate, delete | Yes, max 32 characters | **has** |
| Templates | "New", "Default" and "Photoshop" | **has**: 7 presets (everyday, OBS, meetings, Photoshop, Lightroom, video, game bar) |
| Export / import | `.mhz` file (a zip: the profile JSON plus its pictures and GIF frames); several profiles at once | **has**: HubDeck's own export / import, and Import reads `.mhz` (see below) |
| Open a `.mhz` by double-click | The app registers itself for `.mhz` files | **missing** |
| Cloud backup | Upload a `.mhz` to a Cooler Master account ("MasterID") | **has** a local alternative: daily backups with restore, shared folder (OneDrive, Dropbox) |
| Switch by the app in front | No | **has**, also by window title |
| Several bases | Yes: picks the base, names it, keeps settings per base serial | **missing** (planned, `docs/PAIN-POINTS.md` item 20) |
| Rename base and modules | Yes | **missing** (minor) |

### Settings and system

| Feature | Official app | HubDeck |
|---|---|---|
| Platforms | Windows 10 64-bit or later (macOS code paths stubbed out) | **partial**: Windows; macOS planned, never built |
| Start with the system | Yes | **has** |
| Tray | Open and exit | **has** (profiles, pause, autostart, log, diagnostics) |
| Sleep / resume / shutdown | Hands keys and dial back to hardware mode | **has**; also blanks screens when Windows locks or after idle |
| Software update | Checks, downloads and installs new versions | **missing** (phase 8: Tauri updater, needs a public repo) |
| Firmware update | Per module, "update all", automatic check | **missing by design** (forbidden in this project) |
| Languages | 17 (English, Arabic, German, Spanish, French, Indonesian, Italian, Japanese, Korean, Dutch, Polish, Brazilian Portuguese, Thai, Turkish, Vietnamese, Simplified and Traditional Chinese) | **partial**: English and Polish |
| Account | Cooler Master ID sign-in (needed only for cloud backup) | not needed |
| Clear app cache, recovery | Clear cache; "recovery" is hidden | n/a |
| Debug | A debug panel | **has**: diagnostics file from the tray |
| Base orientation | Horizontal / vertical switch | **has**: sensor plus Auto / 0 / 90 / 180 / 270 |
| Background service | Daemon reads CPU / GPU sensors through a kernel-driver library every second, writes a log line each second, and serves the values on an unauthenticated socket.io port (3636) | **has** a lighter way: no driver, no second process, ~1 % of a core |
| Works without the app | No: actions need the app running ([APH p.2](https://aphnetworks.com/reviews/cooler-master-masterhub/2)) | same limit (the hardware stores nothing); HubDeck hands keys and dial back to their own animation when paused or closed |

### The .mhz file

How the official app stores an exported profile, as read from its code (no official file was
available; the importer is tested with files built in the tests):

- A zip archive. Inside: one JSON file per profile, named after the profile number and the base's
  serial number (no extension); the user's pictures under `uploadImages/` (original JPG, PNG or
  GIF files); pre-cut GIF frames under `gifPools/` (named by a hash, HubDeck ignores them). Exporting
  several profiles at once gives a zip of such `.mhz` files.
- The file name carries the profile name: `masterHubConfig_profile_<name>.mhz`.
- The JSON has one entry per module kind, each keyed by the module's serial number: `keypad` (a list
  of pages, each a list of 15 keys), `knobsList` (three knobs `rotateR`, `rotateG`, `rotateB`),
  `dialEncoder`, `rollerBlock` (`left` and `right` roller), `sliders5List` (faders `0`-`4`), plus
  `deviceLightingEffect`, favourite colours and the unreleased `touch46s` / `touch50s`.
- A key has an action id (`btnFunc`, e.g. `hot-key`, `web-url`, `scenes`), a title and whether it
  is shown, background and text colors as `{r, g, b}`, a picture path, a stored hotkey as a list of
  key names (`["Control", "Z"]`) and the fields of its action (`url`, `appFilePath`, `content`,
  `dropdownOption`, `selectSceneId`...). A multi-task keeps a list of key-like actions; a folder
  key keeps its 15 keys in `folder`.
- Knobs and the dial keep a list of functions (a press goes to the next); each has an action id
  and a `left` / `right` part (rollers: `up` / `down`) with a choice and a hotkey.
- OBS sources, filters and media inputs are stored by OBS's internal id, not by name; app volume by
  a process id. Neither survives a restart, so HubDeck imports those actions with the name left
  empty for the user to fill in.

What HubDeck imports: the first module of each kind; pages (as pages) and folders (as pages with
a Back key); hotkeys (key names translated to HubDeck's), open URL / app / file, type text,
media, volume, scroll, multi-task (steps 280 ms apart, as the official app runs them), deck
brightness, profile, timer and clock, the 12 Twitch actions, OBS record / pause / stream / replay /
virtual camera / studio mode / transition / scene / scene collection / profile / screenshot,
Streamlabs record / stream / studio mode / transition and, without a name, Streamlabs scenes and,
for both apps, source visibility, mute and mixer volume, and OBS filters; knob and dial function
lists become Modes; titles, non-default colors and pictures. Everything else
(Discord, MasterCTRL, Adobe actions without a stored hotkey, recorded macros, lighting effects) becomes an empty control labelled "Not supported: <action>". Knobs and rollers follow
the official app's own event reading (App.vue): `rotateB`, `rotateR`, `rotateG` are knobs 1-3 in
HubDeck's order, the `right` roller is roller 1 and `left` roller 2 (2026-10-12; earlier the order
was guessed).

### Unreleased hardware in the official code

The official code and SDK know modules that never went on sale: an M3704 key module (per-key
picture and color commands), "Touch46" (M4600) and "Touch50" (M5000) modules with a title and a
color picker, and SDK libraries for M3700 and M4300. The store also has leftover sensor fields for
temperature, humidity, light and CO2 ("Hub2", case sensors). None of this is usable with the
hardware sold. HubDeck: not supported; nothing to add unless such modules appear.

## Promised but not delivered

Timeline: shown at CES (2023 or 2024, sources disagree) and Computex 2023, on Kickstarter from
2024-08-20 to 2024-10-19 ($267,491, 691 backers), kits shipped from May 2025. The Kickstarter page
itself could not be read (403, not archived), so its promises come from the creator updates (read
through the project's Atom feed), the press and Kicktraq. "Update N" links to
`https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/N`.

| Promise | Where, when | Status (2026-10-11) |
|---|---|---|
| MasterHUB on sale in late November 2023 | Computex 2023 press release ([APH news](https://aphnetworks.com/news/27234-cooler-master-announces-masterhub-and-mastercontrol)) | Late: Kickstarter in Aug 2024, shipped May 2025 |
| Nine module types sold one by one | Computex 2023 ([APH news](https://aphnetworks.com/news/27234-cooler-master-announces-masterhub-and-mastercontrol)) | Not delivered: five types exist (15 keys, dial, 3 knobs, 5 faders, 2 rollers) ([product page](https://www.coolermaster.com/en-us/products/masterhub.html)) |
| Streamer, Video and Photo starter kits (Amazon US) | Computex 2023 ([APH news](https://aphnetworks.com/news/27234-cooler-master-announces-masterhub-and-mastercontrol)) | Not delivered: replaced by a Starter / Creator kit and a Pro kit |
| 8-inch touch display module, smaller display modules | TechRadar hands-on, 2023-05-30 ([TechRadar](https://www.techradar.com/reviews/coolermaster-masterhub)); Kickstarter 2024 ([KitGuru](https://www.kitguru.net/lifestyle/mobile/laptops/joao-silva/cooler-master-launches-kickstarter-campaign-for-masterhub-pre-orders/)) | Not delivered. The official code still carries unreleased modules (M3704, "Touch46", "Touch50") |
| Bigger bases (5x2, 10x10) | TechRadar 2023 ([TechRadar](https://www.techradar.com/reviews/coolermaster-masterhub)); "a bigger version" again in 2024 ([XDA](https://www.xda-developers.com/cooler-master-masterhub-review/)) | Not delivered |
| "New modules later this year" | Computex 2024 ([CGMagazine](https://www.cgmagonline.com/news/computex-2024-cooler-master-masterhub), [press release](https://www.silicon.co.uk/press-release/cooler-master-unveils-masterhub-a-forward-thinking-stream-kit-positioned-to-revolutionize-creative-workflows)) | Not delivered |
| About 10 app integrations at launch | TechRadar 2023 ([TechRadar](https://www.techradar.com/reviews/coolermaster-masterhub)); "More to come!" on the [product page](https://www.coolermaster.com/en-us/products/masterhub.html) | Partly: OBS, Streamlabs, Twitch, Discord, MasterCTRL, Photoshop, Illustrator, Premiere (the Adobe ones are only shortcuts, [APH p.2](https://aphnetworks.com/reviews/cooler-master-masterhub/2)). Voicemod has UI strings in 1.0.0.113 but no actions. Broader app support was put "on the roadmap" in [update 4417334](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4417334) (2025-06-23) |
| SDK / API for user customization | Computex 2023 ([APH news](https://aphnetworks.com/news/27234-cooler-master-announces-masterhub-and-mastercontrol)); "starting with the SDK" (Kickstarter update, 2025-10-20) | Not delivered: SDK work "paused" ([update 4544476](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4544476), 2025-11-17). No public SDK or repository |
| Plugin marketplace / store | Expected by reviewers ([Tom's Hardware](https://www.tomshardware.com/peripherals/docking-stations-hubs/cooler-master-masterhub-review): integrations "nowhere near" Elgato's) | Not delivered; no marketplace hooks in the 1.0.0.113 code |
| macOS support | Planned, with refunds offered to Mac users ([update 4410055](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4410055), 2025-06-13) | Not delivered: Windows only ([software page](https://www.coolermaster.com/en-us/software-masterhub.html)); macOS code paths are stubs |
| Spotify plugin | Planned for July 2025 ([update 4410055](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4410055), [update 4417334](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4417334)) | Not delivered: no Spotify code in 1.0.0.113 |
| Lighting per knob and per fader | Planned for July 2025 (same updates) | Not delivered: per module or whole kit only in 1.0.0.113 ([Tom's Hardware](https://www.tomshardware.com/peripherals/docking-stations-hubs/cooler-master-masterhub-review), June 2026) |
| Finer volume steps (was 5 %) | Planned for July 2025 (same updates) | Not delivered for system and app volume: still a fixed 5 % step in 1.0.0.113 (OBS, Streamlabs and Discord have a step setting) |
| Hotkey and macro overhaul | Announced for 2025-08-15 ([update 4459431](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4459431)) | Partly: macro recording came in 1.0.0.104 ([release notes](https://www.coolermaster.com/en-au/masterhub-release-notes.html)); still no key picker, F13-F24 only by recording |
| Smart-home features (roadmap 2024-2025) | Computex 2023 ([APH news](https://aphnetworks.com/news/27234-cooler-master-announces-masterhub-and-mastercontrol)) | Not delivered |
| Shipping from mid-September 2024 | Kickstarter ([Kicktraq](https://www.kicktraq.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/), [Tom's Hardware](https://www.tomshardware.com/peripherals/cooler-master-masterhub-modular-control-panel-hits-pre-order-after-four-years-in-development)) | About 8 months late: first shipment 2025-04-30 ([update 4375720](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4375720)) |
| Upgraded air shipping | Kickstarter | Not delivered for most backers, admitted in [update 4410055](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4410055) |
| Stand with the kit | Kickstarter rewards | Late: redesigned, shipped separately Oct-Nov 2025 ([update 4544476](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4544476)) |

Software releases since shipping: 1.0.0.77 (2025-06-19), .88 (2025-07-19), .104 (2025-08-20,
macro recording), .112 (2026-04-23, log export button), .113 (2026-05-27, "stability")
([software page](https://www.coolermaster.com/en-us/software-masterhub.html),
[release notes](https://www.coolermaster.com/en-au/masterhub-release-notes.html)). Bugs Cooler
Master admitted in [update 4417334](https://www.kickstarter.com/projects/coolermaster/masterhub-versatile-and-customizable-modular-control-system/posts/4417334):
device not found (Windows 11, after GPU driver updates), installer failures, audio popping, OBS
mixer not working, module settings reset.

Not verified: the campaign page text and stretch goals, the backers' comments and Reddit (blocked
for the tools).

## Gaps: what HubDeck still lacks vs the official app

In order of what a user switching from the official app would miss first.

1. **OBS media source control** (play / pause / restart with the time on the key). The rest of
   the official OBS actions were added on 2026-10-11.
2. **Import `.mhz` profiles.** Built 2026-10-11 (`engine/mhz.rs`); still to check with one real
   file from a user.
3. **Twitch actions**: built 2026-10-11; needs the HubDeck Twitch client id (owner) to sign in.
4. **Streamlabs Desktop**: built 2026-10-11; to try against a running Streamlabs Desktop.
5. **Timer and clock on a key and on the dial**: built 2026-10-11 (an analog clock face is still missing).
6. **Hardware lighting effects**: built 2026-10-11 (`leds.rs`); effects that differ between knobs of one module are not verified on the hardware yet.
7. **Push-to-talk / push-to-mute**: built 2026-10-11 (microphone, OBS sources).
8. **Deck brightness and next / previous profile on a control**: built 2026-10-11.
9. **Text style on keys**: font size, alignment, a separate icon color. Approach: three fields in
   `Look`, used by `render.rs`.
10. **Play a sound file**: built 2026-10-11 (`sound.rs`, rodio on its own thread, started on the first sound).
11. **Macro recorder**: engine part built 2026-10-11 (`recorder.rs`); needs a Record button in the multi-action editor.
12. **Fast forward / back per media player**: built 2026-10-11 ("Seek in a player").
13. **Illustrator preset**: built 2026-10-11.
14. **More than one base.** Already item 20 of `docs/PAIN-POINTS.md`.
15. **More languages** (the official app has 17). Approach: open `i18n.ts` to community
    translations after the public release.
16. **Software updates.** Already planned (phase 8, Tauri updater).
17. **CPU temperature and AMD / Intel GPUs**: built 2026-10-11 from LibreHardwareMonitor's WMI (`lhm.rs`), read only while shown.
18. **Discord** voice control: blocked for open source (see `STATE.md`); keep the hotkey preset.
19. Small: open `.mhz` and HubDeck files by double-click, favorite colors, names for the base and
    modules.

Not to copy: firmware updates (forbidden here), the Cooler Master account and cloud upload (local
backups and a shared folder do the job), MasterCTRL (Cooler Master hardware only).
