"""MasterHUB HID driver.

Protocol source: cooltek-sdk-probe (projectzero7, MIT), checked against our own
hardware in tools/probe.py and tools/keys_test.py. Every module is a separate HID
device with vendor id 0x2516.

Safety: the module firmware also accepts commands that rewrite the serial number,
save factory data, reset to factory or flash firmware. This driver never sends
them; `_guard` refuses those command bytes even if a caller builds a raw report.
"""
from __future__ import annotations

import io
import threading
import time
from dataclasses import dataclass, field

import hid

VENDOR_ID = 0x2516

PID_BASE = 0x01F6
PID_KEYS = 0x01FE
PID_FADERS = 0x0200
PID_KNOBS = 0x0202
PID_ROLLERS = 0x0204
PID_DIAL = 0x0206

MODULE_NAMES = {
    PID_BASE: "base",
    PID_KEYS: "keys",
    PID_FADERS: "faders",
    PID_KNOBS: "knobs",
    PID_ROLLERS: "rollers",
    PID_DIAL: "dial",
}

# (report id, command byte) pairs that can brick a module or erase its identity.
_FORBIDDEN = {
    # M0 family (base, faders, knobs, rollers): output reports
    (0x02, 0x2A),  # update firmware
    (0x02, 0x2B),  # set serial number
    (0x02, 0x31),  # save factory data
    (0x02, 0x49),  # test fw recovery
    (0x02, 0x4A),  # test factory reset
    (0x02, 0x2F),  # recalibrate the base tilt sensor (no known way back)
    # M4315 (keys)
    (0x02, 0x34),  # update firmware (keys); M0 0x34 is not an output command
    (0x03, 0x35),  # set serial number
    (0x03, 0x39),  # test fw recovery
    (0x03, 0x3C),  # save factory data
    (0x03, 0x3D),  # test factory reset
    (0x03, 0x3E),  # test animation recovery
    (0x02, 0x38),  # save screen picture to flash
    # M131E (dial)
    (0x02, 0x44),  # update firmware
    (0x03, 0x45),  # set serial number
    (0x03, 0x48),  # test fw recovery
    (0x03, 0x4B),  # save factory data
    (0x03, 0x4C),  # test factory reset
    (0x03, 0x4D),  # test animation recovery
    (0x02, 0x47),  # save screen picture to flash
}


class ForbiddenCommand(RuntimeError):
    pass


def to_jpeg(image, size: tuple[int, int], quality: int = 90) -> bytes:
    """PIL.Image -> JPEG bytes at the exact size a screen expects."""
    img = image.convert("RGB")
    if img.size != size:
        img = img.resize(size)
    buf = io.BytesIO()
    img.save(buf, "JPEG", quality=quality)
    return buf.getvalue()


def _output_report_len(descriptor: list[int]) -> int:
    """Length (with report id) of output report 0x02, read from a HID report descriptor."""
    report_id, count, size, i = 0, 0, 0, 0
    while i < len(descriptor):
        prefix = descriptor[i]
        n = (0, 1, 2, 4)[prefix & 0x03]
        value = int.from_bytes(bytes(descriptor[i + 1 : i + 1 + n]), "little")
        tag = prefix & 0xFC
        if tag == 0x84:
            report_id = value
        elif tag == 0x94:
            count = value
        elif tag == 0x74:
            size = value
        elif tag == 0x90 and report_id == 0x02:  # Output item
            return 1 + count * size // 8
        i += 1 + n
    return 0


def _guard(report: bytes | list[int]) -> None:
    if (report[0], report[1]) in _FORBIDDEN:
        raise ForbiddenCommand(f"refusing report {report[0]:#04x} command {report[1]:#04x}")


@dataclass
class Event:
    module: str
    kind: str  # key, fader, roller, knob, knob_press, heartbeat, raw
    index: int = -1
    value: int = 0
    raw: list[int] = field(default_factory=list)


