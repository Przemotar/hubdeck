# Building profiles with an AI assistant (MCP)

HubDeck includes an MCP server so the user's own AI assistant (Claude Desktop, Claude Code or any
client of the [Model Context Protocol](https://modelcontextprotocol.io)) can design and build
profiles: functions per control, layout, hotkeys, pictures, animations, lights and pages. HubDeck
needs no AI key and sends nothing anywhere; the assistant talks only to HubDeck on the same
computer.

## How it works

```
assistant (Claude Desktop / Code)  --stdio, JSON-RPC 2.0-->  hubdeck --mcp
hubdeck --mcp  --HTTP 127.0.0.1, bearer token-->  the running HubDeck (local API, engine/api.rs)
```

- `hubdeck --mcp` is the same program started with one argument; it serves MCP on stdin/stdout
  (one JSON message per line) and exits when the client closes stdin. It opens no window and
  never touches the deck itself.
- Each tool call becomes a request to the running app's local HTTP API (`<data>/api-token`,
  `<data>/api-http-port`, data = `%APPDATA%\dev.hubdeck.app` on Windows,
  `~/Library/Application Support/dev.hubdeck.app` on macOS). The token and port are read on every
  call, so a restarted HubDeck is found again.
- When HubDeck is not running, tools answer with an error that says so (the assistant tells the
  user to start it). An older HubDeck without these routes is reported as too old.
- Protocol versions: `2025-11-25` (newest), `2025-06-18`, `2025-03-26`, `2024-11-05`; the server
  answers with the client's version when it knows it, else the newest. Capabilities: tools and one
  resource (`hubdeck://design-guide`).

## Connecting

Settings > Connections > **AI assistant (MCP)**:

- **Connect Claude Desktop**: adds `mcpServers.hubdeck = {"command": "<path to hubdeck.exe>",
  "args": ["--mcp"]}` to `%APPDATA%\Claude\claude_desktop_config.json`
  (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS). Other servers and
  settings stay; the old file is kept as `claude_desktop_config.json.hubdeck-backup`; a file that
  is not valid JSON is never overwritten. Restart Claude Desktop afterwards.
- **Connect Claude Code**: when the `claude` command line is found (PATH, `~/.local/bin`,
  `~/.claude/local`, npm), runs `claude mcp remove --scope user hubdeck` and
  `claude mcp add --scope user hubdeck -- "<path to hubdeck.exe>" --mcp`. Without it the button
  copies that command.
- **Copy JSON for other apps**: the `mcpServers` snippet for any MCP client.

By hand, for Claude Code:

```
claude mcp add --scope user hubdeck -- "C:\Program Files\HubDeck\hubdeck.exe" --mcp
```

## Tools

| Tool | What it does | HTTP route |
|---|---|---|
| `design_guide` | The design rules (`docs/PROFILE-DESIGN.md`, also resource `hubdeck://design-guide`) | (bundled) |
| `list_modules` | Module kinds attached and owned; controls of each kind | `GET /api/modules` |
| `list_actions` | Actions (compact; with `search`: with params), special keys, key styles, LED effects | `GET /api/catalog` |
| `list_icons` | Built-in icon names and game icons (`game:<name>`) | `GET /api/catalog` |
| `list_apps` | Installed apps (name, target, exe) and apps seen recently | `GET /api/apps` |
| `list_games` | Installed games (Steam, Epic, GOG) with launch targets | `GET /api/games` |
| `list_profiles` | Profiles, the active and the home one | `GET /api/profiles` |
| `get_profile` | One profile | `GET /api/profiles/<id>` |
| `create_profile` | A whole profile in one call; `replace` overwrites one (backup first) | `POST /api/profiles` |
| `update_profile` | Name, apps, colors, edge light, panel | `POST /api/profiles/<id>/update` |
| `set_control` / `clear_control` | One control's binding | `POST /api/profiles/<id>/controls` |
| `add_page` / `delete_page` | Key pages | `POST /api/profiles/<id>/pages`, `.../pages/<page>/delete` |
| `set_lights` | Profile edge light and color, knob and fader LEDs | `POST /api/profiles/<id>/lights` |
| `get_deck_lights` / `set_deck_lights` | Deck-wide lighting style, brightness, reactions | `GET` / `POST /api/lights` |
| `add_image` | Store an SVG or PNG/JPEG/GIF/WebP picture; returns the name for `look.image` | `POST /api/images` |
| `add_animation` | Frames (SVG or pictures) + fps -> animated GIF for a key (80x80), the dial (240x240) or the panel (480x272) | `POST /api/animations` |
| `preview_control` | The PNG the hardware gets for a key or the dial (and its SVG) | `POST /api/preview` |
| `activate_profile` | Show a profile now | `POST /api/profiles/<id>/activate` |

Everything an assistant sends is checked before it is saved (`api::check_profile`): control ids
(`keys.0`..`keys.14`, `dial`, `knobs.N`, `faders.N`, `rollers.N`, `:hold` / `:double`,
`<page>/keys.N`), known actions whose inputs fit the control, select params, hotkeys that parse,
known icons, stored pictures, `#rrggbb` colors, key styles and LED effects. Errors refuse the
whole change with the reasons; warnings (long labels, an LED on a key) come back with the result.

Limits: SVG up to 2 MB, pictures up to 15 MB, animations up to 120 frames at 1-15 fps and 15 MB,
request bodies up to 16 MB.

## The same API without MCP

Every route works with any HTTP client and the token:

```
curl -H "Authorization: Bearer $(cat "$APPDATA/dev.hubdeck.app/api-token")" \
     http://127.0.0.1:$(cat "$APPDATA/dev.hubdeck.app/api-http-port")/api/modules
```

## Trying it without the hardware

`hubdeck --mcp` can be driven by hand: write `initialize`, `notifications/initialized`,
`tools/list` and `tools/call` messages, one per line, to its stdin. The Rust tests in
`engine/mcp.rs` do this against a fake app, and an HTTP round trip against a fake local API.

## The deck while the assistant works (`--mcp-deck`)

A second, small server for using the deck during work, switched on per project (each tool costs
the assistant some context, so it is not global): in the project's folder run

```
claude mcp add --scope project hubdeck-deck -- "<path to hubdeck.exe>" --mcp-deck
```

(Settings > Connections > "Deck tools for a project" copies it.)

| Tool | What it does | HTTP route |
|---|---|---|
| `notify` | A short message on the dial for some seconds, the base edge breathing in a color | `POST /api/notify` |
| `show_status` | A status on the dial until changed or cleared (empty text clears) | `POST /api/status` |
| `ask_physical` | A question on the dial, the answers on the keys; returns the answer pressed (a dial press dismisses; it times out) | `POST /api/ask`, then `GET /api/ask/<id>` until answered |
| `read_controls` | The active profile and page, the values (0-100) faders, knobs and the dial last set | `GET /api/controls` |

`POST /api/ask` answers at once with an id and the tool looks every 0.4 s, so the local API never
waits on a person. While a question shows, key presses answer it instead of running their actions.
