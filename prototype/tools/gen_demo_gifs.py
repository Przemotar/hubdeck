"""Animations for the demo profile "Discover HubDeck" (our own art, MIT), in the house look:
graphite ground, soft group colors.

    .venv/Scripts/python -I prototype/tools/gen_demo_gifs.py

Writes app/src-tauri/resources/demo/*.gif: 72 px key animations and 240 px dial animations.
"""

import colorsys
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parents[2] / "app" / "src-tauri" / "resources" / "demo"
OUT.mkdir(parents=True, exist_ok=True)
BG = (27, 26, 32)
S = 72  # key size


def hue(h, s=0.75, l=0.70):
    r, g, b = colorsys.hls_to_rgb((h % 360) / 360, l, s)
    return (round(r * 255), round(g * 255), round(b * 255))


def save(name, frames, ms):
    frames = [f.convert("RGB") for f in frames]
    # one palette from every frame (the first alone may lack the colors that come later)
    w, h = frames[0].size
    strip = Image.new("RGB", (w, h * len(frames)))
    for k, f in enumerate(frames):
        strip.paste(f, (0, k * h))
    pal = strip.quantize(colors=128, method=Image.Quantize.MEDIANCUT)
    q = [f.quantize(palette=pal, dither=Image.Dither.NONE) for f in frames]
    q[0].save(OUT / name, save_all=True, append_images=q[1:], duration=ms, loop=0, disposal=1, optimize=False)
    print(name, (OUT / name).stat().st_size // 1024, "KB")


def canvas(size=S, bg=BG):
    return Image.new("RGB", (size, size), bg)


def glow(img, radius=3, strength=1.0):
    blurred = img.filter(ImageFilter.GaussianBlur(radius))
    a = np.asarray(img, dtype=np.float32)
    b = np.asarray(blurred, dtype=np.float32)
    return Image.fromarray(np.clip(a + b * strength, 0, 255).astype(np.uint8))


def ping(n=28):
    """A radar ping: rings grow from the middle and fade."""
    out = []
    for i in range(n):
        im = canvas()
        d = ImageDraw.Draw(im)
        for k in range(3):
            t = ((i / n) + k / 3) % 1
            r = 6 + t * 30
            c = tuple(int(BG[j] + (hue(190)[j] - BG[j]) * (1 - t)) for j in range(3))
            d.ellipse((36 - r, 36 - r, 36 + r, 36 + r), outline=c, width=3)
        d.ellipse((31, 31, 41, 41), fill=hue(190, l=0.78))
        out.append(glow(im, 2, 0.8))
    save("ping.gif", out, 50)


def spinner(n=24):
    out = []
    for i in range(n):
        im = canvas(S * 4)
        d = ImageDraw.Draw(im)
        start = i / n * 360
        for k in range(20):
            a = start + k * 12
            c = tuple(int(BG[j] + (hue(270)[j] - BG[j]) * (k / 20)) for j in range(3))
            d.arc((52, 52, 236, 236), a, a + 13, fill=c, width=26)
        out.append(glow(im.resize((S, S), Image.LANCZOS), 2, 0.6))
    save("spinner.gif", out, 40)


def aurora(n=36, size=S, name="aurora.gif"):
    y, x = np.mgrid[0:size, 0:size].astype(np.float32) / size
    out = []
    for i in range(n):
        t = i / n * 2 * math.pi
        img = np.zeros((size, size, 3), np.float32) + np.array(BG, np.float32) * 0.6
        for k, (h, amp, f) in enumerate([(160, 0.10, 3.0), (200, 0.08, 2.3), (280, 0.07, 1.7)]):
            centre = 0.38 + 0.14 * k + amp * np.sin(x * f * math.pi + t + k)
            band = np.exp(-((y - centre) ** 2) / 0.006)
            img += band[..., None] * np.array(hue(h + 15 * math.sin(t + k), 0.8, 0.6), np.float32) * 0.85
        out.append(Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)))
    save(name, out, 70)


def orbit(n=30, size=S, name="orbit.gif"):
    out = []
    k = size / 72
    for i in range(n):
        im = canvas(size)
        d = ImageDraw.Draw(im)
        c = size / 2
        d.ellipse((c - 7 * k, c - 7 * k, c + 7 * k, c + 7 * k), fill=hue(45, l=0.72))
        for j, (r, sp, h) in enumerate([(16, 1, 190), (25, -1, 270), (32, 2, 350)]):
            a = i / n * 2 * math.pi * sp + j
            d.ellipse((c - r * k, c - r * k, c + r * k, c + r * k), outline=(45, 43, 54), width=max(1, int(k)))
            px, py = c + r * k * math.cos(a), c + r * k * math.sin(a)
            d.ellipse((px - 4 * k, py - 4 * k, px + 4 * k, py + 4 * k), fill=hue(h))
        out.append(glow(im, 2 * k, 0.6))
    save(name, out, 50)


