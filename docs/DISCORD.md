# The project's Discord: the plan

The owner's ask (2026-10-30, session 39, through his boosted prompt): a place that gathers the people
who downloaded HubDeck, where they propose what to change and add, rate the living deck's presets and
talk to each other; a lever for the project (more users, more reports and opinions, people bringing
others), run by me so it does not eat his time: he gets digests and approves what goes out in his
name. His picks: "1 a" (five channels to start), "2 a" (ideas voted on Discord, the top three a week
moved to GitHub, decisions on GitHub), "3 a" (cloud tasks as files in `private/cloud-tasks/`).
Earlier: Discord before the living deck's release ("1a", 2026-10-26); the reading side and the
voice-dispatch routine in `docs/ROADMAP.md` > Discord for the project. The research behind this
(facts, sources, what is least sure): `private/reports/2026-10-30-discord-i-chmura.md`, page
https://claude.ai/artifact/8L4hQKMuzeCgEGXEx9SFs3.

## Why it is small

Measured 2026-10-30 (GitHub API): 67 unique visitors to the repo in 14 days (413 views), 19 of them
from pepper.pl, 12 from GitHub, 5 from the site; 9 stars, 6 issues, 4 discussions, about 5 people who
already write to the project. A dead server harms more than no server, so it opens with five
channels and content on the first day.

