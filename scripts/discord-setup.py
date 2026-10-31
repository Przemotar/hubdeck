"""Set HubDeck's Discord server up from `discord/server.json` (docs/DISCORD.md): the server's
settings, roles, channels and forums with their tags and reactions, read-only permissions, the
pinned texts, AutoMod rules and the announcement webhook. Everything is made or updated in place,
so a second run changes nothing and says so.

    python -I scripts/discord-setup.py --dry-run   # the plan, nothing changed
    python -I scripts/discord-setup.py             # make it so
    python -I scripts/discord-setup.py --trim      # at the end: the bot's role down from Administrator

Needs the bot invited with Administrator and the server's Community switched on (the owner's steps
in private/cloud-tasks/00-START.md). The token comes from `secret.py` (`discord-bot`).
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import discord_api as d  # noqa: E402

READ_ONLY_DENY = ["send_messages", "send_messages_in_threads", "create_public_threads", "create_private_threads"]
WRITERS_ALLOW = ["send_messages", "send_messages_in_threads", "create_public_threads"]
AUTOMOD_ACTIONS = {"block": 1, "alert": 2, "timeout": 3}


def colour(hex_: str) -> int:
    return int(hex_.lstrip("#"), 16)


class Setup:
    def __init__(self, api: d.Discord, server: dict, out=print):
        self.api, self.server, self.out = api, server, out
        self.changes: list[str] = []

    def change(self, what: str) -> None:
        self.changes.append(what)
        self.out(("would " if self.api.dry_run else "") + what)

    # ----- the whole run

    def run(self) -> int:
        api = self.api
        self.me = api.me()
        self.guild = api.the_guild()
        g = self.guild["id"]
        if "COMMUNITY" not in self.guild.get("features", []):
            sys.exit("Community is off on the server: Server Settings > Enable Community first (forums and announcement channels need it)")
        self.roles = api.get(f"/guilds/{g}/roles")
        self.bot_role = next((r for r in self.roles if (r.get("tags") or {}).get("bot_id") == self.me["id"]), None)
        if not self.bot_role:
            sys.exit("the bot has no role of its own on the server: invite it again with the bot scope")
        if not d.has(self.bot_role["permissions"], "administrator"):
            self.out("note: the bot's role is not Administrator any more (trimmed?); changes may be refused")
        self.do_guild()
        self.do_roles()
        self.do_everyone()
        self.do_categories()
        self.do_channels()
        self.do_texts()
        self.do_automod()
        self.do_webhook()
        self.do_onboarding()
        if self.changes:
            self.out(f"{len(self.changes)} change(s){' planned, nothing sent (dry run)' if api.dry_run else ' made'}")
        else:
            self.out("nothing to change: the server already matches discord/server.json")
        return len(self.changes)

    # ----- the server

    def do_guild(self) -> None:
        want = self.server["guild"]
        diff = {k: v for k, v in want.items() if self.guild.get(k) != v}
        if diff:
            self.api.patch(f"/guilds/{self.guild['id']}", diff)
            self.guild.update(diff)
            self.change(f"update the server: {', '.join(sorted(diff))}")

    def role_named(self, name: str) -> dict | None:
        if name == "@everyone":
            return next(r for r in self.roles if r["id"] == self.guild["id"])
        if name == "@bot":
            return self.bot_role
        return next((r for r in self.roles if r["name"] == name and not r.get("managed")), None)

    def do_roles(self) -> None:
        g = self.guild["id"]
        for spec in self.server["roles"]:
            want = {
                "name": spec["name"],
                "permissions": str(d.bits(spec["permissions"])),
                "color": colour(spec["color"]),
                "hoist": spec["hoist"],
                "mentionable": spec["mentionable"],
            }
            role = self.role_named(spec["name"])
            if not role:
                role = self.api.post(f"/guilds/{g}/roles", want)
                role = {**want, **role, "position": 1}
                self.roles.append(role)
                self.change(f"create role {spec['name']}")
            else:
                diff = {k: v for k, v in want.items() if role.get(k) != v}
                if diff:
                    self.api.patch(f"/guilds/{g}/roles/{role['id']}", diff)
                    role.update(diff)
                    self.change(f"update role {spec['name']}: {', '.join(sorted(diff))}")
            if spec.get("give_to_owner"):
                self.give_owner(role)
        self.order_roles()

    def give_owner(self, role: dict) -> None:
        g, owner = self.guild["id"], self.guild["owner_id"]
        member = {"roles": []} if role.get("dry_run") else self.api.get(f"/guilds/{g}/members/{owner}")
        if role["id"] not in member["roles"]:
            self.api.put(f"/guilds/{g}/members/{owner}/roles/{role['id']}")
            self.change(f"give role {role['name']} to the server's owner")

    def order_roles(self) -> None:
        """The roles of server.json in its order (top first), in the places they already take."""
        mine = [self.role_named(s["name"]) for s in self.server["roles"]]
        if any(r.get("dry_run") for r in mine):
            self.change("put the roles in order: " + ", ".join(r["name"] for r in mine))
            return
        now = sorted(mine, key=lambda r: (-r["position"], r["id"]))
        if [r["id"] for r in now] == [r["id"] for r in mine]:
            return
        places = sorted({r["position"] for r in mine}, reverse=True)
        if len(places) < len(mine):  # equal positions: count down from the highest
            places = [places[0] - i for i in range(len(mine))]
        body = [{"id": r["id"], "position": p} for r, p in zip(mine, places)]
        self.api.patch(f"/guilds/{self.guild['id']}/roles", body)
        for r, p in zip(mine, places):
            r["position"] = p
        self.change("put the roles in order: " + ", ".join(r["name"] for r in mine))

    def do_everyone(self) -> None:
        everyone = self.role_named("@everyone")
        want = str(d.bits(self.server["everyone"]["permissions"]))
        if everyone["permissions"] != want:
            self.api.patch(f"/guilds/{self.guild['id']}/roles/{everyone['id']}", {"permissions": want})
            everyone["permissions"] = want
            self.change("update @everyone's permissions (no @everyone mentions, no managing)")

    # ----- channels

    def overwrites(self, spec: dict) -> list[dict] | None:
        if not spec.get("read_only"):
            return None
        out = [{"id": self.guild["id"], "type": 0, "allow": "0", "deny": str(d.bits(READ_ONLY_DENY))}]
        for name in ("Moderator", "@bot"):
            role = self.role_named(name)
            out.append({"id": role["id"], "type": 0, "allow": str(d.bits(WRITERS_ALLOW)), "deny": "0"})
        return out

    def tags(self, spec: dict, channel: dict | None) -> list[dict]:
        have = {t["name"]: t for t in (channel or {}).get("available_tags", [])}
        out = []
        for t in spec["tags"]:
            tag = {"name": t["name"], "moderated": t.get("moderated", False)}
            if t["name"] in have:
                tag["id"] = have[t["name"]]["id"]
            out.append(tag)
        named = {t["name"] for t in spec["tags"]}
        out += [{k: t[k] for k in ("id", "name", "moderated") if k in t} for t in have.values() if t["name"] not in named]
        return out

    def do_categories(self) -> None:
        g = self.guild["id"]
        self.channels = self.api.channels(g)
        self.cats: dict[str, dict] = {}
        for spec in self.server.get("categories", []):
            cat = next((c for c in self.channels if c["type"] == d.CATEGORY and c["name"] == spec["name"]), None)
            if not cat:
                cat = self.api.post(f"/guilds/{g}/channels", {"name": spec["name"], "type": d.CATEGORY})
                cat = {"name": spec["name"], "type": d.CATEGORY, "position": 0, "parent_id": None, **cat}
                self.channels.append(cat)
                self.change(f"create the category {spec['name']}")
            self.cats[spec["key"]] = cat

    def parent(self, spec: dict) -> str | None:
        return self.cats[spec["category"]]["id"] if spec.get("category") else None

    def wanted(self, spec: dict, channel: dict | None) -> dict:
        want = {"name": spec["name"], "parent_id": self.parent(spec)}
        if "topic" in spec:
            want["topic"] = d.text(spec["topic"])
        ow = self.overwrites(spec)
        if ow is not None:
            want["permission_overwrites"] = ow
        if spec["type"] == "forum":
            want["available_tags"] = self.tags(spec, channel)
            want["default_reaction_emoji"] = {"emoji_id": None, "emoji_name": spec["default_reaction"]} if spec.get("default_reaction") else None
            want["default_sort_order"] = spec["default_sort_order"]
            want["default_forum_layout"] = spec["default_forum_layout"]
        return want

    @staticmethod
    def same(key: str, have, want) -> bool:
        if key == "permission_overwrites":
            norm = lambda ows: sorted((o["id"], int(o["type"]), str(o["allow"]), str(o["deny"])) for o in ows or [])
            return norm(have) == norm(want)
        if key == "available_tags":
            norm = lambda ts: [(t["name"], bool(t.get("moderated"))) for t in ts or []]
            return norm(have) == norm(want)
        if key == "default_reaction_emoji":
            name = lambda e: (e or {}).get("emoji_name")
            return name(have) == name(want)
        return (have or None) == (want or None)

    def do_channels(self) -> None:
        g = self.guild["id"]
        self.by_key: dict[str, dict] = {}
        for spec in self.server["channels"]:
            kind = d.CHANNEL_TYPES[spec["type"]]
            # a text channel may become an announcement channel in place; no other type changes
            fits = (kind, d.TEXT) if kind == d.ANNOUNCEMENT else (kind,)
            # found by the bare name, so a new emoji or separator renames it in place
            channel = next((c for c in self.channels if d.bare(c["name"]) == d.bare(spec["name"]) and c["type"] in fits), None)
            if spec.get("keep_permissions"):
                if not channel:
                    sys.exit(f"#{spec['name']} is missing: the Community wizard makes it; switch Community on first")
                diff = {k: v for k, v in {"name": spec["name"], "parent_id": self.parent(spec)}.items() if channel.get(k) != v}
                if diff:
                    self.api.patch(f"/channels/{channel['id']}", diff)
                    channel.update(diff)
                    self.change(f"update #{spec['name']}: {', '.join(sorted(diff))}")
                self.by_key[spec["key"]] = channel
                continue
            if not channel and spec.get("existing") and spec["key"] != "general":
                sys.exit(f"#{spec['name']} is missing: the Community wizard makes it; switch Community on first")
            want = self.wanted(spec, channel)
            if not channel:
                channel = self.api.post(f"/guilds/{g}/channels", {**want, "type": kind})
                channel = {**want, "type": kind, "position": 0, **channel}
                self.channels.append(channel)
                self.change(f"create #{spec['name']} ({spec['type']})")
            else:
                diff = {k: v for k, v in want.items() if not self.same(k, channel.get(k), v)}
                if channel["type"] != kind:
                    diff["type"] = kind
                if diff:
                    self.api.patch(f"/channels/{channel['id']}", diff)
                    channel.update(diff)
                    self.change(f"update #{spec['name']}: {', '.join(sorted(diff))}")
            self.by_key[spec["key"]] = channel
        self.order_channels()
        known = {c["id"] for c in [*self.by_key.values(), *self.cats.values()]}
        left = [c for c in self.channels if c["id"] not in known and c["type"] != d.PUBLIC_THREAD]
        if left:
            self.out("not in server.json, left as they are: " + ", ".join(c["name"] for c in left))

    def order_channels(self) -> None:
        # the categories in their order, then the channels in theirs; Discord sorts channels inside
        # a category by position, so one rising count fits both
        mine = [self.cats[s["key"]] for s in self.server.get("categories", [])] + [self.by_key[s["key"]] for s in self.server["channels"]]
        if any(c.get("dry_run") for c in mine):
            self.change("put the channels in order: " + ", ".join("#" + c["name"] for c in mine))
            return
        now = sorted(mine, key=lambda c: (c.get("position", 0), int(c["id"])))
        if [c["id"] for c in now] == [c["id"] for c in mine] and len({c.get("position") for c in mine}) == len(mine):
            return
        body = [{"id": c["id"], "position": i} for i, c in enumerate(mine)]
        self.api.patch(f"/guilds/{self.guild['id']}/channels", body)
        for i, c in enumerate(mine):
            c["position"] = i
        self.change("put the channels in order: " + ", ".join("#" + c["name"] for c in mine))

    # ----- pinned texts

    def do_texts(self) -> None:
        for spec in self.server["channels"]:
            channel = self.by_key[spec["key"]]
            if spec.get("messages"):
                self.channel_messages(spec, channel)
            for pin in spec.get("pins", []):
                self.forum_pin(spec, channel, pin)

    def channel_messages(self, spec: dict, channel: dict) -> None:
        mine = [] if channel.get("dry_run") else [
            m for m in self.api.messages_after(channel["id"], 0) if m["author"]["id"] == self.me["id"]
        ]
        for m in spec["messages"]:
            content = d.linked(d.text(m["text"]), self.channels)
            first = content.split("\n", 1)[0]
            have = next((x for x in mine if x["content"].split("\n", 1)[0] == first), None)
            if not have:
                self.api.post(f"/channels/{channel['id']}/messages", {"content": content})
                self.change(f"post {m['text']} in #{spec['name']}")
            elif have["content"] != content:
                self.api.patch(f"/channels/{channel['id']}/messages/{have['id']}", {"content": content})
                self.change(f"edit {m['text']} in #{spec['name']}")

    def forum_pin(self, spec: dict, forum: dict, pin: dict) -> None:
        title, content = d.titled(pin["text"])
        content = d.linked(content, self.channels)
        threads = [] if forum.get("dry_run") else self.api.threads(self.guild["id"], forum)
        thread = next((t for t in threads if t["name"] == title and t.get("owner_id") == self.me["id"]), None)
        if not thread:
            thread = self.api.post(f"/channels/{forum['id']}/threads", {"name": title, "message": {"content": content}})
            self.change(f"post \"{title}\" in #{spec['name']}")
            thread.setdefault("flags", 0)
        else:
            starter = self.api.get(f"/channels/{thread['id']}/messages/{thread['id']}")
            if starter["content"] != content:
                self.api.patch(f"/channels/{thread['id']}/messages/{thread['id']}", {"content": content})
                self.change(f"edit \"{title}\" in #{spec['name']}")
        pinned = thread.get("flags", 0) & d.THREAD_PINNED
        archived = (thread.get("thread_metadata") or {}).get("archived")
        if not pinned or archived:
            self.api.patch(f"/channels/{thread['id']}", {"flags": d.THREAD_PINNED, "archived": False})
            self.change(f"{'pin' if not pinned else 'reopen'} \"{title}\" in #{spec['name']}")

    # ----- AutoMod and the webhook

    def do_automod(self) -> None:
        g = self.guild["id"]
        alerts = self.by_key[self.server["automod_alerts"]]["id"]
        exempt = sorted(self.role_named(n)["id"] for n in self.server["automod_exempt_roles"])
        rules = self.api.get(f"/guilds/{g}/auto-moderation/rules")
        for spec in self.server["automod"]:
            actions = []
            for a in spec["actions"]:
                meta = {"channel_id": alerts} if a == "alert" else {"duration_seconds": 600} if a == "timeout" else {}
                actions.append({"type": AUTOMOD_ACTIONS[a], "metadata": meta})
            want = {
                "name": spec["name"], "event_type": 1, "trigger_type": spec["trigger_type"],
                "trigger_metadata": spec.get("trigger_metadata", {}), "actions": actions,
                "enabled": True, "exempt_roles": exempt,
            }
            rule = next((r for r in rules if r["name"] == spec["name"]), None)
            if not rule:
                self.api.post(f"/guilds/{g}/auto-moderation/rules", want)
                self.change(f"create AutoMod rule {spec['name']}")
                continue
            if rule["trigger_type"] != spec["trigger_type"]:
                self.out(f"AutoMod rule {spec['name']} has another trigger type; delete it by hand to remake it")
                continue
            diff = {}
            meta = rule.get("trigger_metadata") or {}
            if any(meta.get(k) != v for k, v in want["trigger_metadata"].items()):
                diff["trigger_metadata"] = want["trigger_metadata"]
            norm = lambda acts: sorted((a["type"], tuple(sorted((k, v) for k, v in (a.get("metadata") or {}).items() if k in ("channel_id", "duration_seconds")))) for a in acts)
            if norm(rule["actions"]) != norm(actions):
                diff["actions"] = actions
            if not rule.get("enabled"):
                diff["enabled"] = True
            if sorted(rule.get("exempt_roles", [])) != exempt:
                diff["exempt_roles"] = exempt
            if diff:
                self.api.patch(f"/guilds/{g}/auto-moderation/rules/{rule['id']}", diff)
                self.change(f"update AutoMod rule {spec['name']}: {', '.join(sorted(diff))}")

    def do_webhook(self) -> None:
        spec = self.server["webhook"]
        channel = self.by_key[spec["channel"]]
        hooks = [] if channel.get("dry_run") else self.api.get(f"/channels/{channel['id']}/webhooks")
        if not any(h["name"] == spec["name"] for h in hooks):
            self.api.post(f"/channels/{channel['id']}/webhooks", {"name": spec["name"]})
            self.change(f"create the webhook \"{spec['name']}\" in #{channel['name']}")


    # ----- onboarding

    def do_onboarding(self) -> None:
        """Onboarding on, with the default channels of server.json (Discord asks for at least 7, 5 of
        them open to write). The Server Guide is not in the documented API: the owner's step."""
        spec = self.server.get("onboarding")
        if not spec:
            return
        g = self.guild["id"]
        chans = [self.by_key[k] for k in spec["default_channels"]]
        said = "turn onboarding on with the default channels " + ", ".join("#" + c["name"] for c in chans)
        if any(c.get("dry_run") for c in chans):
            self.change(said)
            return
        want = {"default_channel_ids": sorted(c["id"] for c in chans), "enabled": True, "mode": 0}
        have = self.api.get(f"/guilds/{g}/onboarding")
        if sorted(have.get("default_channel_ids") or []) == want["default_channel_ids"] and have.get("enabled") and have.get("mode") == 0:
            return
        self.api.put(f"/guilds/{g}/onboarding", {**want, "prompts": have.get("prompts") or []})
        self.change(said)


