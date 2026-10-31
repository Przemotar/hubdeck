# Changelog

## 0.2.0

- **The living deck.** The keys module can be one picture instead of fifteen squares: a scene drawn
  live behind your icons, seen through the key windows, moving all the time, and a press does
  something in it (leaves thrown off a key, a gust through the cherry blossom, koi swimming up for
  food). Click **Living deck** under the deck in the window: **Deck look** shows the presets and your
  deck live, the deck shows a preset while you try it, and nothing is kept until **Apply**. Twelve
  presets: Hollow Hill and Lantern Hollow for Halloween, Golden autumn, Mushroom hunt, Fuji and
  sakura, Koi pond, Tokyo Rain, Code rain, Synthwave, Cyber rig, Nebula and Spectrum (it follows what
  plays on your speakers; HubDeck listens only while it is on and records nothing). Mushroom hunt,
  Fuji and Koi pond are drawn for a deck lying flat or standing upright. **Keys** in the same view
  gives every key a style and a light on the icons that stay when you change the preset; **Save as
  mine** keeps your own. The scenes are drawn by the graphics card at up to 24 pictures a second; a
  computer without one HubDeck can use gets the deck without them, and the window says so.
- **A preset colours the rest of the deck**: the base's edge, the knobs and the faders take the
  preset's colours. A light you set yourself on one of them stays yours; each can go back to the
  preset with one click.
- **Screensaver**, beside Living deck under the deck: after the minutes you set, a scene across all
  15 keys (aurora, aquarium, lava lamp, fireplace, summer night, neon rain, haunted night) or any of
  the presets, each tile with a moving preview. The dial can show a clock and the lights breathe in
  the scene's colour meanwhile. One for every profile or a profile's own, and **Preview on the deck**
  for 20 seconds. Using the window ends it. The code rain, night sky, waves and big clock screensavers
  are gone; a profile that had one gets the aurora.
- **Lights to the music** moved under the deck, beside Screensaver: on, off and how they follow the
  beat.
- **A look editor for keys**: Edit look in a key's Look panel has Templates, Background, Icon,
  Label and Extras (patterns, a badge, a frame, motion such as pulse, shine or glow), and styles
  that draw the whole key one way (film, glitch, neon tube, glass, pumpkin). **Full editor** opens
  the key on the whole window with the icon library; drag the icon or the label, and edit several
  keys at once.
- **A key's look can follow what happens** (When): CPU or another value over a number, music
  playing, the microphone muted, an app in front, a time of day; and a label can show a value (CPU,
  memory, temperatures, the time, the song playing).
- **Icon packs both ways**: a `.streamDeckIconPack` opens with HubDeck (double click or drop it on
  the window), and the full editor saves a key as a PNG or a page as an icon pack. A profile can
  have its own style: new keys in it start in that look.
- **Sharper keys.** Every picture goes to the keys in full colour; moving keys used to be sent at a
  lower quality that halved the colour. Only the keys that changed are sent.
- **Smoother moving keys**: several pieces of a picture are in flight at once instead of one after
  another, so a fuller picture still goes out sooner, and moving keys keep their own pace. On the
  same deck (22 GIFs and Mochi on the dial), 0.1.12 -> 0.2.0: the GIFs 15.9 -> 20.0 pictures a
  second, Mochi 20 -> 25; a deck moving on every key went from 10.9 to 22.2.
- **The window keeps up with the deck**: the keys in the window move up to 25 times a second (about
  8 before), and a change of a key's action shows there at once (#9).
- **A key that calls a web address or asks an app no longer holds the deck**: a call that waited
  (a local app that was not running held it about 2 seconds) now runs beside it.
- **What's new** comes up once after an update (this one shows the living deck); it is also in
  Help and feedback.
- **A new icon**, and **Join the Discord** in Help and feedback.

Measured before the release (Windows, the release build, that deck): 12.0 % of one CPU core (7.2 %
in 0.1.12: more pictures, in full colour), memory flat. With the graphics card in use HubDeck holds
about 440 MB (64 MB in 0.1.12), most of it the card's driver.

