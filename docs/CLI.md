# HubDeck from a terminal

The HubDeck program also takes commands, for scripts, shortcuts and other tools. They talk to the
HubDeck that is running (its local API, see `docs/PLUGINS.md`), so start HubDeck first.

```
hubdeck cli status                     the active and the home profile, the modules attached
hubdeck cli profiles                   every profile (* active, ^ home) with its id
hubdeck cli profile <name or id>       show that profile now (it becomes the home profile)
hubdeck cli key <1-15> --label TEXT    change a key's label in the active profile
hubdeck cli key <1-15> --icon NAME     ... or its icon (names: Settings > the icon picker)
hubdeck cli key 3 --label Mute --page tools   a key on another page
hubdeck cli lights <rainbow|color|profile|off> [#rrggbb]   the deck-wide light style
```

Keys are numbered 1-15 left to right, top to bottom. The exit code is 0 when the command worked
and 1 otherwise (the reason is printed). On Windows the program is
`C:\Program Files\HubDeck\hubdeck.exe` (or where you installed it); on macOS
`/Applications/HubDeck.app/Contents/MacOS/hubdeck`.

Examples:

```
# a scheduled task that switches to a focus profile in the morning
hubdeck cli profile "Night focus"

# show a build result on key 15
hubdeck cli key 15 --label "Build OK" --icon check
```
