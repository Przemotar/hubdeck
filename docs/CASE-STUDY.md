# Case study: HubDeck, an open app for an orphaned controller

*Author: Przemysław Tarkowski (przemotar+hubdeck@gmail.com). October 2026.*

## The problem

The Cooler Master MasterHUB is a modular desk controller: a base with an edge light and magnetic
modules placed on it (15 LCD keys, a dial with a round 240x240 screen, knobs with LED rings, faders,
rollers). It was funded on Kickstarter in 2024 and shipped in 2025. The hardware is good; the
official software is not keeping up. Pieces promised to backers (an SDK, macOS support, more
integrations) have not arrived, and owners report lag, lost settings, profiles that switch slowly
and worry about long-term support.

Before writing code, the complaints were collected: **109 pain points from 170 sources** (reviews,
vendor forums, release notes, GitHub issues of comparable decks), each with how often it comes up
and what a fix would be ([PAIN-POINTS.md](PAIN-POINTS.md)). The official app was also read as
data, to know which actions and profile format owners already rely on
([OFFICIAL-APP.md](OFFICIAL-APP.md)).

## The approach

**Protocol first, verified on hardware.** The modules speak USB HID. The starting point was an
MIT-licensed community probe of the protocol; gaps were filled by disassembling the vendor's SDK
library and confirming every command on the real modules, one module kind at a time, with a Python
prototype. The result is written down in [PROTOCOL.md](../PROTOCOL.md) and is the source of truth
for the Rust protocol crate.

**Safety as a rule, not a hope.** The firmware accepts commands that rewrite serial numbers, reset
modules or flash firmware. They are not part of HubDeck's protocol code, and any hand-built report
carrying them is refused. Testing also found a firmware **wedge**: reading a feature report from the
keys or the dial while they are in hardware mode hangs the module until it is power-cycled. The
engine never does that read, detects a module that stops answering, and tells the user how to
recover it (restart the USB device, or reseat the module).

**The owner decides, the agent builds.** Every visual choice was made by the owner from two or three
complete variants (the window, the deck drawing, the base light), and every feature was tried on
his hardware before it was called done.

## Architecture

- `mh-protocol` (Rust): encoding and decoding of the module protocol, no I/O, unit-tested.
- `mh-device` (Rust): USB HID transport and a hot-plug hub. Each module has its own reader thread;
  whole images are sent under one lock.
- The engine (Rust, in the Tauri app): profiles, actions, gestures, the home-profile switching
  model, LEDs, screens (SVG drawn with resvg, the same SVG the window previews), integrations (OBS,
  Streamlabs, Spotify, Home Assistant, MIDI, Twitch, Claude Code), plugins over a local WebSocket,
  a local API, backups, diagnostics.
- The window (React): a live drawing of the deck as it sits on the desk, the action library and an
  inspector. It is created only while open, so the app costs almost nothing in the tray.

More in [ARCHITECTURE.md](../ARCHITECTURE.md).

## Measured results

| What | Before | After | How it was measured |
|---|---|---|---|
| Drawing the 15 keys on a profile switch | 3.3 s | 36 ms | Stage timing in the engine log. The cause was not USB speed: the image was sent as about 100 reports, and between each one the reader thread took the device lock for a full 20 ms read. Sending the whole image under one lock fixed it. |
| CPU while idle in the tray | 28.8 % of a core | 0.76 % of a core | Per-process and per-thread CPU over 30 s on the release build with the deck attached. Causes: a system-info call that cost 1.1 s of CPU each second, a USB rescan every second, a hidden web view. |
| Memory while idle | about 430 MB | 45 MB | Working set of the app and its web view processes, same run. The hidden window held 346 MB; it is now destroyed when closed. |
| Owner's verdict on switching | 3-4 s, timed by feel | "now switching is instant" | The installed release build on the owner's desk. |

| System volume from a knob on macOS | ~0.2 s a detent, lagging behind a fast turn | under 1 ms | AppleScript replaced by Core Audio; timed per call. |
| Window frames while hovering, macOS | 52 ms on average, every tenth over 100 ms | 17 ms | Playwright's WebKit recording `requestAnimationFrame` intervals; the deck drawing's SVG filters were repainted ten times a second by a colour animation. |

Also measured: each screen takes about 32 pictures per second, a full key-panel image is 11-36 ms
over USB.

## How it was built

HubDeck was built with **Claude Code**, Anthropic's coding agent, with the owner as product owner
and tester. The working rules were written into the repository so every session starts from the
same place: the state of the project, the owner's decisions with his words and dates, a log of
problems with their causes and fixes, and the protocol as verified.

From the third day, several agents worked in parallel on one repository: one on the Rust engine and
integrations, one on the design and the window, one coordinating and keeping the documentation
current. The owner answered questions, picked between variants and tested every build on the
hardware. Lessons from that setup (for example, that agents sharing one working tree must stage and
commit only their own changes, in one step) went straight into the knowledge log.

In numbers: a working app on the owner's deck within the first two days; then profiles, about 70 actions,
seven integrations, plugins, a local API, a redesigned window with a light and dark look, game and
creative presets, icon packs and profile sharing over the following days; the macOS port, Stream
Deck plugin support and the assistant within the first week; over 200 Rust tests, frontend tests and
a click-through test of the window on every change.

## Porting to macOS in a day

The first Mac session started from an app that had only ever run on Windows. It compiled on macOS
without a change; the day went into what a Mac does differently, each verified on the owner's deck:
module slots from IOKit's `locationID` (the same slot table as Windows), the installed apps and
their Finder icons, "now playing" through Spotify's and Music's scripting (the system's own is
private on macOS), volume through Core Audio, the Accessibility permission with a clear prompt, and
a module that sometimes stopped answering after the app restarted: traced to reading it too soon
after switching its mode, which the firmware punishes, and fixed by giving it time first.

## Hundreds of integrations without writing them

Owners of these decks already have favourite plugins: those made for Elgato's Stream Deck. HubDeck
speaks their protocol, written from Elgato's public SDK documentation: it starts Node, native and
HTML plugins, maps the 15 keys to a 5x3 keypad and the dial and knobs to dials, shows each plugin's
own settings page, and draws what the plugin sends (titles, pictures, dial layouts, even a song's
cover behind its progress ring). A Spotify plugin made for Stream Deck runs on the MasterHUB as if it
were made for it ([STREAMDECK-PLUGINS.md](STREAMDECK-PLUGINS.md)).

## An assistant that builds the deck with you

Most people never set up a deck well: too many choices, no idea what is possible. HubDeck's
assistant runs on the user's own Claude Code: with permission it looks at which apps they use, asks
a few questions, and sketches whole decks that can be tried on the hardware before one is kept; or
it goes control by control with a gallery of options to try. It can only change drafts; anything
else needs a click ([ASSISTANT.md](ASSISTANT.md)).

## What's next

- The first public release for Windows and macOS, with updates through GitHub Releases.
- The shared profile gallery, and polish of the deck drawing.
- Whatever owners ask for next, through the issue forms and Discussions.

---

*HubDeck is an independent project, not affiliated with or endorsed by Cooler Master. Source code:
MIT OR Apache-2.0.*
