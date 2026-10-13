# Contributing to HubDeck

Thanks for helping. Issues and pull requests are welcome.

## Reporting a problem

Please include:

- your system (Windows or macOS version) and the HubDeck version;
- which modules are attached and where (a photo helps);
- what you did, what you expected and what happened;
- the log: tray menu > "Open log folder", file `hubdeck.log`.

If a key module or the dial stops showing pictures and the window says it is not responding, pull
it off the base and put it back (or use "Restart module" on Windows). Please tell us how it
happened: it is a firmware problem we work around, and every report helps.

## Sharing slot data

HubDeck knows where a module sits from the USB port it uses. Most positions come from a table that
was checked on real hardware for some slots only. If "Arrange modules" shows a module in the wrong
place or turned, arrange it by hand and open an issue with the module kind, the slot shown under it
(e.g. `keys · 2-3`) and the position and rotation you set.

## Code

- Rust: `cargo fmt`, `cargo clippy`, and `cargo test -p mh-protocol -p mh-device -p hubdeck` must
  pass. HID I/O runs only on its own threads, never on the UI thread or the async runtime.
- TypeScript: `app/node_modules/.bin/tsc --noEmit -p app` and `pnpm test` (in `app/`) must pass.
- Every user-facing text goes through `app/src/i18n.ts` (English first; other languages fall back
  to English).
- Never add code that sends firmware update, serial number, factory data, factory reset or flash
  save commands. Never read a feature report from the keys or the dial while they are in hardware
  mode: it wedges their firmware until the module is reseated (see `PROTOCOL.md`).
- Do not commit Cooler Master software, firmware, SDK files, images or fonts.

## Plugins

Plugins live outside this repository; see [docs/PLUGINS.md](docs/PLUGINS.md). A plugin that many
people would use can become a built-in action later.
