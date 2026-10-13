"""Interactive hardware check over the hot-plug hub.

    .venv/Scripts/python -u tools/demo.py [seconds]

- keys: numbers on colored tiles; a pressed key turns yellow until released
- faders: each fader's LED goes red (bottom) -> green (top) as it moves
- base: edge LED static cyan
- rollers / knobs: printed
- pull a module off and put it back: it should reconnect and redraw by itself
Keys go back to hardware mode at the end.
"""
import logging
import sys
import time

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, __file__.rsplit("tools", 1)[0])
from masterhub.driver import Faders, Keys, Module, _M0  # noqa: E402
from masterhub.hub import Hub  # noqa: E402

try:
    FONT = ImageFont.truetype("arialbd.ttf", 26)
except OSError:
    FONT = ImageFont.load_default()


def tile(text: str, bg, fg=(255, 255, 255)) -> Image.Image:
    img = Image.new("RGB", (Keys.key_px, Keys.key_px), bg)
    ImageDraw.Draw(img).text((36, 36), text, fill=fg, font=FONT, anchor="mm")
    return img


def idle_tile(i: int) -> Image.Image:
    return tile(str(i), (20, 30 + i * 11, 90 - i * 3))


PRESSED = tile("!", (250, 200, 0), (0, 0, 0))


def fader_color(v: int):
    return (int(255 * (100 - v) / 100), int(255 * v / 100), 0)


def on_connect(m: Module) -> None:
    if isinstance(m, Keys):
        for i in range(15):
            m.set_key_image(i, idle_tile(i))
    elif isinstance(m, Faders):
        m.set_led_mode(1)
        for i, v in enumerate(m.values()):
            m.set_led(fader_color(v), index=i)
    elif m.name == "base" and isinstance(m, _M0):
        m.set_led_mode(1)
        m.set_led((0, 200, 255))


def main():
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s", datefmt="%H:%M:%S")
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 60
    end = time.time() + seconds
    with Hub(on_connect=on_connect) as hub:
        print(f"running {seconds:.0f}s with: {sorted(m.name for m in hub.modules.values())}", flush=True)
        while time.time() < end:
            for ev in hub.events(timeout=0.5):
                ts = time.strftime("%H:%M:%S")
                if ev.kind in ("key_down", "key_up"):
                    keys = hub.get("keys")
                    if keys:
                        keys.set_key_image(ev.index, PRESSED if ev.kind == "key_down" else idle_tile(ev.index))
                elif ev.kind == "fader":
                    faders = hub.get("faders")
                    if faders:
                        faders.set_led(fader_color(ev.value), index=ev.index)
                print(f"{ts} {ev.module:8s} {ev.kind:12s} idx={ev.index:2d} val={ev.value}", flush=True)
                if time.time() >= end:
                    break
    print("done; keys back in hardware mode", flush=True)


if __name__ == "__main__":
    main()
