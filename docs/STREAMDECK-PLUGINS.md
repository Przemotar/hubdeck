# Stream Deck plugins in HubDeck (research, 2026-10-13)

The aim: HubDeck must work with the apps people already use, without
changing them; the de facto standard for this kind of device is the Stream Deck plugin. This page
is what was found before building it, and the plan.

## What a Stream Deck plugin is

- A folder `<uuid>.sdPlugin` with `manifest.json`, shipped as a `.streamDeckPlugin` file (a zip
  made by Elgato's CLI `pack`). Each plugin runs as its own process.
- Launch: the host starts `CodePath` (or `CodePathMac` / `CodePathWin`) with
  `-port <p> -pluginUUID <uuid> -registerEvent <event> -info <json>`; the plugin opens a WebSocket
  to `127.0.0.1:<p>` and sends `{"event": <registerEvent>, "uuid": <pluginUUID>}`. `-info` describes
  the application, the OS and the devices (`id`, `name`, `size` {columns, rows}, `type`).
- `Nodejs.Version` in the manifest (`"20"` or `"24"`) means `CodePath` is a JavaScript file run by
  Node, which the Stream Deck app bundles. Otherwise `CodePath` is a native program (C#, C++, Go...),
  often built for Windows only.
- Manifest fields that matter here: `Actions[]` (`UUID`, `Name`, `Icon`, `States` with images,
  `Controllers`: `Keypad` / `Encoder` / `Neo`, `Encoder.layout`, `PropertyInspectorPath`,
  `SupportedInMultiActions`), `ApplicationsToMonitor`, `Profiles`, `OS`, `Software.MinimumVersion`
  (6.4 .. 7.6), `SDKVersion` (2 or 3).

### Events the host sends

`keyDown` / `keyUp` (controller, coordinates {column,row}, settings, state), `willAppear` /
`willDisappear`, `dialRotate` (`ticks`, `pressed`), `dialDown` / `dialUp`, `touchTap` (`tapPos`,
`hold`), `didReceiveSettings`, `didReceiveGlobalSettings`, `titleParametersDidChange`,
`deviceDidConnect` / `deviceDidChange` / `deviceDidDisconnect`, `applicationDidLaunch` /
`applicationDidTerminate`, `systemDidWakeUp`, `propertyInspectorDidAppear` / `Disappear`,
`sendToPlugin`, `didReceiveDeepLink`, `didReceiveResources`.

### Commands the plugin sends

`setTitle`, `setImage` (a file in the plugin folder or a base64 data URL, PNG/JPEG; SVG is used by
plugins in practice), `setState`, `showOk`, `showAlert`, `setFeedback` (the dial's touch strip:
title, value, indicator bar, icon, per layout item), `setFeedbackLayout` (`$A0`, `$A1`, `$B1`,
`$B2`, `$C1`, `$X1` or a custom JSON layout), `setTriggerDescription`, `openUrl`, `setSettings` /
`getSettings`, `setGlobalSettings` / `getGlobalSettings`, `switchToProfile`,
`sendToPropertyInspector`, `logMessage`, `setResources` / `getResources` (7.1+).

### Property inspector (the action's settings page)

An HTML page from the plugin folder. The host shows it and calls
`window.connectElgatoStreamDeckSocket(port, uuid, registerEvent, info, actionInfo)` after the page
loads; the page then talks to the same WebSocket. In HubDeck: an iframe in the inspector, served
from the plugin folder by the local server, with that call made after load.

## Who else runs them

- **OpenDeck** (GPL-3.0): runs "the majority" of Stream Deck plugins, Elgato's own Discord plugin
  among them; Windows-only plugins through Wine on macOS/Linux; property inspectors supported;
  only Elgato hardware officially. GPL: we read its behaviour, we do not copy its code (HubDeck is
  MIT OR Apache-2.0). HubDeck implements the protocol from Elgato's public documentation.
- **OpenAction**: an API "backwards-compatible with the Stream Deck SDK" for any device and OS,
  with extensions; plugins that avoid the extensions also run in Stream Deck and Mirabox Stream
  Dock. Its catalog of open-source plugins: the OpenAction Marketplace (marketplace.rivul.us).
- **Mirabox Stream Dock** runs Stream Deck-style plugins on its own decks.

## Where users get plugins

1. `.streamDeckPlugin` files from a plugin's own site or GitHub releases (most plugins are open
   source and publish them). HubDeck installs one opened with it (on Windows it is listed under
   Open with; it becomes the file's app only where nothing else opens these files, so Elgato's
   Stream Deck app keeps them), dropped on its window, or picked in Settings > Plugins, after
   showing its name, author and version.
2. The Plugins tab (in the top bar): one catalog of free plugins that install with one click, built
   every day by `scripts/plugins/catalog.mjs` and published with the site
   (`przemotar.github.io/hubdeck/plugins.json`). Its sources: Elgato Marketplace's 300 most
   downloaded whose authors publish an open file (36 of the top 100, matched by hand on 2026-10-17:
   author and plugin id checked; `elgatoOpen` in `app/src-tauri/src/engine/sdcatalog.json`), the
   OpenAction Marketplace, and GitHub repositories with the topics `streamdeck-plugin`,
   `stream-deck-plugin` or `elgato-stream-deck` that publish a `.streamDeckPlugin` in a release
   and changed in the last two years ("Not tried yet" until checked here). Each plugin carries its
   source, Elgato's rank or its GitHub stars, its last change, its systems, its category and
   whether it has dial actions (from Elgato's list or the plugin's `manifest.json`); filters and
   sorting use them. Plugins only Elgato's app installs, and OpenDeck plugins that add another
   maker's deck, are left out. When the file cannot be read, the app joins the OpenAction list and
   Elgato's itself. On 2026-10-18: 302 plugins (Elgato 36, OpenAction 152, GitHub 114).
3. Plugins the Stream Deck app already installed on the computer
   (`%APPDATA%\Elgato\StreamDeck\Plugins\`, `~/Library/Application Support/com.elgato.StreamDeck/Plugins/`):
   Settings > Plugins lists them, and "Use in HubDeck" copies one (the Stream Deck app keeps its
   own).
4. The Elgato Marketplace installs through the Stream Deck app with an Elgato account ("Open in
   Stream Deck"). Since January 2026 its plugins are protected by Elgato's DRM, which Elgato says
   is there so other vendors cannot run a plugin without its author's consent. Checked on
   2026-10-17 with a plugin installed from the Marketplace (HWiNFO Deck Bridge): the Stream Deck
   app keeps its `manifest.json` encrypted on disk (it starts with `ELGATO\x01`), so HubDeck
   cannot know its actions. HubDeck says so and does not get around it; paid or protected plugins
   stay with Elgato.

## How the MasterHUB modules map

| Module | Stream Deck controller | Notes |
|---|---|---|
| 15 keys (5x3, IPS) | `Keypad`, a 5x3 device | images drawn into our key windows (79x75) |
| Dial (240x240 round screen, turn, press) | `Encoder` with its touch-strip feedback | `$A0`..`$C1` layouts drawn as SVG on the round screen; `touchTap` not possible (no touch) |
| 3 knobs (turn, press, LED) | `Encoder` without a screen | feedback shown on the dial while turned, or ignored |
| 2 rollers, 5 faders | none in the SDK | rollers as `Encoder` ticks; faders as ticks or an OpenAction extension; HubDeck's own plugin protocol keeps faders, rollers and LEDs |

The device in `-info` and `deviceDidConnect`: a keypad 5x3 plus encoders, typed like a Stream
Deck + so encoder actions are offered.

## Limits to expect

- Plugins built only for Windows run only on Windows (no Wine in HubDeck at first).
- Node plugins need Node 20/24: use one already installed, or download the official Node once on
  the first such plugin (about 40 MB, into HubDeck's data folder), with the user's OK.
- Plugins that need things only the Elgato app has (its account, its OBS link, its profiles
  bundled with `Profiles`) may not work fully. Discord's voice control needs Discord's approval of
  the client: it works where the plugin carries Elgato's approved client (OpenDeck reports
  Elgato's Discord plugin working).
- Trademarks: HubDeck says "runs Stream Deck plugins" (what it is compatible with); the product
  name does not use it.

## Plan

1. Host: a second WebSocket dialect in the plugin host (`engine/plugins.rs`): launch with the
   Stream Deck arguments, registration, the events above for keys and the dial/knobs, the
   commands above; settings stored per control in the profile.
2. Install: open a `.streamDeckPlugin` (or drop it on the window), list plugins found from the
   Stream Deck app with Use; Node found or fetched with consent.
3. Actions: plugin actions under "What should it do?" (icon and name from the manifest); their keys
   drawn from `setImage` / `setTitle`; the dial from `setFeedback` layouts.
4. Property inspector in the inspector (iframe).
5. Verify with a handful of open-source plugins on both systems; record which work in
   `docs/STREAMDECK-PLUGINS.md` > "Tried".

## Tried

On Windows (2026-10-16/17/18; 53 checked, 44 run), on a test copy with `node scripts/ui/plugin-check.mjs <catalog ids>`:
each plugin installed from the catalog, started, connected and drew a key it was put on. Not every
action was pressed, and plugins that talk to an app or a service (Home Assistant, Philips Hue,
Twitch, GoXLR...) were not connected to one. These carry "tried in HubDeck" in the catalog
(`TRIED` in `sdcatalog.rs`).

| Result | Plugins |
|---|---|
| Start, connect and draw (26) | Essentials for Spotify (also on the Mac), Discord, System Information, Weather, Home Assistant, Flight Tracker, Google Meet, Twitchat, Audio Output Switch, HWiNFO, Philips Hue, CPU, IFTTT, DevOps for StreamDeck, Mic Mute Toggle, WSProxy, YouTube Music Desktop, JetBrains IDE, Discord Volume Mixer, GoXLR Utility, Open Hardware Sensor, API Request, ScriptDeck, Uptime Kuma, HWiNFO Reader, Sound Deck |
| Start, connect and draw, checked 2026-10-18 (18) | Audio Mixer, GPU Utilization, VS Code commands (streamdeckvsc), Bitfocus Companion, VTube Studio, Multi OBS, VLC, Color Picker, Tomato timer, Dice, Libre Hardware Monitor, RAM, Task Timer, Focused App Volume, Nanoleaf, TeamSpeak 3, Windows Display Settings, On Air Clock |
| Made for another system, say so (5) | VS Code, KMLink, Xcode, OSA Script (macOS only); Volume Controller (Linux only) |
| Cannot run outside Elgato's app (1) | RME TotalMix FX 5.5.1: its files are encrypted for Elgato's Stream Deck app |
| Not a plugin for the deck (1) | Touchy-Pad: drives a touch device of its own in OpenDeck, no actions |
| Made for macOS only, says so (1) | Meld Studio (2026-10-18) |
| Installs but does not run (1) | Stocks: writes its log into Elgato's own plugins folder and stops when it is not there; left out of the catalog (`wontRun` in `sdcatalog.json`) |

Fixed on the way (2026-10-17): a `.streamDeckPlugin` zipped once more (DevOps), a manifest with a
UTF-8 byte order mark (Flight Tracker), a plugin with pre-releases only (Flight Tracker), a release
with one file per system (ScriptDeck: the Windows PowerShell build, not the PowerShell 7 one), and
messages sent as binary WebSocket frames (ScriptDeck). System Information holds 42-50 % of one core
for a whole minute (measured every 10 s on the test copy, 2026-10-17), while Discord on the same
OpenAction library stays at 0 % and the plugin's own loop only refreshes CPU and memory once a
second; HubDeck sends it nothing that loops back. Cause not found yet; Settings > Plugins names a
plugin over 20 %.

Fixed on 2026-10-18: HubDeck sent an empty `application.platformVersion`; .NET plugins read it as a
`Version` and stopped at start (Windows Display Settings, exit code 0xe0434352; the cause is in
Windows' Application log under ".NET Runtime"). It is now the system's version as Stream Deck gives
it (`10.0.19045`). Measured at start on the test copy: Focused App Volume 83 % and Audio Mixer
26 % of one core (not checked over time yet).

| Plugin | System | Result |
|---|---|---|
| Essentials for Spotify 1.2.0.0 (Node, ntanis-dev) | macOS, 2026-10-13 | Installs, its icons on keys and in the window, its setup page opens through `openUrl`, global settings kept; play/pause and next work on the owner's deck. After its first setup it showed "no device" until the plugin was restarted (HubDeck restarted only the plugin); to look into: whether Stream Deck sends something after `setGlobalSettings` that this plugin waits for. Needs the user's own Spotify app (developer.spotify.com) and Premium. |

## Sources

- Elgato SDK docs: events sent https://docs.elgato.com/sdk/plugins/events-sent , events received
  https://docs.elgato.com/sdk/plugins/events-received , manifest
  https://docs.elgato.com/sdk/plugins/manifest , registration
  https://docs.elgato.com/sdk/plugins/registration-procedure , property inspector
  https://docs.elgato.com/sdk/plugins/property-inspector , packaging
  https://docs.elgato.com/sdk/plugins/packaging
- OpenDeck https://github.com/nekename/OpenDeck and its Flathub listing
  https://flathub.org/apps/me.amankhanna.opendeck
- OpenAction https://openaction.amankhanna.me/ , crate https://docs.rs/crate/openaction/latest
- Elgato Marketplace install https://help.elgato.com/hc/en-us/articles/33589587352337
