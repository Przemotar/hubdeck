"""The Discord scripts against a fake Discord API (an in-process HTTP server keeping a server's state).

    python -I -m unittest discover -s scripts/tests -p "test_discord.py" -v
"""

import importlib.util
import io
import json
import re
import shutil
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse
from contextlib import redirect_stdout
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
import discord_api as d  # noqa: E402


def load(name: str):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), SCRIPTS / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


setup_mod, new_mod, seed_mod = load("discord-setup"), load("discord-new"), load("discord-seed")
tally_mod, ideas_mod, announce_mod = load("discord-tally"), load("discord-ideas-to-github"), load("discord-announce")

ADMIN = str(1 << 3)


class Fake:
    """A Discord server as the API shows it: what the owner has after his steps (Community on, #rules
    and #moderator-only from the wizard, #general in "Text Channels", the bot invited as Administrator)."""

    def __init__(self, refuse_bot_role_edit: bool = False):
        self.lock = threading.Lock()
        self.n = 0
        self.start = (int(time.time()) - 86400) * 1000
        self.log: list[tuple[str, str]] = []
        self.limit_once: set[tuple[str, str]] = set()
        self.refuse_bot_role_edit = refuse_bot_role_edit
        self.bot = {"id": self.id(), "username": "HubDeck", "bot": True}
        self.owner = {"id": self.id(), "username": "przemotar", "global_name": "Przemek"}
        g = self.id()
        self.guild = {"id": g, "name": "HubDeck", "owner_id": self.owner["id"], "features": ["COMMUNITY", "NEWS"],
                      "description": None, "verification_level": 1, "explicit_content_filter": 2,
                      "default_message_notifications": 0}
        self.roles = [
            {"id": g, "name": "@everyone", "permissions": "2248473465835073", "position": 0, "color": 0, "hoist": False, "mentionable": False, "managed": False},
            {"id": self.id(), "name": "HubDeck", "permissions": ADMIN, "position": 1, "color": 0, "hoist": False,
             "mentionable": False, "managed": True, "tags": {"bot_id": self.bot["id"]}},
        ]
        self.members = {
            self.owner["id"]: {"user": self.owner, "roles": [], "joined_at": "2026-10-30T07:00:00+00:00"},
            self.bot["id"]: {"user": self.bot, "roles": [self.roles[1]["id"]], "joined_at": "2026-10-30T07:05:00+00:00"},
        }
        self.channels: dict[str, dict] = {}
        self.messages: dict[str, list[dict]] = {}
        cat = self.channel("Text Channels", 4, position=0)
        self.channel("general", 0, position=0, parent_id=cat["id"])
        self.channel("General", 2, position=0, parent_id=self.channel("Voice Channels", 4, position=1)["id"])
        self.channel("rules", 0, position=1)
        self.channel("moderator-only", 0, position=2, permission_overwrites=[{"id": g, "type": 0, "allow": "0", "deny": str(1 << 10)}])
        self.automod: list[dict] = []
        self.webhooks: list[dict] = []
        self.onboarding = {"guild_id": g, "prompts": [], "default_channel_ids": [], "enabled": False, "mode": 0}

    def id(self) -> str:
        # a clock a second further for every new thing, so times in whole seconds tell them apart
        self.n += 1
        return str((self.start + self.n * 1000 - d.DISCORD_EPOCH_MS << 22) + self.n)

    def channel(self, name, type_, **kw) -> dict:
        c = {"id": self.id(), "guild_id": self.guild["id"], "name": name, "type": type_, "topic": None, "parent_id": None,
             "permission_overwrites": [], "position": 0, **kw}
        if type_ == d.FORUM:
            c.setdefault("available_tags", [])
            c.setdefault("default_reaction_emoji", None)
            c.setdefault("default_sort_order", None)
            c.setdefault("default_forum_layout", 0)
        self.channels[c["id"]] = c
        self.messages[c["id"]] = []
        return c

    def message(self, channel_id, author, content, at=None, **kw) -> dict:
        mid = kw.pop("id", None) or self.id()
        m = {"id": mid, "channel_id": channel_id, "author": author, "content": content,
             "timestamp": at or d.iso(d.time_of(mid)), "attachments": [], "reactions": [], "mentions": [], **kw}
        self.messages[channel_id].append(m)
        ch = self.channels[channel_id]
        ch["last_message_id"] = m["id"]
        return m

    def thread(self, forum, author, name, content, tags=(), at=None) -> dict:
        tid = self.id()
        t = {"id": tid, "guild_id": self.guild["id"], "parent_id": forum["id"], "name": name, "type": d.PUBLIC_THREAD,
             "owner_id": author["id"], "flags": 0, "applied_tags": list(tags), "last_message_id": tid,
             "thread_metadata": {"archived": False, "archive_timestamp": d.iso(d.time_of(tid))}}
        self.channels[tid] = t
        self.messages[tid] = []
        self.message(tid, author, content, at=at, id=tid)
        return t

    def named(self, name, type_=None) -> dict:
        return next(c for c in self.channels.values() if d.bare(c["name"]) == name and c["type"] != d.CATEGORY and (type_ is None or c["type"] == type_))

    def changes(self) -> list[tuple[str, str]]:
        return [x for x in self.log if x[0] != "GET"]

    # ----- the routes

    def handle(self, method: str, path: str, query: dict, body):
        g = self.guild["id"]
        routes = [
            ("GET", r"/users/@me", lambda: self.bot),
            ("GET", r"/users/@me/guilds", lambda: [{"id": g, "name": self.guild["name"]}]),
            ("GET", rf"/guilds/{g}", lambda: self.guild),
            ("PATCH", rf"/guilds/{g}", lambda: self.guild.update(body) or self.guild),
            ("GET", rf"/guilds/{g}/roles", lambda: self.roles),
            ("POST", rf"/guilds/{g}/roles", lambda: self.new_role(body)),
            ("PATCH", rf"/guilds/{g}/roles", lambda: self.role_positions(body)),
            ("PATCH", rf"/guilds/{g}/roles/(\d+)", lambda r: self.edit_role(r, body)),
            ("GET", rf"/guilds/{g}/members", lambda: self.list_members(query)),
            ("GET", rf"/guilds/{g}/members/(\d+)", lambda u: self.members[u]),
            ("PUT", rf"/guilds/{g}/members/(\d+)/roles/(\d+)", lambda u, r: self.members[u]["roles"].append(r)),
            ("GET", rf"/guilds/{g}/channels", lambda: [c for c in self.channels.values() if c["type"] != d.PUBLIC_THREAD]),
            ("POST", rf"/guilds/{g}/channels", lambda: self.new_channel(body)),
            ("PATCH", rf"/guilds/{g}/channels", lambda: [self.channels[x["id"]].update(position=x["position"]) for x in body] and None),
            ("GET", rf"/guilds/{g}/threads/active", lambda: {"threads": self.threads(archived=False)}),
            ("GET", r"/channels/(\d+)/threads/archived/public", lambda c: {"threads": self.threads(True, c), "has_more": False}),
            ("PATCH", r"/channels/(\d+)", lambda c: self.edit_channel(c, body)),
            ("GET", r"/channels/(\d+)/messages", lambda c: self.list_messages(c, query)),
            ("POST", r"/channels/(\d+)/messages", lambda c: self.message(c, self.bot, body["content"])),
            ("GET", r"/channels/(\d+)/messages/(\d+)", lambda c, m: self.find(c, m)),
            ("PATCH", r"/channels/(\d+)/messages/(\d+)", lambda c, m: self.edit_message(c, m, body)),
            ("POST", r"/channels/(\d+)/messages/(\d+)/crosspost", lambda c, m: self.find(c, m)),
            ("POST", r"/channels/(\d+)/threads", lambda c: self.new_post(c, body)),
            ("GET", rf"/guilds/{g}/auto-moderation/rules", lambda: self.automod),
            ("POST", rf"/guilds/{g}/auto-moderation/rules", lambda: self.new_rule(body)),
            ("PATCH", rf"/guilds/{g}/auto-moderation/rules/(\d+)", lambda r: self.edit_rule(r, body)),
            ("GET", rf"/guilds/{g}/onboarding", lambda: self.onboarding),
            ("PUT", rf"/guilds/{g}/onboarding", lambda: self.set_onboarding(body)),
            ("GET", r"/channels/(\d+)/webhooks", lambda c: [h for h in self.webhooks if h["channel_id"] == c]),
            ("POST", r"/channels/(\d+)/webhooks", lambda c: self.new_hook(c, body)),
            ("POST", r"/webhooks/(\d+)/([\w-]+)", lambda h, tok: self.hook_post(h, tok, body)),
        ]
        for m, pattern, fn in routes:
            hit = re.fullmatch(pattern, path)
            if m == method and hit:
                return fn(*hit.groups())
        raise KeyError(f"{method} {path}")

    def new_role(self, body):
        for r in self.roles:
            if r["position"] >= 1:
                r["position"] += 1
        r = {"id": self.id(), "position": 1, "managed": False, **body}
        self.roles.append(r)
        return r

    def role_positions(self, body):
        for x in body:
            next(r for r in self.roles if r["id"] == x["id"])["position"] = x["position"]
        return self.roles

    def edit_role(self, rid, body):
        role = next(r for r in self.roles if r["id"] == rid)
        if role.get("managed") and self.refuse_bot_role_edit:
            raise PermissionError
        role.update(body)
        return role

    def list_members(self, query):
        after = int(query.get("after", "0"))
        return sorted((m for m in self.members.values() if int(m["user"]["id"]) > after), key=lambda m: int(m["user"]["id"]))

    def new_channel(self, body):
        body = dict(body)
        type_ = body.pop("type")
        c = self.channel(body.pop("name"), type_, position=max(c["position"] for c in self.channels.values()) + 1)
        self.edit_channel(c["id"], body)
        return c

    def edit_channel(self, cid, body):
        c = self.channels[cid]
        for k, v in body.items():
            if k == "available_tags":
                v = [{"id": t.get("id") or self.id(), "name": t["name"], "moderated": t.get("moderated", False), "emoji_id": None, "emoji_name": None} for t in v]
            if k == "archived":
                c["thread_metadata"]["archived"] = v
                continue
            c[k] = v
        return c

    def threads(self, archived, parent=None):
        return [c for c in self.channels.values() if c["type"] == d.PUBLIC_THREAD and c["thread_metadata"]["archived"] == archived
                and (parent is None or c["parent_id"] == parent)]

    def list_messages(self, cid, query):
        after = int(query.get("after", "0"))
        later = sorted((m for m in self.messages[cid] if int(m["id"]) > after), key=lambda m: int(m["id"]))
        return list(reversed(later[: int(query.get("limit", "50"))]))

    def find(self, cid, mid):
        return next(m for m in self.messages[cid] if m["id"] == mid)

    def edit_message(self, cid, mid, body):
        m = self.find(cid, mid)
        if "content" in body:
            m["content"] = body["content"]
        if "attachments" in body:
            m["attachments"] = [{"id": self.id(), "filename": a["filename"]} for a in body["attachments"]]
        return m

    def new_post(self, forum_id, body):
        t = self.thread(self.channels[forum_id], self.bot, body["name"], body["message"].get("content", ""), body.get("applied_tags", []))
        self.messages[t["id"]][0]["attachments"] = [{"id": self.id(), "filename": a["filename"]} for a in body["message"].get("attachments", [])]
        return t

    def new_rule(self, body):
        meta = {"keyword_filter": [], "regex_patterns": [], "allow_list": []} if body["trigger_type"] == 1 else {}
        meta.update(body.get("trigger_metadata", {}))
        acts = [{"type": a["type"], "metadata": {"custom_message": "", **a.get("metadata", {})} if a["type"] == 1 else a.get("metadata", {})} for a in body["actions"]]
        r = {**body, "id": self.id(), "guild_id": self.guild["id"], "trigger_metadata": meta, "actions": acts, "exempt_channels": []}
        self.automod.append(r)
        return r

    def edit_rule(self, rid, body):
        r = next(x for x in self.automod if x["id"] == rid)
        r.update(body)
        return r

    def set_onboarding(self, body):
        chans = [self.channels[c] for c in body["default_channel_ids"]]
        writable = [c for c in chans if not any(int(o["deny"]) & 1 << 11 for o in c["permission_overwrites"] if o["id"] == self.guild["id"])]
        if len(chans) < 7 or len(writable) < 5:
            raise PermissionError
        self.onboarding.update(body)
        return self.onboarding

    def new_hook(self, cid, body):
        h = {"id": self.id(), "token": "tok-" + str(self.n), "channel_id": cid, "name": body["name"], "type": 1}
        self.webhooks.append(h)
        return h

    def hook_post(self, hid, tok, body):
        h = next(x for x in self.webhooks if x["id"] == hid and x["token"] == tok)
        m = self.message(h["channel_id"], {"id": hid, "username": h["name"], "bot": True}, body.get("content", ""))
        m["embeds"] = body.get("embeds", [])
        return m


