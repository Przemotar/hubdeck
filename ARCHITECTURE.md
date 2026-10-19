# Architecture

Open-source desktop software for the Cooler Master MasterHUB modular controller, for Windows and macOS.
Product name: **HubDeck** (chosen on 2026-10-09; the name must not use "MasterHUB" or
"Cooler Master", the description may say "for the Cooler Master MasterHUB"). Tauri identifier
`dev.hubdeck.app`, Rust package `hubdeck`. The repo folder and the private GitHub repo are still
called `masterhub`; the public repo will be a clean export named `hubdeck`.

## Goals

1. Replace the official app: every module works, settings persist, nothing heavy runs in the background.
2. Customizable by hand (a settings window) and by code (plugins), with the same plugin model for both.
3. Scriptable by agents: a local API, a CLI and an MCP server can drive the deck and read its state.
4. Shippable: signed installers (`.msi`, `.dmg`), auto-update, CI, docs.

## Layout

```
crates/
  mh-protocol   HID protocol: module types, report encoding/decoding, image chunking. No I/O; unit-tested.
  mh-device     hidapi transport, hot-plug hub (enumerate, init, reconnect), event stream, release on exit.
app/            Tauri 2 shell (src-tauri) + React/TypeScript UI (settings window, tray).
plugins/        Built-in plugins (actions) shipped with the app.
prototype/      Python prototype used to discover and verify the protocol on hardware.
PROTOCOL.md     The protocol as verified on real hardware (source of truth for mh-protocol).
```

## Runtime

```
 modules (USB HID) ──> mh-device hub ──> engine ──> profile (controls -> actions)
                         ^  events         │  │
                         └── draw/LED ─────┘  ├─> plugin host (WebSocket, one process per plugin)
                                              ├─> local API (HTTP + WebSocket on 127.0.0.1) <── CLI, MCP, hooks
                                              └─> settings UI (Tauri IPC)
```

- **Hub**: one reader thread per module; a read error means the module was pulled off the base. The hub
  re-enumerates every second, re-initializes returning modules (software mode on keys and dial) and asks
  the engine to redraw them. On exit, sleep and settings changes it hands keys and dial back to hardware mode.
- **Engine**: owns the active profile, routes events to actions, renders key/panel/dial images, drives LEDs.
  All HID I/O runs on blocking threads, never on the async runtime or the UI thread.
- **Profiles**: JSON files in the app data dir, written atomically (temp file + rename), versioned with a
  `"version"` field and forward migrations. A profile can target a layout (which modules are attached), so
  swapping modules switches profiles automatically.
- **Plugins**: separate processes with a `manifest.json` (actions, their parameters as JSON Schema, icons).
  The settings UI builds each action's form from its schema. The wire protocol follows OpenAction / the
  Stream Deck SDK where the concepts match (keys, dials) and extends it for faders, rollers, LEDs and the
  full-panel screen. Users keep private plugins in their own plugins folder, outside this repo.
- **Local API**: HTTP + WebSocket on 127.0.0.1 with a per-install token. Used by the CLI, the MCP server and
  by tools such as Claude Code hooks to push state onto the deck.

## Safety

The module firmware accepts commands that rewrite the serial number, save factory data, reset the module or
flash firmware. `mh-protocol` cannot encode them: they are not part of its API, and the raw-report escape
hatch refuses those command bytes. Firmware updates are out of scope.

## Legal

The protocol implementation is our own code, written from the modules' captured behaviour, the
MIT-licensed `cooltek-sdk-probe` notes and an analysis of the vendor's SDK for interoperability;
none of the vendor's code is copied. The repo never contains Cooler Master binaries,
firmware, SDK DLLs, images or fonts.

## Milestones

1. Prototype (Python): every module verified on hardware. Done 2026-10-09.
2. `mh-protocol` + `mh-device` in Rust, verified on hardware with the same tests as the prototype.
3. Tauri app: tray, deck view mirroring the attached modules, profiles, built-in actions, live preview.
4. Plugin host + local API + CLI + MCP; first private plugin (agent fleet dashboard).
5. macOS pass, installers, auto-update, CI, docs, public release.
