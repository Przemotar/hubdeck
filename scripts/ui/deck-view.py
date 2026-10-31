"""A theme's frame as the deck shows it: only the 15 key windows, with the gaps and the frame.

The engine draws one 480x272 picture; the keys module shows a 79x75 window of it per key
(`render.rs` `PanelGeometry::default`: every 100 px across, 99 down) and hides the rest behind
the grid. The windows are laid out with the window's own proportions (`DeckView.tsx`: keys 54.5
wide, gaps 7.6, a 20 margin), so what this prints is what the owner sees in the app and on the
deck, not the whole canvas (owner, 2026-10-28: "nie całe pole jest widoczne").

`--turn` is the deck's turn (0, 90, 180, 270; `fx_film`'s `HUBDECK_TURN`): the frame is turned
upright for the viewer, so 90 and 270 show the upright deck, 3 columns by 5 rows (owner,
2026-10-29: "niektórzy mogą mieć ustawione te 15 klawiszy w pionie").

    python -I scripts/ui/deck-view.py <frame.png> [<frame2.png> ...] --out <dir> [--scale 2] [--turn 90]
"""
import argparse
import pathlib

from PIL import Image, ImageDraw

KW, KH = 79, 75  # one key window in canvas pixels
DX, DY = 100, 99  # its step across and down
W, H = 480, 272  # the panel
MARGIN = 12
PLATE = round(KW * 18 / 54.5)
PANEL = (20, 19, 25)
RIM = (46, 44, 56)


def turned(rect: tuple[int, int, int, int], turn: int) -> tuple[int, int, int, int]:
    """A panel rectangle where it lands once the frame is turned upright (PIL turns anticlockwise)."""
    x0, y0, x1, y1 = rect
    pts = [(x0, y0), (x1, y1)]
    if turn == 90:
        pts = [(y, W - x) for x, y in pts]
    elif turn == 180:
        pts = [(W - x, H - y) for x, y in pts]
    elif turn == 270:
        pts = [(H - y, x) for x, y in pts]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return min(xs), min(ys), max(xs), max(ys)


def deck_view(frame: Image.Image, scale: int, turn: int = 0) -> Image.Image:
    view = frame.convert("RGB").rotate(turn, expand=True) if turn else frame.convert("RGB")
    vw, vh = view.size
    w = MARGIN * 2 + vw
    h = MARGIN * 2 + vh + 12 + PLATE
    s = scale
    out = Image.new("RGB", (w * s, h * s), (8, 8, 10))
    d = ImageDraw.Draw(out)
    d.rounded_rectangle([2 * s, 2 * s, (w - 2) * s, (h - 2) * s], radius=10 * s, fill=PANEL, outline=RIM, width=s)
    for i in range(15):
        c, r = i % 5, i // 5
        x0, y0, x1, y1 = turned((c * DX, r * DY, c * DX + KW, r * DY + KH), turn)
        key = view.crop((x0, y0, x1, y1)).resize(((x1 - x0) * s, (y1 - y0) * s), Image.LANCZOS)
        x, y = (MARGIN + x0) * s, (MARGIN + y0) * s
        mask = Image.new("L", key.size, 0)
        ImageDraw.Draw(mask).rounded_rectangle([0, 0, key.width - 1, key.height - 1], radius=3 * s, fill=255)
        out.paste(key, (x, y), mask)
        d.rounded_rectangle([x - s, y - s, x + key.width, y + key.height], radius=4 * s, outline=(0, 0, 0), width=s)
    py = (MARGIN + vh + 12) * s
    d.rounded_rectangle([MARGIN * s, py, (w - MARGIN) * s, py + PLATE * s], radius=3 * s, fill=(28, 27, 34))
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("frames", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--scale", type=int, default=2)
    ap.add_argument("--turn", type=int, default=0, choices=[0, 90, 180, 270])
    a = ap.parse_args()
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for f in a.frames:
        p = pathlib.Path(f)
        deck_view(Image.open(p), a.scale, a.turn).save(out / (p.stem + ".png"))
    print(f"{len(a.frames)} frames -> {out}")


if __name__ == "__main__":
    main()
