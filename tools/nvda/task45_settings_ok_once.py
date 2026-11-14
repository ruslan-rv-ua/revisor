"""Live check of TASK-45: after OK in «Налаштування» NVDA names the row or the node of the
difference in focus once, and the formats chosen there show at once.

    python -X utf8 tools\\nvda\\task45_settings_ok_once.py <uk|en> [parts] [exe]

It takes neither the keyboard nor the mouse: the menu commands go as WM_COMMAND, the focus of
the list moves by LVM_SETITEMSTATE, a list of the settings changes by CB_SETCURSEL, and the
buttons of the settings are pressed by WM_COMMAND/BN_CLICKED. So it can run while the author
works; NVDA's speech is read only while revisor is the foreground window, and the
accessibility events (WinEvents), which a screen reader speaks from, always are. The third
argument runs another exe, such as a build from before the fix (by default the release build).

A small difference: a.txt deleted, b.txt of one byte grown to 5 000, d.txt added; the focus on
the row of b.txt.

Parts go comma-separated as the second argument, by default all three.

Part ``list``: Cancel, then OK with nothing changed, then OK with «Розмір» set to bytes. Each
time the list must not be built anew (no destroy event of its client object), the row must get
one focus event, and NVDA must say the row once, as after Cancel (#1); after the new format the
row and the summary give the size in bytes (#3).

Part ``tree``: the difference as a tree, the focus on the node of b.txt. Cancel, then OK with
nothing changed, then OK with «Вузли дерева» set to the short text. The tree's items must
raise no state change once the focus is back, and NVDA must say the node once and never
«розгорнуто» (#2); after the new text the node has no «що змінилося» (#3). The folder of the
node stays open (#4).

Part ``types``: in the tree, the focus on b.txt, «Видалені» off and on again from the menu,
then the text «b» in the field «Фільтр:» and the field emptied (WM_SETTEXT). The folder of
b.txt stays open and the focus on its node; what NVDA says is printed, to compare with a
build from before the fix (#4).

The last line says ``all right`` or names what is wrong.
"""

import ctypes
import ctypes.wintypes as wt
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import revisor_nvda as rn  # noqa: E402

u32 = ctypes.windll.user32
k32 = ctypes.windll.kernel32
oleacc = ctypes.windll.oleacc

lang = sys.argv[1] if len(sys.argv) > 1 else "uk"
parts = sys.argv[2].split(",") if len(sys.argv) > 2 else ["list", "tree", "types"]
exe = sys.argv[3] if len(sys.argv) > 3 else rn.EXE_SRC
source = os.path.join(tempfile.gettempdir(), "revisor-nvda-task45")

WORDS = {
    "uk": {
        "snapshot": "Зробити знімок", "compare": "Порівняти з попереднім",
        "settings": "Налаштування", "tree": "Деревом", "ok": "OK", "cancel": "Скасувати",
        "size": "Розмір", "nodes": "Вузли дерева", "saved": "Налаштування збережено.",
        "kb": "КБ", "bytes": "байт", "what": "розмір", "deleted": "Видалені",
        "filter": "Фільтр",
    },
    "en": {
        "snapshot": "Take snapshot", "compare": "Compare with previous",
        "settings": "Settings", "tree": "Tree", "ok": "OK", "cancel": "Cancel",
        "size": "Size", "nodes": "Tree nodes", "saved": "Settings saved.",
        "kb": "KB", "bytes": "bytes", "what": "size", "deleted": "Deleted",
        "filter": "Filter",
    },
}[lang]
# NVDA says states in its own language, Ukrainian, whatever revisor's.
EXPANDED = "розгорнуто"

wrong = []


def check(what, found):
    print(f"   {'OK     ' if found else 'WRONG  '}{what}")
    if not found:
        wrong.append(what)


