"""Live check of TASK-38: Ctrl+T from the list back to a filled tree, on another entry, names
the node once, and the tree keeps its folders open.

    python -X utf8 tools\\nvda\\task38_ctrl_t_once.py <uk|en> [parts] [rounds]

It takes neither the keyboard nor the mouse: the menu commands go as WM_COMMAND, the focus of
the list moves by LVM_SETITEMSTATE, and the folders of the tree open by TVM_EXPAND. So it
can run while the author works, and reads NVDA's speech only when revisor happens to be the
foreground window; without that it checks the focus events alone, which are what a screen
reader speaks from.

Part ``small`` (default): a small difference, shown as a list, then as a tree; every folder
of the tree is opened. Then, `rounds` times (2), the view goes to the list and back: on the
same entry, on the next row, on the last row, and on the first again. Each time the tree that
gets the focus must raise one focus event for itself and then one for the node of the entry
(#1, #2), NVDA must say the node once, and the folders must be open as before (#3).

Part ``big``: a folder of 5 000 added files, whose difference arrives in the tree with one
focus event for its first node (#2); four of its pages are opened, and the view comes back
from the list on the last file. One focus event for the node again; how long the window
took to answer is printed (it was 1.08 s before TASK-38).

The last line says ``all right`` or names what is wrong.
"""

import ctypes
import ctypes.wintypes as wt
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import revisor_nvda as rn  # noqa: E402
from probe import Probe  # noqa: E402

u32 = ctypes.windll.user32
k32 = ctypes.windll.kernel32
oleacc = ctypes.windll.oleacc

lang = sys.argv[1] if len(sys.argv) > 1 else "uk"
parts = sys.argv[2].split(",") if len(sys.argv) > 2 else ["small"]
rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 2
source = os.path.join(tempfile.gettempdir(), "revisor-nvda-task38")
BIG = 5000

WORDS = {
    "uk": {
        "snapshot": "Зробити знімок", "compare": "Порівняти з попереднім",
        "list": "Списком", "tree": "Деревом", "details": "Подробиці",
        "modified": " — змінено", "deleted": " — видалено", "added": " — додано",
    },
    "en": {
        "snapshot": "Take snapshot", "compare": "Compare with previous",
        "list": "List", "tree": "Tree", "details": "Details",
        "modified": " — modified", "deleted": " — deleted", "added": " — added",
    },
}[lang]

wrong = []


def check(what, found):
    print(f"   {'OK     ' if found else 'WRONG  '}{what}")
    if not found:
        wrong.append(what)


# --- the focus events of the trees, with the item named through MSAA -------------------------
EVENT_FOCUS = 0x8005
WinEventProc = ctypes.WINFUNCTYPE(None, wt.HANDLE, wt.DWORD, wt.HWND, wt.LONG, wt.LONG,
                                  wt.DWORD, wt.DWORD)


class VARIANT(ctypes.Structure):
    _fields_ = [("vt", ctypes.c_ushort), ("r1", ctypes.c_ushort), ("r2", ctypes.c_ushort),
                ("r3", ctypes.c_ushort), ("value", ctypes.c_longlong),
                ("extra", ctypes.c_longlong)]


oleacc.AccessibleObjectFromEvent.argtypes = [wt.HWND, wt.DWORD, wt.DWORD,
                                             ctypes.POINTER(ctypes.c_void_p),
                                             ctypes.POINTER(VARIANT)]


def acc_name(hwnd, obj, child):
    acc = ctypes.c_void_p()
    var = VARIANT()
    if oleacc.AccessibleObjectFromEvent(hwnd, obj & 0xFFFFFFFF, child & 0xFFFFFFFF,
                                        ctypes.byref(acc), ctypes.byref(var)) != 0 or not acc:
        return "?"
    vtbl = ctypes.cast(ctypes.cast(acc, ctypes.POINTER(ctypes.c_void_p))[0],
                       ctypes.POINTER(ctypes.c_void_p))
    get_name = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, VARIANT,
                                  ctypes.POINTER(ctypes.c_wchar_p))(vtbl[10])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(vtbl[2])
    name = ctypes.c_wchar_p()
    try:
        if get_name(acc, var, ctypes.byref(name)) == 0 and name.value is not None:
            return name.value
        return "<none>"
    finally:
        release(acc)


