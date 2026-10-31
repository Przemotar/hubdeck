"""Every living-deck preset with what it is made of and what it cost (owner, 2026-10-29: "Chcę dla każdego
z presetów mieć podgląd - ile linijek zajmują, ile ważą, ile czasu powstawał, ile kosztowały").

Built-in presets (`engine.ts` THEMES): their GPU layers' shader files (lines, bytes); the time from the
first to the last commit that touched those files; the cost as the usage of every model call in this
project's session transcripts whose tool input names one of those files (a call naming several files is
split between them), so it is a lower bound: work on a theme that did not name its file is not counted.
Test presets (A/B branches in `private/ab/`): the shader file, and the agent's own transcript (its whole
usage, its duration).

    python -I scripts/ui/preset-ledger.py [--json out.json] [--md out.md]
"""
import argparse
import collections
import datetime
import json
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
SH = ROOT / "app/src-tauri/src/engine/shaders"
PROJ = pathlib.Path.home() / ".claude/projects/C--Users-przem-claude-projects-masterhub"
KEYS = ("output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "input_tokens")


def themes():
    s = (ROOT / "app/src/engine.ts").read_text(encoding="utf-8")
    blk = s[s.index("export const THEMES"):]
    blk = blk[:blk.index("\n];")]
    out = {}
    for m in re.finditer(r'id: "([a-z0-9-]+)"(.*?)(?=id: "|\Z)', blk, re.S):
        out[m.group(1)] = sorted(set(re.findall(r'kind: "([a-z]+)"', m.group(2))))
    return out


def file_stats(paths):
    lines = sum(len(p.read_text(encoding="utf-8").splitlines()) for p in paths)
    return lines, sum(p.stat().st_size for p in paths)


def commit_span(paths):
    rel = [str(p.relative_to(ROOT)).replace("\\", "/") for p in paths]
    if not rel:
        return None, None, 0
    log = subprocess.run(["git", "log", "--format=%ad", "--date=iso-strict", "--", *rel], cwd=ROOT, capture_output=True, text=True).stdout.split()
    if not log:
        return None, None, 0
    ts = sorted(datetime.datetime.fromisoformat(x) for x in log)
    return ts[0], ts[-1], len(ts)


def usage_of(lines):
    tot = collections.Counter()
    seen = set()
    first = last = None
    for line in lines:
        try:
            d = json.loads(line)
        except Exception:
            continue
        if d.get("type") != "assistant":
            continue
        m = d.get("message") or {}
        if m.get("id") in seen or not m.get("usage"):
            continue
        seen.add(m.get("id"))
        t = datetime.datetime.fromisoformat(d["timestamp"].replace("Z", "+00:00"))
        first = first or t
        last = t
        for k in KEYS:
            tot[k] += m["usage"].get(k, 0) or 0
    return tot, first, last


def attributed(names):
    """Usage of the main sessions' calls whose tool input names one of `names` (file names)."""
    per = {n: collections.Counter() for n in names}
    pat = re.compile("|".join(re.escape(n) for n in names))
    for f in PROJ.glob("*.jsonl"):
        seen = set()
        for line in open(f, encoding="utf-8"):
            if '"tool_use"' not in line or not pat.search(line):
                continue
            try:
                d = json.loads(line)
            except Exception:
                continue
            m = d.get("message") or {}
            if m.get("id") in seen or not m.get("usage"):
                continue
            seen.add(m.get("id"))
            text = json.dumps([c.get("input") for c in m.get("content", []) if c.get("type") == "tool_use"])
            hit = sorted(set(pat.findall(text)))
            for n in hit:
                for k in KEYS:
                    per[n][k] += (m["usage"].get(k, 0) or 0) / len(hit)
    return per


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    ap.add_argument("--md")
    a = ap.parse_args()
    rows = []
    th = themes()
    files = {k: SH / f"{k}.wgsl" for ks in th.values() for k in ks if (SH / f"{k}.wgsl").exists()}
    per = attributed([p.name for p in files.values()])
    for tid, kinds in th.items():
        paths = [files[k] for k in kinds if k in files]
        lines, size = file_stats(paths)
        t0, t1, n = commit_span(paths)
        cost = collections.Counter()
        for p in paths:
            cost.update(per[p.name])
        rows.append({"preset": tid, "kind": "built-in", "layers": kinds, "cpu_layers": [k for k in kinds if k not in files],
                     "lines": lines, "bytes": size, "made": f"{t0:%d.%m %H:%M} - {t1:%d.%m %H:%M}" if t0 else "", "commits": n,
                     "minutes": None, "output": round(cost["output_tokens"]), "cache_read": round(cost["cache_read_input_tokens"]),
                     "cost_note": "lower bound: calls that named its files"})
    # test presets: the branches saved under private/ab with their agents' transcripts
    agents = {}
    for meta in (PROJ).glob("*/subagents/agent-*.meta.json"):
        agents[meta.name[len("agent-"):-len(".meta.json")]] = meta.with_name(meta.name.replace(".meta.json", ".jsonl"))
    ab = json.loads((ROOT / "private/ab/agents.json").read_text(encoding="utf-8")) if (ROOT / "private/ab/agents.json").exists() else {}
    for label, info in ab.items():
        p = ROOT / info["file"]
        if not p.exists():
            continue
        lines, size = file_stats([p])
        tr = agents.get(info.get("agent", ""))
        if tr and tr.exists():
            tot, f0, f1 = usage_of(open(tr, encoding="utf-8"))
            mins = round((f1 - f0).total_seconds() / 60, 1) if f0 else None
        else:
            tot, mins = collections.Counter(), None
        rows.append({"preset": label, "kind": info.get("kind", "test"), "layers": [p.stem], "cpu_layers": [], "lines": lines, "bytes": size,
                     "made": info.get("made", ""), "commits": None, "minutes": mins, "output": tot["output_tokens"],
                     "cache_read": tot["cache_read_input_tokens"], "cost_note": info.get("note", "the agent's whole transcript")})
    if a.json:
        pathlib.Path(a.json).write_text(json.dumps(rows, indent=2, ensure_ascii=False), encoding="utf-8")
    md = ["| Preset | Lines | KB | Made | Minutes | Output tokens | Context read | Note |", "|---|---|---|---|---|---|---|---|"]
    for r in rows:
        md.append(f"| {r['preset']} | {r['lines']} | {r['bytes'] / 1024:.0f} | {r['made']} | {r['minutes'] or ''} | {r['output']:,} | {r['cache_read'] / 1e6:.1f} M | {r['cost_note']} |")
    text = "\n".join(md)
    if a.md:
        pathlib.Path(a.md).write_text(text + "\n", encoding="utf-8")
    print(text)


if __name__ == "__main__":
    main()