class Module:
    """One HID device. Writes are serialized with a lock; reads happen on the caller's thread."""

    pid: int = 0
    output_len: int = 0x40

    def __init__(self, info: dict):
        self.info = info
        self.serial = info.get("serial_number") or ""
        self.name = MODULE_NAMES.get(info["product_id"], hex(info["product_id"]))
        self.dev = hid.device()
        self.dev.open_path(info["path"])
        self._lock = threading.Lock()
        try:
            self.output_len = _output_report_len(self.dev.get_report_descriptor()) or self.output_len
        except (OSError, ValueError, AttributeError):
            pass

    def close(self) -> None:
        self.dev.close()

    def initialize(self) -> None:
        pass

    def release(self) -> None:
        """Hand the module back to its firmware (hardware mode) before closing."""

    def write(self, report: list[int] | bytes) -> None:
        _guard(report)
        buf = list(report) + [0] * (self.output_len - len(report))
        with self._lock:
            self.dev.write(buf)

    def send_feature(self, report: list[int]) -> None:
        _guard(report)
        buf = list(report) + [0] * (0x20 - len(report))
        with self._lock:
            self.dev.send_feature_report(buf)

    def send_chunked(self, command: int, data: bytes, index: int = 0) -> None:
        """Image transfer used by keys (0x35 panel, 0x36 key) and dial (0x45).

        Report: 02 cmd chunk_idx(LE16) chunk_len(LE16) index last_flag data...
        """
        chunk = self.output_len - 8
        remaining, chunk_index = len(data), 0
        while remaining > 0:
            n = min(remaining, chunk)
            offset = len(data) - remaining
            last = remaining <= chunk
            header = [0x02, command, chunk_index & 0xFF, chunk_index >> 8, n & 0xFF, n >> 8, index, int(last)]
            self.write(header + list(data[offset : offset + n]))
            chunk_index += 1
            remaining -= n

    def get_feature(self, report_id: int, size: int = 0x21) -> list[int]:
        with self._lock:
            data = self.dev.get_feature_report(report_id, size)
        if not data or len(data) < 2:
            return []
        return list(data[2 : 2 + data[1]])

    def read_events(self, timeout_ms: int = 100) -> list[Event]:
        data = self.dev.read(64, timeout_ms)
        if not data or data[0] != 0x01 or len(data) < 4:
            return []
        length = data[2] | data[3] << 8
        return self.parse(data[1], list(data[4 : 4 + length]))

    def parse(self, tag: int, payload: list[int]) -> list[Event]:
        return [Event(self.name, "raw", raw=[tag, *payload])]

    def serial_number(self) -> str:
        return bytes(self.get_feature(0x30)).decode(errors="replace").strip("\0")

    def firmware(self) -> str:
        return bytes(self.get_feature(0x2E)[4:]).decode(errors="replace").strip("\0")


class _M0(Module):
    """Base, faders, knobs, rollers. LED target: 0=base edge, 1=faders, 2=knobs."""

    led_target = 0
    led_count = 1

    def set_led(self, rgb: tuple[int, int, int], index: int = 0xFF, save: bool = False) -> None:
        idx = 0 if self.led_target == 0 else index
        self.write([0x02, 0x2D, self.led_target, idx, *rgb, int(save)])

    def set_led_mode(self, mode: int, index: int = 0xFF, save: bool = False) -> None:
        """mode: 0 off, 1 static, 2 breathing, 3 strobe, 4 spring, 5 cycling, 6 random, 7 wave, 8 water, 9 rainbow."""
        idx = 0 if self.led_target == 0 else index
        self.write([0x02, 0x2C, self.led_target, idx, mode, int(save)])

    def set_led_brightness(self, brightness: int, index: int = 0xFF) -> None:
        idx = 0 if self.led_target == 0 else index
        self.write([0x02, 0x2E, self.led_target, idx, max(0, min(100, brightness))])

    def set_led_speed(self, speed: int) -> None:
        self.write([0x02, 0x30, max(0, min(100, speed))])


class Base(_M0):
    pid = PID_BASE

    def parse(self, tag, payload):
        if tag == 0x2B:
            return [Event(self.name, "heartbeat", raw=payload)]
        return super().parse(tag, payload)


class Faders(_M0):
    pid = PID_FADERS
    led_target = 1
    led_count = 5

    def values(self) -> list[int]:
        return self.get_feature(0x33)[:5]

    def parse(self, tag, payload):
        if tag == 0x2A and len(payload) >= 7 and payload[0] == 0x02:
            return [Event(self.name, "fader", i, v) for i, v in enumerate(payload[2:7])]
        return super().parse(tag, payload)


class Knobs(_M0):
    pid = PID_KNOBS
    led_target = 2
    led_count = 3

    def parse(self, tag, payload):
        if tag == 0x2A and len(payload) >= 5:
            if payload[0] == 0x00:  # rotation: 0 ccw, 1 cw, 0xff no change
                return [Event(self.name, "knob", i, 1 if v == 1 else -1) for i, v in enumerate(payload[2:5]) if v != 0xFF]
            if payload[0] == 0x03:
                return [Event(self.name, "knob_press", i, v) for i, v in enumerate(payload[2:5])]
        return super().parse(tag, payload)


class Rollers(_M0):
    pid = PID_ROLLERS

    def parse(self, tag, payload):
        if tag == 0x2A and len(payload) >= 4 and payload[0] == 0x01:  # 0 up, 1 down, 0xff no change
            return [Event(self.name, "roller", i, -1 if v == 1 else 1) for i, v in enumerate(payload[2:4]) if v != 0xFF]
        return super().parse(tag, payload)