# --- the accessibility events of revisor, with the object named through MSAA -------------------
EVENT_FOCUS, EVENT_DESTROY, EVENT_STATE = 0x8005, 0x8001, 0x800A
OBJID_CLIENT = -4
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
    """The focus, state change and destroy events of revisor's lists and trees, in order:
    (event, window, object, child, name)."""

    KINDS = {EVENT_FOCUS: "focus", EVENT_DESTROY: "destroy", EVENT_STATE: "state"}

    def __init__(self, pid):
        self.pid, self.log, self.running = pid, [], False
        self.lock = threading.Lock()

    def _run(self):
        def proc(_h, event, hwnd, obj, child, _thread, _time):
            if event not in self.KINDS or not hwnd:
                return
            if rn.class_name(hwnd) not in ("SysListView32", "SysTreeView32"):
                return
            name = acc_name(hwnd, obj, child) if event == EVENT_FOCUS and child else ""
            with self.lock:
                self.log.append((self.KINDS[event], hwnd, obj, child, name))

        self._proc = WinEventProc(proc)
        hook = u32.SetWinEventHook(EVENT_DESTROY, EVENT_STATE, 0, self._proc, self.pid, 0, 0)
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
        with self.lock:
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
u32.PostMessageW.argtypes = [wt.HWND, wt.UINT, ctypes.c_size_t, ctypes.c_size_t]
k32.OpenProcess.restype = wt.HANDLE
k32.VirtualAllocEx.restype = ctypes.c_void_p
k32.VirtualAllocEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, wt.DWORD]
k32.WriteProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                                   ctypes.c_size_t, ctypes.c_void_p]
k32.VirtualFreeEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD]
WM_COMMAND, WM_CLOSE, WM_SETTEXT = 0x0111, 0x0010, 0x000C
LVM_SETITEMSTATE, CB_SETCURSEL, CB_GETCURSEL = 0x102B, 0x014E, 0x0147
TVM_GETNEXTITEM, TVM_GETITEMSTATE, TVGN_ROOT, TVGN_CARET, TVIS_EXPANDED = (
    0x110A, 0x1127, 0, 9, 0x20)
TVM_MAPHTREEITEMTOACCID = 0x112B


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