Join the [Discord](https://przemotar.github.io/hubdeck/discord) to rate the presets and vote on
what comes next.

## 0.1.12

- **A multi action switch shows the right side after a restart** (#10): when its first press sets
  something HubDeck can read (an audio device, mute, a Home Assistant light, OBS), the key shows
  what Windows really has, also after a change made outside the deck. Anything else keeps its last
  side across restarts, for each profile.
- **The volume knob follows the speakers you switch to** (#10): after switching from speakers to
  headphones it kept turning the speakers.
- **Start with Windows is in Settings > General > Startup** (#10), the same switch as the tray
  icon's. HubDeck also renews its startup entry at every start, and the log says whether it is on.
- **Paste look** (#10): right-click a control > Paste look, or Ctrl+Shift+V, puts the copied
  control's colors, LED color and effect, and style on another one; its action stays.
- **More ways to style a key** (#10): the label at the top, middle or bottom over the icon (as on
  Stream Deck), the label's size, a small or large icon, and emoji as icons (Change icon > Emoji).
- Fixed a crash at start that could happen when an app was in front while HubDeck started.

## 0.1.11

- **Uninstall a plugin** (#8): Settings > Plugins and the plugin's card in the Plugins tab have
  Uninstall. It stops the plugin and deletes its folder; the question first names the keys that
  use it. Those keys stay where they are and say the plugin is missing instead of doing nothing.
- **A key's picture in the window follows the deck** (#9): a clock, a GIF or a counter on a key
  changes in the window too, instead of keeping its first picture.
- **Questions come up in HubDeck's own window**, with the button named for what it does: Delete
  profile, Delete page, Uninstall, Restore backup, Go back, Remove icon pack, and an import that
  runs programs.
- **Deleting the home profile makes your next own profile home**, instead of an assistant's draft
  or a ready-made one.
- **Big plugins install from the catalog**: the download had 5 seconds in all, too little for a
  59 MB plugin such as VTube Studio; it now waits up to 10 minutes and says when GitHub did not
  answer. A catalog plugin says on which system it was tried in HubDeck.

## 0.1.10

- **Picking a profile only switches to it.** A click in the profile list, on a tab or on the deck
  no longer makes that profile the one HubDeck starts with. The house button in a profile's row
  sets the home profile; turning Auto-switch off does not change it either. A game's or an app's
  profile still hands the deck back to the profile you picked last.
- **The profile list is grouped:** Home, Your profiles, and For apps and games (the ones that
  switch in by themselves); Ready for your apps is folded until you open it. The home profile is
  the first tab in the top bar.
- **Closing the window keeps what was open in it.** The X hides the window; coming back within
  10 minutes finds it as you left it, and only after that is its memory given back.
- **A draft you were trying is offered back after a restart.** If HubDeck restarts while an
  assistant's draft is on the deck, the window says so, with Put it back, Use this and Not now.
- **Show on the "made a profile for ..." banner** scrolls to the new profile and lights it up; with
  the profile list already open it used to do nothing.
- The log counts the calls HubDeck makes to other services, by address, once a minute.

## 0.1.9

- **Spotify no longer runs out of requests.** With a Like key on screen HubDeck asked Spotify every
  3 seconds, about 2400 times an hour, and after half an hour Spotify made the app wait. It now
  asks when the song changes and every 30 seconds otherwise, about 160 times an hour, and the log
  counts the calls.
- **Right-click a key, knob, fader or the dial** in the window: Copy, Paste, Clear with their
  shortcuts, and Open page on a page key.
- **Try a ready-made profile before you add it.** New profile > Ready-made shows it on the deck
  with Add and Back; nothing is saved and your home profile stays until you press Add.
- Picking a profile while another one is on the deck for a try ends the try.

## 0.1.8

- **Move keys by dragging them** in the window: drop a key on an empty
  key to move it there, on another key to swap them. Press, hold and double press go together, and
  Ctrl+Z puts it back.
- **Double-click a page key** in the window to open its page there and on the deck (Next page and
  Previous page keys too).
- **Copy and paste of keys is reliable.** Pasting on several keys quickly could lose all but the
  last paste, and the copy could take a key selected before.
- **Live values on a page opened from the deck** load at once; before they could stay at "…"
  until the window opened that page.
- **Stream Deck plugins that draw their own pictures show them.** The HWiNFO plugin's keys kept the
  plugin's logo with a sensor picked: HubDeck refused its pictures over a space in their data.
- **Plugin keys start without a label**, as in Stream Deck, and a plugin that draws its own title
  gets the key's label as that title: type "CPU" and the HWiNFO plugin shows "CPU" instead of a
  long sensor name. The label HubDeck used to put on plugin keys by itself no longer covers them.
- The Plugins tab says what to turn on in HWiNFO before its plugins show any sensors.
- When LibreHardwareMonitor gives no readings, the log says why (no answer, or which sensors it
  had), so a diagnostics file shows where it stops.

## 0.1.7

- **Lights to the music.** The base's edge flashes with the beat of whatever plays (Spotify or any
  player) and glows with how loud it is: pick "To the music" as the lights style, or as a
  profile's edge effect. HubDeck works out the tempo and flashes on the beat it predicts, so the
  light comes with the kick, not after it; snares and claps flash whiter. Settings > Lights >
  Lights to the music has four presets, how dark it goes between beats, from how loud it lights
  up, and How early, set with an 8-second metronome test. The knobs and the faders can flash too
  (off by default). HubDeck listens to the computer's sound only while a light uses it; nothing
  is recorded.
- **LEDs and pictures go out faster.** Colors and pictures no longer wait for HubDeck to finish
  reading the controls: an LED color takes about 8 ms instead of 35, a key picture 15 instead of
  35, a dial picture 7 instead of 28.
- **The faders no longer drop off** while the edge follows the music: the knobs' and faders'
  rainbow changed color up to 50 times a second, more than their firmware takes.
- The screensaver runs at an even 15 frames a second (it was 11-13).
- An app-state key with its own icon keeps it and shows the state in its color.
- For your own scripts and apps: `GET /api/leds` tells what each module's LEDs show now.

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