class Recorder:
    """The focus events of revisor's trees, in order: (window, child, name)."""

    def __init__(self, pid):
        self.pid, self.log, self.running = pid, [], False

    def _run(self):
        def proc(_h, event, hwnd, obj, child, _thread, _time):
            if event == EVENT_FOCUS and hwnd and rn.class_name(hwnd) == "SysTreeView32":
                self.log.append((hwnd, child, acc_name(hwnd, obj, child)))

        self._proc = WinEventProc(proc)
        hook = u32.SetWinEventHook(EVENT_FOCUS, EVENT_FOCUS, 0, self._proc, self.pid, 0, 0)
        msg = wt.MSG()
        while self.running:
            while u32.PeekMessageW(ctypes.byref(msg), 0, 0, 0, 1):
                u32.DispatchMessageW(ctypes.byref(msg))
            time.sleep(0.002)
        u32.UnhookWinEvent(hook)

    def start(self):
        self.running = True
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()
        time.sleep(0.3)

    def take(self):
        out, self.log = self.log, []
        return out

    def stop(self):
        self.running = False
        self.thread.join(5)


# --- driving revisor without the keyboard -----------------------------------------------------
u32.GetMenu.restype = ctypes.c_void_p
u32.GetSubMenu.restype = ctypes.c_void_p
u32.GetSubMenu.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemCount.argtypes = [ctypes.c_void_p]
u32.GetMenuItemID.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemID.restype = wt.UINT
u32.GetMenuStringW.argtypes = [ctypes.c_void_p, wt.UINT, wt.LPWSTR, ctypes.c_int, wt.UINT]
u32.SendMessageW.argtypes = [wt.HWND, wt.UINT, ctypes.c_size_t, ctypes.c_size_t]
u32.SendMessageW.restype = ctypes.c_size_t
k32.OpenProcess.restype = wt.HANDLE
k32.VirtualAllocEx.restype = ctypes.c_void_p
k32.VirtualAllocEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, wt.DWORD]
k32.WriteProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                                   ctypes.c_size_t, ctypes.c_void_p]
k32.VirtualFreeEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD]
WM_COMMAND, WM_CLOSE = 0x0111, 0x0010
LVM_SETITEMSTATE = 0x102B
TVM_EXPAND, TVM_GETNEXTITEM, TVM_SELECTITEM, TVM_GETITEMSTATE = 0x1102, 0x110A, 0x110B, 0x1127
TVM_GETCOUNT = 0x1105
TVGN_NEXT, TVGN_CHILD, TVGN_CARET, TVE_EXPAND, TVIS_EXPANDED = 1, 4, 9, 2, 0x20


def menu_items(hmenu):
    for i in range(u32.GetMenuItemCount(hmenu)):
        buf = ctypes.create_unicode_buffer(256)
        u32.GetMenuStringW(hmenu, i, buf, 256, 0x400)
        sub = u32.GetSubMenu(hmenu, i)
        if sub:
            yield from menu_items(sub)
        else:
            yield buf.value, u32.GetMenuItemID(hmenu, i)


class LVITEM(ctypes.Structure):
    _fields_ = [("mask", wt.UINT), ("iItem", ctypes.c_int), ("iSubItem", ctypes.c_int),
                ("state", wt.UINT), ("stateMask", wt.UINT), ("pszText", ctypes.c_void_p),
                ("cchTextMax", ctypes.c_int), ("iImage", ctypes.c_int),
                ("lParam", ctypes.c_void_p), ("iIndent", ctypes.c_int),
                ("iGroupId", ctypes.c_int), ("cColumns", wt.UINT),
                ("puColumns", ctypes.c_void_p), ("piColFmt", ctypes.c_void_p),
                ("iGroup", ctypes.c_int)]


