"""Halo variants offline: the keys' content (a test build's HUBDECK_DUMP_CONTENT, raw RGBA premultiplied,
480x272) over scene frames (scripts/ui/look-loops.mjs, the scene alone), with the halo drawn each way,
side by side as the deck shows them, plus a number for each: how far the content stands out from what
is right round it (owner, 2026-10-30: "TE cienie pod ikonami strasznie slabo wygladaja").

    python -I scripts/ui/halo-lab.py <content.rgba> <out dir> <scene id>... [--frame 20] [--only now,shape]

The "now" variant is canvas.rs `halo_of` as it was until 2026-10-30; "shape" is what replaced it
(`shadow_of`, his "1 a"); the others were tries.
"""
import argparse
import importlib.util
import pathlib

import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("deck_view", ROOT / "scripts" / "ui" / "deck-view.py")
dv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dv)
W, H = 480, 272


def box(a: np.ndarray, r: int) -> np.ndarray:
    """A box blur of `r` px each way, edges averaged over what is inside (as canvas.rs `blur` passes)."""
    out = a.astype(np.float64)
    for axis in (1, 0):
        c = np.cumsum(np.pad(out, [(r + 1, r) if ax == axis else (0, 0) for ax in range(2)], mode="constant"), axis=axis)
        n = np.cumsum(np.pad(np.ones_like(out), [(r + 1, r) if ax == axis else (0, 0) for ax in range(2)], mode="constant"), axis=axis)
        if axis == 1:
            s, k = c[:, 2 * r + 1:] - c[:, :-2 * r - 1], n[:, 2 * r + 1:] - n[:, :-2 * r - 1]
        else:
            s, k = c[2 * r + 1:, :] - c[:-2 * r - 1, :], n[2 * r + 1:, :] - n[:-2 * r - 1, :]
        out = s / k
    return out


def blur(a: np.ndarray, r: int = 4, passes: int = 2) -> np.ndarray:
    for _ in range(passes):
        a = box(a, r)
    return a


def gauss(a: np.ndarray, sigma: float) -> np.ndarray:
    # three box blurs come close to a gaussian
    r = max(1, round((np.sqrt(12 * sigma * sigma / 3 + 1) - 1) / 2))
    return blur(a, r, 3)


def grow(a: np.ndarray, r: int) -> np.ndarray:
    out = a.copy()
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            out = np.maximum(out, np.roll(np.roll(a, dy, 0), dx, 1))
    return out


def shift(a: np.ndarray, dy: int) -> np.ndarray:
    out = np.zeros_like(a)
    if dy > 0:
        out[dy:] = a[:-dy]
    else:
        out[:] = a
    return out


def luma(rgb: np.ndarray) -> np.ndarray:
    return (rgb[..., 0] * 54 + rgb[..., 1] * 183 + rgb[..., 2] * 19) / 256


# ---- the variants: each takes the scene (float RGB 0..255), alpha 0..1 and tone (lightness of
# the content, premultiplied), and gives the scene with its halo

def v_now(scene, a, tone):
    """canvas.rs today: cover grown 2 px, two 9 px box blurs, doubled, darkened up to 170/255; a light
    pad behind thin dark content."""
    m = a * 255
    cover = blur(grow(m, 2))
    light = blur(tone)
    alpha = blur(m)
    solid = blur(blur(alpha))
    thin = np.clip(110 - solid, 0, 60) * 255 / 60
    t = np.where(alpha > 0, np.minimum(light * 255 / np.maximum(alpha, 1e-6), 255), 255)
    h = np.minimum(np.floor(cover) * 2, 255)
    out = scene.copy()
    dark = (t >= 90) | (thin == 0)
    k = 255 - h * 170 / 255
    out[dark] = out[dark] * (k[dark] / 255)[:, None]
    lift = ~dark & (h > 0)
    kk = (h * 150 / 255 * thin / 255)[lift][:, None]
    to = np.minimum(out[lift] * 1.5 + 64, 220)
    out[lift] = (out[lift] * (255 - kk) + to * kk) / 255
    return out


def v_shape(scene, a, tone, sigma=2.2, dy=1, strength=0.62):
    """A soft shadow that follows each glyph's own shape, a pixel down (as text on a phone's
    wallpaper): no grown box, nothing far from the glyph."""
    s = np.clip(gauss(shift(a, dy), sigma) * 1.8, 0, 1) * strength
    return scene * (1 - s)[..., None]


def v_glow(scene, a, tone, sigma=4.0, strength=0.5):
    """A wider soft dark glow round the glyphs, no offset, lighter than today's."""
    s = np.clip(gauss(a, sigma) * 1.6, 0, 1) * strength
    return scene * (1 - s)[..., None]


