"""Generate HubDeck's own animated sample art (MIT, made here): an aurora for the full key panel,
fire for the round dial (160 px, drawn bigger on the 240 px screen), a loading ring and audio bars for keys.

Run: .venv/Scripts/python -I prototype/tools/gen_animations.py   (needs numpy and pillow)
Writes app/src-tauri/resources/samples/. HubDeck scales animation frames to fit 240 px and plays
them at most every 66 ms, so the files are made at that size and pace.
"""
import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "app", "src-tauri", "resources", "samples")
FRAME_MS = 80


def save_gif(frames, name, colors=96):
    # one shared palette for every frame keeps the GIF small and stops colors flickering
    strip = Image.new("RGB", (frames[0].width, frames[0].height * len(frames)))
    for i, f in enumerate(frames):
        strip.paste(f, (0, i * f.height))
    palette = strip.quantize(colors=colors, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    out = [f.quantize(palette=palette, dither=Image.Dither.NONE) for f in frames]
    path = os.path.join(OUT, name)
    out[0].save(path, save_all=True, append_images=out[1:], duration=FRAME_MS, loop=0, optimize=True, disposal=1)
    print(f"{name}: {len(frames)} frames, {os.path.getsize(path) // 1024} KB")


def aurora(w=240, h=136, n=16):
    """Curtains of green and violet light drifting over a night sky; loops seamlessly."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    frames = []
    for i in range(n):
        t = 2 * math.pi * i / n
        sky = np.stack([8 + 10 * y / h, 10 + 14 * y / h, 30 + 30 * y / h], axis=-1)
        img = sky.copy()
        for k, (amp, freq, speed, color) in enumerate([(14, 0.035, 1, (60, 255, 140)), (10, 0.05, 2, (150, 80, 255)), (8, 0.07, 1, (40, 220, 190))]):
            center = h * (0.35 + 0.12 * k) + amp * np.sin(x * freq + speed * t + k) + 6 * np.sin(x * freq * 2.3 - t + k * 2)
            band = np.exp(-((y - center) ** 2) / (2 * (10 + 4 * k) ** 2))
            shimmer = 0.6 + 0.4 * np.sin(x * 0.12 + 3 * t + k) ** 2
            glow = band * shimmer * (0.5 + 0.5 * np.sin(x * 0.01 + t + k) ** 2)
            img += glow[..., None] * np.array(color, dtype=np.float32) * 0.55
        # a few fixed stars
        rng = np.random.default_rng(7)
        for sx, sy in zip(rng.integers(0, w, 40), rng.integers(0, h // 2, 40)):
            img[sy, sx] += 120 * (0.6 + 0.4 * math.sin(t * 2 + sx))
        frames.append(Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)))
    return frames


def fire(size=160, n=16):
    """Flames rising in a round frame (the dial's screen is round). Sums of moving waves give the
    tongues; whole numbers of turns per loop make it repeat seamlessly."""
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32) / size
    up = 1 - yy  # 0 at the bottom, 1 at the top
    mask = np.clip((size / 2 - 2 - np.hypot(xx * size - size / 2, yy * size - size / 2)) / 3, 0, 1)
    waves = [(9, 3, 2, 0.0, 0.45), (17, 5, 3, 1.3, 0.25), (31, 8, 4, 2.1, 0.14), (5, 2, 1, 0.7, 0.3)]
    frames = []
    for i in range(n):
        t = 2 * math.pi * i / n
        noise = sum(a * np.sin(fx * xx * math.pi + fy * yy * math.pi + m * t + ph) for fx, fy, m, ph, a in waves)
        # hotter in the middle and low down; the noise cuts the top into tongues
        width = 1 - 1.6 * (xx - 0.5) ** 2
        v = np.clip(1.5 * width * (1 - up) ** 0.9 + 0.55 * noise * up - 0.35 * up, 0, 1)
        rgb = np.stack([np.clip(v * 1.8, 0, 1), np.clip(v * 1.8 - 0.65, 0, 1) * 0.9, np.clip(v * 1.8 - 1.3, 0, 1) * 0.75], axis=-1)
        frames.append(Image.fromarray((rgb * mask[..., None] * 255).astype(np.uint8)))
    return frames


def loader(size=144, n=24):
    """A ring with a bright head and a fading tail."""
    frames = []
    s = size * 4
    for i in range(n):
        im = Image.new("RGB", (s, s), (16, 14, 26))
        d = ImageDraw.Draw(im)
        pad, width = s // 6, s // 12
        box = (pad, pad, s - pad, s - pad)
        d.ellipse(box, outline=(44, 40, 62), width=width)
        head = i * 360 / n
        for k in range(28):
            a = k / 28
            color = (int(80 + 150 * a), int(60 + 120 * a), int(200 + 55 * a))
            start = head - 200 + k * 7
            d.arc(box, start, start + 9, fill=color, width=width)
        frames.append(im.resize((size, size), Image.Resampling.LANCZOS))
    return frames


def bars(size=144, n=24, count=7):
    """Audio level bars bouncing to a loop."""
    frames = []
    for i in range(n):
        t = 2 * math.pi * i / n
        im = Image.new("RGB", (size, size), (14, 18, 30))
        d = ImageDraw.Draw(im)
        gap = size // (count * 3)
        bw = (size - gap * (count + 1)) // count
        for b in range(count):
            level = 0.25 + 0.7 * abs(math.sin(t * (1 + b % 3) / 2 + b * 0.9))
            top = int(size * (0.9 - 0.75 * level))
            x0 = gap + b * (bw + gap)
            for y in range(top, int(size * 0.9), 3):
                f = (y - top) / max(1, size * 0.9 - top)
                d.rectangle((x0, y, x0 + bw, y + 1), fill=(int(255 - 120 * f), int(80 + 150 * f), int(180 + 60 * f)))
        frames.append(im.filter(ImageFilter.SMOOTH))
    return frames


def main():
    os.makedirs(OUT, exist_ok=True)
    save_gif(aurora(), "aurora.gif", colors=64)
    save_gif(fire(), "fire.gif", colors=48)
    save_gif(loader(), "loader.gif", colors=64)
    save_gif(bars(), "bars.gif", colors=48)
    total = sum(os.path.getsize(os.path.join(OUT, f)) for f in os.listdir(OUT))
    print(f"all samples: {total // 1024} KB")


if __name__ == "__main__":
    main()