class Handler(BaseHTTPRequestHandler):
    fake: Fake = None

    def log_message(self, *a):
        pass

    def do(self):
        url = urllib.parse.urlparse(self.path)
        path = url.path.removeprefix("/api/v10")
        query = dict(urllib.parse.parse_qsl(url.query))
        raw = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        body = None
        if raw and self.headers.get("Content-Type", "").startswith("multipart/"):
            body = json.loads(re.search(rb'name="payload_json"\r\nContent-Type: application/json\r\n\r\n(.*?)\r\n--', raw, re.S).group(1))
        elif raw:
            body = json.loads(raw)
        fake = self.fake
        with fake.lock:
            if (self.command, path) in fake.limit_once:
                fake.limit_once.discard((self.command, path))
                return self.reply(429, {"message": "You are being rate limited.", "retry_after": 0.05, "global": False})
            fake.log.append((self.command, path))
            if not path.startswith("/webhooks/") and self.headers.get("Authorization") != "Bot test-token":
                return self.reply(401, {"message": "401: Unauthorized", "code": 0})
            try:
                out = fake.handle(self.command, path, query, body)
            except KeyError as e:
                return self.reply(404, {"message": f"no route {e}", "code": 0})
            except (StopIteration, PermissionError):
                return self.reply(403 if sys.exc_info()[0] is PermissionError else 404, {"message": "Missing Permissions", "code": 50013})
        self.reply(200 if out is not None else 204, out)

    def reply(self, status, out):
        data = json.dumps(out).encode() if out is not None else b""
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    do_GET = do_POST = do_PATCH = do_PUT = do_DELETE = do