def v_spot(scene, a, tone, strength=0.38):
    """Each key a soft round shade under its middle (a vignette in reverse), plus the shape shadow
    for crisp edges."""
    yy, xx = np.mgrid[0:H, 0:W]
    shade = np.zeros((H, W))
    for i in range(15):
        c, r = i % 5, i // 5
        cx, cy = c * dv.DX + dv.KW / 2, r * dv.DY + dv.KH / 2
        d = np.hypot((xx - cx) / (dv.KW * 0.55), (yy - cy) / (dv.KH * 0.55))
        has = a[int(r * dv.DY):int(r * dv.DY + dv.KH), int(c * dv.DX):int(c * dv.DX + dv.KW)].max() > 0.05
        if has:
            shade = np.maximum(shade, np.clip(1 - d, 0, 1) ** 1.6 * strength)
    out = scene * (1 - shade)[..., None]
    return v_shape(out, a, tone, sigma=1.6, strength=0.45)


def v_layered(scene, a, tone, contact=0.55, ambient=0.32):
    """Two shadows as a phone draws text on a photo: a tight contact one (crisp edge, a pixel down)
    and a wide soft one (calms the scene round the glyph without a visible shape); thin dark content
    gets a faint light rim instead, so a slate icon gets an edge on a dark scene."""
    near = np.clip(gauss(shift(a, 1), 1.2) * 2.0, 0, 1) * contact
    far = np.clip(gauss(a, 4.5) * 1.4, 0, 1) * ambient
    s = 1 - (1 - near) * (1 - far)
    out = scene * (1 - s)[..., None]
    light = np.where(a > 0.05, tone / np.maximum(a, 1e-6), 255)
    dark_thin = (gauss(np.where(light < 90, a, 0), 1.0) > 0.02) & (gauss(a, 3.0) < 0.43)
    rim = np.clip(gauss(a, 1.4) * 2.2, 0, 1) * 0.35 * dark_thin
    to = np.minimum(scene * 1.5 + 64, 220)
    return out * (1 - rim)[..., None] + to * rim[..., None]


VARIANTS = {"now": v_now, "shape": v_shape, "glow": v_glow, "spot": v_spot, "layered": v_layered}


def stand_out(img: np.ndarray, a: np.ndarray) -> float:
    """How far the content stands out: the median luminance contrast (WCAG ratio) between each glyph
    pixel and the ring 2-3 px round the glyphs, over all keys."""
    inside = a > 0.6
    ring = (grow((a > 0.1).astype(np.float64), 3) > 0) & ~(grow((a > 0.1).astype(np.float64), 1) > 0)
    lum = luma(img) / 255
    lin = np.where(lum <= 0.04045, lum / 12.92, ((lum + 0.055) / 1.055) ** 2.4)
    ring_l = blur(np.where(ring, lin, 0), 3, 1) / np.maximum(blur(ring.astype(np.float64), 3, 1), 1e-6)
    ratio = (np.maximum(lin, ring_l) + 0.05) / (np.minimum(lin, ring_l) + 0.05)
    vals = ratio[inside & (blur(ring.astype(np.float64), 3, 1) > 0.01)]
    return float(np.median(vals)), float(np.percentile(vals, 10))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("content")
    ap.add_argument("out")
    ap.add_argument("scenes", nargs="+")
    ap.add_argument("--frame", type=int, default=20)
    ap.add_argument("--only", default=",".join(VARIANTS))
    ap.add_argument("--scale", type=int, default=2)
    args = ap.parse_args()
    raw = np.frombuffer(pathlib.Path(args.content).read_bytes(), np.uint8).reshape(H, W, 4).astype(np.float64)
    a = raw[..., 3] / 255
    rgb_pre = raw[..., :3]
    tone = luma(rgb_pre)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    names = args.only.split(",")
    rows = []
    for sc in args.scenes:
        frames = sorted((ROOT / "logs" / "look-loops" / sc).glob("*.jpg"))
        scene = np.asarray(Image.open(frames[min(args.frame, len(frames) - 1)]).convert("RGB")).astype(np.float64)
        row = []
        for n in names:
            img = np.clip(rgb_pre + VARIANTS[n](scene, a, tone) * (1 - a)[..., None], 0, 255)
            pic = Image.fromarray(img.astype(np.uint8))
            pic.save(out / f"{sc}-{pathlib.Path(args.content).stem}-{n}.png")
            med, low = stand_out(img, a)
            darkened = float(np.mean(1 - luma(VARIANTS[n](scene, a, tone)) / np.maximum(luma(scene), 1)))
            print(f"{sc:20s} {n:8s} stands out {med:5.2f}:1, the weakest tenth {low:5.2f}:1, scene darkened {darkened * 100:4.1f} %")
            row.append(dv.deck_view(pic, args.scale))
        rows.append(row)
    w, h = rows[0][0].size
    sheet = Image.new("RGB", (w * len(names) + 10 * (len(names) - 1), h * len(rows) + 10 * (len(rows) - 1)), (14, 14, 18))
    for y, row in enumerate(rows):
        for x, pic in enumerate(row):
            sheet.paste(pic, (x * (w + 10), y * (h + 10)))
    sheet.save(out / "sheet.png")
    print(f"sheet: {out / 'sheet.png'} (columns: {', '.join(names)})")


if __name__ == "__main__":
    main()
