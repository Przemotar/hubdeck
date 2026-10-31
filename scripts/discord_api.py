"""The small Discord REST client the discord-*.py scripts share (API v10, standard library only).

The bot's token comes from `secret.py` (`discord-bot`). A dry run sends every GET and records, but
does not send, everything that would change something. Rate limits: a 429 waits `retry_after` and
tries again; a bucket with no requests left waits its `X-RateLimit-Reset-After` first.
"""

import json
import re
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from datetime import datetime, timezone
from pathlib import Path

API = "https://discord.com/api/v10"
USER_AGENT = "DiscordBot (https://github.com/Przemotar/hubdeck, 1)"
ROOT = Path(__file__).resolve().parent.parent
TEXTS = ROOT / "discord" / "texts"
SERVER_JSON = ROOT / "discord" / "server.json"
DISCORD_EPOCH_MS = 1420070400000

# channel types and forum flags (https://discord.com/developers/docs/resources/channel)
TEXT, CATEGORY, ANNOUNCEMENT, FORUM, PUBLIC_THREAD = 0, 4, 5, 15, 11
CHANNEL_TYPES = {"text": TEXT, "announcement": ANNOUNCEMENT, "forum": FORUM}
THREAD_PINNED = 1 << 1

PERMISSIONS = {
    "create_instant_invite": 0, "kick_members": 1, "ban_members": 2, "administrator": 3,
    "manage_channels": 4, "manage_guild": 5, "add_reactions": 6, "view_audit_log": 7,
    "view_channel": 10, "send_messages": 11, "manage_messages": 13, "embed_links": 14,
    "attach_files": 15, "read_message_history": 16, "mention_everyone": 17,
    "use_external_emojis": 18, "change_nickname": 26, "manage_nicknames": 27, "manage_roles": 28,
    "manage_webhooks": 29, "use_application_commands": 31, "manage_threads": 34,
    "create_public_threads": 35, "create_private_threads": 36, "use_external_stickers": 37,
    "send_messages_in_threads": 38, "moderate_members": 40,
}


def bits(names: list[str]) -> int:
    out = 0
    for n in names:
        out |= 1 << PERMISSIONS[n]
    return out


def has(perms: int | str, name: str) -> bool:
    return bool(int(perms) >> PERMISSIONS[name] & 1)


def snowflake_at(when: str) -> int:
    """The smallest id Discord could give at an ISO 8601 UTC time (for `after=`)."""
    ms = int(parse_time(when).timestamp() * 1000)
    return max(0, (ms - DISCORD_EPOCH_MS) << 22)


def time_of(snowflake: str | int) -> datetime:
    return datetime.fromtimestamp(((int(snowflake) >> 22) + DISCORD_EPOCH_MS) / 1000, timezone.utc)


def parse_time(when: str) -> datetime:
    t = datetime.fromisoformat(when.replace("Z", "+00:00"))
    return t if t.tzinfo else t.replace(tzinfo=timezone.utc)


def iso(t: datetime) -> str:
    return t.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def text(key: str) -> str:
    return (TEXTS / f"{key}.md").read_text(encoding="utf-8").strip()


def titled(key: str) -> tuple[str, str]:
    """A text whose first line is `# Title`: the title and the rest."""
    first, _, rest = text(key).partition("\n")
    return first.lstrip("# ").strip(), rest.strip()


def bare(name: str) -> str:
    """A channel's name without its emoji and separator: "💡┃ideas" -> "ideas"; the scripts and texts
    name channels this way, so the server's look can change without them."""
    return name.rsplit("┃", 1)[-1]


def linked(content: str, channels: list[dict]) -> str:
    """`#name` of a channel on the server (its bare name) becomes its mention `<#id>`, which Discord
    shows as a link to the channel; other `#words` (and GitHub's `#7`) stay as they are."""
    ids = {bare(c["name"]): c["id"] for c in channels if c.get("id") and c.get("type") != CATEGORY}
    return re.sub(r"(?<![\w<&/])#([a-z][a-z0-9-]*)\b", lambda m: f"<#{ids[m[1]]}>" if m[1] in ids else m[0], content)


def load_server() -> dict:
    return json.loads(SERVER_JSON.read_text(encoding="utf-8"))


class DiscordError(Exception):
    def __init__(self, status: int, method: str, path: str, body: str):
        self.status, self.body = status, body
        try:
            self.code = json.loads(body).get("code")
        except (ValueError, AttributeError):
            self.code = None
        super().__init__(f"{method} {path}: HTTP {status} {body[:300]}")


