"""Our own art for the showcase profiles (app/src-tauri/resources/showcase/, MIT, made here).

    .venv/Scripts/python -I prototype/tools/gen_showcase.py

Scene cards for streaming, a night sky split over the 15 keys, a soft equalizer for music keys
(animated). Labels are not drawn here: the app draws them on top.
"""

import math
import os
import random
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parents[2] / "app" / "src-tauri" / "resources" / "showcase"
OUT.mkdir(parents=True, exist_ok=True)
rng = random.Random(7)


def hexrgb(h):
    h = h.lstrip("#")
    return np.array([int(h[i : i + 2], 16) for i in (0, 2, 4)], dtype=np.float32)


def gradient(w, h, a, b, angle=35):
    """Linear gradient from color a to b at an angle, as float RGB array."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    t = (x * math.cos(math.radians(angle)) + y * math.sin(math.radians(angle)))
    t = (t - t.min()) / (t.max() - t.min())
    return hexrgb(a)[None, None, :] * (1 - t[..., None]) + hexrgb(b)[None, None, :] * t[..., None]


def vignette(img, strength=0.55):
    h, w, _ = img.shape
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    d = np.sqrt(((x - w / 2) / (w / 2)) ** 2 + ((y - h / 2) / (h / 2)) ** 2)
    return img * (1 - strength * np.clip(d - 0.35, 0, 1)[..., None])


def to_img(arr):
    return Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8))


# ---------- scene cards: 144 x 144 stills with a soft motif ----------

def glow_layer(size, draw_fn, blur):
    layer = Image.new("L", (size, size), 0)
    draw_fn(ImageDraw.Draw(layer))
    return np.asarray(layer.filter(ImageFilter.GaussianBlur(blur)), dtype=np.float32) / 255


def scene_card(name, a, b, accent, motif):
    s = 144
    base = vignette(gradient(s, s, a, b), 0.45)
    shape = glow_layer(s, motif, 1.2)
    halo = glow_layer(s, motif, 9)
    acc = hexrgb(accent)[None, None, :]
    img = base * (1 - 0.55 * halo[..., None]) + acc * 0.55 * halo[..., None]
    img = img * (1 - shape[..., None]) + (acc * 0.35 + 255 * 0.65) * shape[..., None]
    # a faint top sheen like the glass keys
    y = np.linspace(1, 0, s)[:, None, None]
    img = img + 22 * np.clip(y - 0.55, 0, 1) * 2
    to_img(img).save(OUT / f"scene-{name}.png", optimize=True)


def m_start(d):  # rising sun over a horizon
    d.ellipse((44, 40, 100, 96), outline=255, width=6)
    d.rectangle((20, 92, 124, 98), fill=255)


def m_chat(d):  # two speech bubbles
    d.rounded_rectangle((22, 34, 92, 82), 14, outline=255, width=6)
    d.rounded_rectangle((56, 64, 122, 108), 14, outline=255, width=6)


def m_game(d):  # a controller
    d.rounded_rectangle((26, 50, 118, 98), 22, outline=255, width=6)
    d.line((48, 66, 48, 82), fill=255, width=6)
    d.line((40, 74, 56, 74), fill=255, width=6)
    d.ellipse((86, 62, 96, 72), fill=255)
    d.ellipse((98, 74, 108, 84), fill=255)


def m_brb(d):  # a coffee cup
    d.rounded_rectangle((38, 52, 94, 104), 10, outline=255, width=6)
    d.arc((86, 62, 112, 90), -90, 90, fill=255, width=6)
    for x in (52, 66, 80):
        d.arc((x - 6, 26, x + 6, 46), 200, 340, fill=255, width=4)


def m_end(d):  # a waving hand, simplified to a star
    pts = []
    for i in range(10):
        r = 40 if i % 2 == 0 else 17
        ang = -math.pi / 2 + i * math.pi / 5
        pts.append((72 + r * math.cos(ang), 74 + r * math.sin(ang)))
    d.polygon(pts, outline=255, width=6)


for args in [
    ("start", "#1b1036", "#5b2a86", "#f59e0b", m_start),
    ("chat", "#0f1e3a", "#2a4d8f", "#38bdf8", m_chat),
    ("game", "#2a0f1e", "#8a1d4a", "#fb7185", m_game),
    ("brb", "#1d1a10", "#6b4a1a", "#fbbf24", m_brb),
    ("end", "#0f2a24", "#1b6b57", "#34d399", m_end),
]:
    scene_card(*args)


# ---------- night sky for the whole key panel: 480 x 272, 24 frames ----------

def night_sky():
    w, h, frames = 480, 272, 16
    base = gradient(w, h, "#05060f", "#16123a", angle=95)
    stars = [(rng.uniform(0, w), rng.uniform(0, h * 0.85), rng.uniform(0.4, 1.6), rng.uniform(0, 2 * math.pi)) for _ in range(140)]
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    out = []
    for f in range(frames):
        t = f / frames * 2 * math.pi
        img = base.copy()
        # a slow aurora ribbon
        band = np.exp(-(((y - (h * 0.42 + 26 * np.sin(x / 70 + t))) / 30) ** 2))
        shimmer = 0.6 + 0.4 * np.sin(x / 23 + t * 2)
        img += band[..., None] * shimmer[..., None] * np.array([40, 150, 120], dtype=np.float32) * 0.55
        img += np.exp(-(((y - (h * 0.30 + 18 * np.sin(x / 90 - t))) / 22) ** 2))[..., None] * np.array([120, 70, 200], dtype=np.float32) * 0.35
        frame = to_img(img)
        d = ImageDraw.Draw(frame)
        for sx, sy, r, ph in stars:
            a = 0.45 + 0.55 * (0.5 + 0.5 * math.sin(t * 2 + ph))
            c = int(200 * a + 55)
            d.ellipse((sx - r, sy - r, sx + r, sy + r), fill=(c, c, min(255, c + 20)))
        # a horizon of hills
        d.polygon([(0, h)] + [(xx, h - 34 - 14 * math.sin(xx / 60) - 8 * math.sin(xx / 23)) for xx in range(0, w + 8, 8)] + [(w, h)], fill=(6, 7, 14))
        if f == 0:
            # the same sky under each key (key i's window on the 480 x 272 panel), so the keys read as
            # one picture with their icons and labels on top
            for i in range(15):
                x0, y0 = (i % 5) * 100, (i // 5) * 99
                frame.crop((x0, y0, x0 + 79, y0 + 75)).resize((158, 150), Image.LANCZOS).save(OUT / f"sky-{i}.png", optimize=True)
        out.append(frame.convert("P", palette=Image.ADAPTIVE, colors=64))
    # the animated sky (out) is not shipped yet: the keys use stills; kept for a future screen saver


night_sky()


# ---------- equalizer for music keys: 144 x 144, 16 frames ----------

def equalizer():
    s, frames, bars = 144, 16, 5
    out = []
    phases = [rng.uniform(0, 2 * math.pi) for _ in range(bars)]
    for f in range(frames):
        t = f / frames * 2 * math.pi
        img = to_img(vignette(gradient(s, s, "#06140c", "#0f3d22"), 0.4))
        d = ImageDraw.Draw(img)
        bw = 14
        gap = (s - 2 * 22 - bars * bw) / (bars - 1)
        for i in range(bars):
            hgt = 22 + 62 * (0.5 + 0.5 * math.sin(t * (1 + i * 0.37) + phases[i])) ** 1.4
            x0 = 22 + i * (bw + gap)
            d.rounded_rectangle((x0, 110 - hgt, x0 + bw, 110), 5, fill=(30, 215, 96))
        out.append(img.convert("P", palette=Image.ADAPTIVE, colors=48))
    out[0].save(OUT / "equalizer.gif", save_all=True, append_images=out[1:], duration=90, loop=0, optimize=True, disposal=1)


equalizer()

total = sum(os.path.getsize(p) for p in OUT.iterdir())
print("showcase art:", ", ".join(f"{p.name} {os.path.getsize(p) // 1024} KB" for p in sorted(OUT.iterdir())), f"total {total // 1024} KB")