def scope(n=30):
    """An oscilloscope line."""
    out = []
    for i in range(n):
        im = canvas(S * 3)
        d = ImageDraw.Draw(im)
        for gx in range(0, S * 3, 36):
            d.line((gx, 0, gx, S * 3), fill=(36, 35, 44), width=2)
            d.line((0, gx, S * 3, gx), fill=(36, 35, 44), width=2)
        pts = [(x, 108 + 50 * math.sin(x / 216 * 4 * math.pi + i / n * 2 * math.pi) * math.sin(x / 216 * math.pi)) for x in range(0, 217, 3)]
        d.line(pts, fill=hue(160), width=7, joint="curve")
        out.append(glow(im.resize((S, S), Image.LANCZOS), 2, 0.9))
    save("scope.gif", out, 45)


def lava(n=36):
    """Soft blobs rising and merging (metaballs)."""
    y, x = np.mgrid[0:S, 0:S].astype(np.float32)
    out = []
    blobs = [(0.3, 0.9, 13, 0.0), (0.65, 0.7, 11, 2.0), (0.5, 0.4, 9, 4.0), (0.2, 0.2, 8, 1.0)]
    for i in range(n):
        t = i / n * 2 * math.pi
        field = np.zeros((S, S), np.float32)
        for bx, by, r, ph in blobs:
            cx = S * (bx + 0.12 * math.sin(t + ph))
            cy = S * ((by - i / n) % 1.0 * 1.2 - 0.1)
            field += r * r / ((x - cx) ** 2 + (y - cy) ** 2 + 1)
        inside = np.clip((field - 0.9) * 3, 0, 1)[..., None]
        col = np.array(hue(15, 0.85, 0.6), np.float32) * (0.75 + 0.25 * (y / S))[..., None]
        img = np.array(BG, np.float32) * (1 - inside) + col * inside
        out.append(Image.fromarray(img.astype(np.uint8)))
    save("lava.gif", out, 60)


def rain(n=24):
    rng = np.random.default_rng(3)
    drops = [(rng.uniform(0, S), rng.uniform(0, S), rng.uniform(1.5, 3.0)) for _ in range(26)]
    out = []
    for i in range(n):
        im = canvas()
        d = ImageDraw.Draw(im)
        for x0, y0, sp in drops:
            yy = (y0 + i * sp * S / n * 1.0) % (S + 10) - 10
            d.line((x0, yy, x0, yy + 7), fill=hue(210, 0.6, 0.65), width=1)
        out.append(im)
    save("rain.gif", out, 45)


def typing(n=18):
    out = []
    for i in range(n):
        im = canvas()
        d = ImageDraw.Draw(im)
        d.rounded_rectangle((12, 22, 60, 50), 12, fill=(44, 42, 54))
        for k in range(3):
            up = max(0.0, math.sin((i / n) * 2 * math.pi - k * 0.9))
            d.ellipse((22 + k * 11, 32 - up * 4, 30 + k * 11, 40 - up * 4), fill=hue(250, l=0.6 + 0.2 * up))
        out.append(im)
    save("typing.gif", out, 60)


def rainbow(n=36):
    y, x = np.mgrid[0:S, 0:S].astype(np.float32)
    ang = (np.degrees(np.arctan2(y - 36, x - 36)) + 360) % 360
    out = []
    for i in range(n):
        h = (ang + i / n * 360) % 360
        rgb = np.zeros((S, S, 3), np.float32)
        for j in range(S):
            rgb[j] = [colorsys.hls_to_rgb(v / 360, 0.6, 0.8) for v in h[j]]
        rgb *= 255
        r = np.sqrt((x - 36) ** 2 + (y - 36) ** 2)
        ring = ((r > 18) & (r < 30)).astype(np.float32)[..., None]
        img = np.array(BG, np.float32) * (1 - ring) + rgb * ring
        out.append(glow(Image.fromarray(img.astype(np.uint8)), 2, 0.5))
    save("rainbow.gif", out, 50)