class Base(unittest.TestCase):
    def setUp(self):
        self.fake = self.make_fake()
        handler = type("H", (Handler,), {"fake": self.fake})
        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()
        self.base = f"http://127.0.0.1:{self.httpd.server_port}/api/v10"
        self.slept: list[float] = []
        self.out: list[str] = []

    def make_fake(self):
        return Fake()

    def tearDown(self):
        self.httpd.shutdown()
        self.httpd.server_close()

    def api(self, dry_run=False):
        return d.Discord("test-token", base=self.base, dry_run=dry_run, sleep=self.slept.append, log=self.out.append)

    def setup(self, dry_run=False):
        return setup_mod.Setup(self.api(dry_run), d.load_server(), out=self.out.append).run()


class SetupTest(Base):
    def test_from_the_wizards_server_then_a_second_run_changes_nothing(self):
        made = self.setup()
        self.assertGreater(made, 20)
        f = self.fake
        by_pos = lambda cs: sorted(cs, key=lambda c: c["position"])
        cats = by_pos(c for c in f.channels.values() if c["type"] == d.CATEGORY and c["name"][0] not in "TV")
        self.assertEqual([c["name"] for c in cats], ["📌 Start here", "💬 Community", "🎛️ HubDeck", "🔒 Staff"])
        inside = [[c["name"] for c in by_pos(c for c in f.channels.values() if c["parent_id"] == cat["id"])] for cat in cats]
        self.assertEqual(inside, [["📜┃rules", "📣┃announcements"], ["💬┃general", "📸┃setups"],
                                  ["💡┃ideas", "🎨┃presets", "🧩┃plugins", "🆘┃help"], ["moderator-only"]])
        self.assertEqual(sum(1 for c in f.channels.values() if d.bare(c["name"]) == "general" and c["type"] == 0), 1)  # the wizard's, renamed in place
        self.assertTrue(f.onboarding["enabled"])
        self.assertEqual(len(f.onboarding["default_channel_ids"]), 8)
        self.assertEqual(f.named("announcements")["type"], d.ANNOUNCEMENT)
        ideas = f.named("ideas")
        self.assertEqual(ideas["type"], d.FORUM)
        self.assertEqual(ideas["default_reaction_emoji"]["emoji_name"], "👍")
        self.assertEqual(f.named("presets")["default_reaction_emoji"]["emoji_name"], "⭐")
        self.assertEqual(f.named("presets")["default_forum_layout"], 2)
        moderated = {t["name"] for t in ideas["available_tags"] if t["moderated"]}
        self.assertEqual(moderated, {"planned", "in progress", "done", "not planned"})
        self.assertEqual(f.named("moderator-only")["permission_overwrites"][0]["deny"], str(1 << 10))
        everyone_deny = int(f.named("announcements")["permission_overwrites"][0]["deny"])
        self.assertTrue(everyone_deny & 1 << 11)  # members cannot send there
        self.assertFalse(int(f.roles[0]["permissions"]) & 1 << 17)  # no @everyone mentions
        roles = sorted((r for r in f.roles if not r.get("managed") and r["name"] != "@everyone"), key=lambda r: -r["position"])
        self.assertEqual([r["name"] for r in roles], ["Owner", "Moderator", "Contributor", "Beta tester"])
        self.assertIn(roles[0]["id"], f.members[f.owner["id"]]["roles"])
        self.assertEqual(len(f.automod), 3)
        self.assertEqual(f.webhooks[0]["name"], "HubDeck releases")
        self.assertEqual(len(f.messages[f.named("rules")["id"]]), 2)
        pinned = [t["name"] for t in f.channels.values() if t["type"] == d.PUBLIC_THREAD and t["flags"] & d.THREAD_PINNED]
        self.assertEqual(sorted(pinned), ["A bug? Report it on GitHub", "How ideas work here", "How to rate presets"])
        self.assertEqual(f.guild["description"], "Free, open-source app for the MasterHUB deck")

        before = len(f.log)
        self.out.clear()
        self.assertEqual(self.setup(), 0)
        self.assertEqual([x for x in f.log[before:] if x[0] != "GET"], [])
        self.assertIn("nothing to change: the server already matches discord/server.json", self.out)

    def test_an_edited_text_is_edited_in_place(self):
        self.setup()
        rules = self.fake.named("rules")["id"]
        self.fake.messages[rules][1]["content"] = "**Rules**\n\nold text"
        self.assertEqual(self.setup(), 1)
        self.assertEqual(len(self.fake.messages[rules]), 2)
        self.assertEqual(self.fake.messages[rules][1]["content"], d.text("rules"))

    def test_channel_names_become_links(self):
        self.setup()
        ideas = self.fake.named("ideas")["id"]
        welcome = self.fake.messages[self.fake.named("rules")["id"]][0]["content"]
        self.assertIn(f"<#{ideas}>", welcome)
        self.assertNotIn(" #ideas", welcome)
        chans = [{"name": "ideas", "id": "5"}]
        self.assertEqual(d.linked("in #ideas, issue #7, #idea, <#5>, a/#ideas", chans), "in <#5>, issue #7, #idea, <#5>, a/#ideas")
        self.assertEqual(self.setup(), 0)

    def test_dry_run_changes_nothing(self):
        planned = self.setup(dry_run=True)
        self.assertGreater(planned, 20)
        self.assertEqual(self.fake.changes(), [])
        self.assertTrue(any(line.startswith("would create #💡┃ideas") for line in self.out))

    def test_trim(self):
        self.setup()
        self.assertEqual(setup_mod.trim(self.api(True), d.load_server(), out=self.out.append), 1)
        self.assertEqual(self.fake.roles[1]["permissions"], ADMIN)
        setup_mod.trim(self.api(), d.load_server(), out=self.out.append)
        self.assertEqual(int(self.fake.roles[1]["permissions"]), d.bits(d.load_server()["bot_role"]["trimmed_permissions"]))
        self.assertEqual(setup_mod.trim(self.api(), d.load_server(), out=self.out.append), 0)


