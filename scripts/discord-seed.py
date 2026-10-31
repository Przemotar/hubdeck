"""The Discord's opening-day posts: one #presets post for every preset the app lists in Deck look >
Presets (with its moving picture from `app/public/looks/<id>.webp` when the file is there), and an
#ideas post for each open GitHub request in IDEAS. A post that exists already is left alone, or gets
its picture when it was posted without one and the picture is there now.

    python -I scripts/discord-seed.py --dry-run
    python -I scripts/discord-seed.py

The presets come from the app's own sources: `THEMES` in `app/src/engine.ts` (with the upright copies
it adds), minus `HIDDEN` in `app/src/components/DeckLook.tsx`, named as `themeTitle` names them.
Needs `gh` signed in for the GitHub requests' titles.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import discord_api as d  # noqa: E402

APP = d.ROOT / "app"
LOOKS = APP / "public" / "looks"
IDEAS = [7, 10]
REPO = "Przemotar/hubdeck"
MAX_UPLOAD = 10 * 1024 * 1024  # a server without boosts takes files up to 10 MB


def string_list(source: str, name: str) -> list[str]:
    m = re.search(rf"{name}\s*(?::[^=]+)?=\s*(?:new Set\()?\[([^\]]*)\]", source)
    if not m:
        raise SystemExit(f"{name} not found in the app's sources: the seed's reader needs an update")
    return re.findall(r'"([^"]+)"', m.group(1))


def english(i18n: str) -> dict[str, str]:
    """The English texts (the first table in i18n.ts) of the presets."""
    out = {}
    for key, value in re.findall(r'^\s*"(effects\.(?:theme\.[^"]+|upright))":\s*"((?:[^"\\]|\\.)*)",', i18n, re.M):
        out.setdefault(key, json.loads(f'"{value}"'))
    return out


def bare(name: str) -> str:
    s = re.sub(r"^[^:：]+[:：]\s*", "", name)
    return s[:1].upper() + s[1:]


def presets(app: Path = APP) -> list[dict]:
    """The presets Deck look > Presets lists, in its order: id, name, help, group."""
    engine = (app / "src" / "engine.ts").read_text(encoding="utf-8")
    texts = english((app / "src" / "i18n.ts").read_text(encoding="utf-8"))
    block = engine[engine.index("export const THEMES"):]
    block = block[: block.index("\n];")]
    ids = re.findall(r'^\s{2,4}(?:\{ )?id: "([^"]+)"', block, re.M)
    upright, redrawn = string_list(engine, "UPRIGHT_THEMES"), string_list(engine, "REDRAWN_THEMES")
    for base in upright:
        if base in ids:
            ids.insert(ids.index(base) + 1, f"{base}-up")
    hidden = set(string_list(engine, "PRESETS_HIDDEN"))
    out = []
    for id_ in ids:
        if id_ in hidden:
            continue
        base = id_[:-3] if id_.endswith("-up") and id_[:-3] in upright else id_
        full = texts[f"effects.theme.{base}"]
        name = bare(full)
        if base != id_ and base not in redrawn:
            name = texts["effects.upright"].replace("{name}", name)
        out.append({
            "id": id_, "name": name, "help": texts.get(f"effects.theme.{base}.help", ""),
            "group": full.split(":")[0].split()[0] if ":" in full else "",
        })
    return out


def gh_issue(number: int) -> dict | None:
    r = subprocess.run(
        ["gh", "issue", "view", str(number), "-R", REPO, "--json", "title,url,state"],
        capture_output=True, text=True, encoding="utf-8",
    )
    return json.loads(r.stdout) if r.returncode == 0 else None


class Seed:
    def __init__(self, api: d.Discord, looks: Path = LOOKS, app: Path = APP, issue=gh_issue, out=print):
        self.api, self.looks, self.app, self.issue, self.out = api, looks, app, issue, out
        self.changes = 0

    def did(self, what: str) -> None:
        self.changes += 1
        self.out(("would " if self.api.dry_run else "") + what)

    def run(self) -> int:
        self.me = self.api.me()
        guild = self.api.the_guild()
        self.guild_id = guild["id"]
        channels = self.api.channels(guild["id"])
        self.seed_presets(self.api.channel_named(guild["id"], "presets", channels))
        self.seed_ideas(self.api.channel_named(guild["id"], "ideas", channels))
        self.out(f"{self.changes} post(s) {'planned (dry run)' if self.api.dry_run else 'made or changed'}" if self.changes else "nothing to post: every post is there")
        return self.changes

    def picture(self, id_: str) -> Path | None:
        p = self.looks / f"{id_}.webp"
        if not p.exists():
            self.out(f"note: no moving picture for {id_} (looks/{p.name} is missing): posted without one; a run after it is made adds it")
            return None
        if p.stat().st_size > MAX_UPLOAD:
            self.out(f"note: {p.name} is over 10 MB: posted without a picture")
            return None
        return p

    def existing(self, forum: dict) -> dict[str, dict]:
        return {t["name"]: t for t in self.api.threads(self.guild_id, forum) if t.get("owner_id") == self.me["id"]}

    def seed_presets(self, forum: dict) -> None:
        have = self.existing(forum)
        template = d.text("preset-post")
        for p in presets(self.app):
            content = template.replace("{help}", p["help"]).replace("{name}", p["name"])
            pic = self.picture(p["id"])
            thread = have.get(p["name"])
            if thread:
                if pic:
                    starter = self.api.get(f"/channels/{thread['id']}/messages/{thread['id']}")
                    if not starter.get("attachments"):
                        payload = {"attachments": [{"id": 0, "filename": pic.name}]}
                        self.api.request("PATCH", f"/channels/{thread['id']}/messages/{thread['id']}", payload, files=[(pic.name, pic.read_bytes())])
                        self.did(f"add the picture to \"{p['name']}\" in #presets")
                continue
            tag = self.api.tag_id(forum, p["group"]) if p["group"] else None
            body = {"name": p["name"], "message": {"content": content}, "applied_tags": [tag] if tag else []}
            files = None
            if pic:
                body["message"]["attachments"] = [{"id": 0, "filename": pic.name}]
                files = [(pic.name, pic.read_bytes())]
            self.api.post(f"/channels/{forum['id']}/threads", body, files=files)
            self.did(f"post \"{p['name']}\" in #presets" + ("" if pic else " (no picture)"))

    def seed_ideas(self, forum: dict) -> None:
        have = self.existing(forum)
        linked = {t["name"] for t in have.values()}
        for number in IDEAS:
            issue = self.issue(number)
            if not issue:
                self.out(f"note: GitHub #{number} could not be read (is gh signed in?): skipped")
                continue
            if issue.get("state", "OPEN").upper() != "OPEN":
                self.out(f"note: GitHub #{number} is closed: skipped")
                continue
            title, content = d.titled(f"idea-{number}")
            title = title.replace("{title}", issue["title"])[:100]
            content = content.replace("{url}", issue["url"]).replace("{title}", issue["title"])
            if title in linked:
                continue
            self.api.post(f"/channels/{forum['id']}/threads", {"name": title, "message": {"content": content}})
            self.did(f"post \"{title}\" in #ideas")


def main() -> None:
    d.utf8_stdout()
    Seed(d.client(dry_run="--dry-run" in sys.argv)).run()


if __name__ == "__main__":
    main()