class Discord:
    def __init__(self, token: str, base: str | None = None, dry_run: bool = False, sleep=time.sleep, log=None):
        self.token, self.dry_run, self.sleep = token, dry_run, sleep
        self.base = (base or os.environ.get("DISCORD_API_BASE") or API).rstrip("/")
        self.log = log or (lambda s: print(s, file=sys.stderr))
        self.planned: list[str] = []
        self._wait_until = 0.0
        self._fake = 0

    def request(self, method: str, path: str, body=None, query: dict | None = None, files=None, auth: bool = True):
        if method != "GET" and self.dry_run:
            self.planned.append(f"{method} {path}")
            self._fake += 1
            return {"id": f"dry-run-{self._fake}", "name": (body or {}).get("name", ""), "dry_run": True}
        url = self.base + path + ("?" + urllib.parse.urlencode(query) if query else "")
        headers = {"User-Agent": USER_AGENT}
        if auth:
            headers["Authorization"] = f"Bot {self.token}"
        if files:
            data, ctype = multipart(body or {}, files)
            headers["Content-Type"] = ctype
        elif body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        else:
            data = None
        for attempt in range(6):
            pause = self._wait_until - time.monotonic()
            if pause > 0:
                self.sleep(pause)
            req = urllib.request.Request(url, data=data, method=method, headers=headers)
            try:
                with urllib.request.urlopen(req, timeout=30) as r:
                    self._note_bucket(r.headers)
                    raw = r.read()
                    return json.loads(raw) if raw else None
            except urllib.error.HTTPError as e:
                raw = e.read().decode("utf-8", "replace")
                if e.code == 429 and attempt < 5:
                    try:
                        wait = float(json.loads(raw).get("retry_after", 1))
                    except ValueError:
                        wait = float(e.headers.get("Retry-After", 1))
                    self.log(f"rate limited on {method} {path}: waiting {wait:.2f} s")
                    self.sleep(wait)
                    continue
                raise DiscordError(e.code, method, path, raw) from None
        raise DiscordError(429, method, path, "still rate limited after 5 tries")

    def _note_bucket(self, headers) -> None:
        if headers.get("X-RateLimit-Remaining") == "0":
            self._wait_until = time.monotonic() + float(headers.get("X-RateLimit-Reset-After", "0"))

    def get(self, path, **query):
        return self.request("GET", path, query=query or None)

    def post(self, path, body=None, files=None, query=None):
        return self.request("POST", path, body, query=query, files=files)

    def patch(self, path, body):
        return self.request("PATCH", path, body)

    def put(self, path, body=None):
        return self.request("PUT", path, body)

    # ----- lookups the scripts share

    def me(self) -> dict:
        return self.get("/users/@me")

    def the_guild(self) -> dict:
        """The one server the bot is in (it is made for exactly one)."""
        guilds = self.get("/users/@me/guilds")
        if len(guilds) != 1:
            names = ", ".join(g["name"] for g in guilds) or "none"
            sys.exit(f"the bot should be in exactly one server, it is in: {names}")
        return self.get(f"/guilds/{guilds[0]['id']}")

    def channels(self, guild_id: str) -> list[dict]:
        return self.get(f"/guilds/{guild_id}/channels")

    def channel_named(self, guild_id: str, name: str, channels: list[dict] | None = None) -> dict:
        for c in channels or self.channels(guild_id):
            if bare(c["name"]) == name and c["type"] not in (PUBLIC_THREAD, CATEGORY):
                return c
        sys.exit(f"no #{name} on the server: run scripts/discord-setup.py first")

    def threads(self, guild_id: str, forum: dict) -> list[dict]:
        """A forum's posts: the active ones and the archived ones."""
        active = [t for t in self.get(f"/guilds/{guild_id}/threads/active")["threads"] if t.get("parent_id") == forum["id"]]
        archived, before = [], None
        while True:
            q = {"limit": 100}
            if before:
                q["before"] = before
            page = self.get(f"/channels/{forum['id']}/threads/archived/public", **q)
            archived += page["threads"]
            if not page.get("has_more") or not page["threads"]:
                break
            before = page["threads"][-1]["thread_metadata"]["archive_timestamp"]
        seen = {t["id"] for t in active}
        return active + [t for t in archived if t["id"] not in seen]

    def messages_after(self, channel_id: str, after: int) -> list[dict]:
        """Every message after a snowflake, oldest first."""
        out = []
        while True:
            page = self.get(f"/channels/{channel_id}/messages", after=str(after), limit=100)
            if not page:
                return out
            page.sort(key=lambda m: int(m["id"]))
            out += page
            after = int(page[-1]["id"])
            if len(page) < 100:
                return out

    def tag_id(self, forum: dict, name: str) -> str | None:
        return next((t["id"] for t in forum.get("available_tags", []) if t["name"].lower() == name.lower()), None)


def multipart(payload: dict, files: list[tuple[str, bytes]]) -> tuple[bytes, str]:
    boundary = uuid.uuid4().hex
    parts = [
        f'--{boundary}\r\nContent-Disposition: form-data; name="payload_json"\r\n'
        f"Content-Type: application/json\r\n\r\n".encode() + json.dumps(payload).encode() + b"\r\n"
    ]
    for i, (name, data) in enumerate(files):
        parts.append(
            f'--{boundary}\r\nContent-Disposition: form-data; name="files[{i}]"; filename="{name}"\r\n'
            f"Content-Type: application/octet-stream\r\n\r\n".encode() + data + b"\r\n"
        )
    parts.append(f"--{boundary}--\r\n".encode())
    return b"".join(parts), f"multipart/form-data; boundary={boundary}"


def client(dry_run: bool = False) -> Discord:
    """The client with the bot's token from the credential store."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import secret

    return Discord(secret.need("discord-bot"), dry_run=dry_run)


def utf8_stdout() -> None:
    # -I ignores PYTHONIOENCODING and a Windows console is cp1250: emoji in posts crashed print
    sys.stdout.reconfigure(encoding="utf-8")
