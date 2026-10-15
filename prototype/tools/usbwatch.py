"""Watches which deck modules are on USB (enumeration only, nothing opened), prints each change."""
import hid, time, sys, datetime
dur = float(sys.argv[1]) if len(sys.argv) > 1 else 180
names = {0x1fe: "keys", 0x206: "dial", 0x202: "knobs", 0x204: "rollers", 0x1f6: "base"}
def now(): return {names.get(d["product_id"], hex(d["product_id"])) for d in hid.enumerate(0x2516)}
seen = now(); t0 = time.time(); drops = 0
print(datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3], "start:", sorted(seen), flush=True)
while time.time() - t0 < dur:
    cur = now()
    for m in sorted(seen - cur):
        drops += m == "keys"
        print(datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3], "GONE", m, flush=True)
    for m in sorted(cur - seen):
        print(datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3], "BACK", m, flush=True)
    seen = cur
    time.sleep(0.1)
print("done: keys gone", drops, "times", flush=True)
