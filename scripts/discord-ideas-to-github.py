"""The week's top three ideas in #ideas that are not on GitHub yet become GitHub issues: the issue
quotes the idea and links its post, the post gets the "planned" tag and a reply with the issue's link.

    python -I scripts/discord-ideas-to-github.py          # dry run: what would be made
    python -I scripts/discord-ideas-to-github.py --go     # make them (after the owner's OK)

An idea is on GitHub already when its post or a reply links an issue, pull request or discussion of
the repository, or it has a status tag (planned, in progress, done, not planned). Needs `gh` signed in.
"""

import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import discord_api as d  # noqa: E402

REPO = "Przemotar/hubdeck"
LINK = re.compile(r"github\.com/Przemotar/hubdeck/(issues|pull|discussions)/\d+", re.I)
STATUS = {"planned", "in progress", "done", "not planned"}
HOW_MANY = 3
MAX_TAGS = 5


def gh_issue(title: str, body: str) -> str:
    out = subprocess.run(
        ["gh", "issue", "create", "-R", REPO, "--title", title, "--body", body],
        capture_output=True, text=True, encoding="utf-8", check=True,
    ).stdout
    return out.strip().splitlines()[-1]


def candidates(api: d.Discord, guild_id: str, forum: dict) -> list[dict]:
    status = {t["id"] for t in forum.get("available_tags", []) if t["name"].lower() in STATUS}
    out = []
    for t in api.threads(guild_id, forum):
        if status & set(t.get("applied_tags", [])):
            continue
        msgs = api.messages_after(t["id"], 0)
        if any(LINK.search(m["content"]) for m in msgs):
            continue
        starter = next((m for m in msgs if m["id"] == t["id"]), None)
        if not starter:
            continue
        votes = next((r["count"] - (1 if r.get("me") else 0) for r in starter.get("reactions") or [] if r["emoji"].get("name") == "👍"), 0)
        if votes > 0:
            out.append({"thread": t, "starter": starter, "votes": votes})
    out.sort(key=lambda c: (-c["votes"], int(c["thread"]["id"])))
    return out[:HOW_MANY]


def issue_body(c: dict, url: str) -> str:
    quoted = "\n".join("> " + line for line in c["starter"]["content"].strip().splitlines()) or "> (no text)"
    author = c["starter"]["author"].get("global_name") or c["starter"]["author"]["username"]
    return f"An idea from HubDeck's Discord by {author}, {c['votes']} 👍:\n\n{quoted}\n\nThe post and its thread: {url}"


def run(api: d.Discord, go: bool, make_issue=gh_issue, out=print) -> list[str]:
    guild = api.the_guild()
    forum = api.channel_named(guild["id"], "ideas")
    planned = api.tag_id(forum, "planned")
    made = []
    picked = candidates(api, guild["id"], forum)
    if not picked:
        out("no idea with votes waits for GitHub")
    for c in picked:
        t = c["thread"]
        url = f"https://discord.com/channels/{guild['id']}/{t['id']}"
        if not go:
            out(f"would open an issue: \"{t['name']}\" ({c['votes']} 👍) {url}")
            continue
        issue = make_issue(t["name"], issue_body(c, url))
        tags = [x for x in t.get("applied_tags", []) if x != planned][: MAX_TAGS - 1] + ([planned] if planned else [])
        api.patch(f"/channels/{t['id']}", {"applied_tags": tags})
        api.post(f"/channels/{t['id']}/messages", {
            "content": f"This idea is now a GitHub issue: {issue}\nThe work and the decisions happen there; you can follow it or comment on it.",
        })
        made.append(issue)
        out(f"opened {issue} for \"{t['name']}\" ({c['votes']} 👍), the post tagged planned")
    return made


def main() -> None:
    d.utf8_stdout()
    run(d.client(), go="--go" in sys.argv)


if __name__ == "__main__":
    main()
