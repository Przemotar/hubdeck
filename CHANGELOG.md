# Changelog

## 0.1.2

- Stream Deck plugins from the catalog: of 33 popular ones checked, 26 now start and draw on
  Windows, and the catalog marks them "tried in HubDeck". Fixed on the way: plugins with a build
  per target, zips made on Windows or zipped twice, manifests with a byte order mark, plugins
  with pre-releases only, releases with one file per system (the Windows build is picked), and
  plugins that send binary messages. A plugin made for another system, encrypted for Elgato's
  app, or one that drives a device of its own says why it cannot run.
- Plugins end with HubDeck on Windows, also when HubDeck crashes, and stopping a plugin ends
  everything it started.
- A timer that ran out blinks red on its key; one on another page or profile shows in the window
  with Stop. The same key on two profiles holds two timers.
- Lights of this profile: one choice at the top (like every profile, one color, its own effect).
- "Connecting to your deck..." for the first seconds after HubDeck starts, instead of "No
  MasterHUB found".

## 0.1.1

- Updates reach you: HubDeck looks for a new version at start and every 6 hours while it runs,
  shows it in the window when you open it (with What's new and Install and restart), and puts
  "Update to HubDeck x.y.z..." at the top of the tray menu. In 0.1.0 the notice could be missed;
  Settings > Updates > Check now always works.
- Mochi, the dial pet, shows what it wants and which gesture gives it.

## 0.1.0 (preview)

The first public version.

- Every MasterHUB module: 15 keys, the dial with its round screen, knobs, faders, rollers and the
  base with its edge light; modules found and placed as they sit on the base, while it runs.
- Profiles that switch with the app in front, key pages, press / hold / double press, multi-actions,
  modes, toggles; a home profile you pick; sharing and import of the official app's profiles.
- Actions: apps, files, websites, hotkeys, text, scrolling, volumes, media with cover art,
  monitor brightness, system monitor, clock and timer, sounds; OBS, Streamlabs, Spotify, Home
  Assistant, MIDI, Claude Code sessions; your own apps through a small HTTP contract.
- Plugins made for Stream Deck run in HubDeck (keys as a 5x3 keypad, the dial and knobs as dials,
  their settings pages in the panel on the right); a catalog of about 200 open-source plugins to install
  with one click (Settings > Plugins).
- A tour that needs no AI: one question about what you do most, then a deck made at once for
  your modules and your apps, shown on the hardware and walked through part by part (the control
  to try blinks on the deck), each part kept or left out; nothing of yours changes until you keep it.
  If you let HubDeck count presses, the next day it names a control you never used, to swap it.
- The deck assistant (Claude Code on your account) that sketches whole decks for you, and a
  step-by-step mode with a gallery of options to try on the deck.
- Lights: one style for the whole deck with reactions (calls, microphone, agents waiting).
- The window, rebuilt around what you want a control to do: pick a key, then "What should it do?"
  with ideas from other deck owners, or drag an action onto a key; names as in the most popular
  decks (Open, Multimedia, Text, Multi action, Pages); a new page gets Previous and Next page
  keys and the page before it a Next page key, and a page nothing opens says so. The window
  shows what the hardware shows (live values, covers, clocks).
- A pressed key shows a short frame; a key whose last press failed shows a red corner until a
  press works. Fields that name an app, scene, source, device or entity are lists, not typing.
- Settings > Updates: go back to the version before. Settings > Plugins: stop and start a plugin by
  hand; a plugin over 20 % CPU says so.
- A guard hands the deck back and starts HubDeck again if it ever stops, and the window shows a
  crash report; one HubDeck at a time drives the deck.
- Reduce motion (Settings > General, taken from your system the first time): nothing blinks or
  flashes on the deck or in the window.
- Windows 10/11 and macOS (Apple silicon and Intel). On macOS: calls, profiles for apps, CPU and
  GPU temperatures, the built-in screen's brightness, and each app's own volume (macOS 14.2 or
  later; it asks once for System Audio Recording).

Measured on the release build before publishing (Windows 10, the busiest deck: 22 animations and
Mochi): 9.4 % of one CPU core on average, memory 59 -> 62 MB over 5 minutes, keys 16 pictures a
second; 3 restarts with every module back in 6 s; at rest in the tray about 3 % and 40 MB; the
window 162 frames a second with no slow frames. Installer 9.0 MB, installed 21.2 MB; cargo audit
and pnpm audit clean.

Known issues:

- macOS: this version's window was last checked on a Mac before its rebuild (2026-10-15); the
  macOS build is made and signed by CI but not yet tried on hardware. Please report what you see.
- The window shows the deck's animations (GIFs, the screensaver) at about 8 frames a second; the
  deck itself plays them at 15 or more. With the window open WebView2 uses about 300-350 MB; closed,
  HubDeck takes about 70 MB.

- macOS: after the app restarts, the keys or the dial module sometimes stops taking pictures;
  HubDeck tries to restart it by itself, and taking it off the base and putting it back always helps.
- macOS: the app is not notarised; the first time, allow it in System Settings > Privacy & Security >
  Open Anyway (see the README).
  Windows: the installer is not code-signed, so SmartScreen asks once (More info > Run anyway).
- Elgato Marketplace plugins with copy protection do not run; plugins from their authors' own pages do.
- macOS 12 to 14.1: one app's own volume cannot be set (the system and the player can).
- Linux is not supported (a Dependabot alert about `glib` concerns only the Linux build).
