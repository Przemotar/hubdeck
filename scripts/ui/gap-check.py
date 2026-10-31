"""Marks what in a scene falls into the gaps between the key windows (owner's "2 a", 2026-10-30).

The deck shows a 79x75 window of the 480x272 picture per key, every 100 px across and 99 down
(`deck-view.py`); the rest hides behind the grid. A horizon, a branch or a beam that runs along a
gap, or a sun, a face or a boat whose middle sits in one, is cut or lost on the deck (session 38:
"to słońce nie jest za wysoko?", "ten łuk ma podporę na łączeniu"). This finds:

- lines: long horizontal or vertical edges (a strong change of brightness that runs on for at least
  a third of the picture) and where they lie, in a window or in a gap;
- shapes: blobs that stand out from what is around them (brightness against a blurred copy), with
  their middle and how much of them the gaps hide.

It prints a list and writes `<frame>-gaps.png`: the gaps in red, the flagged lines and middles
marked. A tool for my reworks, not a rule of the skill. Frames are the engine's panel frames
(480x272, as `fx_film` writes them), whichever way the deck stands: the gaps are the panel's.

    python -I scripts/ui/gap-check.py <frame.png> [<frame2.png> ...] [--min-area 60]
"""
import argparse
import pathlib
from collections import deque

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

KW, KH = 79, 75  # one key window in canvas pixels
DX, DY = 100, 99  # its step across and down
W, H = 480, 272


def windows_mask() -> np.ndarray:
    m = np.zeros((H, W), bool)
    for i in range(15):
        c, r = i % 5, i // 5
        m[r * DY : r * DY + KH, c * DX : c * DX + KW] = True
    return m


def gap_rows() -> set[int]:
    return {y for y in range(H) if not any(r * DY <= y < r * DY + KH for r in range(3))}


def gap_cols() -> set[int]:
    return {x for x in range(W) if not any(c * DX <= x < c * DX + KW for c in range(5))}


def lines(lum: np.ndarray, axis: int, gaps: set[int]) -> list[tuple[int, float, bool]]:
    """Rows (axis 0) or columns (axis 1) with a long strong edge: (where, share of the length, in a gap)."""
    edge = np.abs(np.diff(lum, axis=axis))
    strong = edge > 0.08
    run = strong.mean(axis=1 - axis)  # the share of the row (column) with a strong edge
    found = []
    for i in np.argsort(-run):
        if run[i] < 0.33:
            break
        if any(abs(int(i) - f[0]) <= 3 for f in found):
            continue
        found.append((int(i), float(run[i]), int(i) in gaps or int(i) + 1 in gaps))
    return sorted(found)


def blobs(img: Image.Image, lum: np.ndarray, min_area: int) -> list[dict]:
    """Shapes that stand out from their surroundings, as connected regions."""
    blur = np.asarray(img.convert("L").filter(ImageFilter.GaussianBlur(14)), np.float32) / 255
    diff = np.abs(lum - blur)
    on = diff > max(0.10, float(np.percentile(diff, 92)))
    seen = np.zeros_like(on)
    out = []
    for y0, x0 in zip(*np.nonzero(on)):
        if seen[y0, x0]:
            continue
        q = deque([(y0, x0)])
        seen[y0, x0] = True
        pts = []
        while q:
            y, x = q.popleft()
            pts.append((y, x))
            for ny, nx in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
                if 0 <= ny < H and 0 <= nx < W and on[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    q.append((ny, nx))
        if len(pts) < min_area:
            continue
        ys, xs = np.array([p[0] for p in pts]), np.array([p[1] for p in pts])
        bw, bh = xs.max() - xs.min() + 1, ys.max() - ys.min() + 1
        if (bw > W / 3 and bh < 10) or (bh > H / 3 and bw < 10):
            continue  # an edge, listed with the lines
        out.append({"cx": float(xs.mean()), "cy": float(ys.mean()), "area": len(pts), "box": (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())), "ys": ys, "xs": xs})
    return out


def check(path: pathlib.Path, min_area: int) -> int:
    img = Image.open(path).convert("RGB")
    if img.size != (W, H):
        raise SystemExit(f"{path}: {img.size[0]}x{img.size[1]}, a panel frame is {W}x{H}")
    lum = np.asarray(img.convert("L"), np.float32) / 255
    win = windows_mask()
    rows, cols = gap_rows(), gap_cols()
    flagged = 0
    out = img.copy()
    shade = np.zeros((H, W, 4), np.uint8)
    shade[~win] = (255, 0, 0, 70)
    over = Image.fromarray(shade, "RGBA")
    d = ImageDraw.Draw(over)
    print(f"{path.name}")
    for y, share, gap in lines(lum, 0, rows):
        flagged += gap
        print(f"  line across  y={y:3d}  {share:4.0%} of the width  {'IN A GAP' if gap else 'in the windows'}")
        if gap:
            d.line([(0, y), (W, y)], fill=(255, 220, 0, 255), width=1)
    for x, share, gap in lines(lum, 1, cols):
        flagged += gap
        print(f"  line down    x={x:3d}  {share:4.0%} of the height  {'IN A GAP' if gap else 'in the windows'}")
        if gap:
            d.line([(x, 0), (x, H)], fill=(255, 220, 0, 255), width=1)
    for b in sorted(blobs(img, lum, min_area), key=lambda b: -b["area"])[:20]:
        hidden = 1 - float(win[b["ys"], b["xs"]].mean())
        centre_hidden = not win[int(round(b["cy"])), int(round(b["cx"]))]
        bad = centre_hidden or hidden > 0.4
        flagged += bad
        x0, y0, x1, y1 = b["box"]
        print(f"  shape at ({b['cx']:5.1f},{b['cy']:5.1f})  {b['area']:5d} px  box {x0}-{x1} x {y0}-{y1}  {hidden:4.0%} hidden"
              f"{'  MIDDLE IN A GAP' if centre_hidden else ''}{'  MOSTLY HIDDEN' if hidden > 0.4 else ''}")
        colour = (255, 220, 0, 255) if bad else (0, 255, 120, 200)
        d.rectangle([x0, y0, x1, y1], outline=colour)
        cx, cy = b["cx"], b["cy"]
        d.line([(cx - 4, cy), (cx + 4, cy)], fill=colour)
        d.line([(cx, cy - 4), (cx, cy + 4)], fill=colour)
    out = Image.alpha_composite(out.convert("RGBA"), over).convert("RGB").resize((W * 2, H * 2), Image.NEAREST)
    dest = path.with_name(path.stem + "-gaps.png")
    out.save(dest)
    print(f"  {flagged} flagged; marked picture: {dest}")
    return flagged


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("frames", nargs="+", type=pathlib.Path)
    ap.add_argument("--min-area", type=int, default=60, help="the smallest shape counted, in pixels")
    a = ap.parse_args()
    total = sum(check(f, a.min_area) for f in a.frames)
    print(f"{total} flagged in all")


if __name__ == "__main__":
    main()
