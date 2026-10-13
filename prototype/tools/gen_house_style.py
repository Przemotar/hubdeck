"""One calm house style for every key HubDeck makes (owner, 2026-10-12: "something sensible that
fits app icons and pictures"), three variants to pick from.

    .venv/Scripts/python -I prototype/tools/gen_house_style.py <out-dir>

Keys are drawn the way the engine draws them (render.rs: icon 30 px at 21,8, label 11.5 px at
y 61; a picture fills the key with the label on a dark strip). App icons stand centred like an
icon. The app icons and the game cover here are made-up stand-ins, not real logos.
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # run with -I: our own folder only
from gen_demo_art import glyph, hsl, panorama  # noqa: E402

# (label, icon or None, group) ; group picks the tint; "app:<n>" an app icon, "cover" a game cover
KEYS = [
    ("Animation", "sparkles", "show"), ("Clock", "clock", "live"), ("CPU", "zap", "live"), ("Weather", "sun", "live"), ("Mic", "mic", "sound"),
    ("Editor", "app:0", "app"), ("Browser", "app:1", "app"), ("Chat", "app:2", "app"), ("My game", "cover", "app"), ("More", "folder", "nav"),
    ("Prev", "skip-back", "media"), ("Play", "play", "media"), ("Next", "skip-forward", "media"), ("Screenshot", "camera", "tool"), ("Profiles", "layers", "nav"),
]
HUES = {"show": 270, "live": 190, "sound": 350, "app": 230, "nav": 250, "media": 160, "tool": 30}


def app_icon(n, x, y, size):
    """A made-up app icon: a rounded square with a simple mark (no real brand)."""
    a, b, mark = [("#4f8cff", "#2a5bd7", "M7 16l5-8 5 8z"), ("#ff8a3d", "#e8572a", "M12 6a6 6 0 1 0 0.01 0zM12 9a3 3 0 1 1-0.01 0z"), ("#3ddc97", "#1fa86f", "M6 8h12v7H11l-4 3v-3H6z")][n]
    s = size / 24
    return (f'<g transform="translate({x} {y}) scale({s})"><defs><linearGradient id="ai{n}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{a}"/>'
            f'<stop offset="1" stop-color="{b}"/></linearGradient></defs><rect x="1" y="1" width="22" height="22" rx="5.5" fill="url(#ai{n})"/>'
            f'<path d="{mark}" fill="#ffffff" fill-rule="evenodd"/></g>')


def text(x, y, size, fill, s):
    return f'<text x="{x}" y="{y}" text-anchor="middle" font-family="IBM Plex Sans, sans-serif" font-weight="600" font-size="{size}" fill="{fill}">{s}</text>'


def key(variant, label, ic, group):
    h = HUES[group]
    if variant == "graphite":
        bg, fg, icol, lab = "#1d1c23", "#f0eef6", "#f0eef6", "#c9c6d4"
        extra = f'<rect x="30" y="66" width="12" height="2.5" rx="1.25" fill="{hsl(h, 0.65, 0.62)}"/>'
    elif variant == "tint":
        bg, fg, icol, lab = hsl(h, 0.28, 0.14), "#f0eef6", "#f0eef6", "#d8d5e2"
        extra = ""
    else:  # accent
        bg, fg, icol, lab = "#1b1a20", "#f0eef6", hsl(h, 0.75, 0.70), "#e6e4ee"
        extra = ""
    top = f'<rect width="72" height="72" fill="{bg}"/><rect width="72" height="36" fill="#ffffff" opacity="0.025"/>'
    if ic == "cover":
        pic = f'<svg width="72" height="72" viewBox="250 90 120 120" preserveAspectRatio="xMidYMid slice">{panorama()}</svg>'
        return pic + f'<rect y="52.45" width="72" height="19.55" fill="#000" fill-opacity="0.55"/>' + text(36, 66.1, 11.5, "#f0eef6", label)
    body = app_icon(int(ic[4:]), 21, 8, 30) if ic.startswith("app:") else glyph(ic, 21, 8, 30, icol, 2)
    label_y = 61 if variant != "graphite" else 59
    return top + body + text(36, label_y, 11.5, lab, label) + extra


def dial(variant):
    h = 270
    if variant == "graphite":
        bg, ring, track = "#1d1c23", "#f0eef6", "#34323d"
    elif variant == "tint":
        bg, ring, track = hsl(h, 0.28, 0.14), hsl(h, 0.8, 0.78), hsl(h, 0.2, 0.25)
    else:
        bg, ring, track = "#1b1a20", hsl(h, 0.75, 0.70), "#302e38"
    return (f'<rect width="240" height="240" fill="{bg}"/><circle cx="120" cy="120" r="104" fill="none" stroke="{track}" stroke-width="12"/>'
            f'<circle cx="120" cy="120" r="104" fill="none" stroke="{ring}" stroke-width="12" stroke-linecap="round" stroke-dasharray="470.5 999" transform="rotate(-90 120 120)"/>'
            + text(120, 142, 64, "#f0eef6", "72") + text(120, 182, 20, "#a8a4b5", "Volume"))


VARIANTS = [
    ("graphite", "1. Graphite", "Every key the same quiet graphite, white icon and label; a short colored line at the bottom tells the group. App icons and covers carry the color."),
    ("tint", "2. Soft tint", "A dark key tinted by group (what the ready-made profiles use today), white icon and label. Calm, groups readable at a glance."),
    ("accent", "3. Colored icon", "Graphite key, the icon in its group's soft color, white label. Color only where the eye goes."),
]

_UID = [0]


def unique(svg):
    _UID[0] += 1
    p = f"u{_UID[0]}-"
    svg = re.sub(r'id="([^"]+)"', lambda m: f'id="{p}{m.group(1)}"', svg)
    return re.sub(r"url\(#([^)]+)\)", lambda m: f"url(#{p}{m.group(1)})", svg)


def page(out):
    sections = []
    for vid, title, blurb in VARIANTS:
        keys = "".join(f'<figure class="key">{unique(f"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 72 72\">{key(vid, *k)}</svg>")}</figure>' for k in KEYS)
        d = unique(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240">{dial(vid)}</svg>')
        sections.append(f"""<section class="style"><header><h2>{title}</h2><p>{blurb}</p></header>
