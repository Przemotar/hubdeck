"""Hot-plug aware runtime over the driver.

Modules are magnetic and lose contact when the deck is moved, so the hub:
- keeps one reader thread per connected module,
- treats a read error as a disconnect and closes the handle,
- re-enumerates every second and re-initializes modules that come back,
- calls `on_connect(module)` so the app can redraw keys or LEDs,
- turns raw reports into edge events (key_down/key_up, fader changes only),
- hands keys and dial back to hardware mode on close().

    with Hub(on_connect=redraw) as hub:
        for ev in hub.events():
            ...
"""
from __future__ import annotations

import logging
import queue
import threading
import time
from typing import Callable, Iterator

import hid

from .driver import _CLASSES, VENDOR_ID, Event, Module

log = logging.getLogger("masterhub.hub")


class Hub:
    def __init__(self, on_connect: Callable[[Module], None] | None = None, rescan_s: float = 1.0):
        self.on_connect = on_connect
        self.rescan_s = rescan_s
        self.modules: dict[str, Module] = {}  # by serial number
        self._queue: queue.Queue[Event] = queue.Queue()
        self._stop = threading.Event()
        self._lock = threading.Lock()
        self._scanner: threading.Thread | None = None
        self._readers: list[threading.Thread] = []

    # -- lifecycle -----------------------------------------------------

    def start(self) -> "Hub":
        self._scan()
        self._scanner = threading.Thread(target=self._scan_loop, name="hub-scan", daemon=True)
        self._scanner.start()
        return self

    def close(self) -> None:
        self._stop.set()
        if self._scanner:
            self._scanner.join(timeout=2)
        # readers poll with a 200 ms timeout; closing a handle mid-read can crash hidapi
        for t in self._readers:
            t.join(timeout=1)
        with self._lock:
            modules = list(self.modules.values())
            self.modules.clear()
        for m in modules:
            self._release(m)

    def __enter__(self) -> "Hub":
        return self.start()

    def __exit__(self, *exc) -> None:
        self.close()

    # -- access ----------------------------------------------------------

    def get(self, name: str) -> Module | None:
        """First connected module of a kind: 'keys', 'faders', 'base', ..."""
        with self._lock:
            return next((m for m in self.modules.values() if m.name == name), None)

    def events(self, timeout: float | None = None) -> Iterator[Event]:
        """Yield events until close(); with a timeout, stop after that many idle seconds."""
        while not self._stop.is_set():
            try:
                yield self._queue.get(timeout=timeout if timeout is not None else 0.5)
            except queue.Empty:
                if timeout is not None:
                    return

    # -- internals -------------------------------------------------------

    def _scan_loop(self) -> None:
        while not self._stop.wait(self.rescan_s):
            self._scan()

    def _scan(self) -> None:
        for info in hid.enumerate(VENDOR_ID):
            serial = info.get("serial_number") or info["path"].decode(errors="replace")
            with self._lock:
                if serial in self.modules:
                    continue
            cls = _CLASSES.get(info["product_id"], Module)
            try:
                m = cls(info)
                m.initialize()
            except Exception as e:  # module still settling after reconnect; next scan retries
                log.warning("init %s %s failed: %s", cls.__name__, serial, e)
                try:
                    m.close()
                except Exception:
                    pass
                continue
            with self._lock:
                self.modules[serial] = m
            reader = threading.Thread(target=self._read_loop, args=(m, serial), name=f"read-{m.name}", daemon=True)
            self._readers = [t for t in self._readers if t.is_alive()] + [reader]
            reader.start()
            self._queue.put(Event(m.name, "connected", raw=[]))
            log.info("connected %s %s", m.name, serial)
            if self.on_connect:
                try:
                    self.on_connect(m)
                except Exception:
                    log.exception("on_connect for %s failed", m.name)

    def _read_loop(self, m: Module, serial: str) -> None:
        keys_prev = [0] * 15
        knob_prev = [0] * 3
        faders_prev: list[int] | None = None
        while not self._stop.is_set():
            try:
                evs = m.read_events(200)
            except (OSError, ValueError) as e:
                log.info("disconnected %s %s (%s)", m.name, serial, e)
                break
            for ev in evs:
                if ev.kind == "keys":
                    for i, (old, new) in enumerate(zip(keys_prev, ev.raw)):
                        if old != new:
                            self._queue.put(Event(m.name, "key_down" if new else "key_up", i, 1 if new else 0))
                    keys_prev = list(ev.raw)
                elif ev.kind == "knob_press":
                    if knob_prev[ev.index] != ev.value:
                        knob_prev[ev.index] = ev.value
                        self._queue.put(Event(m.name, "knob_down" if ev.value else "knob_up", ev.index, ev.value))
                elif ev.kind == "fader":
                    # one report carries all five; emit only the ones that moved
                    if faders_prev is None:
                        faders_prev = [-1] * 5
                    if faders_prev[ev.index] != ev.value:
                        faders_prev[ev.index] = ev.value
                        self._queue.put(ev)
                elif ev.kind != "heartbeat":
                    self._queue.put(ev)
        if self._stop.is_set():
            return  # close() releases and closes the handle
        with self._lock:
            if self.modules.get(serial) is m:
                del self.modules[serial]
        try:
            m.close()
        except Exception:
            pass
        self._queue.put(Event(m.name, "disconnected", raw=[]))

    @staticmethod
    def _release(m: Module) -> None:
        try:
            m.release()
        except Exception:
            pass
        try:
            m.close()
        except Exception:
            pass
