"""Full-panel image test: one 480x272 picture across all 15 keys (command 02 35).

    .venv/Scripts/python -u tools/screen_test.py [seconds]
"""
import sys
import time

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, __file__.rsplit("tools", 1)[0])
from masterhub.driver import Keys, open_all  # noqa: E402


def panel_image(t: float) -> Image.Image:
    w, h = Keys.screen_size
    img = Image.new("RGB", (w, h))
    px = img.load()
    for x in range(w):
        for y in range(h):
            px[x, y] = (int(255 * x / w), int(80 + 60 * ((y + t * 40) % h) / h), int(255 * (1 - x / w)))
    d = ImageDraw.Draw(img)
    try:
        big, small = ImageFont.truetype("arialbd.ttf", 54), ImageFont.truetype("arial.ttf", 22)
    except OSError:
        big = small = ImageFont.load_default()
    d.text((w / 2, h / 2 - 20), "masterhub", fill="white", font=big, anchor="mm")
    d.text((w / 2, h / 2 + 35), time.strftime("%H:%M:%S"), fill="white", font=small, anchor="mm")
    return img


def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 15
    mods = open_all()
    keys: Keys = mods["keys"]
    keys.initialize()
    frames, start = 0, time.time()
    while time.time() - start < seconds:
        t0 = time.time()
        keys.set_screen_image(panel_image(time.time() - start))
        frames += 1
        time.sleep(max(0, 1.0 - (time.time() - t0)))
    print(f"{frames} full-panel frames in {time.time() - start:.1f}s")
    keys.release()
    for m in mods.values():
        m.close()


if __name__ == "__main__":
    main()
