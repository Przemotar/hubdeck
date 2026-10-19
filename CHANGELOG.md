# Changelog

## 0.1.6

- **CPU temperature and AMD or Intel GPUs with a current LibreHardwareMonitor.** Its version 0.9.5
  stopped publishing readings the way HubDeck read them, so these keys showed "-". HubDeck now
  reads LibreHardwareMonitor's own web server: turn on Options > Remote Web Server > Run in it.
  Older versions keep working as before.
- **Microsoft Store apps keep their icon** on a key after they update (Spotify lost its logo when
  it went from 1.304 to 1.305), and their keys no longer say "Not found on this PC".
- **The deck assistant uses your Spotify playlists**: playlist keys get your real playlists instead
  of empty "Playlist 1" keys and a request for links. It keeps an app's shortcuts in that app's
  own profile (a shortcut goes to the app in front), and keys it fixes later get the app's icon too.
- An app's own updater that leaves a copy running under a backup name (`app.bak-….exe`) counts as
  the same app in "apps used most".

## 0.1.5

- **Spotify Like and playlists work again** (#4). In March 2026 Spotify changed its Web API for
  apps like the one you connect HubDeck with, and Like answered "Forbidden". HubDeck uses the new
  endpoints, and after "Too Many Requests" it waits as long as Spotify asks. Nothing to set up
  again: your connection stays.
- **Connecting Spotify, step by step** (Settings > Connections > Spotify): why it needs an app of
  your own and Spotify Premium, a button to open Spotify's developer dashboard, the address to
  copy, then your Client ID. Once connected it says which account it is and warns if Spotify
  reports no Premium.
- **Stream Deck plugins work right after HubDeck starts** (#5). Plugins learn which of their keys
  are on screen; HubDeck told them only when the profile changed, so with Auto-switch off their
  keys (Discord mute and deafen, for example) did nothing after a restart until you edited a key.
- **Any module in any connector.** The window knew 88 module-and-connector positions, the ones
  Cooler Master's app knows. The other 42, many where a module hangs over the base's edge (keys
  above the base, for example), are worked out from where the connectors sit: the window draws
  the module where it is, and the pictures on the keys are turned the right way. In Edit layout a
  module can hang over the edge too.
- Mochi keeps its name and body when a key takes it off the dial and puts it back (Discussions #3).

## 0.1.4

- A **Plugins** tab in the top bar: about 300 Stream Deck plugins that install with one click, from
  three sources (the open ones on Elgato Marketplace, the OpenAction catalog and GitHub
  repositories that publish a plugin file), with filters (source, this computer, dial and knobs,
  tried in HubDeck, category), search and sorting. Only plugins that can be installed are listed:
  every repository and download is checked daily. After Install, **Use it** opens the deck with
  the plugin's actions open in the library (plugins sit below the built-in groups). A plugin's group in the library shows the plugin's own
  picture, as in Stream Deck.
- Dragging an action onto a key works again on Windows (broken since 0.1.3).
- Plugin settings pages fit the panel: no fields past its edge, no empty space under them.
- Plugins written in .NET start: HubDeck now tells plugins the system's version, as Stream Deck
  does (Windows Display Settings stopped at once without it).
- **Text on the deck** (Settings): any font on your computer for the text on keys and the dial;
  letters HubDeck's font lacks (Japanese, Chinese, Korean) come from a system font instead of
  empty boxes.
- **Japanese** built in, and language files of your own (Settings > General > Make a language
  file).
- **Mochi with a body of your own**: pick a picture for the dial pet's body; its eyes, mouth, moods
  and every reaction are drawn over it. The picture keeps its own shape (a PNG with a see-through
  background for a cut-out). Fit shows it happy, petted, hungry and asleep, large; drag it, size it, Reset. Mochi has one body: its settings on the dial and on the key that brings it show the same picture. A picture
  set on the key that puts Mochi on the dial is its body there too.
- While the deck shows its screensaver, the window shows the keys as they are set and says so,
  with End; a click on the deck in the window ends it.
- Deleting a profile or a page, restoring a backup, going back a version and removing an icon pack
  ask first and wait for your answer; before, they went ahead without the question. A deleted
  profile is in Settings > Backups ("before delete"), and from now on a deleted page too.
- Get started no longer shows the tour's big button for a moment when it opens.

## 0.1.3

- Stream Deck plugins from where you have them: the catalog in Settings > Plugins now also lists
  the 300 most downloaded plugins on Elgato Marketplace, most downloaded first. 36 of its top 100
  publish an open plugin file and install with one click; the others say they come only through
  Elgato's app.
- Plugins your Stream Deck app already installed show in Settings > Plugins; "Use in HubDeck"
  copies one. Plugins Elgato encrypted for its own app say so (they cannot run anywhere else).
- Open a `.streamDeckPlugin` with HubDeck (Windows lists it under Open with, without taking these
  files from Elgato's app) or drop it on the window: HubDeck shows what it is and installs it when
  you say so.

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
