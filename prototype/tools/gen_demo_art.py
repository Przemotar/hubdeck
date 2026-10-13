"""Art for the demo profile "Discover HubDeck" in three styles, for the owner to pick (2026-10-12).

    .venv/Scripts/python -I prototype/tools/gen_demo_art.py <out-dir> [style]

Without a style: writes `variants.html` (all three styles on the 15 keys and the dial, labels drawn
the way the app draws them over a picture). With a style (neon, glass, panorama): writes that
style's SVG files (`key-<n>.svg`, `dial.svg`) for `app/src-tauri/resources/demo/`.
All art is ours (MIT), built from the Lucide icons the app already bundles (ISC).
"""

import math
import re
import sys
from pathlib import Path

ICONS = Path(__file__).resolve().parents[2] / "app" / "src-tauri" / "assets" / "icons"

# the 15 keys of the demo's main page: what each one shows off, its label, icon and hue
KEYS = [
    ("animation", "Animation", "sparkles", 280),
    ("clock", "Clock", "clock", 200),
    ("cpu", "CPU", "zap", 50),
    ("weather", "Weather", "sun", 35),
    ("live", "Live value", "radio", 160),
    ("twolooks", "Mic", "mic", 350),
    ("hold", "Hold me", "mouse-pointer-click", 260),
    ("page", "More", "folder", 215),
    ("macro", "Macro", "list-ordered", 120),
    ("open", "Open app", "app-window", 190),
    ("timer", "Timer", "timer", 15),
    ("lights", "Lights", "lightbulb", 55),
    ("shortcut", "Screenshot", "camera", 300),
    ("profiles", "Profiles", "layers", 230),
    ("music", "Play", "play", 140),
]


def icon_body(name):
    text = (ICONS / f"{name}.svg").read_text(encoding="utf-8")
    inner = text.split(">", 2)[2] if text.startswith("<!--") else text.split(">", 1)[1]
    inner = inner.rsplit("</svg>", 1)[0]
    return re.sub(r"\s+", " ", inner).strip()


