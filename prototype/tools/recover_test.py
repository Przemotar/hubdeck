"""Experiment: can a wedged keys module be recovered without pulling it off the base? (Windows only)

    python prototype/tools/recover_test.py <hub instance id> <port number>

1. check state, 2. wedge keys on purpose (one feature read in hardware mode), 3. try recoveries
from gentle to hard, checking the state after each.
"""
import ctypes
import sys
import time
from ctypes import wintypes

import hid

sys.stdout.reconfigure(encoding="utf-8")
KEYS = 0x01FE


def keys_state() -> str:
    found = hid.enumerate(0x2516, KEYS)
    if not found:
        return "absent"
    d = hid.device()
    d.open_path(found[0]["path"])
    try:
        d.send_feature_report([0x03, 0x3A, 1] + [0] * 29)
        time.sleep(0.25)
        fw = bytes(d.get_feature_report(0x38, 0x21))
        d.send_feature_report([0x03, 0x3A, 0] + [0] * 29)
    finally:
        d.close()
    return f"fw={fw[6:17].decode(errors='replace')!r}" if len(fw) > 6 else "WEDGED (empty answer)"


def wedge():
    d = hid.device()
    d.open_path(hid.enumerate(0x2516, KEYS)[0]["path"])
    d.get_feature_report(0x38, 0x21)  # hardware mode read: the trigger
    d.close()


def reopen_and_toggle():
    d = hid.device()
    d.open_path(hid.enumerate(0x2516, KEYS)[0]["path"])
    for on in (0, 1, 0, 1):
        d.send_feature_report([0x03, 0x3A, on] + [0] * 29)
        time.sleep(0.2)
    d.close()


def cycle_port(hub_instance_id: str, port: int):
    GUID_DEVINTERFACE_USB_HUB = "{f18a0e88-c30c-11d0-8815-00a0c906bed8}"
    path = "\\\\?\\" + hub_instance_id.replace("\\", "#") + "#" + GUID_DEVINTERFACE_USB_HUB
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    k32.CreateFileW.restype = wintypes.HANDLE
    GENERIC_WRITE, FILE_SHARE_RW, OPEN_EXISTING = 0x40000000, 0x3, 3
    h = k32.CreateFileW(path, GENERIC_WRITE, FILE_SHARE_RW, None, OPEN_EXISTING, 0, None)
    if h in (None, wintypes.HANDLE(-1).value):
        return f"open hub failed, error {ctypes.get_last_error()} ({path})"

    class CycleParams(ctypes.Structure):
        _fields_ = [("ConnectionIndex", wintypes.ULONG), ("StatusReturned", wintypes.ULONG)]

    params = CycleParams(port, 0)
    returned = wintypes.DWORD(0)
    IOCTL_USB_HUB_CYCLE_PORT = 0x220444
    ok = k32.DeviceIoControl(h, IOCTL_USB_HUB_CYCLE_PORT, ctypes.byref(params), ctypes.sizeof(params),
                             ctypes.byref(params), ctypes.sizeof(params), ctypes.byref(returned), None)
    err = ctypes.get_last_error()
    k32.CloseHandle(h)
    return f"cycle port ok={bool(ok)} error={err} status={params.StatusReturned}"


def main():
    hub, port = sys.argv[1], int(sys.argv[2])
    print("1 before:", keys_state())
    wedge()
    print("2 after wedge:", keys_state())
    reopen_and_toggle()
    print("3 after reopen + mode toggles:", keys_state())
    print("4", cycle_port(hub, port))
    for _ in range(10):
        time.sleep(1)
        if hid.enumerate(0x2516, KEYS):
            break
    time.sleep(1)
    print("5 after port cycle:", keys_state())


if __name__ == "__main__":
    main()