def write(rel, text):
    full = os.path.join(source, "watched", rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


class Run:
    """A running revisor, driven by messages."""

    def __init__(self):
        shutil.rmtree(source, ignore_errors=True)
        write("a.txt", "a")
        write("b.txt", "b")
        write("c.txt", "c")
        scratch = os.path.join(source, "app")
        paths = rn.prepare({"Small": ([os.path.join(source, "watched")], [])},
                           scratch=scratch, exe=exe)
        self.settings_db = rn.make_settings(scratch, language=lang)
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
        """Whether a window of revisor, the main one or the settings, is in front."""
        front = u32.GetForegroundWindow()
        return bool(front) and u32.GetWindowThreadProcessId(front, None) == self.thread

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
        assert rn.wait_for(lambda: rn.item_count(self.snaps) == wanted, 60), "no snapshot"
        assert rn.wait_for(lambda: rn.progress_text(self.main) is None, 60, 0.02)
        time.sleep(1.0)

    def compare(self):
        self.command("compare")
        rn.wait_for(lambda: rn.progress_text(self.main) is not None, 1, 0.01)
        assert rn.wait_for(lambda: rn.progress_text(self.main) is None, 60, 0.02)
        time.sleep(1.0)

    def list_focus(self, row):
        """Moves the focus and the selection of the list with the focus to `row`, as an
        arrow key would."""
        hwnd = self.focus()
        assert rn.class_name(hwnd) == "SysListView32", rn.class_name(hwnd)
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

    def dialog(self):
        found = []
        u32.EnumThreadWindows(self.thread, rn.EnumWindowsProc(
            lambda h, _: found.append(h) or True), 0)
        for h in found:
            if u32.IsWindowVisible(h) and rn.window_text(h) == WORDS["settings"]:
                return h
        return None

    def settings(self, name, button="ok", choose=None, wait=3.0):
        """Opens the settings, picks the option `index` of the list named `label` when
        `choose` is `(label, index)`, presses `button` and gives the events and NVDA's
        lines from the moment the button is pressed."""
        self.command("settings")
        assert rn.wait_for(lambda: self.dialog() is not None, 5, 0.02), "no settings"
        dialog = self.dialog()
        self.step(f"{name}: the settings", 2.0)
        if choose:
            label, index = choose
            boxes = [c for c in rn.children(dialog) if rn.class_name(c) == "ComboBox"
                     and acc_name(c, OBJID_CLIENT, 0).startswith(label)]
            assert boxes, f"no list {label!r}"
            u32.SendMessageW(boxes[0], CB_SETCURSEL, index, 0)
            check(f"«{label}» shows the option {index}",
                  u32.SendMessageW(boxes[0], CB_GETCURSEL, 0, 0) == index)
        for c in rn.children(dialog):
            if rn.class_name(c) == "Button" and rn.window_text(c).replace("&", "") == WORDS[button]:
                u32.PostMessageW(dialog, WM_COMMAND, u32.GetDlgCtrlID(c) & 0xFFFF, c)
                break
        else:
            sys.exit(f"no button {WORDS[button]!r}")
        return self.step(f"{name}: {WORDS[button]}", wait)

    def listen(self):
        """From now on NVDA's speech is read, while revisor is in front."""
        self.speech = rn.Speech() if self.in_front() else None
        print(f"revisor is the foreground window: {self.in_front()}"
              + ("" if self.speech else " — NVDA's speech is not read, the events are"))

    def step(self, name, wait=2.0):
        """Waits, then prints and gives the events and NVDA's lines, or `None` for the
        lines when revisor was not in front."""
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
        for kind, hwnd, obj, child, text in events:
            print(f"    {kind:<8}{hwnd:#x} obj={obj} child={child} {text}")
        for line in said or []:
            print("    NVDA  ", line)
        sys.stdout.flush()
        return events, said

    def stored(self, key):
        conn = sqlite3.connect(self.settings_db)
        try:
            row = conn.execute("SELECT value FROM setting WHERE key = ?", (key,)).fetchone()
        finally:
            conn.close()
        return row[0] if row else None

    def close(self):
        self.recorder.stop()
        u32.PostMessageW(self.main, WM_CLOSE, 0, 0)
        time.sleep(1.5)
        if self.proc.poll() is None:
            self.proc.terminate()


def row_of(hwnd, name):
    """The row of the list `hwnd` whose first column is `name`."""
    for row in range(rn.item_count(hwnd)):
        if acc_name(hwnd, OBJID_CLIENT, row + 1) == name:
            return row
    sys.exit(f"no row {name!r}")


def lines_with(said, start):
    return [line for line in said or [] if line.startswith(f"['{start}")]


def once_after_ok(events, said, view, start, saved=True):
    """The checks of OK, or Cancel with `saved` false: one focus event on the item, nothing
    that rebuilds the list or changes a state of the tree, the item said once."""
    focused = [e for e in events if e[0] == "focus" and e[3] > 0]
    check(f"one focus event on an item of the {view}: {[e[4] for e in focused]}",
          len(focused) == 1 and focused[0][4].startswith("b.txt"))
    rebuilt = [e for e in events if e[0] == "destroy" and e[2] == OBJID_CLIENT and e[3] == 0]
    check(f"the {view} is not built anew ({len(rebuilt)} destroy events)", not rebuilt)
    states = [e for e in events if e[0] == "state" and e[3] > 0]
    check(f"no state change of an item ({len(states)})", not states)
    if said is None:
        return None
    items = lines_with(said, start)
    check(f"NVDA says the {view}'s item once ({len(items)} times)", len(items) == 1)
    check(f"NVDA does not say «{EXPANDED}»", not any(EXPANDED in line for line in said))
    if saved:
        check("NVDA says the settings are saved",
              any(WORDS["saved"] in line for line in said))
    return items[0] if items else None


def the_list(run):
    print("\n#### the list")
    list_hwnd = run.focus()
    run.list_focus(row_of(list_hwnd, "b.txt"))
    run.step("the focus on b.txt", 1.5)

    events, said = run.settings("Cancel", "cancel")
    once_after_ok(events, said, "list", "b.txt;", saved=False)

    events, said = run.settings("OK, nothing changed")
    before = once_after_ok(events, said, "list", "b.txt;")
    if before is not None:
        check(f"the size is in {WORDS['kb']} first", WORDS["kb"] in before)

    events, said = run.settings("OK, «Розмір» in bytes", choose=(WORDS["size"], 2))
    after = once_after_ok(events, said, "list", "b.txt;")
    check(f"the setting is saved: {run.stored('size_format')!r}",
          run.stored("size_format") is not None)
    if after is not None:
        check("the row gives the size in bytes at once",
              WORDS["bytes"] in after and WORDS["kb"] not in after)
        summary = [line for line in said if "b.txt" not in line and WORDS["bytes"] in line]
        check("the summary gives the size in bytes at once", bool(summary))
    check("the focus is still in the list", run.focus() == list_hwnd)


def root_open(tree):
    """Whether the first item of the tree, the profile's folder above b.txt, is open."""
    root = u32.SendMessageW(tree, TVM_GETNEXTITEM, TVGN_ROOT, 0)
    return bool(u32.SendMessageW(tree, TVM_GETITEMSTATE, root, TVIS_EXPANDED) & TVIS_EXPANDED)


def selected_node(tree):
    """The name of the item selected in the tree, through MSAA."""
    item = u32.SendMessageW(tree, TVM_GETNEXTITEM, TVGN_CARET, 0)
    child = u32.SendMessageW(tree, TVM_MAPHTREEITEMTOACCID, item, 0) if item else 0
    return acc_name(tree, OBJID_CLIENT, child) if child else None


def to_the_tree(run):
    """The difference as a tree, the focus in it; gives the tree."""
    if rn.class_name(run.focus()) != "SysTreeView32":
        run.command("tree")
        run.step("«Деревом»: the tree", 2.5)
    tree = run.focus()
    check("the focus is in the tree", rn.class_name(tree) == "SysTreeView32")
    return tree


def the_tree(run):
    print("\n#### the tree")
    tree = to_the_tree(run)
    check("the folder of b.txt is open", root_open(tree))

    events, said = run.settings("Cancel", "cancel")
    once_after_ok(events, said, "tree", "b.txt —", saved=False)

    events, said = run.settings("OK, nothing changed")
    before = once_after_ok(events, said, "tree", "b.txt —")
    if before is not None:
        check("the node says what changed first", WORDS["what"] in before)

    events, said = run.settings("OK, «Вузли дерева» short", choose=(WORDS["nodes"], 1))
    after = once_after_ok(events, said, "tree", "b.txt —")
    if after is not None:
        check("the node has the short text at once", WORDS["what"] not in after)
    check("the folder of b.txt is still open", root_open(tree))
    check("the focus is still in the tree", run.focus() == tree)


def types_and_filter(run):
    print("\n#### types and the filter in the tree")
    tree = to_the_tree(run)
    node = selected_node(tree)
    print(f"   the node in focus: «{node}»")
    check("the focus is on b.txt", (node or "").startswith("b.txt"))
    fields = [c for c in rn.children(run.main) if rn.class_name(c) == "Edit"
              and acc_name(c, OBJID_CLIENT, 0).startswith(WORDS["filter"])]
    check("the field «Фільтр:» is there", bool(fields))

    def type_filter(text):
        buf = ctypes.create_unicode_buffer(text)
        u32.SendMessageW(fields[0], WM_SETTEXT, 0, ctypes.addressof(buf))

    acts = [(f"«{WORDS['deleted']}» off", lambda: run.command("deleted")),
            (f"«{WORDS['deleted']}» on", lambda: run.command("deleted"))]
    if fields:
        acts += [("the filter «b»", lambda: type_filter("b")),
                 ("the filter emptied", lambda: type_filter(""))]
    for name, act in acts:
        act()
        _, said = run.step(name, 2.5)
        check(f"{name}: the folder of b.txt is still open", root_open(tree))
        check(f"{name}: the focus is still on b.txt",
              run.focus() == tree and selected_node(tree) == node)
        if said is not None:
            check(f"{name}: NVDA does not say «{EXPANDED}»",
                  not any(EXPANDED in line for line in said))


def main():
    run = Run()
    try:
        run.snapshot(1)
        os.remove(os.path.join(source, "watched", "a.txt"))
        write("b.txt", "b" * 5000)
        write("d.txt", "d")
        time.sleep(1)
        run.snapshot(2)
        run.compare()
        check("the focus is in the difference list", rn.class_name(run.focus()) == "SysListView32")
        run.listen()
        run.step("the difference", 1.0)
        if "list" in parts:
            the_list(run)
        if "tree" in parts:
            the_tree(run)
        if "types" in parts:
            types_and_filter(run)
    finally:
        # Stop reading speech before closing: the focus goes back to whatever was behind
        # revisor, and NVDA reads it into the log.
        print("\n(stop reading speech) closing")
        run.speech = None
        run.close()


main()
print("\nall right" if not wrong else f"\nwrong: {wrong}")
sys.exit(1 if wrong else 0)
