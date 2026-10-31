"""Packs the pictures of `look-loops.mjs` into the Deck look view's moving thumbnails.

Each logs/look-loops/<id>/NNN.jpg becomes one frame of app/public/looks/<id>.webp (15 a second,
looping) plus a still poster <id>.jpg (the liveliest frame), shown as the deck shows it: the 15 key
windows with their gaps, cropped tight to them (no frame or plate: the effect gets the room). An
item kept for 26 s (a ghost crossing now and then) gives its liveliest 2 s.

    python -I scripts/ui/look-loops.py [--width 300] [--quality 80]
"""
import argparse
import importlib.util
import pathlib

from PIL import Image, ImageChops, ImageStat

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("deck_view", ROOT / "scripts" / "ui" / "deck-view.py")
dv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dv)

LOOP = 30  # frames in a loop: 2 s at 15 a second


def tight(frame: Image.Image) -> Image.Image:
    """The key windows with their gaps only (the deck view without its frame and plate)."""
    pic = dv.deck_view(frame, 1)
    m = dv.MARGIN - 3
    return pic.crop((m, m, pic.width - m, m + 2 * dv.DY + dv.KH + 6))


def energy(a: Image.Image, b: Image.Image) -> float:
    return sum(ImageStat.Stat(ImageChops.difference(a, b)).mean)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--width", type=int, default=300)
    ap.add_argument("--quality", type=int, default=80)
    ap.add_argument("--stride", type=int, default=1, help="keep every n-th picture (2: 7.5 a second)")
    a = ap.parse_args()
    src = ROOT / "logs" / "look-loops"
    out = ROOT / "app" / "public" / "looks"
    out.mkdir(parents=True, exist_ok=True)
    total = 0
    for d in sorted(p for p in src.iterdir() if p.is_dir()):
        raw = [Image.open(f).convert("RGB") for f in sorted(d.glob("*.jpg"))]
        if not raw:
            continue
        if len(raw) > LOOP:
            # the liveliest 2 s: the most change from frame to frame and light on screen
            small = [r.resize((96, 54)) for r in raw]
            score = [energy(small[i], small[i - 1]) + sum(ImageStat.Stat(small[i]).mean) / 40 for i in range(1, len(small))]
            start = max(range(len(score) - LOOP), key=lambda s: sum(score[s : s + LOOP]))
            raw = raw[start : start + LOOP]
        raw = raw[:: a.stride]
        frames = []
        for r in raw:
            pic = tight(r)
            frames.append(pic.resize((a.width, round(pic.height * a.width / pic.width)), Image.LANCZOS))
        poster = max(frames, key=lambda f: sum(ImageStat.Stat(f).mean))
        poster.save(out / f"{d.name}.jpg", quality=86)
        to = out / f"{d.name}.webp"
        frames[0].save(to, save_all=True, append_images=frames[1:], duration=66 * a.stride, loop=0, quality=a.quality, method=4)
        total += to.stat().st_size + (out / f"{d.name}.jpg").stat().st_size
    print(f"{len(list(out.glob('*.webp')))} loops, {total / 1e6:.1f} MB -> {out}")


if __name__ == "__main__":
    main()
