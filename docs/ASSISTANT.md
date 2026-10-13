# The deck assistant

An assistant inside HubDeck that gets to know how the user works and builds deck profiles with
them: it looks at the apps they have and use, proposes whole decks as sketches the user can try
and compare on the deck, polishes the one they pick and keeps talking about the next experiment.

It is **not built into HubDeck**. It runs through **Claude Code** installed on the user's
computer, signed in with their own Claude plan (Pro or Max) or an Anthropic API key. Every answer
counts toward that plan's usage limits (or is billed by use with an API key). HubDeck has no
account and no costs of its own.

The design (session 6, 2026-10-12) follows two independent reviews (`private/assistant-design/`):
**the deck reacts first, the model is the layer of taste**. HubDeck makes sketches and the tour
itself, at once; the model refines them and talks. Safety lives in code, not in the prompt.

## The parts

| Part | File | What it does |
|---|---|---|
| Window | `app/src/components/Assistant.tsx` | The Assistant tab: what it runs on, the taste card, suggestions, the scan card, the talk (streamed text, idea cards with Try / Compare / Pick, answer buttons, "Remembered" lines), drafts, What runs underneath |
| Conversation | `app/src-tauri/src/engine/assistant.rs` | The scan, the instructions (`assistant-rules.txt`), one Claude Code process per talk, reading its stream, quick sketches, the personal tour, drafts and trying them |
| Tools | `app/src-tauri/src/engine/mcp.rs` | `hubdeck --mcp-assistant`: the assistant's own MCP server (the builder's tools limited to drafts, plus propose_idea, offer_choices, remember, make_tour, list_shortcuts) |
| Guard | `app/src-tauri/src/engine/guard.rs` | What may go on a control: no commands; closing, quitting or deleting only on a hold |
| Shortcut packs | `app/src-tauri/src/engine/shortcuts.json`, `shortcuts.rs` | Default Windows shortcuts of 33 apps read from their vendors' pages (each pack has its sources) |
| Taste | `app/src-tauri/src/engine/taste.rs` | `taste.json`: notes the user can edit, ideas kept or thrown |
| Suggestions | `app/src-tauri/src/engine/nudge.rs` | `nudges.json`: an app without a deck, a new kind of module; the brakes |
| Design rules | `docs/PROFILE-DESIGN.md` | What the `design_guide` tool hands the assistant |
| House look | `app/src-tauri/src/engine/house.rs` | The key look every profile HubDeck makes uses |
| Local API | `app/src-tauri/src/engine/api.rs` | What the MCP tools call in the running app (`/api/profiles`, `/api/assistant/idea`...) |
| Time in front | `app/src-tauri/src/engine/usage.rs` | How long each app is in front, counted on this computer only (`usage.json`) |
| Persona checks | `scripts/ui/personas.mjs`, `scripts/personas/` | Six made-up users in test copies, one real turn each, timings and draft quality |

## A talk, step by step

```
HubDeck window ──assistant_send──▶ HubDeck engine
                                     │ first message: quick sketches of the top apps (no AI),
                                     │ shown at once as idea cards
                                     │ starts once per talk (no window):
                                     │ claude -p --input-format stream-json --output-format stream-json
                                     │        --verbose --include-partial-messages --tools ""
                                     │        --mcp-config mcp.json --strict-mcp-config
                                     │        --allowedTools mcp__hubdeck --system-prompt <rules>
                                     │        --setting-sources local --disable-slash-commands
                                     │        --settings {"autoMemoryEnabled":false}
                                     │        --model <yours or picked> --effort <yours or picked>
                                     │        [--resume <id>]
                                     ▼
                                   Claude Code ──(your account)──▶ Claude
                                     │ calls tools
                                     ▼
                                   hubdeck.exe --mcp-assistant ──HTTP──▶ HubDeck's local API
                                     (drafts only, guard)                makes drafts, idea cards
                                     │
   window ◀──assistant://event── engine: text pieces, the whole text, ideas with their drafts,
                                  answer buttons, remembered notes, tool use, the end of a turn
```

1. **Scan** (`assistant_scan`): apps used most (time in front), apps in front lately, installed
   apps (developer tools left out), installed games, attached and owned modules, profile names,
   and what HubDeck remembers about the user's taste. Nothing is sent yet; the card shows exactly
   this text.
2. **First message**: HubDeck makes **quick sketches** of the three apps used most (their preset
   or a starter, for the attached modules, in the house look) as drafts and shows them at once.
   Then it starts Claude Code once for the talk and writes the scan, the list of quick sketches,
   a compact **catalog** (every action with its inputs and params, hotkey names, LED effects, the
   packs) and the user's words as a `stream-json` input line.
3. The model proposes its own ideas with **`propose_idea`**, one call each, as soon as it has
   one: HubDeck makes a draft of the sketch at once, the window adds a card, and the controls that
   did not check out come back to the model with why. Answers as buttons come with
   **`offer_choices`**; a lasting preference with **`remember`** (shown as "Remembered: ...",
   with Forget). Text streams in as it is written.
4. A turn ends with the `result` line; the process stays for the next message. Stop or a new model
   ends it; the next message starts it again with `--resume`.
5. **Try** shows a draft on the deck without making it the home profile; **Compare all** puts the
   drafts on the deck and turning the dial flips between them; **Pick** sends the draft id to the
   model, which polishes it in place and shows it with `try_on_deck`. **Use this** makes it a
   normal profile; **Throw away** deletes it. Both go into `taste.json` as kept or thrown.
