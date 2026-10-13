"""Write test for the 15-key module: software mode, info reads, colors, one image, key presses.

    .venv/Scripts/python tools/keys_test.py [listen_seconds]

Restores hardware mode at the end.
"""
import sys
import time

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, __file__.rsplit("tools", 1)[0])
from masterhub.driver import Keys, open_all  # noqa: E402


def label(text, bg, fg=(255, 255, 255), size=72):
    img = Image.new("RGB", (size, size), bg)
    d = ImageDraw.Draw(img)
    try:
        font = ImageFont.truetype("arialbd.ttf", 22)
    except OSError:
        font = ImageFont.load_default()
    d.text((size / 2, size / 2), text, fill=fg, font=font, anchor="mm")
    return img


def main():
    listen = float(sys.argv[1]) if len(sys.argv) > 1 else 5
    mods = open_all()
    keys: Keys = mods["keys"]
    keys.initialize()
    time.sleep(0.3)
    print("serial   ", keys.serial_number())
    print("firmware ", keys.firmware())
    print("infos    ", " ".join(f"{b:02x}" for b in keys.device_infos()))
    print("backlight", keys.backlight())

    t = time.time()
    keys.fill_screen((10, 10, 30))
    for i in range(15):
        keys.fill_key(i, (40 + i * 13, 80, 200 - i * 10))
    print(f"colors sent in {time.time() - t:.2f}s")

    t = time.time()
    for i in range(15):
        keys.set_key_image(i, label(str(i), (20, 20 + i * 12, 60)))
    print(f"15 images sent in {time.time() - t:.2f}s")

    print(f"press keys for {listen:.0f}s...")
    end = time.time() + listen
    while time.time() < end:
        for ev in keys.read_events(100):
            if ev.kind == "keys":
                print("  pressed:", [i for i, v in enumerate(ev.raw) if v])
            else:
                print("  ", ev)

    keys.set_software_mode(False)
    for m in mods.values():
        m.close()


if __name__ == "__main__":
    main()
