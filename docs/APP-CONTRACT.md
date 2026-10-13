# Your app on the deck: the HubDeck contract

How any app (yours, a script, a tool you build with Claude Code) shows its state on the deck and
offers actions for keys, without knowing anything about HubDeck. HubDeck listens; the app only
publishes: the app publishes, HubDeck subscribes, others can listen too.

Everything is optional. An app that only has `GET /hubdeck/status` already works.

## 1. The state: `GET /hubdeck/status`

An address on the app's own local port (`http://127.0.0.1:<port>/hubdeck/status`), answering JSON:

```json
{
  "app": "Agent Desk",
  "items": [
    { "id": "1", "label": "Agent 1", "state": "waiting", "text": "Needs your OK" },
    { "id": "2", "label": "Agent 2", "state": "working", "text": "Writing tests", "value": 0.4 }
  ]
}
```

- `id`: stable per item (a session, a job, a build). HubDeck names it `<source>.<id>`
  (`launcher.1`); a key shows one item, the dial can count them (`launcher.*`).
- `state`, one of the shared scale below; anything else shows as text in a neutral color.
- `label` (optional): the item's name; `text` (optional): one short line about now;
  `value` (optional): a number (a progress 0..1, a count).

| state | means | on a key |
|---|---|---|
| `waiting` | it needs the user | orange, a hand; the deck's edge pulses orange |
| `working` | busy, nothing to do | blue, turning arrows |
| `done` | finished, worth a look | green, a check |
| `error` | failed | red, a cross |
| `idle` | nothing going on | grey dot |
| `offline` | not running / unreachable | dim, a dash |

HubDeck reads it when a key or the dial shows the app (every 5 seconds at the fastest) unless the
app has an event stream.

## 2. Changes as they happen: `GET /hubdeck/events`

A Server-Sent Events stream (`Content-Type: text/event-stream`). Each event carries the items
that changed, in the same shape:

```
data: {"items":[{"id":"1","state":"working","text":"Back to work"}]}

```

HubDeck subscribes while the app is connected, reconnects when the stream drops, and reads
`/hubdeck/status` once after each (re)connect. Any other program may subscribe too.

## 3. What keys can do: `GET /hubdeck/manifest`

The app describes itself once; HubDeck and its assistant build keys from it:

```json
{
  "app": "Agent Desk",
  "exe": "agent-desk.exe",
  "status": "http://127.0.0.1:4317/hubdeck/status",
  "events": "http://127.0.0.1:4317/hubdeck/events",
  "actions": [
    { "id": "focus", "label": "Focus agent", "icon": "bot", "method": "POST", "url": "http://127.0.0.1:4317/agents/{id}/focus" },
    { "id": "new", "label": "New agent", "icon": "plus", "method": "POST", "url": "http://127.0.0.1:4317/agents" },
    { "id": "open", "label": "Open", "icon": "app-window", "link": "agent-desk://open" }
  ]
}
```

- An action is an HTTP call (`method`, `url`, optional `body`) or a link (`link`: a deep link
  the app registered, opened like any address). `{id}` in a URL is the item's id when the key
  shows an item.
- `icon`: one of HubDeck's icon names (`list_icons`), optional.
- A manifest may also live in a file (`hubdeck.json`) next to the app; then the user picks it.

## 4. No server at all: push

A script that cannot serve HTTP can tell HubDeck directly (HubDeck's local API, its port in
`%APPDATA%\dev.hubdeck.app\api-http-port`, its token in `api-token`):

```
curl -X POST http://127.0.0.1:<port>/api/state -H "Authorization: Bearer <token>" \
  -d '{"source":"backup","items":[{"id":"nightly","state":"done","text":"12 GB"}]}'
```

## 5. Connecting an app in HubDeck

**By itself (recommended):** when the app starts, it announces its manifest, and HubDeck connects
it without a click and says so in its window:

```
POST http://127.0.0.1:<port>/api/apps/announce   Authorization: Bearer <token>
{"url": "http://127.0.0.1:4317/hubdeck/manifest"}
-> {"ok": true, "id": "example-app", "new": true}
```

`<port>` is in `api-http-port` and `<token>` in `api-token` in HubDeck's data folder
(`%APPDATA%\dev.hubdeck.app\` on Windows, `~/Library/Application Support/dev.hubdeck.app/` on
macOS). If the files are missing or the call fails, HubDeck is not running: carry on quietly. An
app already connected is refreshed (a changed manifest is taken in); an app the user disconnected
is refused (`400`) until they add it again by hand. `scripts/example-app.py` shows it.

**By hand:** Settings > Connections > Your apps > Add: the address of its manifest (or of its
status). The deck assistant can do it too (`connect_app`), for addresses on this computer.

## 6. A prompt to add this to your app (for Claude Code in the app's repo)

> Add a HubDeck integration to this app, following https://github.com/<owner>/hubdeck/blob/main/docs/APP-CONTRACT.md:
> serve on the app's existing local port (or a new one on 127.0.0.1 only) `GET /hubdeck/status`
> with one item per <session / job / thing worth seeing>, using the shared states (waiting when
> it needs the user, working, done, error, idle, offline), `GET /hubdeck/events` as Server-Sent
> Events with the items that changed, and `GET /hubdeck/manifest` listing the app's useful
> actions as HTTP calls or deep links. On start, announce the manifest to HubDeck
> (`POST /api/apps/announce`, section 5; quiet when HubDeck is not running). Keep it read-only
> except for the actions; no auth beyond binding to 127.0.0.1. Add a test that the three endpoints
> answer in the contract's shapes.