6. **Show me on my deck** makes the **personal tour** at once (`make_tour` / `assistant_tour`):
   "Discover HubDeck" with the user's own top apps (their real icons) on the first page and a page
   of their games with covers; it goes on the deck and the model walks them through it.

Measured on the test copy (2026-10-12, Opus 5.5, medium effort): quick sketches in 0.7-0.9 s,
the first idea of the model in 12-17 s, a whole first turn in 30-42 s (before: the first card
after ~70 s, a tour ~6 min).

## What the assistant can and cannot do

- `--tools ""` turns off Claude Code's own tools: no files, no commands, no web.
- `--strict-mcp-config` with `--allowedTools mcp__hubdeck`: only HubDeck's tools.
- **Drafts only, in code** (`mcp.rs` `assistant_call`): `create_profile` always makes a draft
  (`replace` is dropped); `update_profile`, `set_control`, `clear_control`, `add_page`,
  `delete_page`, `set_lights` and `try_on_deck` are refused for the user's own profiles;
  `activate_profile` and `set_deck_lights` are not offered.
- **Guard** (`guard.rs`, also for sketches): `core.run` and `ha.service` are refused; shortcuts
  that close, quit or delete (Alt+F4, Ctrl+W, Ctrl+Q, Shift+Delete... and the ones marked
  destructive in the packs, like Zoom's Alt+Q) and text with Enter only on a hold, never a press
  or a turn; steps of multi-actions and modes are checked too.
- **Checked shortcuts**: `list_shortcuts` gives an app's pack; a shortcut outside the packs in a
  profile the assistant made shows a "Shortcut not checked for this app" mark on the control and
  "N shortcuts not checked" on its card.
- `--system-prompt`, `--setting-sources local`, `--disable-slash-commands` and
  `autoMemoryEnabled: false`: none of the user's own Claude Code setup reaches it (no hooks,
  CLAUDE.md files, skills, permissions or auto-memory). The sign-in stays the user's; their
  default model and effort are read from `~/.claude/settings.json` and passed on.
- App names, game names and web values are data, never instructions (said in the rules).

## The start of a talk (owner's picks, session 6)

The first message names two or three things only HubDeck can do for this person (from the scan,
with their real apps), then a short interview: three or four questions, one at a time, only
about what the scan cannot tell (what the deck is for, what annoys them, what one press away,
the style), answers as buttons that can be ticked several at once (All, Send). Each answer gets
one concrete possibility back. Then three sketches from the answers, at least one HubDeck-only
(live state, the dial, lights, an animation). "Skip the questions: surprise me" goes straight to
ideas. The sketches show one at a time (arrows, dots or the dial), the window and the deck in step.

## More power (a switch, off by default)

With "Let it reach further" on, Claude Code also gets its own Read, Glob, Grep, Bash, WebFetch
and WebSearch, and `--permission-prompt-tool mcp__hubdeck__permission_prompt`: every command and
web fetch (Claude Code itself lets read-only commands like `echo` through) shows in the talk as
a card (Allow / Allow in this talk / No) and waits up to ten minutes. The assistant may then also
put commands on keys (`core.run`). Without it, `test_address` still lets it GET a local app API
(127.0.0.1 / localhost only). Keys can call an app's API with the HTTP request action
(`core.http`: method, address, body, headers; five seconds at most; the key shows the status).

## The user's own apps (phase 12)

Apps that follow `docs/APP-CONTRACT.md` are connected once (Settings > Connections > Your apps, or
`connect_app` for a local address); HubDeck listens (`states.rs`). The assistant reads them with
`list_app_states` and builds `core.state` keys (an item in its state's color and icon; `app.*` on
the dial counts what waits) and `core.http` keys for their actions. More tools: `hubdeck_state`
(what is set up, so it promises nothing that is off), `tried` (what the user touched while a
draft was on the deck), `key_usage` (how often each control is pressed, only if the user switched
counting on in the Assistant tab).

## Suggestions outside the talk

HubDeck notices on its own, with no AI: an app in front 30+ minutes on 2+ days with no profile,
or a kind of module attached for the first time. The Assistant tab gets a dot and a calm card:
Show me a sketch (its quick sketch, tried on the deck), Ask the assistant (the message waits in
the first box), Not now (7 quiet days), Don't suggest this (30). At most one a day, never during
a call, with a game in front or while a draft is tried; one switch in "What it remembers about
you". No pop-ups, nothing taken over on the deck.

## What is sent and where it is kept

- Sent to Claude (through Claude Code): the scan text the user saw (with the taste notes), the
  catalog, their messages, and what the tools return (actions, icons, drafts, shortcut packs).
  Not sent: the screen, files, window titles.
- Claude Code keeps the conversation like any of its sessions:
  `~/.claude/projects/C--Users-<user>-AppData-Local-Temp-hubdeck-assistant/<session>.jsonl`. To
  read or continue it in a terminal: `cd %TEMP%\hubdeck-assistant` and
  `claude --resume <session id>` (the window shows the id).
- `usage.json` (time in front), `taste.json` and `nudges.json` stay in HubDeck's data folder.

## Without Claude Code

The tab says the assistant needs Claude Code and a Claude plan or an API key, links to
claude.com/claude-code, and points to the ways without AI: + > For an app or game… and the
ready-made profiles. A suggestion's "Show me a sketch" works without it (HubDeck's own sketch);
quick sketches and the tour are HubDeck's own too but are offered inside the talk for now.