def trim(api: d.Discord, server: dict, out=print) -> int:
    """The bot's own role from Administrator down to what the other scripts need."""
    me, guild = api.me(), api.the_guild()
    roles = api.get(f"/guilds/{guild['id']}/roles")
    role = next(r for r in roles if (r.get("tags") or {}).get("bot_id") == me["id"])
    want = d.bits(server["bot_role"]["trimmed_permissions"])
    if int(role["permissions"]) == want:
        out("nothing to change: the bot's role already has only what it needs")
        return 0
    if api.dry_run:
        out(f"would set the bot's role {role['name']} to: {', '.join(server['bot_role']['trimmed_permissions'])}")
        return 1
    try:
        api.patch(f"/guilds/{guild['id']}/roles/{role['id']}", {"permissions": str(want)})
        out(f"the bot's role {role['name']} now has: {', '.join(server['bot_role']['trimmed_permissions'])}")
    except d.DiscordError as e:
        # a bot may not be allowed to change its own highest role; the invite link sets it instead
        out(f"Discord refused the change ({e.status}). Open this link, pick the server and Authorize; it sets the bot's role to these permissions:")
        out(f"https://discord.com/oauth2/authorize?client_id={me['id']}&scope=bot&permissions={want}&guild_id={guild['id']}&disable_guild_select=true")
    return 1


def main() -> None:
    d.utf8_stdout()
    dry = "--dry-run" in sys.argv
    api = d.client(dry_run=dry)
    server = d.load_server()
    if "--trim" in sys.argv:
        trim(api, server)
    else:
        Setup(api, server).run()


if __name__ == "__main__":
    main()
