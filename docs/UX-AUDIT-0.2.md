# The window and working with the deck: audit before 0.2.0

Asked by the owner on 2026-10-27 (session 34): "jak porobisz te tryby, efekty, presety to chciałym
żebyś zrobić audyt, analizę naszego UI, ustawienia, pracy z deckiem, konfigurowania go i sprawdził
czy nei trzeba czegoś poprawić, dopolishować, usprawnić albo uproscic". Done alone in the night
session 35 on the test copy with his own profiles; every finding below is a proposal with variants
for his pick (looks are decided with him), except the ones marked **done**, which were plain bugs
or numbers that lied, fixed the same night and named in the morning page.

How it was walked: `scripts/ui/living-deck.mjs` (111 checks), `saver-panel.mjs` (32),
`window-flicker.mjs` (each theme by a click), `theme-cost.mjs` (every theme's cost), screenshots
of each step looked at; the tasks of `docs/FLOWS.md` T1-T12 with `scripts/ui/tasks.mjs` (clicks).

## How others do it (the facts every proposal starts from)

- **Stream Deck**: a page's background is set by right-clicking any key > "Set from file…", one
  picture over that page's keys; the screensaver is in Settings > Devices > "Set Screensaver" (a
  picture or a GIF), the sleep timer in Settings > General > "Sleep After"; looks (icon packs,
  screensavers, profiles) come from the Marketplace, a separate site with a picture of each, opened
  from the app and installed with "Open in Stream Deck". Sources:
  [backgrounds and screensaver](https://www.elgato.com/explorer/products/stream-deck/how-to-add-a-background-and-screensaver/),
  [icon packs](https://help.elgato.com/hc/en-us/articles/4402426848013-Elgato-Stream-Deck-Using-Icon-packs).
- **The official MasterHUB app**: animated GIF pictures on keys (its `background.js` cuts GIFs into
  frames: `resizeAndExportFramesGif`, `gifPools`); no themes, scenes across the keys or effects
  found in it (searched for theme, wallpaper, screensaver, animation; checked in session 35).

## Findings

### F1. The Living deck panel has grown long (owner's own example: "the Living deck's 50+ tiles")
Today, top to bottom: the load bar and the live rate, the frame cost, **Theme** (18 tiles of text,
None + 17 themes), then six categories with about 60 effect tiles, each switched-on effect's
settings under its category. A newcomer who wants "a nice look" meets 78 tiles of names without a
picture. Stream Deck shows looks as pictures in its Marketplace and keeps the app's own panel short.
- **a) (recommended)** Themes as a gallery: a sheet over the window with a live picture of each
  theme (the engine's own frame, as in `logs/theme-cost/`), grouped (Seasons, Places, Developers,
  Gamers, Creators, Space, Music), a click tries it on the deck for 20 s like the screensaver's
  preview, "Use" keeps it. The panel shows the chosen theme as one card with "Change theme", and the
  60 effect tiles move under "Customize effects" (closed).
- b) The theme tiles stay in the panel but carry a small picture each (two columns), the effects stay.
- c) Text tiles as now, grouped under headings; the effects under "Customize effects".

### F2. The load bar called a light theme heavy (**done**, wybrałem: the bar follows the measured time)
The bar estimated the load from a table of guesses; Halloween's lanterns read "Heavy" with a
warning at 8 ms of the 41.7 ms a picture has. Now it shows the frame cost's measured time (an
estimate only until the first second's measurement); the walk checks the two agree.

### F3. The frame cost debugger (**done**, his "ok" to the sketch)
Closed by default under the live rate (the panel stays short); open, each effect's ms, the
heaviest marked, the picture's parts. Cut names ("Halo behind the ico…") were found in its first
screenshot and fixed; the walk now fails on an ellipsis too.

### F4. The everyday tasks did not get longer
`scripts/ui/tasks.mjs` on the browser demo (session 35): a key opens an app 4 clicks, a shortcut 4,
a knob sets an app's volume 5, mute the mic 3, a key's colour and icon 4 (a dialog), a deck that
comes with an app 3, the edge light in one colour 3, a second page and its key 6 clicks and 1 typed:
33 actions in all, the same as before the living deck. A theme takes 2 clicks (the Living deck
chip, the theme), a screensaver 2 (its chip, its tile).

### F5. His grey keys stand as grey squares in every scene
A key with its own lighter colour keeps 80 % of it under a scene (his word of 2026-10-26: under the
aurora "szare przyciski … robią się niewidoczne"), so in every theme his X, X and CPU keys sit as
flat grey squares, holes in the forest, the lake or the nebula (the morning page shows them). This
touches that pick of his.
- **a) (recommended)** On a scene such a key becomes frosted glass: the scene behind it blurred and
  tinted with his colour, a light rim; it still reads as his grey key and stays readable, and it
  belongs to the picture.
- b) As now (his pick of 2026-10-26).
- c) A theme switch "Keys show the scene": his flat colours give way while a theme is on.

### F6. A theme goes on the deck at once, with no look first
A click on a theme puts it on his deck and his profile; trying several means changing his profile
each time ("None" takes it off). The screensaver's tiles already preview for 20 s first. Stream
Deck shows a picture of each look before it is installed.
- **a) (recommended)** With F1 a: a click previews the theme on the deck and in the window for 20 s,
  "Use" keeps it (as the screensaver panel does).
- b) Pictures on the tiles only (F1 b), a click keeps it as now.
- c) As now.
