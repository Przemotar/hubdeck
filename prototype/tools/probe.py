"""Probe: lists MasterHUB modules and reads their info feature reports.

Keys and dial are switched to software mode before reading and back to hardware mode after:
reading them in hardware mode wedges their firmware until the module is pulled off the base
(verified 2026-10-09). M0 modules (base, faders, knobs, rollers) are only read.

    .venv/Scripts/python tools/probe.py
"""
import time

import hid

VENDOR_ID = 0x2516

# product_id -> (model, {name: feature report id to GET})
MODULES = {
    0x01F6: ("M0064 Base", {"fw_boot": 0x2D, "fw": 0x2E, "fw_2": 0x2F, "serial": 0x30, "led_brightness": 0x32, "led_mode": 0x34, "direction": 0x35, "config": 0x36}),
    0x01FE: ("M4315 15xKey IPS", {"fw_0": 0x37, "fw": 0x38, "fw_2": 0x39, "serial": 0x3A, "device_infos": 0x3B, "backlight": 0x3C, "animation": 0x3D, "config": 0x3E}),
    0x0200: ("M005S 5xFader", {"fw": 0x2E, "serial": 0x30, "led_brightness": 0x32, "slider_values": 0x33, "led_mode": 0x34, "direction": 0x35, "config": 0x36}),
    0x0202: ("M003K 3xKnob", {"fw": 0x2E, "serial": 0x30, "led_brightness": 0x32, "led_mode": 0x34, "direction": 0x35, "config": 0x36}),
    0x0204: ("M032LE 2xRoller", {"fw": 0x2E, "serial": 0x30, "direction": 0x35, "config": 0x36}),
    0x0206: ("M131E 1xDial IPS", {"fw": 0x48, "serial": 0x4A, "device_infos": 0x4B, "backlight": 0x4C}),
}
# feature command that toggles software mode; reading these modules in hardware mode wedges them
SOFTWARE_MODE = {0x01FE: 0x3A, 0x0206: 0x49}


def get_feature(dev, report_id, size=0x21):
    try:
        data = dev.get_feature_report(report_id, size)
    except (OSError, ValueError) as e:
        return f"ERROR {e}"
    if not data:
        return "empty"
    hexs = " ".join(f"{b:02x}" for b in data[: 2 + min(data[1] if len(data) > 1 else 0, 31)])
    text = bytes(b for b in data[2:] if 0x20 <= b < 0x7F).decode()
    return f"{hexs}   |{text}|"


def main():
    found = hid.enumerate(VENDOR_ID)
    if not found:
        print("No MasterHUB modules found.")
        return
    for info in found:
        pid = info["product_id"]
        model, reports = MODULES.get(pid, (f"UNKNOWN 0x{pid:04x}", {"serial_30": 0x30, "serial_4a": 0x4A}))
        print(f"== {model}  pid=0x{pid:04x}  {info['product_string']}  sn={info['serial_number']}")
        dev = hid.device()
        dev.open_path(info["path"])
        sw = SOFTWARE_MODE.get(pid)
        try:
            if sw:
                dev.send_feature_report([0x03, sw, 0x01] + [0] * 29)
                time.sleep(0.25)
            for name, rid in reports.items():
                print(f"   {name:15s} 0x{rid:02x}: {get_feature(dev, rid)}")
        finally:
            if sw:
                dev.send_feature_report([0x03, sw, 0x00] + [0] * 29)
            dev.close()


if __name__ == "__main__":
    main()