class TrimRefusedTest(Base):
    def make_fake(self):
        return Fake(refuse_bot_role_edit=True)

    def test_the_invite_link_when_discord_refuses(self):
        self.setup()
        setup_mod.trim(self.api(), d.load_server(), out=self.out.append)
        link = next(line for line in self.out if line.startswith("https://discord.com/oauth2/authorize"))
        self.assertIn(f"permissions={d.bits(d.load_server()['bot_role']['trimmed_permissions'])}", link)


class RateLimitTest(Base):
    def test_a_429_waits_retry_after_and_tries_again(self):
        self.fake.limit_once.add(("GET", "/users/@me"))
        self.assertEqual(self.api().me()["username"], "HubDeck")
        self.assertEqual(self.slept, [0.05])
        self.assertEqual(self.fake.log.count(("GET", "/users/@me")), 1)


class SeedTest(Base):
    def test_presets_and_ideas_once_then_the_missing_picture(self):
        self.setup()
        looks = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, looks)
        (looks / "matrix-rain.webp").write_bytes(b"RIFF fake webp")
        issues = {7: {"title": "Support for multiple MasterHUBs", "url": "https://github.com/Przemotar/hubdeck/issues/7", "state": "OPEN"},
                  10: {"title": "Some suggestions", "url": "https://github.com/Przemotar/hubdeck/issues/10", "state": "OPEN"}}
        seed = lambda dry=False: seed_mod.Seed(self.api(dry), looks=looks, issue=issues.get, out=self.out.append).run()

        self.assertEqual(seed(dry=True), 14)
        self.assertEqual([x for x in self.fake.changes() if "/threads" in x[1] and "presets" in x[1]], [])
        made_before = len(self.fake.changes())
        self.assertEqual(seed(dry=True), 14)
        self.assertEqual(len(self.fake.changes()), made_before)

        self.assertEqual(seed(), 14)
        presets = self.fake.named("presets")["id"]
        posts = {t["name"]: t for t in self.fake.channels.values() if t.get("parent_id") == presets and t["type"] == d.PUBLIC_THREAD}
        self.assertEqual(len(posts), 12 + 1)  # and the pinned "How to rate presets"
        rain = posts["Code rain"]
        self.assertEqual(self.fake.messages[rain["id"]][0]["attachments"][0]["filename"], "matrix-rain.webp")
        self.assertEqual([t["name"] for t in self.fake.named("presets")["available_tags"] if t["id"] in rain["applied_tags"]], ["Matrix"])
        self.assertIn("Deck look > Presets > Code rain", self.fake.messages[rain["id"]][0]["content"])
        self.assertIn("Fuji and sakura", posts)
        ideas = self.fake.named("ideas")["id"]
        seven = next(t for t in self.fake.channels.values() if t.get("parent_id") == ideas and t["name"] == "Support for multiple MasterHUBs")
        self.assertIn("https://github.com/Przemotar/hubdeck/issues/7", self.fake.messages[seven["id"]][0]["content"])

        self.assertEqual(seed(), 0)
        (looks / "space-nebula.webp").write_bytes(b"RIFF fake webp")
        self.assertEqual(seed(), 1)
        self.assertEqual(seed(), 0)

    def test_the_preset_list_is_the_apps(self):
        names = [p["name"] for p in seed_mod.presets()]
        self.assertIn("Hollow Hill", names)
        self.assertNotIn("Haunted night", names)  # hidden in Deck look
        self.assertEqual(len(names), len(set(names)))


