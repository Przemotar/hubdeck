"""The site's link preview (site/images/og.png, 1280x640: Discord, Pepper, Reddit) with the living deck.

The left half (the icon, the name, the line under it) is kept from the current og.png; the right half
is the keys module as the deck shows it with a preset on, from a picture of `scripts/ui/presets-take.mjs`
(logs/presets-film/NN/MMM.jpg), drawn as `scripts/ui/deck-view.py` does, with a soft glow of the
scene's own colour behind it.

    python -I scripts/og.py <picture.jpg> <out.png> [--line "Now with the living deck"]
"""
import argparse
import importlib.util
import pathlib

from PIL import Image, ImageDraw, ImageFilter, ImageFont, ImageStat

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("deck_view", ROOT / "scripts" / "ui" / "deck-view.py")
dv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dv)

W, H = 1280, 640
BG = (13, 13, 18)
SPLIT = 560  # where the kept left half ends


def keys_card(picture: pathlib.Path, width: int) -> Image.Image:
    """The 15 key windows on their panel, `width` px across (drawn at 3x, brought down)."""
    big = dv.deck_view(Image.open(picture), 3)
    m = dv.MARGIN * 3
    keys = big.crop((m - 3, m - 3, big.width - m + 3, m + dv.H * 3 + 3))
    panel = Image.new("RGBA", (big.width, keys.height + 2 * (m - 3)), (0, 0, 0, 0))
    ImageDraw.Draw(panel).rounded_rectangle([0, 0, panel.width - 1, panel.height - 1], radius=36, fill=dv.PANEL + (255,), outline=dv.RIM + (255,), width=4)
    panel.paste(keys, (m - 3, m - 3))
    return panel.resize((width, round(panel.height * width / panel.width)), Image.LANCZOS)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("picture")
    ap.add_argument("out")
    ap.add_argument("--base", default=str(ROOT / "site" / "images" / "og.png"), help="the og.png whose left half is kept")
    ap.add_argument("--line", default="", help="a short line under the left half's own")
    a = ap.parse_args()
    base = Image.open(a.base).convert("RGB")
    out = Image.new("RGB", (W, H), BG)
    out.paste(base.crop((0, 0, SPLIT, H)), (0, 0))
    card = keys_card(pathlib.Path(a.picture), 640)
    x, y = W - card.width - 36, (H - card.height) // 2
    # a glow of the scene's own colour behind the card
    # (its mean brought up to a light colour of the same hue), and a shadow under it
    mean = ImageStat.Stat(Image.open(a.picture).convert("RGB").resize((48, 27))).mean
    lift = 200 / max(mean)
    tint = tuple(min(255, int(v * lift)) for v in mean)
    glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(glow).rounded_rectangle([x - 10, y - 10, x + card.width + 10, y + card.height + 10], radius=60, fill=tint + (130,))
    glow = glow.filter(ImageFilter.GaussianBlur(48))
    shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle([x + 10, y + 24, x + card.width - 10, y + card.height + 24], radius=40, fill=(0, 0, 0, 200))
    shadow = shadow.filter(ImageFilter.GaussianBlur(18))
    out = Image.alpha_composite(Image.alpha_composite(out.convert("RGBA"), glow), shadow)
    out.alpha_composite(card, (x, y))
    out = out.convert("RGB")
    if a.line:
        f = ImageFont.truetype("C:/Windows/Fonts/Inter-SemiBold.ttf", 22)
        ImageDraw.Draw(out).text((72, 478), a.line, font=f, fill=(176, 150, 255))
    out.save(a.out, optimize=True)
    print(a.out)


if __name__ == "__main__":
    main()
