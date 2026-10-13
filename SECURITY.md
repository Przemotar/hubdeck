# Security

## Reporting a vulnerability

Please do not open a public issue for a security problem. Write to przemotar+hubdeck@gmail.com
with what you found, how to reproduce it and what it affects. You will get an answer within a few
days; a fix is released as soon as it is ready, and you are credited unless you ask not to be.

## What HubDeck does that matters for security

- It talks to the MasterHUB modules over USB HID only. It never sends the firmware update, serial
  number, factory data, factory reset or flash save commands; they are refused in code
  (`crates/mh-protocol/src/safety.rs`).
- Its local API and plugin server listen on `127.0.0.1` only and need the token in
  `api-token` (in HubDeck's data folder) for anything that changes something.
- Plugins (HubDeck's own and those made for Stream Deck) are programs you install; they run with
  your user's rights, like any app. Install plugins you trust.
- Secrets (OBS, Home Assistant, Streamlabs) live in the system's credential store, never in files.
- The deck assistant runs Claude Code on your own account; it can change only draft profiles
  unless you switch on "Let it reach further", and then every command and web request waits for
  your OK. See [docs/ASSISTANT.md](docs/ASSISTANT.md).