def heartbeat(n=24):
    """An ECG line drawing itself."""
    pts = []
    for x in range(0, 73):
        u = x / 72
        yv = 36
        if 0.35 < u < 0.42:
            yv = 36 - (u - 0.35) / 0.07 * 22
        elif 0.42 <= u < 0.5:
            yv = 14 + (u - 0.42) / 0.08 * 40
        elif 0.5 <= u < 0.56:
            yv = 54 - (u - 0.5) / 0.06 * 18
        pts.append((x, yv))
    out = []
    for i in range(n):
        im = canvas()
        d = ImageDraw.Draw(im)
        head = int((i / n) * 90)
        for j in range(1, len(pts)):
            age = head - j
            if 0 <= age < 60:
                f = 1 - age / 60
                c = tuple(int(BG[k] + (hue(350)[k] - BG[k]) * f) for k in range(3))
                d.line((pts[j - 1], pts[j]), fill=c, width=3)
        out.append(glow(im, 2, 0.7))
    save("heartbeat.gif", out, 45)


def fire(n=24):
    rng = np.random.default_rng(5)
    h, w = S // 2, S // 2
    field = np.zeros((h + 2, w), np.float32)
    frames = []
    for i in range(n + 30):
        # hot spots flicker along the bottom; heat rises, spreads and cools into tongues of flame
        field[-1] = (rng.random(w) > 0.55) * rng.uniform(0.8, 1.3, w)
        nxt = field.copy()
        nxt[:-1] = (field[1:] + np.roll(field[1:], 1, 1) + np.roll(field[1:], -1, 1) + field[:-1]) / 4.2
        field = nxt
        if i >= 30:
            v = np.clip(field[:-2], 0, 1)
            rgb = np.stack([np.clip(v * 2.2, 0, 1), np.clip(v * 1.4 - 0.25, 0, 1), np.clip(v * 1.2 - 0.7, 0, 1)], -1) * 255
            base = np.array(BG, np.float32)
            img = np.maximum(rgb, base)
            frames.append(Image.fromarray(img.astype(np.uint8)).resize((S, S), Image.BILINEAR))
    save("fire.gif", frames, 60)


def bars(n=24):
    rng = np.random.default_rng(9)
    targets = rng.uniform(0.2, 1.0, (n, 5))
    out = []
    for i in range(n):
        im = canvas()
        d = ImageDraw.Draw(im)
        for k in range(5):
            v = 0.5 * targets[i, k] + 0.5 * targets[(i - 1) % n, k]
            x = 12 + k * 10.5
            d.rounded_rectangle((x, 60 - v * 44, x + 7, 60), 3, fill=hue(160 + k * 12))
        out.append(im)
    save("bars.gif", out, 70)


def dots_grid(n=30):
    out = []
    for i in range(n):
        im = canvas()
        d = ImageDraw.Draw(im)
        for gx in range(5):
            for gy in range(5):
                ph = i / n * 2 * math.pi - (gx + gy) * 0.6
                r = 2 + 3 * (0.5 + 0.5 * math.sin(ph))
                cx, cy = 12 + gx * 12, 12 + gy * 12
                d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=hue(270 - (gx + gy) * 10))
        out.append(im)
    save("dots.gif", out, 50)


def dial_sweep(n=40):
    """The dial: a light sweeping round a ring, like a radar."""
    size = 240
    y, x = np.mgrid[0:size, 0:size].astype(np.float32)
    ang = (np.degrees(np.arctan2(y - 120, x - 120)) + 360) % 360
    r = np.sqrt((x - 120) ** 2 + (y - 120) ** 2)
    out = []
    for i in range(n):
        head = i / n * 360
        trail = ((head - ang) % 360) / 360
        fade = np.clip(1 - trail * 1.6, 0, 1) ** 2
        ring = np.exp(-((r - 96) ** 2) / 60)
        disc = (r < 100).astype(np.float32) * 0.18 * fade
        col = np.array(hue(190, 0.8, 0.62), np.float32)
        img = np.array(BG, np.float32)[None, None, :] + (ring * fade + disc)[..., None] * (col - np.array(BG, np.float32))
        im = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8))
        out.append(glow(im, 4, 0.5))
    save("dial-sweep.gif", out, 50)


if __name__ == "__main__":
    ping()
    spinner()
    aurora()
    orbit()
    scope()
    lava()
    rain()
    typing()
    rainbow()
    heartbeat()
    fire()
    bars()
    dots_grid()
    aurora(40, 240, "dial-aurora.gif")
    orbit(36, 240, "dial-orbit.gif")
    dial_sweep()
