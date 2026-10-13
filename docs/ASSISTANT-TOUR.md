# Show, then ask, then build: the deck assistant, redesigned (spec, 2026-10-13)

The owner's verdict on the assistant as it is (0.1.0 rc): "even for me it is weak and not useful
enough". What was wrong: the Assistant tab opens with too much (two equal primary buttons, a
three-point description, the Claude Code card with a switch, a privacy line, "What it remembers");
it asks what the user wants before they know what the deck can do; the first answer was a whole
profile at once ("dry"). The owner's direction: first show what the deck can do (an onboarding /
demo the user clicks through), then a short interview, then building; design for what users
usually do, not for the owner's own setup.

Status: designed, not built. Design boards: https://claude.ai/artifact/Kz2xZxK1hmrWcevVfchzXX
(owner's picks: the tour stop layout 2A; the stops on both kits; the end page).

## What people use decks like this for (measured)

Elgato Marketplace's public product list sorted by downloads (2026-10-13; 1530 plugins, 995
profiles), grouped (details in `docs/PRESETS.md`):

1. Volume per app and audio (Volume Controller is the most downloaded plugin of all; audio
   switcher, mute, Discord volume mixer, VoiceMeeter, Wave Link, GoXLR).
2. Streaming (OBS #2, Twitch, YouTube, Streamlabs, TikTok LIVE, counters, timers).
3. Voice and calls (Discord #3, Voicemod #4, Zoom; profiles: Discord #2, Zoom #4, Teams #6).
4. Music (Spotify plugins #5 and #9, YouTube Music).
5. Windows tools and app shortcuts (Windows 11 Essentials profile #3; Excel, Word, Chrome).
6. Live information (clocks #13, weather #17, PC vitals).
7. Games (Helldivers 2 stratagems is the most downloaded profile; clips; Star Citizen, WoW).
8. Smart home and lights (Home Assistant, Hue, Govee).
9. Fun (soundboards, emoji keyboard, Tamagotchi). AI prompt profiles exist but are niche.

## The hardware

MasterHUB: a base (4x6 cells, an edge light) and modules: 15 LCD keys, a dial with a 240x240 round
screen (turn and press), 3 knobs with LED rings, 5 faders with LEDs, 2 rollers. Two common kits:
**keys + faders (+ rollers)** and **keys + dial + knobs (+ rollers)**. Modules are hot-plugged;
HubDeck knows the slot and rotation of each and draws the deck as it sits. Profiles bind control
kinds (`keys.4`, `dial`, `faders.2`), not serial numbers; settings of owned but detached modules
are kept on a "My modules" shelf.

## The flow

1. **Assistant tab, one way in.** "Show me what it can do" (8 short stops on your deck, ~3 min,
   no AI needed) or "I know what I want" (a text box straight to the assistant). Claude Code in
   one line ("Runs on your Claude Code (plan) · Details"); the switch and "What it remembers" in
   Details. The first-start setup page leads here too.
2. **The tour** (layout 2A: stops on the left, the stop in the middle, the live deck on the right).
   First screen: "Try each thing on your deck. Heart what you like; at the end we build your deck
   from your hearts." A liked counter in the header. Each stop:
   - title, one or two lines, **Try it** with a task done by hand on the deck and a live check
     ("✓ You turned it: Spotify at 40%");
   - the control to try highlighted on the drawing and on the hardware (a knob glows, a key
     blinks), wherever the module sits;
   - **Looks**: a row of looks for that stop, each shown on the deck at a click, each liked on its
     own;
   - **I want this** and **Next**; Exit at any time restores the user's deck as it was; the tour
     can be left and resumed.
   Everything shown is temporary (a tour draft), nothing of the user's changes.
3. **The end**: what you liked (with the looks picked); **a set of decks made from the likes at
   once, without AI**: the home deck plus decks for the apps the likes touch (call app, OBS, each
   game with an action bar, Chrome, Spotify), each previewable on the deck and deselectable; Use
   these decks / Change it step by step; **More looks** (full galleries: dial looks, key styles,
   animations and screensavers, icon packs) with hearts; the assistant (Claude Code) asks what
   else (which games get an action bar, what takes too many clicks every day) with answer buttons,
   a text box and More ideas. Without Claude Code: "Make the deck without AI" and one line on what
   Claude Code adds.
4. **Step by step** (the builder from session 7, layout A): a gallery per control (dial, knobs,
   keys, lights), the liked things first, then ready options, then "More ideas" from the assistant.

## The stops

Order from the measured popularity; each shown only when it fits this computer and these modules;
at most 8 plus the finale. Each stop has a version per module kind; a stop whose module is missing
moves to what is attached or is left out. A module owned but detached: one line ("You also have a
dial: attach it to see this on it"); attaching it during the tour switches the stop to it within
~2 s.

| # | Stop | Shown when | Kit with faders | Kit with dial and knobs |
|---|---|---|---|---|
| 1 | A knob or fader for each app's volume | always | faders: music, game, chat, browser, rest; LED level | knobs with LED rings; dial: app in front, ring on screen |
| 2 | Mute and talk | always; Discord/Zoom/Teams keys when installed | mic key red, edge red while muted, call volume on a fader | same, call volume on a knob; the call profile comes by itself during a call (edge green) |
| 3 | What's playing | a music player found, else media keys | cover on a key, music volume on a fader | cover on the dial (turn seeks, press pauses) |
| 4 | Your apps, and shortcuts that follow you | always | app keys with real icons; the app in front brings its keys | same |
| 5 | Streaming | OBS / Streamlabs / TikTok LIVE installed | scenes, record/live red, OBS mixer on faders | mic/desktop levels on knobs |
| 6 | Games | games found | covers, then the game's action bar, clips | same, dial: game volume or a timer |
| 7 | Clock, weather, your PC | always | values on keys, colored when hot | on the dial |
| 8 | Scroll and zoom anywhere | rollers attached | rollers | rollers |
| 9 | Lights that mean something | always; smart home with Home Assistant or Hue | edge reactions, room light or monitor brightness on a fader | edge, knob LEDs as levels, monitor brightness on a knob |
| 10 | One key, many jobs | always, late | press / hold / double press, pages, macros, macro recorder | same |
| 11 | Handy extras | always, late | soundboard, timer, emoji and snippets, screenshot and clipboard, Stream Deck plugins and the catalog, your own apps | same |
| Finale | Super mode | always, last | animations on keys, code rain / night sky / waves screensaver, one picture over 15 keys, rainbow edge and fader LEDs | same plus an animated dial |

## The user's own profiles are never overwritten (owner, 2026-10-13)

The tour only ever creates new profiles; nothing the user made is changed or deleted. On the end
page, where a new deck meets an existing one (an app that already has a profile, a home profile
already picked), the user chooses per app: keep mine, use the new one, or keep both (the new one
stays in the list, switched to by hand). The new home deck becomes home only when the user says
so. Old profiles stay until the user removes them.

## More

- Several runs: each run ends in its own set of decks named by kit ("My deck · faders", "My deck ·
  dial and knobs"); a new run starts fresh or from the previous likes. New: **a deck per kit** —
  when the user swaps knobs for faders, HubDeck switches to the deck made for that kit.
- macOS: the Accessibility permission is asked at the first stop that needs shortcuts, with why.
- Languages: English and Polish.
- What already exists to build on: presets (`engine/presets.rs`), quick sketches and the personal
  tour (`engine/assistant.rs`, `make_tour`), the "Discover HubDeck" showcase profile, the dial
  gallery (`engine/gallery.rs`), key styles and the house look (`engine/house.rs`), animations and
  screensavers (`engine/saver.rs`), installed apps and games (`engine/installed.rs`), time in front
  (`engine/usage.rs`), the step-by-step builder (`engine/builder.rs`, `components/Builder.tsx`),
  the identify marks of Edit layout, the "try a draft on the deck" mechanism.