class ReaderTest(Base):
    def test_the_digest_of_a_fixed_day(self):
        self.setup()
        f = self.fake
        since = d.iso(d.time_of(f.id()))
        alice = {"id": f.id(), "username": "alice", "global_name": "Alice"}
        bob = {"id": f.id(), "username": "bob"}
        f.members[alice["id"]] = {"user": alice, "roles": [], "joined_at": "2099-01-01T00:00:00+00:00"}
        f.members[bob["id"]] = {"user": bob, "roles": [], "joined_at": "2020-01-01T00:00:00+00:00"}
        general = f.named("general")["id"]
        f.message(general, alice, "Does the dial work on macOS?")
        f.message(general, f.owner, "my own words, left out")
        f.message(general, bob, "Mine too, I would like to know?")
        win = next(t["id"] for t in f.named("help")["available_tags"] if t["name"] == "Windows")
        f.thread(f.named("help"), bob, "Keys dark after sleep", "After sleep the keys stay dark.", tags=[win])
        idea = f.thread(f.named("ideas"), alice, "A clock on the dial", "Show the time on the dial.")
        f.messages[idea["id"]][0]["reactions"] = [{"emoji": {"name": "👍"}, "count": 4, "me": False}]

        text = new_mod.digest(self.api(), since)
        self.assertNotIn("my own words", text)
        self.assertIn("Alice  #general", text)
        self.assertIn('#help > "Keys dark after sleep" (new post; tags: Windows)', text)
        self.assertIn('#ideas > "A clock on the dial" (new post; 👍 4)', text)
        self.assertTrue(text.splitlines()[-3].startswith(f"Discord since {since}: 4 message(s), 2 new post(s), 1 new member(s)"), text)
        self.assertEqual(text.splitlines()[-2], "New members: Alice")
        self.assertIn('#general (bob)', text.splitlines()[-1])
        self.assertIn('#help > "Keys dark after sleep" (bob)', text.splitlines()[-1])
        self.assertNotIn("clock", text.splitlines()[-1])  # an idea with no question waits for nobody

        self.assertIn("my own words", new_mod.digest(self.api(), since, keep_mine=True))
        f.members[alice["id"]]["joined_at"] = "2020-01-01T00:00:00+00:00"
        later = d.iso(d.time_of(f.id()))
        self.assertEqual(new_mod.digest(self.api(), later), "")

    def test_since_comes_from_the_routine_first(self):
        import os
        os.environ["ROUTINE_LAST_SUCCESS"] = "2026-10-30T06:00:00Z"
        self.addCleanup(os.environ.pop, "ROUTINE_LAST_SUCCESS")
        self.assertEqual(new_mod.since_from(["2026-10-01T00:00:00Z"]), "2026-10-30T06:00:00Z")
        os.environ["ROUTINE_LAST_SUCCESS"] = ""
        self.assertEqual(new_mod.since_from(["2026-10-01T00:00:00Z"]), "2026-10-01T00:00:00Z")


