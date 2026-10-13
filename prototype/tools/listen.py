"""Read-only listener: prints raw input reports from every MasterHUB module.

    .venv/Scripts/python tools/listen.py [seconds]

Input report layout (from cooltek-sdk-probe): [0]=0x01, [1]=channel tag,
[2:4]=payload length (LE), [4:]=payload = event id, length, values...
"""
import sys
import threading
import time

import hid

VENDOR_ID = 0x2516
NAMES = {0x01F6: "base", 0x01FE: "keys", 0x0200: "faders", 0x0202: "knobs", 0x0204: "rollers", 0x0206: "dial"}


def listen(info, stop_at):
    name = NAMES.get(info["product_id"], hex(info["product_id"]))
    dev = hid.device()
    dev.open_path(info["path"])
    try:
        while time.time() < stop_at:
            data = dev.read(64, 200)
            if data:
                ln = data[2] | data[3] << 8 if len(data) > 3 else 0
                payload = " ".join(f"{b:02x}" for b in data[4 : 4 + ln])
                print(f"{time.strftime('%H:%M:%S')} {name:8s} tag={data[1]:02x} payload={payload}", flush=True)
    finally:
        dev.close()


def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 30
    stop_at = time.time() + seconds
    threads = [threading.Thread(target=listen, args=(i, stop_at)) for i in hid.enumerate(VENDOR_ID)]
    for t in threads:
        t.start()
    print(f"listening on {len(threads)} modules for {seconds:.0f}s", flush=True)
    for t in threads:
        t.join()


if __name__ == "__main__":
    main()