<div class="deck"><div class="panel">{keys}</div><div class="dialwrap"><div class="dial">{d}</div><span class="cap">Dial: volume</span></div></div></section>""")
    html = f"""<title>HubDeck House Style</title>
<style>
:root {{
  /* a dark stage like the deck on a desk; the keys are the subject */
  --bg: #0c0b10; --panel: #111016; --line: #24222c; --fg: #ecebf2; --muted: #9a98a8; --acc: #b9a8ff;
  --display: "Space Mono", ui-monospace, monospace; --body: "IBM Plex Sans", system-ui, sans-serif;
  color-scheme: dark;
}}
body {{ background: var(--bg); color: var(--fg); font-family: var(--body); font-size: 15px; line-height: 1.5; padding: 32px 16px 56px; }}
main {{ max-width: 980px; margin: 0 auto; display: grid; gap: 36px; }}
h1 {{ font-family: var(--display); font-size: 24px; margin: 0 0 6px; text-wrap: balance; }}
.lead {{ color: var(--muted); max-width: 66ch; margin: 0; }}
.style header {{ display: grid; gap: 2px; margin-bottom: 12px; }}
.style h2 {{ font-family: var(--display); font-size: 17px; margin: 0; color: var(--acc); }}
.style p {{ margin: 0; color: var(--muted); max-width: 70ch; }}
.deck {{ display: flex; flex-wrap: wrap; gap: 18px; align-items: center; padding: 22px; border-radius: 22px; background: var(--panel); box-shadow: inset 0 0 0 1px var(--line); }}
.panel {{ display: grid; grid-template-columns: repeat(5, minmax(0, 72px)); gap: 12px; padding: 16px; border-radius: 12px; background: #050507; }}
.key {{ margin: 0; aspect-ratio: 1; max-width: 100%; border-radius: 9px; overflow: hidden; box-shadow: 0 0 0 1px #000, 0 4px 10px -4px #000; }}
.key svg, .dial svg {{ display: block; width: 100%; height: 100%; }}
.dialwrap {{ display: grid; justify-items: center; gap: 8px; }}
.dial {{ width: 180px; max-width: 100%; aspect-ratio: 1; border-radius: 50%; overflow: hidden; box-shadow: 0 0 0 10px #1a191f, 0 0 0 12px #2a2830; }}
.cap {{ color: var(--muted); font-size: 13px; }}
@media (max-width: 520px) {{ .panel {{ gap: 8px; padding: 10px; }} .deck {{ padding: 14px; }} }}
</style>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;600&family=Space+Mono:wght@700&display=swap">
<main>
  <div><h1>One house style, three calm options</h1>
  <p class="lead">Keys HubDeck makes (demo, ready-made profiles, suggestions) in one quiet look, drawn by the app itself, so they sit well next to app icons and game covers. Middle row: three made-up app icons (centred, not stretched) and a game cover. Real keys are 72 px; these are the same size.</p></div>
  {"".join(sections)}
</main>"""
    (out / "house.html").write_text(html, encoding="utf-8")
    print(out / "house.html")


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    page(out)
