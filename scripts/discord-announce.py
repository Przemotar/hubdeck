"""Post a release's notes to #announcements through the server's webhook, as one embed in HubDeck's
colour with the release's link, and publish it to the servers that follow the channel.

    gh release view v0.2.0 -R Przemotar/hubdeck --json tagName,name,body,url > release.json
    python -I scripts/discord-announce.py --release release.json --dry-run
    python -I scripts/discord-announce.py --file discord/texts/announce-0.2.0.md --url <release url>

A file's first line `# Title` is the embed's title; `{RELEASE_URL}` in it becomes the link. A text
with a `{PLACEHOLDER}` left in it is refused. --dry-run prints what would be posted.
"""

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import discord_api as d  # noqa: E402

COLOUR = 0x6E56FF
LONGEST = 4000  # an embed's description holds 4096


def from_release(path: str) -> tuple[str, str, str]:
    r = json.loads(Path(path).read_text(encoding="utf-8"))
    return r.get("name") or r["tagName"], r["body"], r["url"]


def from_file(path: str, url: str) -> tuple[str, str, str]:
    first, _, rest = Path(path).read_text(encoding="utf-8").strip().partition("\n")
    return first.lstrip("# ").strip(), rest.strip().replace("{RELEASE_URL}", url), url


def embed(title: str, body: str, url: str) -> dict:
    left = re.findall(r"\{[A-Z_]+\}", title + body)
    if left:
        sys.exit("placeholders left in the text: " + ", ".join(sorted(set(left))))
    if len(body) > LONGEST:
        cut = body.rfind("\n", 0, LONGEST - 60)
        body = body[: cut if cut > 0 else LONGEST - 60].rstrip() + f"\n\n… the full notes: {url}"
    return {"title": title[:256], "url": url, "description": body, "color": COLOUR}


def announce(api: d.Discord, server: dict, title: str, body: str, url: str, out=print) -> dict:
    spec = server["webhook"]
    guild = api.the_guild()
    payload = {"embeds": [embed(title, d.linked(body, api.channels(guild["id"])), url)], "allowed_mentions": {"parse": []}}
    channel = api.channel_named(guild["id"], spec["channel"])
    hook = next((h for h in api.get(f"/channels/{channel['id']}/webhooks") if h["name"] == spec["name"]), None)
    if not hook:
        sys.exit(f"no webhook \"{spec['name']}\" in #{channel['name']}: run scripts/discord-setup.py first")
    if api.dry_run:
        out(f"would post to #{channel['name']} through \"{hook['name']}\":")
        out(json.dumps(payload, ensure_ascii=False, indent=2))
        return payload
    msg = api.request("POST", f"/webhooks/{hook['id']}/{hook['token']}", payload, query={"wait": "true"}, auth=False)
    if channel["type"] == d.ANNOUNCEMENT:
        api.post(f"/channels/{channel['id']}/messages/{msg['id']}/crosspost")
    out(f"posted: https://discord.com/channels/{guild['id']}/{channel['id']}/{msg['id']}")
    return msg


def main() -> None:
    d.utf8_stdout()
    argv = sys.argv[1:]
    opt = lambda name: argv[argv.index(name) + 1] if name in argv and argv.index(name) + 1 < len(argv) else None
    if opt("--release"):
        title, body, url = from_release(opt("--release"))
    elif opt("--file") and opt("--url"):
        title, body, url = from_file(opt("--file"), opt("--url"))
    else:
        sys.exit("usage: discord-announce.py (--release <gh json> | --file <notes.md> --url <release url>) [--dry-run]")
    announce(d.client(dry_run="--dry-run" in argv), d.load_server(), title, body, url)


if __name__ == "__main__":
    main()