class TallyTest(unittest.TestCase):
    def test_the_draft_on_fixed_counts(self):
        ideas = [{"id": "1", "name": "Clock on the dial", "votes": 9, "url": "u1"},
                 {"id": "2", "name": "Two bases", "votes": 12, "url": "u2"},
                 {"id": "3", "name": "Nobody wants", "votes": 0, "url": "u3"}]
        presets = [{"id": "4", "name": "Hollow Hill", "votes": 5, "url": "u4"}]
        first = tally_mod.draft([dict(p) for p in ideas], [dict(p) for p in presets], {})
        self.assertIn("1. Two bases: 12 👍 u2\n2. Clock on the dial: 9 👍 u1\n", first)
        self.assertNotIn("Nobody wants", first)
        self.assertIn("1. Hollow Hill: 5 ⭐ u4", first)
        week = tally_mod.draft([dict(p) for p in ideas], [dict(p) for p in presets], {"1": 2, "2": 11, "4": 5})
        self.assertIn("1. Clock on the dial: 9 👍 (+7 this week) u1\n2. Two bases: 12 👍 (+1 this week) u2", week)
        self.assertIn("1. Hollow Hill: 5 ⭐ u4", week)
        self.assertIn("no 👍 yet.", tally_mod.draft([], [], {}))

    def test_votes_leave_the_bots_own_out(self):
        self.assertEqual(tally_mod.count({"reactions": [{"emoji": {"name": "⭐"}, "count": 3, "me": True}]}, "⭐"), 2)