class Run:
    """A running revisor, driven by messages."""

    def __init__(self, profile_folder, view):
        scratch = os.path.join(source, "app")
        paths = rn.prepare({"Small": ([profile_folder], [])}, scratch=scratch)
        rn.make_settings(scratch, language=lang, difference_view=view)
        self.proc = subprocess.Popen([paths["exe"], "--profile", paths["Small"]], cwd=scratch)
        assert rn.wait_for(rn.find_main, 15), "revisor's main window did not appear"
        self.main = rn.find_main()
        self.thread = u32.GetWindowThreadProcessId(self.main, None)
        time.sleep(1.0)
        # No more than a request: Windows grants it only when nobody else is typing.
        u32.SetForegroundWindow(self.main)
        self.recorder = Recorder(self.proc.pid)
        self.recorder.start()
        self.snaps = rn.snapshot_list(self.main)
        self.speech = None

    def in_front(self):
        return u32.GetForegroundWindow() == self.main

    def focus(self):
        """The window with the focus in revisor's own thread, in front or not."""
        info = rn.GUITHREADINFO()
        info.cbSize = ctypes.sizeof(info)
        u32.GetGUIThreadInfo(self.thread, ctypes.byref(info))
        return info.hwndFocus

    def command(self, word):
        for label, ident in menu_items(u32.GetMenu(self.main)):
            if label.replace("&", "").startswith(WORDS[word]):
                u32.PostMessageW(self.main, WM_COMMAND, ident, 0)
                return
        sys.exit(f"no menu item {WORDS[word]!r}")

    def snapshot(self, wanted):
        self.command("snapshot")
        assert rn.wait_for(lambda: rn.item_count(self.snaps) == wanted, 300), "no snapshot"
        assert rn.wait_for(lambda: rn.progress_text(self.main) is None, 300, 0.02)
        time.sleep(1.0)

    def compare(self):
        self.command("compare")
        rn.wait_for(lambda: rn.progress_text(self.main) is not None, 1, 0.01)
        assert rn.wait_for(lambda: rn.progress_text(self.main) is None, 120, 0.02)

    def listen(self):
        """From now on NVDA's speech is read, while revisor is in front."""
        self.speech = rn.Speech() if self.in_front() else None
        print(f"revisor is the foreground window: {self.in_front()}"
              + ("" if self.speech else " — NVDA's speech is not read, the events are"))

    def step(self, name, wait=2.0):
        """Waits, then prints and gives the trees' focus events and NVDA's lines, or
        `None` for the lines when revisor was not in front."""
        time.sleep(wait)
        said = None
        if self.speech:
            lines = self.speech.new()
            if self.in_front():
                said = lines
            else:
                # Whatever came is of another window: not read from here on.
                self.speech = None
        events = self.recorder.take()
        print(f"\n== {name}")
        for hwnd, child, text in events:
            print(f"    focus  {hwnd:#x} child={child} {text}")
        for line in said or []:
            print("    NVDA  ", line)
        sys.stdout.flush()
        return events, said

    def list_focus(self, row):
        """Moves the focus and the selection of the list with the focus to `row`, as an
        arrow key would; a negative row counts from the end."""
        hwnd = self.focus()
        assert rn.class_name(hwnd) == "SysListView32", rn.class_name(hwnd)
        if row < 0:
            row += rn.item_count(hwnd)
        proc = k32.OpenProcess(0x0008 | 0x0020 | 0x0010, False, self.proc.pid)
        mem = k32.VirtualAllocEx(proc, None, 4096, 0x3000, 0x04)

        def set_state(index, state, mask):
            item = LVITEM(state=state, stateMask=mask)
            k32.WriteProcessMemory(proc, mem, ctypes.byref(item), ctypes.sizeof(item), None)
            u32.SendMessageW(hwnd, LVM_SETITEMSTATE, index & 0xFFFFFFFFFFFFFFFF, mem)

        set_state(-1, 0, 2)
        set_state(row, 3, 3)
        k32.VirtualFreeEx(proc, mem, 0, 0x8000)
        k32.CloseHandle(proc)

    def close(self):
        self.recorder.stop()
        u32.PostMessageW(self.main, WM_CLOSE, 0, 0)
        time.sleep(1.5)
        if self.proc.poll() is None:
            self.proc.terminate()


def shape(hwnd, item=0, top=True, expand=False):
    """The items of a tree as a string: `+` for an open folder, `-` for a closed one with
    its items there, `.` for the rest, the items inside in brackets. With `expand`, every
    item that takes it is opened first, as a click on its button would."""
    out = ""
    item = u32.SendMessageW(hwnd, TVM_GETNEXTITEM, 0 if top else TVGN_CHILD, item)
    while item:
        if expand:
            u32.SendMessageW(hwnd, TVM_EXPAND, TVE_EXPAND, item)
        state = u32.SendMessageW(hwnd, TVM_GETITEMSTATE, item, TVIS_EXPANDED)
        child = u32.SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_CHILD, item)
        out += ("+" if state & TVIS_EXPANDED else "-") if child else "."
        if child:
            out += "[" + shape(hwnd, item, False, expand) + "]"
        item = u32.SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_NEXT, item)
    return out


def last_item(hwnd):
    """The last item of the deepest open folder on the way down the first items."""
    item = u32.SendMessageW(hwnd, TVM_GETNEXTITEM, 0, 0)
    while True:
        child = u32.SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_CHILD, item)
        if not child:
            break
        item = child
    while True:
        following = u32.SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_NEXT, item)
        if not following:
            return item
        item = following


