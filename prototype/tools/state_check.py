"""Check whether keys and dial are in the good state: correct USB strings and answering feature reports.

    python prototype/tools/state_check.py [repeat]

Each round: enumerate (reads USB strings), open, software mode on, read firmware, software mode off.
"""
import sys
import time

import hid

sys.stdout.reconfigure(encoding="utf-8")
MODULES = {0x01FE: ("keys", 0x3A, 0x38), 0x0206: ("dial", 0x49, 0x48)}


def check_once():
    out = []
    for pid, (name, sw, fw_id) in MODULES.items():
        found = hid.enumerate(0x2516, pid)
        if not found:
            out.append(f"{name}: absent")
            continue
        info = found[0]
        d = hid.device()
        d.open_path(info["path"])
        try:
            d.send_feature_report([0x03, sw, 1] + [0] * 29)
            time.sleep(0.25)
            fw = bytes(d.get_feature_report(fw_id, 0x21))
            fw_text = fw[6:17].decode(errors="replace") if len(fw) > 6 else "EMPTY"
            d.send_feature_report([0x03, sw, 0] + [0] * 29)
        finally:
            d.close()
        out.append(f"{name}: serial={info['serial_number']!r} fw={fw_text!r}")
    return out


if __name__ == "__main__":
    for i in range(int(sys.argv[1]) if len(sys.argv) > 1 else 1):
        print(i, " | ".join(check_once()), flush=True)
