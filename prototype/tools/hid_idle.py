"""HID class requests GET_IDLE / SET_IDLE to a deck module over USB control transfers (libusb).

Why: on the owner's Mac the rollers module sends no input reports while the same module works on
Windows (2026-10-14/15). Windows' HID driver sends SET_IDLE(0) to every HID device at start;
macOS does not to all. This reads the idle rate (read-only) and, with --set, sends SET_IDLE(0).
Neither touches the firmware's storage. Needs libusb (brew install libusb) and pyusb.

    .venv/bin/python -I prototype/tools/hid_idle.py rollers [--set]
"""
import sys

import usb.backend.libusb1
import usb.core
import usb.util

PIDS = {"base": 0x01F6, "keys": 0x01FE, "faders": 0x0200, "knobs": 0x0202, "rollers": 0x0204, "dial": 0x0206}
LIBS = ["/opt/homebrew/lib/libusb-1.0.dylib", "/usr/local/lib/libusb-1.0.dylib", "libusb-1.0"]


def backend():
    for lib in LIBS:
        be = usb.backend.libusb1.get_backend(find_library=lambda _x, lib=lib: lib)
        if be:
            return be
    sys.exit("libusb not found (brew install libusb)")


def main():
    kind = sys.argv[1] if len(sys.argv) > 1 else "rollers"
    dev = usb.core.find(idVendor=0x2516, idProduct=PIDS[kind], backend=backend())
    if dev is None:
        sys.exit(f"no {kind} module")
    # pyusb claims the interface for interface requests, which macOS refuses (its HID driver holds
    # it); the backend call goes through the device's default pipe without a claim.
    be = dev._ctx.backend
    handle = dev._ctx.managed_open()
    try:
        idle = usb.util.create_buffer(1)
        n = be.ctrl_transfer(handle, 0xA1, 0x02, 0, 0, idle, 1000)
        idle = idle[:n]  # GET_IDLE, report 0, interface 0
        print(f"{kind}: GET_IDLE -> {list(idle)} (x4 ms; 0 = only on change)")
    except usb.core.USBError as e:
        print(f"{kind}: GET_IDLE failed: {e}")
    if "--set" in sys.argv:
        try:
            be.ctrl_transfer(handle, 0x21, 0x0A, 0, 0, usb.util.create_buffer(0), 1000)  # SET_IDLE(0), interface 0
            print(f"{kind}: SET_IDLE(0) sent")
        except usb.core.USBError as e:
            print(f"{kind}: SET_IDLE failed: {e}")


if __name__ == "__main__":
    main()
