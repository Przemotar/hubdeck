"""The app icon "k1" (owner, 2026-10-30: "k1 chyba najlepszy. idziemy w to", "przyciski nie mogą być
ciemne", "Nie rób tego jednego podświetlonego", "bez tego outline i ikona dociągnięta do brzegów"): six light
keys, 3 x 2, on a tile in the app's violet (--acc #8b7cf6), no outline. Made with the skill `app-icon`
(rounds and the art director's notes: private/icon-process/). Contrast at 3:1: 62-65 % of it on a light and
76 % on a dark Windows taskbar at 16-32 px.

Writes app/src-tauri/icons/hubdeck-win.svg (the tile to the edges: Windows' neighbours fill 97-100 % of their
square), hubdeck.svg (the macOS grid, 824 of 1024, for the .icns, the site and README) and
hubdeck-discord.svg (square, the circle crop), then renders the PNGs into icons/build through headless Edge.
Then, in app/: `pnpm tauri icon src-tauri/icons/build/hubdeck-1024.png` (remove icons/android and icons/ios),
and `python -I scripts/icon/pack-icon.py`.

    python -I scripts/icon/build-icon.py
"""
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
ICONS = ROOT / "app" / "src-tauri" / "icons"
BUILD = ICONS / "build"
RENDER = ROOT / ".claude" / "skills" / "app-icon" / "tools" / "render.mjs"

# the app's violet, a narrow ramp so every part of the tile keeps 3:1 on both taskbars
TILE_TOP, TILE_BOTTOM = "#8273f6", "#5b46e4"


def drawing(tile_rx: float) -> str:
    """The icon on a 1024 square: the tile filling it, the six keys."""
    cols, rows, pad, gap = 3, 2, 140, 30
    span = 1024 - 2 * pad
    w = (span - (cols - 1) * gap) / cols
    h = w
    y0 = (1024 - (rows * h + (rows - 1) * gap)) / 2 + 6
    rx = w * 0.24
    defs = f'''<defs>
<linearGradient id="tile" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{TILE_TOP}"/><stop offset="1" stop-color="{TILE_BOTTOM}"/></linearGradient>
<linearGradient id="wall" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#9f93e6"/><stop offset="1" stop-color="#6c5cd0"/></linearGradient>
<linearGradient id="face" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f4f2ff"/><stop offset="1" stop-color="#d4cdfb"/></linearGradient>
<linearGradient id="glass" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".28"/><stop offset=".45" stop-color="#fff" stop-opacity="0"/></linearGradient>
<filter id="drop" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="{w * 0.05:.1f}" stdDeviation="{w * 0.05:.1f}" flood-color="#000" flood-opacity=".45"/></filter>
</defs>'''
    body = f'<rect x="0" y="0" width="1024" height="1024" rx="{tile_rx}" fill="url(#tile)"/>'
    for r in range(rows):
        for c in range(cols):
            x, y = pad + c * (w + gap), y0 + r * (h + gap)
            body += f'<g filter="url(#drop)"><rect x="{x:.1f}" y="{y + w * 0.04:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx:.1f}" fill="url(#wall)"/>'
            body += f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx:.1f}" fill="url(#face)"/></g>'
            body += f'<rect x="{x + 3:.1f}" y="{y + 3:.1f}" width="{w - 6:.1f}" height="{h * 0.55:.1f}" rx="{rx - 3:.1f}" fill="url(#glass)"/>'
            body += f'<rect x="{x + 1.5:.1f}" y="{y + 1.5:.1f}" width="{w - 3:.1f}" height="{h - 3:.1f}" rx="{rx - 1.5:.1f}" fill="none" stroke="#fff" stroke-opacity=".16" stroke-width="3"/>'
    return defs + body


def svg(inner: str, scale: float = 1.0) -> str:
    off = (1024 - 1024 * scale) / 2
    g = f'<g transform="translate({off:.1f} {off:.1f}) scale({scale:.4f})">{inner}</g>' if scale != 1.0 else inner
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">{g}</svg>'


def main() -> None:
    BUILD.mkdir(parents=True, exist_ok=True)
    files = {
        "hubdeck-win": svg(drawing(230)),
        "hubdeck": svg(drawing(230), 824 / 1024),
        "hubdeck-discord": svg(drawing(0)),
    }
    for name, text in files.items():
        (ICONS / f"{name}.svg").write_text(text, encoding="utf-8")
    subprocess.run(
        ["node", str(RENDER), str(BUILD),
         f"{ICONS / 'hubdeck-win.svg'}:16,20,24,32,48,64,128,256",
         f"{ICONS / 'hubdeck.svg'}:1024,256",
         f"{ICONS / 'hubdeck-discord.svg'}:512"],
        check=True, cwd=ROOT,
    )


if __name__ == "__main__":
    main()
