# Changelog

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
  their settings pages in the inspector); a catalog of about 200 open-source plugins to install
  with one click (Settings > Plugins).
- A tour that needs no AI: one question about what you do most, then a deck made at once for
  your modules and your apps, shown on the hardware and walked through part by part (the control
  to try blinks on the deck), each part kept or left out; nothing of yours changes until you keep it.
  If you let HubDeck count presses, the next day it names a control you never used, to swap it.
- The deck assistant (Claude Code on your account) that sketches whole decks for you, and a
  step-by-step mode with a gallery of options to try on the deck.
- Lights: one style for the whole deck with reactions (calls, microphone, agents waiting).
- Reduce motion (Settings > General, taken from your system the first time): nothing blinks or
  flashes on the deck or in the window.
- Windows 10/11 and macOS (Apple silicon and Intel). On macOS: calls, profiles for apps, CPU and
  GPU temperatures, the built-in screen's brightness, and each app's own volume (macOS 14.2 or
  later; it asks once for System Audio Recording).

Known issues:

- macOS: after the app restarts, the keys or the dial module sometimes stops taking pictures until
  it is taken off the base and put back.
- macOS: the app is not notarised; the first time, allow it in System Settings > Privacy & Security >
  Open Anyway (see the README).
  Windows: the installer is not code-signed, so SmartScreen asks once (More info > Run anyway).
- Elgato Marketplace plugins with copy protection do not run; plugins from their authors' own pages do.
- macOS 12 to 14.1: one app's own volume cannot be set (the system and the player can).
- Linux is not supported (a Dependabot alert about `glib` concerns only the Linux build).