class IdeasToGithubTest(Base):
    def test_the_top_three_without_a_link(self):
        self.setup()
        f = self.fake
        forum = f.named("ideas")
        planned = next(t["id"] for t in forum["available_tags"] if t["name"] == "planned")
        people = [{"id": f.id(), "username": f"user{i}"} for i in range(6)]
        votes = {"A": 5, "B": 9, "C": 1, "D": 7, "Linked": 20, "Tagged": 30, "None": 0}
        for i, (name, n) in enumerate(votes.items()):
            t = f.thread(forum, people[i % 6], name, f"idea {name}", tags=[planned] if name == "Tagged" else [])
            f.messages[t["id"]][0]["reactions"] = [{"emoji": {"name": "👍"}, "count": n, "me": False}] if n else []
            if name == "Linked":
                f.message(t["id"], people[0], "see https://github.com/Przemotar/hubdeck/issues/12")
        made = []
        stub = lambda title, body: made.append((title, body)) or f"https://github.com/Przemotar/hubdeck/issues/{100 + len(made)}"

        before = len(f.changes())
        self.assertEqual(ideas_mod.run(self.api(), go=False, make_issue=stub, out=self.out.append), [])
        self.assertEqual(len(f.changes()), before)
        self.assertEqual(made, [])
        self.assertEqual([line.split('"')[1] for line in self.out if line.startswith("would open")], ["B", "D", "A"])

        issues = ideas_mod.run(self.api(), go=True, make_issue=stub, out=self.out.append)
        self.assertEqual(issues, [f"https://github.com/Przemotar/hubdeck/issues/{n}" for n in (101, 102, 103)])
        self.assertIn("> idea B", made[0][1])
        b = next(t for t in f.channels.values() if t.get("name") == "B")
        self.assertEqual(b["applied_tags"], [planned])
        self.assertIn("issues/101", f.messages[b["id"]][-1]["content"])

        self.out.clear()
        again = ideas_mod.run(self.api(), go=True, make_issue=stub, out=self.out.append)
        self.assertEqual(again, ["https://github.com/Przemotar/hubdeck/issues/104"])  # C is next, the rest are on GitHub
        self.assertEqual(made[-1][0], "C")


