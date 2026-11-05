"""Probes of a running revisor for its live checks: the accessibility events (WinEvents) its
windows raise, by kind, and how long its GUI thread takes to answer while something happens.

    with Probe(pid, main) as probe:
        key(VK_F7)
        ...
    probe.report()

The events come through an out-of-context hook on a thread of this script, as a screen reader
gets them; the answers through WM_NULL sent every few milliseconds. A flood of events, or a GUI
thread that does not answer, is what makes NVDA stall on a window (TASK-13).
"""

import collections
import ctypes
import ctypes.wintypes as wt
import threading
import time

u32 = ctypes.windll.user32

EVENT_MIN, EVENT_MAX = 0x8000, 0x80FF
NAMES = {
    0x8000: "create", 0x8001: "destroy", 0x8002: "show", 0x8003: "hide", 0x8004: "reorder",
    0x8005: "focus", 0x8006: "selection", 0x8007: "selection add", 0x8008: "selection remove",
    0x8009: "selection within", 0x800A: "state change", 0x800B: "location change",
    0x800C: "name change", 0x800D: "description change", 0x800E: "value change",
    0x8019: "live region changed",
}
WINEVENT_OUTOFCONTEXT, PM_REMOVE, WM_NULL, SMTO_ABORTIFHUNG = 0, 1, 0, 2
WinEventProc = ctypes.WINFUNCTYPE(
    None, wt.HANDLE, wt.DWORD, wt.HWND, wt.LONG, wt.LONG, wt.DWORD, wt.DWORD
)


class Probe:
    def __init__(self, pid, main):
        self.pid = pid
        self.main = main
        self.events = collections.Counter()
        self.latencies = []
        self.running = False

    def _hook_thread(self):
        def proc(_hook, event, hwnd, id_object, id_child, _thread, _time):
            self.events[NAMES.get(event, hex(event))] += 1

        self._proc = WinEventProc(proc)
        hook = u32.SetWinEventHook(EVENT_MIN, EVENT_MAX, 0, self._proc, self.pid, 0,
                                   WINEVENT_OUTOFCONTEXT)
        msg = wt.MSG()
        while self.running:
            while u32.PeekMessageW(ctypes.byref(msg), 0, 0, 0, PM_REMOVE):
                u32.TranslateMessage(ctypes.byref(msg))
                u32.DispatchMessageW(ctypes.byref(msg))
            time.sleep(0.002)
        u32.UnhookWinEvent(hook)

    def _ping_thread(self):
        result = ctypes.c_size_t()
        while self.running:
            started = time.time()
            u32.SendMessageTimeoutW(self.main, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 20000,
                                    ctypes.byref(result))
            self.latencies.append((started, time.time() - started))
            time.sleep(0.01)

    def __enter__(self):
        self.running = True
        self.started = time.time()
        self.threads = [threading.Thread(target=self._hook_thread, daemon=True),
                        threading.Thread(target=self._ping_thread, daemon=True)]
        for t in self.threads:
            t.start()
        time.sleep(0.2)
        return self

    def __exit__(self, *_):
        self.running = False
        for t in self.threads:
            t.join(25)

    def report(self, name):
        print(f"\n-- probe: {name}")
        for event, count in self.events.most_common():
            print(f"   {count:8d}  {event}")
        slow = [(t - self.started, d) for t, d in self.latencies if d > 0.1]
        worst = max((d for _, d in self.latencies), default=0)
        print(f"   GUI answers: {len(self.latencies)}, worst {worst:.3f} s, slower than 0.1 s: "
              + ", ".join(f"{d:.2f} s at +{t:.1f} s" for t, d in slow[:10]))