Whether it lives does not depend on the layout but on three things:
1. something is on it the day it opens (every preset with its moving picture, the open requests from
   GitHub in #ideas, the 0.2.0 announcement, a pinned word from the owner);
2. someone answers within a day (said in #help);
3. one fixed thing happens every week (an announcement, the votes' results, a new preset or effect).

## Channels

Layout A (his pick, 2026-10-30: "Wolę chyba opcjęA, bez kanały polskiego (cały discord po angielsku)";
the three layouts: https://claude.ai/artifact/NaoDycrtJwiMCFDcgLMKcn). The whole server is in English.
Names carry an emoji and "┃"; the scripts and texts use the bare name (`discord_api.bare`).

```
HubDeck
📌 START HERE
├─ 📜┃rules           rules (Community needs it), one line on what HubDeck is, links: download,
│                    report a bug, suggest; read-only
└─ 📣┃announcements   releases, "what's new this week"; read-only; others can follow it
💬 COMMUNITY
├─ 💬┃general         talk, everything else
└─ 📸┃setups          FORUM, gallery: photos and videos of decks; tags Windows, macOS
🎛️ HUBDECK
├─ 💡┃ideas           FORUM: one idea a post; 👍 is the vote; tags anyone sets: preset, feature,
│                    plugin, look, hardware; status tags only we set: planned, in progress, done,
│                    not planned
├─ 🎨┃presets         FORUM, gallery: one post per preset (moving picture, a line, Deck look > …);
│                    ⭐ is the rating
├─ 🧩┃plugins         FORUM: Stream Deck plugins in HubDeck; tags request, question, made one
└─ 🆘┃help            FORUM: questions; tags Windows, macOS, solved; pinned: "A bug? GitHub issues,
                     with the diagnostics file"
🔒 STAFF
└─ moderator-only    Discord's Community updates and AutoMod alerts; private
```

Onboarding is on (mode 0): all eight open channels are default channels (Discord asks for at least
7, 5 of them open to write), so there is no question on entry: with every channel already default a
question would add nothing. The Server Guide (welcome sign, 3-5 new-member to-dos) is not in the
documented API: his step in Server Settings > Onboarding, with my texts. Resource pages there take
read-only channels off the channel list (Discord's Server Guide article), so none for now.
Later: #themes for the community gallery (living deck stage 5). No #polski (his word above).

## Roles

- **Owner** (him), **HubDeck Bot** (the bot, his "1 a" 2026-10-30), **Moderator** (nobody yet).
- **Contributor**: given by hand to authors of reports and ideas that went in.
- **Beta tester**: once there are builds to try before a release.

## Rules (English, his approval before they go up)

Be kind; English in the main channels; bugs go to GitHub (link); no ads and no downloads from
anywhere but GitHub; HubDeck is not affiliated with Cooler Master; **the HubDeck bot reads messages to
make summaries for the maker, and the summaries are made with the help of AI** (the README promises
"no account, no cloud, no telemetry"; hiding this would go against the project).

## Looks

- Name "HubDeck" (never "MasterHUB" or "Cooler Master" in a name); description "Free, open-source
  app for the MasterHUB deck".
- Icon: the app's (`site/images/icon.png`). Banner and invite background need boosts (levels 2 and
  1), so none at the start; `site/images/og.png` in the first announcement.
- Colour `#6e56ff` (the README's release badge) in the bot's embeds.
- Tone as the README: plain, no marketing, no emoji in every line; posts in his name start from his
  points, without the marks of AI text (CLAUDE.md).
- Address: a permanent invite behind our own page, `przemotar.github.io/hubdeck/discord` (a
  redirect), one address for README, app and notes; discord.gg/hubdeck would need 14 boosts.

## The bot

| | |
|---|---|
| Reads | new messages and posts, 👍 and ⭐, new members: at each session (`scripts/discord-new.py`, named in my first message like GitHub's) and once a day (the voice-dispatch routine, a morning digest to his phone) |
| Writes | only what he approved: release announcements (`scripts/discord-announce.py` through a webhook), "what's new this week", the weekly results, replies started from his words |
| Once | sets the server up from `discord/server.json` (`scripts/discord-setup.py`: channels, forums with tags and reactions, roles, permissions, the webhook); a second run changes nothing |
| Weekly | `scripts/discord-tally.py`: the top ideas (👍) and presets (⭐); `scripts/discord-ideas-to-github.py`: the top three ideas become GitHub issues linked both ways, their Discord status "planned" |
| Never | chats by itself, answers by itself, bans (AutoMod and he do), runs 24 h (no hosting) |
| Token | Windows Credential Manager on the PC (`scripts/secret.py`, a hidden prompt to set it), never in the cloud or the repo |

The bot needs the Message Content intent: below 10,000 users of the app it is a switch in the
Developer Portal, no review (https://support-dev.discord.com/hc/articles/40281523410967).

## Ideas and votes (his "2 a")

#ideas is where people propose and vote; GitHub stays where decisions and work live. Every week the
three ideas with the most 👍 not yet on GitHub become issues (link to the post), and the post gets
"planned" with the issue's link. On opening day the open requests from GitHub (#7 several MasterHUBs,
#10 scaling and state) are posted to #ideas so the forum is not empty and their authors can vote.

## Preset ratings

#presets has a post for every preset (the 92 moving thumbnails in `app/public/looks/` exist), ⭐ as
the rating, the weekly ranking in the announcement. The app does not collect ratings itself ("no
telemetry"); it may link "Rate presets on Discord" next to the presets (a window change: sketch to
his "ok" first). Later, one of the things (not the axis): a preset idea from the community made by
the skill's agent, shown in #presets, into a release with its author named.

## Bringing people

- The app: "Join the Discord" in Help and feedback beside GitHub (today every way to report needs a
  GitHub account); a link by the presets in Deck look. Sketch to his "ok" first.
- README: a Discord badge beside release and CI; Discord beside Discussions in "Questions, setups,
  profiles".
- Releases: a link at the end of the notes; the webhook posts them to #announcements.
- GitHub: a pinned discussion "We have a Discord"; #help linked in the issue template for questions.
- pepper.pl (19 of 67 visitors) and Reddit (`private/announce/drafts.md`): with 0.2.0, from his
  points (his rule on posts in his name).
- The site: a Discord button.

## Running it

- Moderation: AutoMod (spam, mass mentions, scam links), verified e-mail to write, no @everyone for
  members; he is the owner, I read and draft.
- Replies as on GitHub: the original, his words in Polish, I check and translate, sent on his OK;
  simple ones (a FAQ link, "that is a bug, report it here") as ready drafts he approves with one word.
- Knowledge does not get lost: Google does not index Discord, so good answers from #help go to a FAQ
  on GitHub once a week.
- His time: ~10 minutes once, ~5 minutes a week.

## Who does what

| Who | What |
|---|---|
| owner | creates the server, switches Community on, makes the bot in the Developer Portal, invites it, approves the texts, weekly approvals, his points for Pepper and Reddit |
| cloud (`private/cloud-tasks/01-discord.md`) | `discord/server.json`, the scripts above with tests on a fake API, `scripts/secret.py`, the texts (rules, welcome, channel descriptions, pinned messages, the first announcement, one line per preset) |
| me on the PC | run the scripts with the token, post the presets and the ideas, README/site/notes links, the app's link after his sketch "ok", digests, drafts, the weekly rhythm |

## Order (before 0.2.0)

1. He: the server and the bot (steps in `private/cloud-tasks/00-START.md`), ~10 minutes.
2. Cloud, at the same time: task 01 on its branch.
3. Me: merge, a dry run, the setup run; he reads the texts on the server, "ok" or corrections.
4. Me: presets and ideas posted; the bot's rights cut to what it needs.
5. Me: README, site, notes links; the app's "Join the Discord" after his "ok" on the sketch (0.2.0 or
   0.2.1).
6. 0.2.0: the announcement, the link in the notes, his points to Pepper and Reddit.
7. Week 1: the rhythm and digests; after two weeks the numbers (members, posts, votes).

## Least sure

Whether anyone comes at 67 visitors in two weeks (checked by numbers after two weeks); Onboarding's
7/5 rule and the boost levels come from guides, not Discord's own pages; whether a bot can open
Discord's own polls (not known).

## Done on the server (2026-10-30, session 40)

- `discord-setup.py` run with the bot's token (26 changes; a second run: nothing to change).
- The server's own defaults deleted on his "2 a": the categories "Kanały tekstowe" and "Kanały
  głosowe", the text channel "ogólny" (only the bot's join message in it) and the voice channel
  "Ogólne" (the server was made with a Polish client, so the defaults have Polish names). The system
  channel (join messages) moved from "ogólny" to #general first; not in `server.json`.
- `discord-seed.py`: 12 presets in #presets (6 without a moving picture until his review: Hollow Hill,
  Lantern Hollow, Mushroom hunt, Fuji and sakura, Koi pond, Tokyo Rain; a later run adds them) and #7,
  #10 in #ideas; a second run posts nothing.
- A permanent invite to #rules: `https://discord.gg/GMSAfNPEsb`, behind `site/discord/index.html`
  (`przemotar.github.io/hubdeck/discord`, public with the next export of the site).
- Not yet: his reading of the texts on the server, `--trim` (after his corrections: the setup needs
  Administrator to edit), README and app links, the announcement.
