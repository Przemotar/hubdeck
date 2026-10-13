# Status board and live values

Keys and the dial can show values that refresh by themselves: anything a web address returns, a
Home Assistant sensor, a command's output, the weather and the next calendar event. Thresholds
color a key orange or red; on the dial a value with a maximum is drawn as a ring. The "Status
board" preset puts a set of them on the deck.

## The "Live value" action

Add it to a key or the dial (group System) and pick where the value comes from:

| From | Fields | Shows |
|---|---|---|
| A web address | Address (`https://...`), Value at (a JSON path such as `data.items.0.price` or `data.items[0].price`; empty for a plain-text answer), Token (optional) | The number or text at that path |
| The weather | Place (city search), °C or °F | Temperature in whole degrees and a short word (Clear, Cloudy, Rain, Snow, Storm...) |
| Next calendar event | Address: the calendar's secret ICS address | Time until the next event ("30 min", "4 h", "Fri"), its title; "Now" while it runs |
| Home Assistant | Entity (`sensor.living_room_temperature`) | The entity's state with its unit |
| A command | Command line | The first line of its output |

Common fields:

- **Refresh every (s)**: 5 s at the least (weather: 10 min, calendars: downloaded every 5 min and
  counted in between).
- **Before / After the value, Decimals**: `$` + `41250` + ` USD`; decimals -1 keeps the value as it
  comes (the weather uses whole degrees unless you set decimals).
- **Warning at / Alert at, Worse when**: the key turns orange from the warning value and red from
  the alert value; "Lower" for things like battery or free space. Empty = never.
- **Maximum**: on the dial the value is drawn as a ring of that maximum (CPU 100, a fan 2000 RPM).
- **Press opens**: a page or file a press opens; empty = a press refreshes at once.
- The key's own label becomes the small caption under the value.

Values that need something show "Set up" (no address, place or entity yet), "…" (waiting for the
first answer), "–" with the caption (could not be read; the log has the reason) or "Allow?" (a
command waiting for the user's yes).

### Tokens

A web address that needs a token gets it from the system's credential store (Windows Credential
Manager / macOS Keychain, entry `HubDeck` / `live:<name>`). Type it in the Token field and press
Save; the profile keeps only the entry's name, so exporting or sharing it never carries the token.
It is sent as `Authorization: Bearer <token>`.

### Commands

A shared profile must not run programs by itself, so a command runs only after the user allows it
on this computer: the key shows "Allow?" and a press allows that exact command line (listed in
`<app data>/live-commands.json`; delete a line there to take it back). Commands run through `cmd /C`
(Windows) or `sh -c` (macOS), without a window, for at most 10 s.

### Weather

From [Open-Meteo](https://open-meteo.com) (free, no account, no key): the Place field searches its
geocoding API and stores `Name|latitude|longitude`; a bare name is looked up once. Data is attributed
to Open-Meteo (CC BY 4.0).

### Calendars

Use the calendar's private ICS address:

- Google Calendar: Settings > (the calendar) > Integrate calendar > Secret address in iCal format.
- Outlook / Microsoft 365: Settings > Calendar > Shared calendars > Publish a calendar > ICS link.
- Apple iCloud: Calendar > share > Public Calendar (a `webcal://` link works).

What is understood: single and repeating events (daily, weekly on chosen days, monthly, yearly; with
an interval, a count or an end date), skipped and moved occurrences, cancelled events. All-day
events are left out (they are not meetings). Times with a time zone (`TZID`) are taken as local
time, which is right when the calendar and the computer are in the same zone. In the 5 minutes
before an event the key blinks orange once a second.

## How it works

- `engine/live.rs`: one worker thread fetches only the live values on screen (the keys of the page
  shown and the dial). It starts the first time a live value is shown, sleeps while none is, and
  each source keeps its own rhythm; a press refreshes at once. Results go to the engine as
  `IntegrationValue { key: "live:<hash of the source>", value: <JSON reading> }` and are drawn with
  the clock's big-number face (keys) or the timer face with a ring (dial).
- A reading is `{text, num, sub, unit, places, attention, error, locked}`; formatting and thresholds
  are applied when drawing, so editing the format needs no new fetch.

## The "Status board" preset

| Control | Shows | To set up |
|---|---|---|
| keys.0 | Time and date | - |
| keys.1 | Weather | Place |
| keys.2 | Next meeting | Calendar's ICS address |
| keys.3, 4, 9 | CPU, memory, GPU (System monitor) | - |
| keys.5 | Website (any text or JSON answer) | Address (e.g. a health check) |
| keys.6 | Build: `workflow_runs.0.conclusion` | Address, e.g. `https://api.github.com/repos/<owner>/<repo>/actions/runs?per_page=1` (a private repo needs a token) |
| keys.7 | Bitcoin in USD (CoinGecko, no key) | Works as it is; change the coin in the address |
| keys.8 | Free space on C: / the system disk (a command, orange under 50 GB, red under 20 GB) | Press once to allow the command |
| keys.10, 11 | Room temperature, phone battery (Home Assistant) | Home Assistant in Settings > Connections; the entity names |
| keys.12, 13 | Desk light on/off, Home Assistant in the browser | Entity, address |
| keys.14 | Now playing | - |
| dial | CPU with a ring | - |
| knobs | Volume, desk light brightness, monitor brightness | Entity |
| faders | System, music, browser, Discord, microphone | - |
| rollers | Scrolling | - |
