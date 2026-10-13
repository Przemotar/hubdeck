# Privacy

HubDeck has no account, no cloud and no telemetry. Everything it keeps stays on your computer, in
its data folder (`%APPDATA%\dev.hubdeck.app` on Windows, `~/Library/Application Support/dev.hubdeck.app`
on macOS).

What leaves your computer, and only when you use the feature:

| Feature | What is sent, and where |
|---|---|
| Checking for updates (on by default, Settings > Updates) | a request for the newest release to GitHub |
| Shared profiles gallery | a download of the gallery list from GitHub |
| Weather, calendar, live values | requests to the addresses you set (Open-Meteo for weather) |
| OBS, Home Assistant, Streamlabs, Spotify, Twitch | requests to those apps or services, with the access you gave them |
| Stream Deck plugins | whatever each plugin does; they are separate programs |
| The deck assistant | your messages, and with your OK a list of the apps you used most, go to Anthropic through your own Claude Code; nothing is sent until you start a talk |
| Report a problem | a GitHub issue form opens in your browser; you choose what to send |

Counting which keys you press (for the assistant) is off until you switch it on, stays on your
computer, and switching it off forgets it. Claude Code sessions on keys send only their state, the
project folder's name and the tool's name to HubDeck, never your prompts.
