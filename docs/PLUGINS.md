# Plugins and the local API

HubDeck runs plugins as separate programs and talks to them over a WebSocket on `127.0.0.1`. The
model follows the Stream Deck SDK where the concepts match (keys, dials) and adds what this deck
has (faders, the round dial screen). Any language works: a plugin is just a program that opens a
WebSocket.

## Installing a plugin

Put the plugin's folder into the plugins folder of the app data and restart HubDeck:

| System | Folder |
|---|---|
| Windows | `%APPDATA%\dev.hubdeck.app\plugins\<plugin>\` |
| macOS | `~/Library/Application Support/dev.hubdeck.app/plugins/<plugin>/` |

Its actions then appear in the action library under the plugin's name. A working example is in
`plugins/example-counter/` (Node 22 or later, no packages).

## manifest.json

```json
{
  "id": "com.example.counter",
  "name": "Counter",
  "version": "1.0.0",
  "command": "node plugin.js",
  "actions": [
    {
      "id": "count",
      "name": "Count presses",
      "description": "Shown under the action in the inspector.",
      "icon": "zap",
      "inputs": ["press", "turn", "slide"],
      "params": [
        { "key": "start", "label": "Start at", "type": "number", "default": 0, "min": 0, "max": 100 },
        { "key": "mode", "label": "Mode", "type": "select", "default": "a", "options": [["a", "First"], ["b", "Second"]] }
      ]
    }
  ]
}
```

- `id` and action ids: letters, digits, `.`, `-`, `_`. A binding stores `<plugin id>/<action id>`.
- `command` runs in the plugin's folder; a program in that folder wins over one on the `PATH`.
- `inputs`: `press` (keys, knob and dial pushes), `turn` (knobs, rollers, the dial), `slide` (faders).
- Param `type`: `text`, `multiline`, `number`, `select`, `app`, `target` (file or URL), `hotkey`.
  The inspector builds the form from them; the values arrive as `settings`.
- `icon`: one of the built-in icon names (see the icon picker).
- `permissions` (optional, top level): what the plugin uses, e.g. `["network", "files"]`. HubDeck
  shows it on the Plugins page so people know what they install; it does not sandbox plugins.

## Protocol

The plugin gets `HUBDECK_PORT`, `HUBDECK_PLUGIN` (its id) and `HUBDECK_TOKEN` in its environment,
connects to `ws://127.0.0.1:<port>` and first sends:

```json
{ "event": "register", "plugin": "<HUBDECK_PLUGIN>", "token": "<HUBDECK_TOKEN>" }
```

Every message is one JSON object with an `event`. A `context` is the control's id in the profile
(`keys.4`, `dial`, `knobs.1`, `<page>/keys.2`, with `:hold` or `:double` for those gestures).

From HubDeck to the plugin:

| event | fields | when |
|---|---|---|
| `willAppear` | action, context, settings | a control with this plugin's action is on screen (also right after registering) |
| `willDisappear` | action, context | it left (other profile or page) |
| `keyDown` / `keyUp` | action, context, settings | pressed / released |
| `dialRotate` | action, context, settings, ticks | turned; ticks > 0 clockwise or up |
| `slide` | action, context, settings, value | a fader moved, 0..100 |
| `state` | state | answer to `getState` |
| `error` | message | something the plugin sent was wrong |

From the plugin to HubDeck:

| event | fields | effect |
|---|---|---|
| `setTitle` | context, title | the key's label (null = back to the profile's) |
| `setImage` | context, image | the key's picture: a `data:image/png|jpeg|gif|webp|svg+xml;base64,...` URI |
| `setState` | context, state | true shows the key's "on" look (red by default, or the profile's second look) |
| `showFeedback` | value (0..1, optional), caption | shows a value ring or a caption on the dial for 1.5 s |
| `setLed` | context (`knobs.1`, `faders.0`, `base`), color (`#rrggbb`, null = back to the profile's) | lights one LED |
| `setPanelImage` | image (data URI, null = off) | one picture across all 15 keys (e.g. a game or a graph); keys still act |
| `switchProfile` | profile (id or name) | activates a profile |
| `getState` | | answers with `state` (profiles, active profile, page, app in front) |
| `playGames` | on (true/false) | starts or ends game mode on the deck |
| `log` | message | written to the HubDeck log |

## Local API

Scripts and tools (a CLI, Claude Code hooks, an MCP server) use the same server. The port is in
`<app data>/api-port` and the token in `<app data>/api-token` (created on first start; keep it
private). Register with `"plugin": "api"` and that token, then send any of the plugin messages
above with `"control"` (or `"context"`) naming the control, e.g.

```json
{ "event": "setTitle", "control": "keys.0", "title": "Build OK" }
{ "event": "setState", "control": "keys.0", "state": true }
```

An API client also hears every control of the deck, whatever it is bound to:

```json
{ "event": "input", "control": "keys.3", "kind": "down" }
{ "event": "input", "control": "dial", "kind": "turn", "value": -2 }
{ "event": "input", "control": "faders.0", "kind": "slide", "value": 64 }
```

`kind` is `down`, `up`, `turn` (value = detents) or `slide` (value 0..100). The control still does
its own action too.

What the API sets stays until it is changed again or HubDeck restarts.

### Over HTTP

The same messages work as plain HTTP requests, handy for scripts and hooks. The port is in
`<app data>/api-http-port`; send the token as `Authorization: Bearer <token>`. The path names the
event and the body holds its fields:

```sh
TOKEN=$(cat "$APPDATA/dev.hubdeck.app/api-token")
PORT=$(cat "$APPDATA/dev.hubdeck.app/api-http-port")
curl -X POST "http://127.0.0.1:$PORT/api/setTitle" -H "Authorization: Bearer $TOKEN" \
     -d '{"control": "keys.0", "title": "Build OK"}'
curl "http://127.0.0.1:$PORT/api/state" -H "Authorization: Bearer $TOKEN"
```

`POST` answers `202` once HubDeck has the message; `GET /api/state` answers with the state
(profiles, active profile, page, app in front, game mode).
