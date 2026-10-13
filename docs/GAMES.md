# Games on the deck

The MasterHUB is an odd little console: 15 keys over one screen, a round screen on a dial, knobs
that click and push, rollers, faders with lights, and a base that knows when it is turned. No other
console has this mix, so games built for it play like nothing else. This is the plan; the owner's
idea from 2026-10-10.

**Status (2026-10-11): switched off.** The owner played the first build and decided: features
first, games later, and "a few polished games rather than many average ones". Game mode is hidden
behind `GAMES_ENABLED` (`games.rs`, `src/engine.ts`); the code and its tests stay. The findings and
the plan for when the work resumes are in "Polish plan" at the end.

## Budget (measured)

- The key panel and the dial each take about 32 pictures a second, also both at once
  (`PROTOCOL.md`). Games animate both at up to 30 fps.
- The engine already gets every press, release, detent and fader position with a few ms delay.
- Knob and fader LEDs change in one short report each; the base edge light too.

## How it works

- **Game mode**: a "Games" action on any key (or "Play on the deck" in the window) turns the deck
  into a console. The profile's actions pause; the keys and the dial belong to the game.
- **Launcher**: every key is a game (icon and name on the key); the dial shows the best score of
  the game under the last touch. Press a key to play.
- **Starting**: the Games action on a key, "Play on the deck" in the window's Games tab, or the tray
  menu.
- **Leaving**: hold the dial for one second (or the top-right key in the launcher) goes back one
  level: game -> launcher -> your profile. The dial says so while a game waits to start.
- **Scores**: the best score per game is kept in `<app data>/games.json` and shown in the launcher
  and in the window's Games tab.
- **Adapts to the modules**: a game says what it needs (keys, dial, knobs, faders, rollers) and
  uses extras when they are there; one that cannot run without a module shows it greyed out.

## First games

| Game | Uses | Idea |
|---|---|---|
| **Mole** | keys, dial, knob LEDs | Moles pop up under random keys; hit them before they hide. They come faster; three misses and it is over. The dial is the score ring and the timer. |
| **Simon** | keys, knob and fader LEDs, dial | The deck plays a growing sequence of lit keys (and LEDs); play it back. The dial shows the round. |
| **Safe** | dial, keys | Crack a combination lock: turn the dial, feel for the click (the screen shakes on the right number), push to lock it in. Three numbers, against the clock; the keys show the tumblers. |
| **Snake** | keys (whole panel), dial | The whole 480x272 panel is the field, seen through the 15 key windows; the gaps hide parts of it. Steer with the dial; food only appears where it can be seen. |
| **Arena** | dial, keys | Things fly at the core on the round screen; turn the dial to move the shield into them. Lives on the keys and the knob LEDs. |
| **Balance** | faders, keys, fader LEDs | Five targets drift up and down; keep every fader on its target. The lit keys show where, the fader lights turn green when you are on it. |
| **Rhythm** | keys, knob LEDs, dial | Notes come down the three rows of each key column on a beat; hit them when they reach the bottom row. Streaks score more; eight misses end it. |
| **Pong** | keys (whole panel), dial, knobs or rollers | The panel is the court; your paddle on the left follows the dial, the deck plays the right one until someone turns a knob or a roller: then it is a two-player game. First to 7. |
| **Duel** | keys | Two players: columns 1-2 against 4-5, the middle column is the referee. A key lights on each side (mirrored); first to hit theirs wins the point, a press before it lights costs one. First to 5. |
| **Memory** | keys | Seven pairs of pictures under 14 keys; the 15th counts the moves. Fewer moves, higher score. |
| **Reflex** | keys, dial | A key lights at a random moment; press it as fast as you can. Five rounds, the average in ms; pressing early restarts the wait. |
| **Wheel** | dial, keys | Spin the dial and the round screen picks: lunch, who pays, what to play (2-12 fields, edited in the Games tab). Never ends; hold the dial to leave. |

## Later

- **Rhythm to your music**: beats from what is playing (needs the audio level of the system).
- Games as plugins: the plugin API already has keys, titles and pictures; a `setPanelImage` and LED
  commands would let anyone write games in any language.

## Polish plan (owner's test, 2026-10-11)

What the owner saw on the hardware:

- **Snake disappears in the gaps.** The field is the whole 480x272 panel, but only the 15 key
  windows show; the snake can crawl through the hidden strips between keys and vanish.
- **Pong misses hits at high speed.** The ball moves several pixels per frame and the hit test
  only looks at where it ends up, so a fast ball jumps over the paddle ("as if it flew through").
- **No explanation.** A game starts without saying what to do or with which control.
- **Not everyone has a dial.** The basic MasterHUB kit has keys and faders, no dial and no
  three-knob module, so games must be split into "for every kit" and "needs the dial", and
  leaving a game must not depend on holding the dial.

Fixes planned for every game:

1. **How to play** before the start: name, two or three lines and the controls on the key panel
   (and the dial when there is one); a key press starts.
2. **Leave without the dial**: hold the top-right key for a second, in every game; holding the dial
   stays as a second way.
3. **Launcher in two groups**: "every kit" (keys, faders) and "needs the dial"; games the attached
   modules cannot run are hidden, not greyed out.
4. **Panel games without gaps**: Snake and Pong live on a grid made of the visible windows only;
   moving from one window to the next skips the gap, so nothing is ever drawn where it cannot be
   seen. Collisions are tested along the whole path of a frame (swept), not at its end point.

A smaller, polished set (proposed 2026-10-11, not yet chosen by the owner):

| Group | Games |
|---|---|
| Every kit | Memory, Reflex, Simon, Balance (faders) |
| Needs the dial | Snake, Pong, Safe |
| Hidden until polished ("Show unfinished games") | Mole, Arena, Rhythm, Duel, Wheel |