class AnnounceTest(Base):
    def test_dry_run_then_post_and_publish(self):
        self.setup()
        notes = ("HubDeck 0.2.0", "Line one.\n\nLine two.", "https://github.com/Przemotar/hubdeck/releases/tag/v0.2.0")
        before = len(self.fake.changes())
        announce_mod.announce(self.api(True), d.load_server(), *notes, out=self.out.append)
        self.assertEqual(len(self.fake.changes()), before)
        msg = announce_mod.announce(self.api(), d.load_server(), *notes, out=self.out.append)
        self.assertEqual(msg["embeds"][0]["color"], 0x6E56FF)
        self.assertEqual(msg["embeds"][0]["url"], notes[2])
        self.assertIn(("POST", f"/channels/{self.fake.named('announcements')['id']}/messages/{msg['id']}/crosspost"), self.fake.log)

    def test_long_notes_are_cut_with_the_link_and_placeholders_are_refused(self):
        e = announce_mod.embed("T", "a line\n" * 1000, "URL")
        self.assertLessEqual(len(e["description"]), 4096)
        self.assertTrue(e["description"].endswith("the full notes: URL"))
        with self.assertRaises(SystemExit):
            announce_mod.embed("T", "{PRESET_COUNT} presets", "URL")

    def test_the_first_announcements_file(self):
        title, body, url = announce_mod.from_file(str(d.TEXTS / "announce-0.2.0.md"), "https://example/r")
        self.assertIn("https://example/r", body)
        self.assertEqual(title, "HubDeck 0.2.0: the living deck")


class TextsTest(unittest.TestCase):
    def test_every_text_server_json_names_exists_and_fits(self):
        server = d.load_server()
        for c in server["channels"]:
            if "topic" in c:
                self.assertLessEqual(len(d.text(c["topic"])), 4096 if c["type"] == "forum" else 1024, c["topic"])
            for m in c.get("messages", []) + c.get("pins", []):
                self.assertLessEqual(len(d.text(m["text"])), 2000, m["text"])
        for t in ("pin-help", "pin-ideas", "pin-presets", "idea-7", "idea-10", "announce-0.2.0"):
            self.assertTrue(d.text(t).startswith("# "), t)
        for t in d.TEXTS.glob("*.md"):
            self.assertNotRegex(t.read_text(encoding="utf-8"), r"(?i)masterhub app|cooler master app", t.name)

    def test_no_secret_store_on_linux(self):
        import secret
        if sys.platform.startswith("linux"):
            with self.assertRaises(secret.SecretError):
                secret.get("discord-bot")


if __name__ == "__main__":
    unittest.main()
