"""Generate HubDeck's own sample pictures for the test profiles (MIT, made here, no third-party art).

Run: .venv/Scripts/python -I prototype/tools/gen_samples.py   (macOS: .venv/bin/python)
Writes app/src-tauri/resources/samples/: spinning ring and bouncing dot GIFs for keys, a spinning
GIF for the dial, three gradient PNGs with simple shapes, and a 480x272 panel picture.
"""
import math
import os

from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "app", "src-tauri", "resources", "samples")


def save_gif(frames, name, ms):
    frames[0].save(os.path.join(OUT, name), save_all=True, append_images=frames[1:], duration=ms, loop=0, optimize=True)


def spinning_ring(size, n, color, bg):
    frames = []
    for i in range(n):
        im = Image.new("RGB", (size, size), bg)
        d = ImageDraw.Draw(im)
        pad = size // 8
        box = (pad, pad, size - pad, size - pad)
        d.ellipse(box, outline=(60, 56, 80), width=max(4, size // 14))
        start = i * 360 / n
        d.arc(box, start, start + 100, fill=color, width=max(4, size // 14))
        frames.append(im)
    return frames


def bouncing_dot(size, n):
    frames = []
    for i in range(n):
        im = Image.new("RGB", (size, size), (20, 24, 40))
        d = ImageDraw.Draw(im)
        t = i / n
        y = size * 0.2 + abs(math.sin(t * math.pi)) * 0 + (1 - abs(math.sin(t * math.pi))) * size * 0.5
        r = size // 9
        x = size / 2
        d.ellipse((x - r, y - r, x + r, y + r), fill=(250, 190, 60))
        d.line((size * 0.15, size * 0.85, size * 0.85, size * 0.85), fill=(90, 90, 120), width=3)
        frames.append(im)
    return frames


def gradient(size, a, b, shape):
    im = Image.new("RGB", (size, size))
    px = im.load()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * size - 2)
            px[x, y] = tuple(int(a[k] + (b[k] - a[k]) * t) for k in range(3))
    d = ImageDraw.Draw(im)
    c, r = size / 2, size / 4
    if shape == "circle":
        d.ellipse((c - r, c - r, c + r, c + r), outline="white", width=5)
    elif shape == "square":
        d.rectangle((c - r, c - r, c + r, c + r), outline="white", width=5)
    else:
        d.polygon([(c, c - r), (c + r, c + r * 0.8), (c - r, c + r * 0.8)], outline="white", width=5)
    return im


def panel():
    w, h = 480, 272
    im = Image.new("RGB", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            px[x, y] = (int(30 + 90 * x / w), int(20 + 60 * y / h), int(90 + 120 * (1 - x / w)))
    d = ImageDraw.Draw(im)
    for i in range(6):
        d.ellipse((40 + i * 70, 100 - i * 8, 110 + i * 70, 170 - i * 8), outline=(255, 255, 255), width=3)
    d.text((20, 240), "HubDeck panel picture", fill="white")
    return im


def main():
    os.makedirs(OUT, exist_ok=True)
    save_gif(spinning_ring(144, 16, (196, 181, 253), (22, 20, 30)), "ring.gif", 70)
    save_gif(bouncing_dot(144, 16), "bounce.gif", 70)
    save_gif(spinning_ring(240, 20, (250, 140, 60), (12, 12, 18)), "dial-spin.gif", 70)
    gradient(144, (120, 40, 200), (20, 160, 220), "circle").save(os.path.join(OUT, "violet.png"))
    gradient(144, (240, 120, 40), (200, 30, 80), "square").save(os.path.join(OUT, "sunset.png"))
    gradient(144, (30, 160, 90), (20, 60, 140), "triangle").save(os.path.join(OUT, "sea.png"))
    panel().save(os.path.join(OUT, "panel.png"), optimize=True)
    for f in sorted(os.listdir(OUT)):
        print(f, os.path.getsize(os.path.join(OUT, f)), "bytes")


if __name__ == "__main__":
    main()
