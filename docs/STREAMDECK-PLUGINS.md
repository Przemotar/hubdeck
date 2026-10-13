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
   source and publish them).
2. The OpenAction Marketplace (open-source catalog).
3. Plugins the Stream Deck app already installed on the computer
   (`%APPDATA%\Elgato\StreamDeck\Plugins\`, `~/Library/Application Support/com.elgato.StreamDeck/Plugins/`):
   HubDeck can offer to use them.
4. The Elgato Marketplace installs through the Stream Deck app with an Elgato account ("Open in
   Stream Deck"). HubDeck does not get around that; paid or protected plugins stay with Elgato.

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
3. Actions: plugin actions in the action library (icon and name from the manifest); their keys
   drawn from `setImage` / `setTitle`; the dial from `setFeedback` layouts.
4. Property inspector in the inspector (iframe).
5. Verify with a handful of open-source plugins on both systems; record which work in
   `docs/STREAMDECK-PLUGINS.md` > "Tried".

## Tried

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
