# Setting up the deck for a game

The short way: add the game's preset, play, then change what does not suit you. Everything below
happens in HubDeck's window; nothing has to be installed in the game.

## 1. Start from a preset

Profile tabs > **+** > *From a preset* lists the built-in games (Diablo IV, Baldur's Gate 3,
Counter-Strike 2, Valorant, Fortnite, Minecraft, League of Legends) and *Game action bar* for any
other game. The preset uses the game's default key bindings, game icons and the volumes of the game,
Discord and your music on the dial, knobs and faders. It switches in by itself whenever the game is
in front, and back to your home profile when you leave it.

No preset for your game? Take *Game action bar*, or *Empty profile*, and link the game in the
profile panel (click the active tab > the app list: pick the game while it runs).

## 2. Tweak the keys

- **Rebound something in the game?** Select the key on the deck drawing, then in the inspector click
  *Record* next to *Keys* and press the key the game uses.
- **Keep "Send as: For games".** Games read the keyboard differently from other apps; this mode
  sends hardware key codes and holds them a moment. If a game still misses presses, raise *Hold (ms)*
  to 50-80. A game started as administrator needs HubDeck started as administrator too.
- **Several keys at once or in order** (a combo, a buy sequence): choose the *Multi action* action
  and add steps, or click *Record keys* and play the sequence.
- **More than 15 keys?** Add a key page (the + next to *Key pages*) and put a *Go to page* key on
  the main page.
- **Turning things**: a knob or the dial can send one key per direction (*Hotkey* with *Turn
  clockwise* / *counter-clockwise*), e.g. zoom or weapon cycle.

## 3. Your own pictures

- **Icons**: in the look editor pick from *Interface*, *Games* (300 game-icons.net icons; search
  "sword", "potion", "map") or an installed icon pack.
- **Pictures**: *Choose picture* takes PNG, JPG, WebP, SVG or an animated GIF; it fills the key and
  the label stays on top. Screenshots of the game's ability icons work well (keep them for your own
  use; do not share game artwork).
- **Icon packs**: the + at the end of the icon tabs installs a pack: a zip or folder with
  `manifest.json`, or a Stream Deck `.streamDeckIconPack`. Use packs you are allowed to use.
- **Style and color**: Flat, Gradient, Glass or Outline, and the color presets, set the key's frame;
  the label and text color sit on top.

## 4. Share it

Profile panel > *Export* > *HubDeck profile with its pictures* writes one `.hubdeck` file with
everything inside. Send it to a friend (they use *Import*), or submit it to the shared gallery as
described in `docs/SHARING.md`. Before sharing, replace pictures you may not share (game artwork) with
game icons.
