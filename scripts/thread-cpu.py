"""HubDeck's CPU per thread, by the threads' names (Windows): where the app's CPU goes, without
performance counters (they do not work on the owner's PC). Rust names its threads with
SetThreadDescription, so each thread's name is read back with GetThreadDescription and its CPU time
with GetThreadTimes, at the start and the end of the window.

    .venv/Scripts/python -I scripts/thread-cpu.py [seconds=30] [exe name=hubdeck.exe]

Prints each thread's share of one core over the window, the busiest first, and the total."""
import ctypes, ctypes.wintypes as wt, subprocess, sys, time

seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 30.0
exe = sys.argv[2] if len(sys.argv) > 2 else "hubdeck.exe"
k32 = ctypes.WinDLL("kernel32", use_last_error=True)

TH32CS_SNAPTHREAD = 0x4
THREAD_QUERY_LIMITED_INFORMATION = 0x0800


class THREADENTRY32(ctypes.Structure):
    _fields_ = [("dwSize", wt.DWORD), ("cntUsage", wt.DWORD), ("th32ThreadID", wt.DWORD), ("th32OwnerProcessID", wt.DWORD),
                ("tpBasePri", wt.LONG), ("tpDeltaPri", wt.LONG), ("dwFlags", wt.DWORD)]


k32.CreateToolhelp32Snapshot.restype = wt.HANDLE
k32.OpenThread.restype = wt.HANDLE
GetThreadDescription = getattr(k32, "GetThreadDescription", None)


def pids():
    out = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {exe}", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout
    return [int(line.split('","')[1]) for line in out.splitlines() if line.startswith('"')]


def threads(pid):
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    e = THREADENTRY32()
    e.dwSize = ctypes.sizeof(e)
    found = []
    ok = k32.Thread32First(snap, ctypes.byref(e))
    while ok:
        if e.th32OwnerProcessID == pid:
            found.append(e.th32ThreadID)
        ok = k32.Thread32Next(snap, ctypes.byref(e))
    k32.CloseHandle(snap)
    return found


def sample(pid):
    """{thread id: (name, CPU seconds)}"""
    out = {}
    for tid in threads(pid):
        h = k32.OpenThread(THREAD_QUERY_LIMITED_INFORMATION, False, tid)
        if not h:
            continue
        name = ""
        if GetThreadDescription:
            p = ctypes.c_wchar_p()
            if GetThreadDescription(h, ctypes.byref(p)) >= 0 and p.value:
                name = p.value
                k32.LocalFree(p)
        times = [wt.FILETIME() for _ in range(4)]
        if k32.GetThreadTimes(h, *[ctypes.byref(t) for t in times]):
            secs = sum(((t.dwHighDateTime << 32) | t.dwLowDateTime) for t in times[2:]) / 1e7
            out[tid] = (name or f"thread {tid}", secs)
        k32.CloseHandle(h)
    return out


for pid in pids():
    a = sample(pid)
    t0 = time.time()
    time.sleep(seconds)
    b = sample(pid)
    span = time.time() - t0
    rows = []
    for tid, (name, secs) in b.items():
        before = a.get(tid, (name, 0.0))[1]
        rows.append((100 * (secs - before) / span, name))
    rows.sort(reverse=True)
    total = sum(r[0] for r in rows)
    print(f"pid {pid}: {total:.1f} % of one core over {span:.0f} s, {len(rows)} threads")
    by_name = {}
    for pct, name in rows:
        by_name[name] = by_name.get(name, 0.0) + pct
    for name, pct in sorted(by_name.items(), key=lambda kv: -kv[1]):
        if pct >= 0.1:
            print(f"  {pct:5.1f} %  {name}")