def hsl(h, s, l):
    """CSS-free hex from HSL (degrees, 0-1, 0-1), so resvg and every browser read it the same."""
    s, l = max(0, min(1, s)), max(0, min(1, l))
    c = (1 - abs(2 * l - 1)) * s
    x = c * (1 - abs((h / 60) % 2 - 1))
    m = l - c / 2
    r, g, b = [(c, x, 0), (x, c, 0), (0, c, x), (0, x, c), (x, 0, c), (c, 0, x)][int(h % 360 // 60)]
    return "#%02x%02x%02x" % tuple(round((v + m) * 255) for v in (r, g, b))


def glyph(name, x, y, size, stroke, width=2.0, extra=""):
    s = size / 24
    return f'<g transform="translate({x} {y}) scale({s})" fill="none" stroke="{stroke}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round" {extra}>{icon_body(name)}</g>'


# ---------- style A: neon line ----------
def neon_key(i, k):
    _, _, icon, h = k
    bg1, bg2, line = hsl(h, 0.55, 0.13), hsl(h + 25, 0.6, 0.06), hsl(h, 1.0, 0.68)
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{bg1}"/><stop offset="1" stop-color="{bg2}"/></linearGradient>
<filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="2.2"/></filter></defs>
<rect width="72" height="72" fill="url(#g)"/>
<circle cx="36" cy="30" r="22" fill="{line}" opacity="0.10"/>
{glyph(icon, 20, 14, 32, line, 2.2, 'filter="url(#glow)" opacity="0.9"')}
{glyph(icon, 20, 14, 32, "#ffffff", 1.6, 'opacity="0.95"')}
</svg>"""


def neon_dial():
    h = 280
    line = hsl(h, 1, 0.68)
    ticks = "".join(
        f'<line x1="120" y1="22" x2="120" y2="{32 if t % 5 else 40}" stroke="{line}" stroke-width="{2 if t % 5 else 3}" stroke-linecap="round" opacity="{0.35 if t > 42 else 0.95}" transform="rotate({t * 6} 120 120)"/>'
        for t in range(60)
    )
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">
<defs><radialGradient id="bg" cx="0.5" cy="0.45" r="0.6"><stop offset="0" stop-color="{hsl(h, 0.6, 0.16)}"/><stop offset="1" stop-color="{hsl(h + 30, 0.6, 0.04)}"/></radialGradient>
<filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4"/></filter></defs>
<rect width="240" height="240" fill="url(#bg)"/>
<g filter="url(#glow)">{ticks}</g>{ticks}
{glyph("volume-2", 88, 70, 64, "#ffffff", 1.8)}
<text x="120" y="172" text-anchor="middle" font-family="IBM Plex Sans" font-weight="600" font-size="34" fill="#ffffff">72</text>
</svg>"""


# ---------- style B: gradient glass ----------
def glass_key(i, k):
    _, _, icon, h = k
    a, b = hsl(h, 0.85, 0.58), hsl(h + 40, 0.9, 0.42)
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{a}"/><stop offset="1" stop-color="{b}"/></linearGradient>
<linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff" stop-opacity="0.45"/><stop offset="1" stop-color="#ffffff" stop-opacity="0"/></linearGradient>
<filter id="sh" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="1.6"/></filter></defs>
<rect width="72" height="72" fill="url(#g)"/>
<circle cx="58" cy="10" r="30" fill="#ffffff" opacity="0.10"/>
<path d="M0 0H72V24C52 32 22 30 0 22Z" fill="url(#sheen)" opacity="0.55"/>
{glyph(icon, 20, 15, 32, "#000000", 2.6, 'filter="url(#sh)" opacity="0.28" transform="translate(21 17) scale(1.3333)"')}
{glyph(icon, 20, 14, 32, "#ffffff", 2.3)}
</svg>"""


def glass_dial():
    h = 280
    a, b = hsl(h, 0.85, 0.55), hsl(h + 50, 0.9, 0.40)
    r = 96
    arc = 0.72 * 2 * math.pi * r
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{a}"/><stop offset="1" stop-color="{b}"/></linearGradient>
<linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff" stop-opacity="0.4"/><stop offset="1" stop-color="#ffffff" stop-opacity="0"/></linearGradient></defs>
<rect width="240" height="240" fill="url(#g)"/>
<path d="M0 0H240V80C170 104 70 100 0 74Z" fill="url(#sheen)" opacity="0.6"/>
<circle cx="120" cy="120" r="{r}" fill="none" stroke="#ffffff" stroke-opacity="0.22" stroke-width="12"/>
<circle cx="120" cy="120" r="{r}" fill="none" stroke="#ffffff" stroke-width="12" stroke-linecap="round" stroke-dasharray="{arc:.1f} 999" transform="rotate(-90 120 120)"/>
{glyph("volume-2", 90, 66, 60, "#ffffff", 2)}
<text x="120" y="170" text-anchor="middle" font-family="IBM Plex Sans" font-weight="600" font-size="34" fill="#ffffff">72</text>
</svg>"""


# ---------- style C: one panorama across the keys ----------
PANEL_W, PANEL_H, DX, DY, KW = 480, 272, 100, 99, 79


def panorama():
    """A dusk landscape for the whole 480x272 panel (sun, layered hills, stars)."""
    stars = "".join(f'<circle cx="{(i * 137) % 480}" cy="{(i * 71) % 120}" r="{0.6 + (i % 3) * 0.4}" fill="#ffffff" opacity="{0.35 + (i % 4) * 0.15}"/>' for i in range(46))
    hills = ""
    for n, (base, amp, col) in enumerate([(150, 26, "#3b2a6b"), (180, 22, "#2a1d52"), (212, 18, "#1b1338"), (244, 14, "#0f0a22")]):
        pts = " ".join(f"{x},{base - amp * math.sin(x / (60 + n * 17) + n * 1.7) - amp * 0.5 * math.sin(x / 23 + n)}" for x in range(0, 481, 8))
        hills += f'<polygon points="0,272 {pts} 480,272" fill="{col}"/>'
    return f"""<defs><linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#140f33"/><stop offset="0.55" stop-color="#6b2d6e"/><stop offset="0.8" stop-color="#f08a5d"/></linearGradient>
<radialGradient id="sun" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#ffd27a"/><stop offset="0.6" stop-color="#ff9a62" stop-opacity="0.8"/><stop offset="1" stop-color="#ff9a62" stop-opacity="0"/></radialGradient></defs>
<rect width="480" height="272" fill="url(#sky)"/>{stars}<circle cx="300" cy="150" r="70" fill="url(#sun)"/>{hills}"""


def panorama_key(i, k):
    _, _, icon, _ = k
    x, y = (i % 5) * DX, (i // 5) * DY
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="{x} {y} {KW} {KW}">
{panorama()}
<g transform="translate({x} {y}) scale({KW / 72})">
<circle cx="36" cy="30" r="17" fill="#000000" opacity="0.28"/>
{glyph(icon, 23, 17, 26, "#ffffff", 2.1)}
</g></svg>"""


def panorama_dial():
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="120 16 240 240">
{panorama()}
<g transform="translate(120 16)">
<circle cx="120" cy="120" r="54" fill="#000000" opacity="0.30"/>
{glyph("volume-2", 92, 74, 56, "#ffffff", 2)}
<text x="120" y="178" text-anchor="middle" font-family="IBM Plex Sans" font-weight="600" font-size="30" fill="#ffffff">72</text>
</g></svg>"""


STYLES = {
    "neon": ("A. Neon", "Dark tinted keys, one glowing line icon each; grows from the current showcase art.", neon_key, neon_dial),
    "glass": ("B. Glass", "Vivid gradient per feature, a white glyph with a soft shadow and a glossy top.", glass_key, glass_dial),
    "panorama": ("C. Panorama", "One dusk landscape across all 15 keys and the dial; small glyphs on top. Shows off a picture split over the panel.", panorama_key, panorama_dial),
}


def label_strip(label):
    """The app's own label over a picture: a dark strip at the bottom, 11.5 px semibold."""
    return f'<rect y="52.45" width="72" height="19.55" fill="#000" fill-opacity="0.55"/><text x="36" y="66.1" text-anchor="middle" font-family="IBM Plex Sans, sans-serif" font-weight="600" font-size="11.5" fill="#f0eef6">{label}</text>'


def with_label(svg, label):
    # put the strip in the key's own 72x72 space (panorama keys use a shifted viewBox)
    m = re.search(r'viewBox="([-\d.]+) ([-\d.]+) ([-\d.]+) ([-\d.]+)"', svg)
    x, y, w, _ = (float(v) for v in m.groups())
    strip = f'<g transform="translate({x} {y}) scale({w / 72})">{label_strip(label)}</g>'
    return svg.replace("</svg>", strip + "</svg>")


_UID = [0]


def unique(svg):
    """Ids made unique per picture: on one HTML page all inline SVGs share one id space."""
    _UID[0] += 1
    p = f"s{_UID[0]}-"
    svg = re.sub(r'id="([^"]+)"', lambda m: f'id="{p}{m.group(1)}"', svg)
    return re.sub(r"url\(#([^)]+)\)", lambda m: f"url(#{p}{m.group(1)})", svg)


def page(out):
    sections = []
    for sid, (title, blurb, key, dial) in STYLES.items():
        keys = "".join(f'<figure class="key">{unique(with_label(key(i, k), k[1]))}</figure>' for i, k in enumerate(KEYS))
        sections.append(
            f"""<section class="style" id="{sid}">
  <header><h2>{title}</h2><p>{blurb}</p></header>
  <div class="deck">
    <div class="panel">{keys}</div>
    <div class="dialwrap"><div class="dial">{unique(dial())}</div><span class="cap">Dial: volume 72%</span></div>
  </div>
</section>"""
        )
    legend = "".join(f"<li><b>{k[1]}</b> {d}</li>" for k, d in zip(KEYS, [
        "an animated picture (GIF)", "the time, live", "CPU load, live", "weather from Open-Meteo", "any number from a website or Home Assistant",
        "a second look while muted", "a different action on hold and double press", "a page of more keys", "several steps on one press",
        "opens an app", "a countdown with a ring", "a lighting effect on knobs and the edge", "a shortcut picked by name", "switches profiles", "media keys with the track's state"]))
    html = f"""<title>HubDeck Demo Art</title>
<style>
:root {{
  /* one dark stage, like the deck on a desk; the art is the subject */
  --bg: #0c0b10; --panel: #111016; --line: #24222c; --fg: #ecebf2; --muted: #9a98a8; --acc: #b9a8ff;
  --display: "Space Mono", ui-monospace, monospace; --body: "IBM Plex Sans", system-ui, sans-serif;
  color-scheme: dark;
}}
body {{ background: var(--bg); color: var(--fg); font-family: var(--body); font-size: 15px; line-height: 1.5; padding: 32px 16px 56px; }}
main {{ max-width: 980px; margin: 0 auto; display: grid; gap: 40px; }}
h1 {{ font-family: var(--display); font-size: 26px; letter-spacing: 0.02em; margin: 0 0 6px; text-wrap: balance; }}
.lead {{ color: var(--muted); max-width: 64ch; margin: 0; }}
.style header {{ display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 14px; margin-bottom: 14px; }}
.style h2 {{ font-family: var(--display); font-size: 18px; margin: 0; color: var(--acc); }}
.style p {{ margin: 0; color: var(--muted); }}
.deck {{ display: flex; flex-wrap: wrap; gap: 18px; align-items: center; padding: 22px; border-radius: 22px; background: var(--panel); box-shadow: inset 0 0 0 1px var(--line); }}
.panel {{ display: grid; grid-template-columns: repeat(5, minmax(0, 72px)); gap: 12px; padding: 16px; border-radius: 12px; background: #050507; }}
.key {{ margin: 0; aspect-ratio: 1; max-width: 100%; border-radius: 9px; overflow: hidden; box-shadow: 0 0 0 1px #000, 0 4px 10px -4px #000; }}
.key svg, .dial svg {{ display: block; width: 100%; height: 100%; }}
.dialwrap {{ display: grid; justify-items: center; gap: 8px; }}
.dial {{ width: 200px; max-width: 100%; aspect-ratio: 1; border-radius: 50%; overflow: hidden; box-shadow: 0 0 0 10px #1a191f, 0 0 0 12px #2a2830; }}
.cap {{ color: var(--muted); font-size: 13px; }}
.legend {{ columns: 2 280px; color: var(--muted); padding-left: 18px; margin: 0; }}
.legend b {{ color: var(--fg); font-weight: 600; }}
@media (max-width: 520px) {{ .panel {{ gap: 8px; padding: 10px; }} .deck {{ padding: 14px; }} }}
</style>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;600&family=Space+Mono:wght@700&display=swap">
<main>
  <div>
    <h1>Discover HubDeck: three looks</h1>
    <p class="lead">The demo profile shows one feature per key. Pick the look you like; labels are drawn the way the app draws them over a picture. Real keys are 72 px; these are drawn at the same size.</p>
  </div>
  {"".join(sections)}
  <section><h2 style="font-family:var(--display);font-size:16px;margin:0 0 8px">What each key shows</h2><ul class="legend">{legend}</ul></section>
</main>"""
    (out / "variants.html").write_text(html, encoding="utf-8")
    print(out / "variants.html")


def files(out, style):
    _, _, key, dial = STYLES[style]
    for i, k in enumerate(KEYS):
        (out / f"key-{k[0]}.svg").write_text(key(i, k), encoding="utf-8")
    (out / "dial.svg").write_text(dial(), encoding="utf-8")
    print(f"{len(KEYS) + 1} files in {out}")


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    if len(sys.argv) > 2:
        files(out, sys.argv[2])
    else:
        page(out)
