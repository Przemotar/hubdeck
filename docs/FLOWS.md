# How HubDeck is used: tasks, ideal paths, the window's map

Written 2026-10-15 (session 13) after the owner's question: "Trzeba ogólnie przemyśleć flow
działania w tej apce … zapisać jak chcemy żeby te przypadki się odbywały i pod to robić UI i UX …
ten UI się pomieszał". The window grew one feature at a time; this file says first what people do
with HubDeck and how each task should go, then where everything lives, then what today's window
does differently. Screens change only after the owner has judged this map (his pick 2a/3a: the
action-choice variants A/B/C come after it).

Status: **picked by the owner** (2026-10-16: "Zrób tak jak zalecasz" on the mockup https://claude.ai/artifact/4B9EECEUV6iFjxRyFV1PWb): variant A for the action choice, then differences 4, 2, 3, 6, 5. First draft: https://claude.ai/artifact/WJ6ZSCJKefaSJmgk2x7rqc. Each difference below says "done" once built.

## Who uses it

- **A new owner of the kit** (most users after the release): bought the modules cheap, never used
  the official app, wants something useful on the deck in minutes and no manual.
- **The everyday user** (the owner): one home deck, decks that come with apps (Spotify, games,
  Claude), changes a key or a knob now and then, never wants to wait or hunt for a setting.
- **The tinkerer**: plugins, OBS, Home Assistant, the local API, the assistant.

## The tasks (most frequent first)

| # | Task | How often | Today (clicks, from `scripts/ui/tasks.mjs`) |
|---|---|---|---|
| T1 | Give a control a job (a key opens an app, a knob sets a volume) | constantly | 4-5 clicks; the action is chosen in two places |
| T2 | Change how a key looks (label, icon, color, picture) | often | 4 clicks, a dialog |
| T3 | A deck that comes with an app by itself | sometimes | 3 clicks, 3 screens |
| T4 | Switch decks by hand (on the deck, in the window) | often | a tab in the top bar, or the Profiles tab |
| T5 | More keys than 15: pages | sometimes | 6 clicks and typing |
| T6 | Lights and screens (edge color, brightness, screensaver) | rarely | Settings, or a click on the edge |
| T7 | First start: something useful on the deck | once | the tour ("Show me on my deck") |
| T8 | Connect an app or service (OBS, Spotify, plugins) and use it | once per service | Settings > Connections |
| T9 | Ask the assistant to build or change a deck | sometimes | Get started > AI assistant |
| T10 | Move modules on the base | rarely | Edit layout |
| T11 | Something is wrong (a module silent, a crash) | rarely | a banner, Settings > Help |
| T12 | Keep and move settings, update | rarely | Settings > Backups, Updates |

## The ideal path of each task

**T1. Give a control a job** (the heart of the app)
1. Touch the control on the deck, or click it in the drawing. (Both select it; today: yes.)
2. The right panel asks **"What should it do?"**: a search field, then **suggestions for this kind
   of control** (a key: open an app, a shortcut, mute the mic, media; a knob or fader: system
   volume, an app's volume, brightness, scroll), then the recently used actions, then every group
   with icons and one line each. Only actions that fit the control are offered.
3. Picking one shows its few settings right there, each a **choice from a list** wherever a list
   exists (apps, OBS scenes and sources, audio devices, profiles, pages; owner's rule, 2026-10-15:
   "Tam gdzie mamy mieć coś z apek to powinna być jakaś lista albo jakiś wybór apek a nie miejsce
   na nazwę"). The label and icon follow what was picked (now for apps).
4. It works at once on the deck; nothing to save.
5. Hold and double press are tabs above "What should it do?" (secondary, never in the way).

**T2. Change how a key looks**: below the action in the same panel, "Look": label, icon (a
searchable grid), background color, a picture; the drawing and the deck change as you type.

**T3. A deck for an app**: "New deck > For an app" lists installed apps with icons (apps that have a
ready deck first); picking one makes the deck, links the app and switches in with it.

**T4. Switch decks**: the deck tabs in the top bar (one click); on the hardware, a key or the dial
with "Decks". The left column lists decks with their apps (the place to rename, link apps, delete).

**T5. Pages**: "+ page" in the page bar above the drawing; a key with "Open page" is offered right
away in the new page's empty key.

**T6. Lights and screens**: a click on the edge of the drawing or a knob's ring opens Lights for
that thing; the deck-wide defaults live in Settings > Lights & screens.

**T7. First start**: the tour builds a deck on the hardware in about five minutes and walks through
it; at its end the deck stays as home. After that, Get started is no longer the first thing shown.

**T8. Connect**: Settings > Connections, one card per service with Connect; an action of a service
that is not connected says so in its panel and offers Connect (it is never put on a deck by the
assistant).

**T9. Assistant**: one place, the assistant's tab; it shows a draft on the deck to try, and Keep or
Throw away.

**T10. Move modules**: Edit layout above the drawing; drag modules; the deck re-maps itself.

**T11. Something is wrong**: a banner says what and what to do (reseat the module, the crash
report); Settings > Help keeps the diagnostics.

**T12. Keep and move**: Settings > Backups (export, import), Updates.

## The window's map (proposed)

| Area | Holds | Never holds |
|---|---|---|
| Top bar | the deck tabs (switch, + New deck), Auto-switch, modules status, Settings | actions, panels that cover the editor |
| Left column | **Decks** (list with apps, rename, link, delete); **Assistant**; **Get started** only until the first deck is kept, then under Help | the action library as a separate place to start from |
| Centre | the drawing, the page bar, Edit layout | |
| Right column | the selected control: hold/double tabs, **What should it do?** (search, suggestions, groups), its settings (lists), Look. Nothing selected: this deck (its apps, its lights) | Settings, plugins, galleries (those get the whole window or a dialog) |
| Settings (whole window) | General, Lights & screens, Connections, Plugins, Backups, Updates, Help | |

## Where today's window differs

1. **An action is chosen in two places**: the left Actions tab (a library with search, groups and
   presets that does not know which control is being set) and a long drop-down in the right panel
   (knows the control, ~76 entries). -> One place: "What should it do?" in the right panel (variants
   A/B/C to judge: A only there; B there plus "Browse all" for drag and drop; C the Actions tab
   kept but tied to the selected control). **Done 2026-10-16, variant A** (`ActionChooser.tsx`;
   tasks.mjs 33 -> 32 actions, a key opens Spotify 5 -> 4 clicks on a set key, 3 on an empty one).
2. **Decks have four ways in**: the Profiles tab, the deck tabs, the profile panel from the top bar,
   and New deck > For an app. -> Deck tabs to switch, the left Decks list to manage, New deck to make.
   **Done 2026-10-16**: one word, "profile" (owner: as the official app and Stream Deck call it;
   "deck" is only the hardware): tabs in the top bar switch, the left Profiles list manages, a
   profile's settings open as a sheet, New profile makes one.
3. **Get started stays the first tab for good**, also on a deck that has long been set up.
   **Done 2026-10-16, owner's 2a**: Get started keeps its places; once set up (a tour profile kept,
   or two profiles of one's own) Profiles comes first and is open at start; the tour and Build it
   yourself are also in Settings > Help.
4. **Panels take the inspector's place** (For an app, Gallery, Games, the profile panel): a touch on a
   control then shows nothing (UX audit H4). -> They open as a dialog or the whole window.
   **Done 2026-10-16**: For an app, Gallery, Games and a deck's settings open in a sheet over the
   window (`Sheet` in `Panels.tsx`; Esc or a click outside closes it); the editor stays on the right.
5. **Names typed from memory**: the app of a volume (now a list), OBS scenes and sources (now a list
   while OBS is connected); still typed: Streamlabs sources, Home Assistant entities, MIDI notes,
   window titles of "only in this app". -> Lists wherever the app or service can say what exists.
   **Done 2026-10-16, owner's 4a**: every such field is a list (`ParamKind::Named`, `names_list`):
   OBS scenes, sources, transitions, collections, profiles, filters of the picked source; Streamlabs
   scenes, sources, audio sources; Home Assistant entities, lights, scenes and scripts; Spotify
   playlists; Twitch categories as you type; the music app of Now playing from the app list. Checked
   on sample replies (tests) and in the browser demo; live once the owner connects each service.
   `node scripts/ui/inputs.mjs`: 0 typed listable fields (was 16).
6. **Lights live in three places**: Settings, the edge of the drawing, and a deck's own lights in the
   profile panel. -> Settings for defaults, the drawing for this deck.
   **Done 2026-10-16, owner's 3a**: Edge under the drawing opens "Lights of this profile" (its own
   edge, its color) with "Lights for every profile" to Settings; the profile's settings no longer
   hold lights. "The edge light in one color" for every profile: 2 -> 3 clicks (tasks.mjs).

## Next

1. The owner judges this map (artifact) and the six differences.
2. Variants A/B/C of "What should it do?" from the real window (before/after screenshots), the owner
   picks; then build, and measure T1 again with `scripts/ui/tasks.mjs` (clicks before and after).
3. Then differences 2-6 in the order the owner picks.