class Keys(Module):
    """15 keys over one 480x272 IPS panel; device_infos (0x3b) reports a 5x3 grid of 72x72 px keys."""

    pid = PID_KEYS
    output_len = 0x140
    cols, rows, key_px = 5, 3, 72

    def initialize(self, attempts: int = 3) -> None:
        """Enter software mode and confirm the module answers.

        On 2026-10-09 the module once accepted every command but returned empty
        feature reports; a fresh software-mode switch is the cheapest retry.
        """
        for attempt in range(attempts):
            if attempt:
                self.set_software_mode(False)
                time.sleep(0.3)
            self.set_software_mode(True)
            time.sleep(0.2)
            if self.firmware():
                return
        raise RuntimeError(f"keys module {self.serial} does not answer after {attempts} software-mode attempts")

    def release(self) -> None:
        self.set_software_mode(False)

    def set_software_mode(self, on: bool) -> None:
        self.send_feature([0x03, 0x3A, 0x01 if on else 0x00])

    def serial_number(self) -> str:
        return bytes(self.get_feature(0x3A)).decode(errors="replace").strip("\0")

    def firmware(self) -> str:
        return bytes(self.get_feature(0x38)[4:]).decode(errors="replace").strip("\0")

    def device_infos(self) -> list[int]:
        return self.get_feature(0x3B)

    def backlight(self) -> int:
        v = self.get_feature(0x3C)
        return v[0] if v else -1

    def set_backlight(self, brightness: int) -> None:
        self.send_feature([0x03, 0x34, max(0, min(100, brightness))])

    def fill_screen(self, rgb: tuple[int, int, int]) -> None:
        self.send_feature([0x03, 0x36, *rgb])

    def fill_key(self, index: int, rgb: tuple[int, int, int]) -> None:
        self.send_feature([0x03, 0x37, index, *rgb])

    screen_size = (480, 272)

    def set_key_jpeg(self, index: int, jpeg: bytes) -> None:
        """Send a JPEG (72x72 recommended) to one key."""
        self.send_chunked(0x36, jpeg, index)

    def set_key_image(self, index: int, image, quality: int = 90) -> None:
        """image: PIL.Image of any size; resized to key_px and sent as JPEG."""
        self.set_key_jpeg(index, to_jpeg(image, (self.key_px, self.key_px), quality))

    def set_screen_image(self, image, quality: int = 85) -> None:
        """One 480x272 image across the whole panel (all 15 keys)."""
        self.send_chunked(0x35, to_jpeg(image, self.screen_size, quality))

    def parse(self, tag, payload):
        if tag == 0x34 and len(payload) >= 17 and payload[0] == 0x02:
            return [Event(self.name, "keys", -1, 0, raw=payload[2:17])]
        return super().parse(tag, payload)


class Dial(Module):
    """1xDial with a 240x240 IPS screen (M131E).

    Commands from CoolTEK_SDK_CM_M131E_V03.02.00.dll (disassembled 2026-10-09):
    software mode 03 49, backlight 03 44, fill color 03 46, picture 02 45 (chunked JPEG).
    """

    pid = PID_DIAL
    screen_size = (240, 240)

    def initialize(self) -> None:
        self.send_feature([0x03, 0x49, 0x01])
        time.sleep(0.2)

    def release(self) -> None:
        self.send_feature([0x03, 0x49, 0x00])

    def serial_number(self) -> str:
        return bytes(self.get_feature(0x4A)).decode(errors="replace").strip("\0")

    def set_backlight(self, brightness: int) -> None:
        self.send_feature([0x03, 0x44, max(0, min(100, brightness))])

    def fill_screen(self, rgb: tuple[int, int, int]) -> None:
        self.send_feature([0x03, 0x46, *rgb])

    def set_image(self, image, quality: int = 85) -> None:
        self.send_chunked(0x45, to_jpeg(image, self.screen_size, quality))

    def parse(self, tag, payload):
        # tag 0x44: 03 01 dir (00 clockwise, 01 counter-clockwise), 04 01 state (01 down, 00 up)
        if tag == 0x44 and len(payload) >= 3:
            if payload[0] == 0x03:
                return [Event(self.name, "dial", 0, 1 if payload[2] == 0 else -1)]
            if payload[0] == 0x04:
                return [Event(self.name, "dial_down" if payload[2] else "dial_up", 0, payload[2])]
        return super().parse(tag, payload)


_CLASSES = {c.pid: c for c in (Base, Faders, Knobs, Rollers, Keys, Dial)}


def open_all() -> dict[str, Module]:
    """Open every connected module, keyed by name (base, keys, faders, ...)."""
    modules: dict[str, Module] = {}
    for info in hid.enumerate(VENDOR_ID):
        cls = _CLASSES.get(info["product_id"], Module)
        try:
            m = cls(info)
        except OSError:
            continue  # module mid-attach; Hub retries, one-shot tools just skip it
        modules[m.name] = m
    return modules
