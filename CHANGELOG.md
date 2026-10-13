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
- The deck assistant (Claude Code on your account) that sketches whole decks for you, and a
  step-by-step mode with a gallery of options to try on the deck.
- Lights: one style for the whole deck with reactions (calls, microphone, agents waiting).
- Windows 10/11 and macOS (Apple silicon and Intel).

Known issues:

- macOS: after the app restarts, the keys or the dial module sometimes stops taking pictures until
  it is taken off the base and put back.
- macOS: the app is not notarised; the first time, allow it in System Settings > Privacy & Security >
  Open Anyway (see the README).
  Windows: the installer is not code-signed, so SmartScreen asks once (More info > Run anyway).
- Elgato Marketplace plugins with copy protection do not run; plugins from their authors' own pages do.