def write(rel, text):
    full = os.path.join(source, "tree", rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


def arrival(run, name, events, said, node):
    """The tree with the focus raised one focus event for itself, then one for `node`, and
    NVDA said the node once."""
    tree = run.focus()
    check(f"{name}: the focus is in a tree", rn.class_name(tree) == "SysTreeView32")
    mine = [(child, text) for hwnd, child, text in events if hwnd == tree]
    on_items = [text for child, text in mine if child != 0]
    check(f"{name}: one focus event for the tree, then one for a node — {len(on_items)} for nodes",
          len(mine) == 2 and mine[0][0] == 0 and len(on_items) == 1)
    check(f"{name}: the event is for «{node}»", any(t.startswith(node) for t in on_items))
    if said is None:
        print("   (NVDA's speech not read: revisor is not the foreground window)")
    else:
        count = " ".join(said).count(node)
        check(f"{name}: NVDA says «{node}» once — said {count} time(s)", count == 1)


def small():
    for rel in ("a.txt", "b.txt", r"docs\one.txt", r"docs\two.txt", r"keep\k.txt"):
        write(rel, "x")
    run = Run(os.path.join(source, "tree"), "list")
    try:
        run.snapshot(1)
        write("a.txt", "aaaa")
        os.remove(os.path.join(source, "tree", "b.txt"))
        write(r"docs\one.txt", "1111")
        write(r"docs\sub\deep.txt", "deep")
        write(r"new\n1.txt", "n1")
        write(r"new\n2.txt", "n2")
        time.sleep(1)
        run.snapshot(2)
        run.listen()
        run.compare()
        run.step("the difference, as a list")
        run.command("tree")
        events, said = run.step("the tree, first time")
        arrival(run, "first time", events, said, "a.txt" + WORDS["modified"])
        print("   folders:", shape(run.focus()))
        opened = shape(run.focus(), expand=True)
        print("   folders, all opened:", opened)
        run.step("every folder opened", 1.0)
        cases = [
            ("the same entry", None, "a.txt" + WORDS["modified"]),
            ("the next row", 1, "b.txt" + WORDS["deleted"]),
            ("the last row", -1, "n2.txt" + WORDS["added"]),
            ("the first row again", 0, "a.txt" + WORDS["modified"]),
        ]
        for i in range(rounds):
            for case, row, node in cases:
                name = f"{case} #{i + 1}"
                run.command("list")
                run.step("to the list", 1.5)
                if row is not None:
                    run.list_focus(row)
                    run.step(f"the list's focus to row {row}", 1.0)
                run.command("tree")
                events, said = run.step(f"back to the tree: {name}", 2.5)
                arrival(run, name, events, said, node)
                now = shape(run.focus())
                check(f"{name}: the folders are open as before — {now}", now == opened)
    finally:
        run.close()


def big():
    os.makedirs(os.path.join(source, "tree", "b"))
    write("keep.txt", "x")
    run = Run(os.path.join(source, "tree"), "tree")
    try:
        run.snapshot(1)
        for i in range(BIG):
            open(os.path.join(source, "tree", "b", f"g{i:06}.txt"), "w").close()
        run.snapshot(2)
        run.listen()
        run.compare()
        events, said = run.step("the difference, as a tree", 2.5)
        arrival(run, "a new difference", events, said, "g000000.txt" + WORDS["added"])
        tree = run.focus()
        for _ in range(3):
            u32.SendMessageW(tree, TVM_SELECTITEM, TVGN_CARET, last_item(tree))
            time.sleep(0.5)
            run.command("details")
            time.sleep(1.5)
        items = u32.SendMessageW(tree, TVM_GETCOUNT, 0, 0)
        check(f"four pages are open — {items} items", items > 4000)
        run.step("four pages opened", 0.5)
        run.command("list")
        run.step("to the list", 2.0)
        run.list_focus(-1)
        run.step("the list's focus to the last row", 1.0)
        with Probe(run.proc.pid, run.main) as probe:
            run.command("tree")
            events, said = run.step("back to the tree: the last file", 3.0)
        slowest = max((d for _, d in probe.latencies), default=0)
        print(f"   the window's slowest answer: {slowest:.3f} s (1.08 s before TASK-38)")
        arrival(run, "the last file", events, said, f"g{BIG - 1:06}.txt" + WORDS["added"])
        items = u32.SendMessageW(run.focus(), TVM_GETCOUNT, 0, 0)
        check(f"the folder shows one page again — {items} items", 1000 <= items < 1010)
    finally:
        run.close()


for part in parts:
    shutil.rmtree(source, ignore_errors=True)
    print(f"\n######## {part}")
    try:
        {"small": small, "big": big}[part]()
    finally:
        shutil.rmtree(source, ignore_errors=True)

print("\n" + ("all right" if not wrong else "WRONG: " + "; ".join(wrong)))
