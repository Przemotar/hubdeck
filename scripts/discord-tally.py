"""The week's top ideas (👍 in #ideas) and presets (⭐ in #presets) as a draft post for
#announcements. Printed only; --post posts it (after the owner's OK) and keeps this week's counts in
`logs/discord-tally.json`, so next week's draft counts the votes gained since.

    python -I scripts/discord-tally.py
    python -I scripts/discord-tally.py --post
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import discord_api as d  # noqa: E402

SNAPSHOT = d.ROOT / "logs" / "discord-tally.json"
TOP = 5
CLOSED = {"done", "not planned"}


def count(message: dict, emoji: str) -> int:
    for r in message.get("reactions") or []:
        if r["emoji"].get("name") == emoji:
            return r["count"] - (1 if r.get("me") else 0)
    return 0


def posts(api: d.Discord, guild_id: str, forum: dict, emoji: str) -> list[dict]:
    """A forum's open posts with their votes: id, name, link, votes."""
    closed = {t["id"] for t in forum.get("available_tags", []) if t["name"].lower() in CLOSED}
    out = []
    for t in api.threads(guild_id, forum):
        if closed & set(t.get("applied_tags", [])):
            continue
        try:
            starter = api.get(f"/channels/{t['id']}/messages/{t['id']}")
        except d.DiscordError:
            continue
        out.append({
            "id": t["id"], "name": t["name"], "votes": count(starter, emoji),
            "url": f"https://discord.com/channels/{guild_id}/{t['id']}",
        })
    return out


def ranked(items: list[dict], before: dict) -> list[dict]:
    for p in items:
        p["gained"] = p["votes"] - before.get(p["id"], 0) if before else p["votes"]
    return sorted((p for p in items if p["votes"] > 0), key=lambda p: (-p["gained"], -p["votes"], p["name"]))[:TOP]


def draft(ideas: list[dict], presets: list[dict], before: dict) -> str:
    lines = ["**This week on the HubDeck Discord**", ""]
    for title, emoji, items in (("Ideas with the most votes", "👍", ideas), ("Presets rated highest", "⭐", presets)):
        top = ranked(items, before)
        lines.append(f"{title}:")
        if not top:
            lines.append(f"no {emoji} yet.")
        for i, p in enumerate(top, 1):
            gained = f" (+{p['gained']} this week)" if before and p["gained"] > 0 else ""
            lines.append(f"{i}. {p['name']}: {p['votes']} {emoji}{gained} {p['url']}")
        lines.append("")
    lines.append("Every week the three ideas with the most 👍 that are not on GitHub yet become GitHub issues.")
    return "\n".join(lines)


def main() -> None:
    d.utf8_stdout()
    post = "--post" in sys.argv
    api = d.client()
    guild = api.the_guild()
    channels = api.channels(guild["id"])
    ideas = posts(api, guild["id"], api.channel_named(guild["id"], "ideas", channels), "👍")
    presets = posts(api, guild["id"], api.channel_named(guild["id"], "presets", channels), "⭐")
    before = json.loads(SNAPSHOT.read_text(encoding="utf-8")) if SNAPSHOT.exists() else {}
    text = draft(ideas, presets, before)
    print(text)
    if post:
        channel = api.channel_named(guild["id"], "announcements", channels)
        msg = api.post(f"/channels/{channel['id']}/messages", {"content": text, "allowed_mentions": {"parse": []}})
        if channel["type"] == d.ANNOUNCEMENT:
            api.post(f"/channels/{channel['id']}/messages/{msg['id']}/crosspost")
        SNAPSHOT.parent.mkdir(exist_ok=True)
        SNAPSHOT.write_text(json.dumps({p["id"]: p["votes"] for p in ideas + presets}), encoding="utf-8")
        print(f"\nposted to #{channel['name']}")


if __name__ == "__main__":
    main()
