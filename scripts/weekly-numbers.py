"""The weekly number, without tracking anyone (owner's strategy, 2026-11-01: "1 a"; ROADMAP > Now N1).

What it reads, all public counters or the server's own member list:
  - update checks: every installed HubDeck downloads `latest.json` of the release that is Latest at
    the moment, at start and every 6 hours (`update.rs` CHECK_EVERY); GitHub counts the downloads per
    release asset. Checks a day / 4 (always on) .. / 2 (started once a day, closed after a few hours)
    gives a rough range of running copies. The owner's own restarts and the release gate add some.
  - installers downloaded per release, stars, forks, GitHub's 14-day views and clones;
  - Discord members (when the bot token is set up for scripts/discord_api.py).

Each run appends a snapshot to private/metrics/numbers.jsonl and prints the change since the last
one, so a week's number is the difference between two runs.

  python -I scripts/weekly-numbers.py            # print and save a snapshot
  python -I scripts/weekly-numbers.py --dry-run  # print only
"""

import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = "Przemotar/hubdeck"
ROOT = Path(__file__).resolve().parent.parent
LOG = ROOT / "private" / "metrics" / "numbers.jsonl"


def gh(path: str):
    out = subprocess.run(["gh", "api", path], capture_output=True, text=True, encoding="utf-8")
    if out.returncode != 0:
        raise SystemExit(f"gh api {path}: {out.stderr.strip()}")
    return json.loads(out.stdout)


def parse(t: str | None) -> datetime | None:
    return datetime.fromisoformat(t.replace("Z", "+00:00")) if t else None


def discord_members() -> int | None:
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import discord_api  # noqa: E402

        api = discord_api.client()
        guild = api.the_guild()
        full = api.get(f"/guilds/{guild['id']}", with_counts="true")
        return full.get("approximate_member_count")
    except Exception:  # no token here, or the API refused: the number is left out
        return None


def snapshot() -> dict:
    now = datetime.now(timezone.utc)
    releases = [r for r in gh(f"repos/{REPO}/releases?per_page=100") if not r.get("draft")]
    releases.sort(key=lambda r: r["published_at"])
    rows = []
    for i, r in enumerate(releases):
        start = parse(r["published_at"])
        end = parse(releases[i + 1]["published_at"]) if i + 1 < len(releases) else now
        assets = {a["name"]: a["download_count"] for a in r["assets"]}
        installers = sum(n for name, n in assets.items() if name.endswith(("setup.exe", ".dmg", ".msi")))
        rows.append({"tag": r["tag_name"], "latest_from": start.isoformat(), "latest_to": end.isoformat(), "checks": assets.get("latest.json", 0), "installers": installers})
    repo = gh(f"repos/{REPO}")
    views = gh(f"repos/{REPO}/traffic/views")
    clones = gh(f"repos/{REPO}/traffic/clones")
    return {
        "at": now.isoformat(),
        "releases": rows,
        "stars": repo["stargazers_count"],
        "forks": repo["forks_count"],
        "views14": views["count"],
        "visitors14": views["uniques"],
        "cloners14": clones["uniques"],
        "discord": discord_members(),
    }


def report(s: dict, last: dict | None) -> str:
    out = [f"HubDeck numbers, {s['at'][:16].replace('T', ' ')} UTC"]
    # update checks while each release was Latest (the last three)
    for r in s["releases"][-3:]:
        days = max((parse(r["latest_to"]) - parse(r["latest_from"])).total_seconds() / 86400, 0.01)
        a_day = r["checks"] / days
        out.append(f"  {r['tag']}: Latest for {days:.1f} days, {r['checks']} update checks ({a_day:.0f} a day: ~{a_day / 4:.0f}-{a_day / 2:.0f} running copies), {r['installers']} installers downloaded")
    out.append(f"  GitHub: {s['stars']} stars, {s['forks']} forks, {s['visitors14']} visitors and {s['cloners14']} cloners in 14 days")
    out.append(f"  Discord: {s['discord'] if s['discord'] is not None else 'n/a (no bot token here)'} members")
    if last:
        days = (parse(s["at"]) - parse(last["at"])).total_seconds() / 86400
        checks = sum(r["checks"] for r in s["releases"]) - sum(r["checks"] for r in last["releases"])
        inst = sum(r["installers"] for r in s["releases"]) - sum(r["installers"] for r in last["releases"])
        a_day = checks / max(days, 0.01)
        line = f"  since {last['at'][:10]} ({days:.1f} days): {checks} update checks ({a_day:.0f} a day: ~{a_day / 4:.0f}-{a_day / 2:.0f} running copies), {inst} installers, stars {s['stars'] - last['stars']:+d}"
        if s["discord"] is not None and last.get("discord") is not None:
            line += f", Discord {s['discord'] - last['discord']:+d}"
        out.append(line)
    return "\n".join(out)


def main() -> None:
    dry = "--dry-run" in sys.argv
    last = None
    if LOG.exists():
        lines = [line for line in LOG.read_text(encoding="utf-8").splitlines() if line.strip()]
        last = json.loads(lines[-1]) if lines else None
    s = snapshot()
    print(report(s, last))
    if not dry:
        LOG.parent.mkdir(parents=True, exist_ok=True)
        with LOG.open("a", encoding="utf-8") as f:
            f.write(json.dumps(s) + "\n")


if __name__ == "__main__":
    main()
