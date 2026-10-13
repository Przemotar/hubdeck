"""Layout A check: keys + dial + knobs + rollers.

    .venv/Scripts/python -u tools/layout_a_test.py [seconds]

- keys: one 480x272 picture across the panel (command 02 35); pressing a key redraws it
- dial screen: a value 0..100; dial rotation changes it by 1, each knob by 5, dial press resets to 50
- knob LEDs: knob i shows the value as red -> green
- every event is printed, dial events with raw bytes (dial input format not yet decoded)
"""
import logging
import sys
import time

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, __file__.rsplit("tools", 1)[0])
from masterhub.driver import Dial, Keys, Knobs, Module  # noqa: E402
from masterhub.hub import Hub  # noqa: E402


def font(size, bold=True):
    try:
        return ImageFont.truetype("arialbd.ttf" if bold else "arial.ttf", size)
    except OSError:
        return ImageFont.load_default()


state = {"value": 50, "last": "-"}


def color(v):
    return (int(255 * (100 - v) / 100), int(255 * v / 100), 40)


def dial_image():
    v = state["value"]
    img = Image.new("RGB", Dial.screen_size, (10, 10, 20))
    d = ImageDraw.Draw(img)
    d.ellipse((8, 8, 232, 232), outline=(60, 60, 80), width=10)
    d.arc((8, 8, 232, 232), start=-90, end=-90 + int(3.6 * v), fill=color(v), width=10)
    d.text((120, 112), str(v), fill="white", font=font(72), anchor="mm")
    d.text((120, 170), state["last"], fill=(170, 170, 190), font=font(18, False), anchor="mm")
    return img


def panel_image(pressed=None):
    w, h = Keys.screen_size
    img = Image.new("RGB", (w, h), (15, 18, 30))
    d = ImageDraw.Draw(img)
    d.rectangle((0, 0, int(w * state["value"] / 100), 10), fill=color(state["value"]))
    d.text((w / 2, h / 2 - 25), "masterhub", fill="white", font=font(54), anchor="mm")
    sub = f"key {pressed} pressed" if pressed is not None else time.strftime("%H:%M:%S")
    d.text((w / 2, h / 2 + 35), sub, fill=(180, 190, 220), font=font(24, False), anchor="mm")
    return img


def redraw(hub: Hub, pressed=None):
    if (dial := hub.get("dial")) is not None:
        dial.set_image(dial_image())
    if (knobs := hub.get("knobs")) is not None:
        knobs.set_led(color(state["value"]))
    if (keys := hub.get("keys")) is not None:
        keys.set_screen_image(panel_image(pressed))


def on_connect(m: Module) -> None:
    if isinstance(m, Knobs):
        m.set_led_mode(1)
        m.set_led(color(state["value"]))
    elif isinstance(m, Dial):
        m.set_image(dial_image())
    elif isinstance(m, Keys):
        m.set_screen_image(panel_image())


def main():
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s", datefmt="%H:%M:%S")
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 90
    end = time.time() + seconds
    with Hub(on_connect=on_connect) as hub:
        print(f"running {seconds:.0f}s with {sorted(m.name for m in hub.modules.values())}", flush=True)
        while time.time() < end:
            for ev in hub.events(timeout=0.5):
                ts = time.strftime("%H:%M:%S")
                raw = " ".join(f"{b:02x}" for b in ev.raw) if ev.module == "dial" else ""
                print(f"{ts} {ev.module:8s} {ev.kind:12s} idx={ev.index:2d} val={ev.value:3d} {raw}", flush=True)
                changed, pressed = False, None
                if ev.kind == "knob":
                    state["value"] = max(0, min(100, state["value"] + 5 * ev.value))
                    state["last"], changed = f"knob {ev.index + 1} {'+' if ev.value > 0 else '-'}", True
                elif ev.kind == "knob_down":
                    state["last"], changed = f"knob {ev.index + 1} press", True
                elif ev.kind == "roller":
                    state["value"] = max(0, min(100, state["value"] + ev.value))
                    state["last"], changed = f"roller {ev.index + 1}", True
                elif ev.kind == "key_down":
                    pressed, changed = ev.index, True
                elif ev.module == "dial" and ev.kind == "raw":
                    state["last"], changed = "dial " + raw[3:17], True
                if changed:
                    redraw(hub, pressed)
                if time.time() >= end:
                    break
    print("done; keys and dial back in hardware mode", flush=True)


if __name__ == "__main__":
    main()
