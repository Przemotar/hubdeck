## What this changes

<!-- What does the user see or get? One or two sentences. -->

## How it was checked

<!-- Tests run, and on hardware: which modules, which system. -->

- [ ] `cargo fmt`, `cargo clippy -p hubdeck --all-targets -- -D warnings`, `cargo test -p mh-protocol -p mh-device -p hubdeck`
- [ ] `app/node_modules/.bin/tsc --noEmit -p app` and `pnpm test` in `app/`
- [ ] Texts through `app/src/i18n.ts`
- [ ] No forbidden device commands (`crates/mh-protocol/src/safety.rs`)
