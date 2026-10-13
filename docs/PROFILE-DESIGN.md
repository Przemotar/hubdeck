# Designing great HubDeck profiles

Rules for a profile that feels polished: for people building profiles by hand and for AI
assistants building them through the MCP server (`docs/MCP.md`; the `design_guide` tool serves
this file). The hardware: a 15-key module with a screen per key, a round dial screen that turns
and presses, 3 knobs with LED rings, 5 faders with LEDs, 2 rollers, and the base's edge light.

## 1. Start from the user, not from the hardware

- Ask (or infer) what the profile is for: one app (Photoshop, OBS), an activity (streaming,
  meetings, a game) or the everyday desk. Name it after that, short: "Photoshop", "Streaming".
- Build for the modules the user owns (`list_modules`). A function on a module they do not have is
  invisible; put the important ones where they can reach them. Filling the other kinds too is fine:
  they work the day that module is attached.
- Link app profiles to their app (`apps`: `Photoshop.exe`), so the deck switches by itself.
- Fill keys with what is useful and checked (their apps, media, volume, checked shortcuts from
  `list_shortcuts`); leave a key empty rather than guess a shortcut or add noise.

## 2. Layout: the strongest positions for the most used actions

- Keys are numbered `keys.0`..`keys.14`, 5 per row, 3 rows, from the top left. The middle row and
  the two left columns are the easiest to reach without looking; put the 3-5 most used actions
  there.
- Group by job, in blocks the hand learns: e.g. top row = navigation / scenes, middle = the main
  tools, bottom = toggles and less frequent things. Keep the same job in the same place across
  profiles (mute is always on the same key, "back" is always `keys.14` on a page).
- Pairs sit side by side in reading order: previous / next, undo / redo, zoom out / zoom in.
- More than 15 actions: a page (`add_page`) per group ("Tools", "Scenes"), a key on the main page
  to open it, and `keys.14` on the page to go back. One level of pages only.

## 3. Turn, slide, press, hold: the right input for the job

- Continuous values go on things that turn or slide: volume, brush size, zoom, scrolling, timeline
  scrubbing, brightness. Faders for levels you set and leave (per-app volumes, mic level), knobs
  and the dial for values you nudge, rollers for scrolling and jogging.
- Actions go on keys: one press, one result.
- Hold (`:hold`) for destructive or rare actions (delete, end stream, close app) and for a second
  function of a busy key. Double press (`:double`) sparingly: it delays nothing, but users forget
  it exists; label it if you use it.
- A knob or the dial can hold a few turn jobs with `core.modes` (a press goes to the next). Use it
  for 2-4 related jobs, not as a junk drawer.
- Keys that start something long (record, stream, mute) show their state: they have a second look
  while on (`lookOn`, or the default red version).

## 4. Look: one family, clear contrast, short words

- Labels: 10 characters a line or fewer, one or two lines, a verb or a noun ("Record", "Next",
  "Mic"). No punctuation, no "Toggle", no app name repeated on every key.
- Icons: one family per profile (the built-in line icons, or game icons `game:<name>` for a game
  profile, or your own pictures in one consistent style). Never mix line icons with photos on the
  same row.
- Color: HubDeck's house look (the owner's pick, 2026-10-12): graphite keys, the icon in its
  group's muted color, a white label. HubDeck applies it to drafts; use other backgrounds only
  when the user asks for them. One accent (the profile color and edge light) for the profile.
- Contrast: text and icon at least 4.5:1 against the background. Check with `preview_control`;
  bright backgrounds need dark text.
- Key styles: the house look is flat; `glass`, `gradient` or `outline` only when the user asks.
- The dial is round (240x240 visible inside a circle): keep content inside the middle 170 px.

## 5. Light with meaning

- LED colors mean the same thing everywhere: red = live or muted, orange = needs attention,
  green = ok / ready, the profile color = normal. Never use red as decoration.
- Knobs and faders that set a volume show their level (`ledLevel: true`).
- The base edge in the profile color is calm; effects (breathing, strobe) only for states that
  need attention (recording, an agent waiting). Reactions in the deck-wide lighting already cover
  mic muted, live, agents and timers: do not fight them with constant effects.
- Brightness is the user's choice (deck lights); do not change deck-wide settings without asking.

## 6. Pictures and animations

- Draw your own SVG for keys and the dial: crisp at 72x72 / 240x240, tiny files. Simple shapes,
  2-3 colors from the profile's palette, 4-6 px margins, no thin lines under 2 px, no small text.
- Full panel pictures (480x272 across all 15 keys) for a mood or a dashboard; the keys still act.
- Animations: subtle and purposeful (a pulse on the record key, a slow shimmer on the dial), at
  most 15 fps (8-12 is enough), 8-30 frames, looping cleanly (the last frame leads into the first).
  One animated control per screen area at most; never animate every key.
- Pictures from the internet need the right to use them; prefer your own drawings and the user's
  own files. Game covers come from the user's own launcher and stay on their computer.

## 7. Reactions inform, they do not distract

- A change on the deck should answer a question the user has: "am I live?", "is the mic muted?",
  "does an agent need me?". If nobody would ask, do not show it.
- Show values briefly when they change (the dial already shows a value for 1.5 s), not all the
  time.

## 8. Finish

- Go through every control once: does it do one clear thing, is it where the hand expects it, is
  its label readable, does its color say what group it belongs to?
- Preview the important keys and the dial (`preview_control`) and fix what does not read well.
- Tell the user in one line what is special about what you built (the window and the deck show
  the controls), what they can change, and that HubDeck kept a backup if you replaced anything.
- Never replace or delete an existing profile, page or deck-wide setting without asking first.
