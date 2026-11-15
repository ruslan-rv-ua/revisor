"""Live NVDA check of TASK-50: several changes of the difference list can be selected and their
paths copied.

    python -X utf8 tools\\nvda\\task50_copy_several_paths.py [exe]

The exe goes first (the debug build by default). The interface is Ukrainian, the Ukrainian
layout is on. One profile, «Small», over a scratch folder of five files: F5 takes a snapshot,
the script deletes a.txt, changes b.txt and c.txt and adds f.txt and g.txt, and Ctrl+F5 shows
the difference, a.txt b.txt c.txt f.txt g.txt in the order of paths, the focus on a.txt. The
check puts paths of the scratch on the clipboard; it reads the clipboard only once a copy of
revisor changed it.

1. Shift+Down twice: a, b and c are selected; NVDA reads each row (#1, #2).
2. Ctrl+Down: the focus goes to f, which stays unselected, and NVDA says «не виділено»;
   Ctrl+Space selects it, and NVDA says «виділено» (#1, #2).
3. Ctrl+Shift+C: the four paths, a line each, CRLF, no quotes, none after the last; the
   status bar says «Скопійовано шляхів: 4.» (#3, #4).
4. «За спаданням» from the menu (WM_COMMAND): the same four stay selected, the focus on f;
   copied again, they come in the new order (#3, #6). «За зростанням» puts it back.
5. Enter opens the details of f, Esc closes them: the selection is as it was (#5).
6. Ctrl+A: all five selected; Ctrl+Shift+C copies five paths (#1, #3).
7. Home, then Ctrl+Down twice and Ctrl+Space: a and c selected, the focus on c. Settings,
   «Різниця», Alt+C, Down, Space (hides «Зміна»), Enter: the rebuilt list still selects a
   and c, the focus on c (#6, the author's decision).
8. Ctrl+F and a letter of the filter: only the change in focus stays selected (#6).
9. Ctrl+T: in the tree Ctrl+A does nothing, and Ctrl+Shift+C copies one path; Ctrl+T back:
   one change selected (#6, #7).
10. One change selected: Ctrl+Shift+C as before TASK-50, «Шлях скопійовано.» and one path;
    Ctrl+Space deselects it, and Ctrl+Shift+C copies the change in focus (#3).

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import ctypes
import ctypes.wintypes as wt
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    REPO, SCRATCH, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_F5, VK_F6, VK_HOME, VK_MENU, VK_RETURN,
    VK_SHIFT, VK_SPACE, VK_TAB, App, children, class_name, control_text, focused_class,
    focused_window, foreground_title, key, make_settings, prepare, type_text, u32,
    ukrainian_layout, vk, wait_for,
)

EXE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "target", "debug", "revisor.exe")
TINY = os.path.join(SCRATCH, "tiny")
SETTINGS = "Налаштування"
COPIED_ONE = "Шлях скопійовано."
SELECTED, UNSELECTED = "виділено", "не виділено"
missing = []

CF_UNICODETEXT = 13
WM_COMMAND = 0x0111
LVM_GETNEXTITEM, LVM_GETSELECTEDCOUNT = 0x100C, 0x1032
LVNI_FOCUSED, LVNI_SELECTED = 0x1, 0x2

k32 = ctypes.windll.kernel32
u32.SendMessageW.restype = ctypes.c_ssize_t
u32.PostMessageW.argtypes = [wt.HWND, wt.UINT, ctypes.c_size_t, ctypes.c_size_t]
u32.OpenClipboard.argtypes = [wt.HWND]
u32.GetClipboardData.restype = ctypes.c_void_p
u32.GetClipboardData.argtypes = [wt.UINT]
u32.GetClipboardSequenceNumber.restype = wt.DWORD
k32.GlobalLock.restype = ctypes.c_void_p
k32.GlobalLock.argtypes = [ctypes.c_void_p]
k32.GlobalUnlock.argtypes = [ctypes.c_void_p]
u32.GetMenu.restype = ctypes.c_void_p
u32.GetMenu.argtypes = [wt.HWND]
u32.GetSubMenu.restype = ctypes.c_void_p
u32.GetSubMenu.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemCount.argtypes = [ctypes.c_void_p]
u32.GetMenuItemID.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemID.restype = wt.UINT
u32.GetMenuStringW.argtypes = [ctypes.c_void_p, wt.UINT, wt.LPWSTR, ctypes.c_int, wt.UINT]


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)
    return ok


def joined(lines):
    return " ".join(lines)


def in_revisor():
    title = foreground_title()
    if not title.endswith("revisor"):
        sys.exit(f"the focus left revisor for «{title}»: stopped; missing so far: {missing}")


def press(*vks):
    """Presses keys in revisor's main window only."""
    in_revisor()
    key(*vks)


def write(name, text):
    with open(os.path.join(TINY, name), "w", encoding="utf-8") as f:
        f.write(text)


def path(name):
    return os.path.join(TINY, name)


# --- the difference list ------------------------------------------------------------------------
def difference_list(app):
    """The difference list shown: the enabled list with a size after the snapshot list."""
    lists = [c for c in children(app.main) if class_name(c) == "SysListView32"
             and u32.IsWindowEnabled(c) and u32.IsWindowVisible(c)]
    return lists[-1] if lists else None


def selected(hwnd):
    rows, row = [], -1
    while True:
        row = u32.SendMessageW(hwnd, LVM_GETNEXTITEM, ctypes.c_ssize_t(row), LVNI_SELECTED)
        if row < 0:
            return rows
        rows.append(row)


def focused_row(hwnd):
    return u32.SendMessageW(hwnd, LVM_GETNEXTITEM, ctypes.c_ssize_t(-1), LVNI_FOCUSED)


oleacc = ctypes.windll.oleacc
OBJID_CLIENT, VT_I4 = -4, 3


class VARIANT(ctypes.Structure):
    _fields_ = [("vt", ctypes.c_ushort), ("r1", ctypes.c_ushort), ("r2", ctypes.c_ushort),
                ("r3", ctypes.c_ushort), ("value", ctypes.c_longlong), ("extra", ctypes.c_longlong)]


class GUID(ctypes.Structure):
    _fields_ = [("a", ctypes.c_ulong), ("b", ctypes.c_ushort), ("c", ctypes.c_ushort),
                ("d", ctypes.c_ubyte * 8)]


IID_IACCESSIBLE = GUID(0x618736E0, 0x3C3D, 0x11CF,
                       (ctypes.c_ubyte * 8)(0x81, 0x0C, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71))
ctypes.windll.oleaut32.SysFreeString.argtypes = [ctypes.c_void_p]


def row_names(hwnd, rows):
    """The names of the rows `rows` of the list `hwnd`, through MSAA: the first column's text."""
    acc = ctypes.c_void_p()
    if oleacc.AccessibleObjectFromWindow(hwnd, OBJID_CLIENT, ctypes.byref(IID_IACCESSIBLE),
                                         ctypes.byref(acc)) != 0 or not acc:
        return []
    vtbl = ctypes.cast(ctypes.cast(acc, ctypes.POINTER(ctypes.c_void_p))[0],
                       ctypes.POINTER(ctypes.c_void_p))
    get_name = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, VARIANT,
                                  ctypes.POINTER(ctypes.c_void_p))(vtbl[10])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(vtbl[2])
    names = []
    try:
        for row in rows:
            child = VARIANT()
            child.vt, child.value = VT_I4, row + 1
            name = ctypes.c_void_p()
            if get_name(acc, child, ctypes.byref(name)) != 0 or not name:
                names.append("")
                continue
            names.append(ctypes.wstring_at(name))
            ctypes.windll.oleaut32.SysFreeString(name)
    finally:
        release(acc)
    return names


def selection(app):
    """The names of the rows selected in the difference list shown, top to bottom, and the
    name of the row with the focus."""
    hwnd = difference_list(app)
    rows = selected(hwnd)
    focus = focused_row(hwnd)
    return row_names(hwnd, rows), (row_names(hwnd, [focus]) or [""])[0] if focus >= 0 else ""


def expect_selection(app, names, focus, what):
    got, got_focus = selection(app)
    print(f"   selected {got}, focus on {got_focus!r}")
    check(got == names, f"{what}: {names} selected")
    check(got_focus == focus, f"{what}: the focus on {focus}")


# --- the clipboard and the status bar ----------------------------------------------------------
def sequence():
    return u32.GetClipboardSequenceNumber()


def clipboard_text():
    for _ in range(40):
        if u32.OpenClipboard(None):
            break
        time.sleep(0.05)
    else:
        return None
    try:
        handle = u32.GetClipboardData(CF_UNICODETEXT)
        if not handle:
            return None
        data = k32.GlobalLock(handle)
        try:
            return ctypes.wstring_at(data)
        finally:
            k32.GlobalUnlock(handle)
    finally:
        u32.CloseClipboard()


def status_text(main):
    for h in children(main):
        if class_name(h) == "msctls_statusbar32":
            return control_text(h)
    return None


def copy(app, name, paths, said):
    """Ctrl+Shift+C: the clipboard must hold `paths`, a line each, and NVDA and the status bar
    must say `said`."""
    before = sequence()
    press(VK_CONTROL, VK_SHIFT, vk("C"))
    lines = app.step(name, 1.5)
    changed = wait_for(lambda: sequence() != before, 2)
    text = clipboard_text() if changed else None
    want = "\r\n".join(paths)
    print(f"   clipboard: {text!r}")
    check(text == want, f"{name}: the clipboard holds {want!r}")
    check(text is not None and '"' not in text and not text.endswith("\n"),
          f"{name}: no quotes, no break after the last")
    check(said in joined(lines), f"{name}: NVDA said {said!r}")
    check(status_text(app.main) == said, f"{name}: the status bar says {said!r}")


def menu_command(app, label):
    def items(hmenu):
        for i in range(u32.GetMenuItemCount(hmenu)):
            buf = ctypes.create_unicode_buffer(256)
            u32.GetMenuStringW(hmenu, i, buf, 256, 0x400)
            sub = u32.GetSubMenu(hmenu, i)
            if sub:
                yield from items(sub)
            else:
                yield buf.value.replace("&", "").split("\t")[0], u32.GetMenuItemID(hmenu, i)
    for text, ident in items(u32.GetMenu(app.main)):
        if text.startswith(label):
            u32.PostMessageW(app.main, WM_COMMAND, ident, 0)
            return True
    return False


# --- the steps ----------------------------------------------------------------------------------
def run(app):
    key(VK_F5)
    app.step("F5: a snapshot", 4)
    os.remove(path("a.txt"))
    write("b.txt", "b" * 5000)
    write("c.txt", "c" * 7000)
    write("f.txt", "f")
    write("g.txt", "g")
    time.sleep(1)
    press(VK_CONTROL, VK_F5)
    app.step("Ctrl+F5: the difference", 4)
    if focused_window() != difference_list(app):
        press(VK_F6)
        app.step("F6: the difference list", 1.5)
    check(focused_window() == difference_list(app), "the focus is in the difference list")
    expect_selection(app, ["a.txt"], "a.txt", "a new difference")

    print("\n## 1. Shift+Down")
    press(VK_SHIFT, VK_DOWN)
    app.step("Shift+Down", 1.2)
    press(VK_SHIFT, VK_DOWN)
    said = joined(app.step("Shift+Down", 1.2))
    check("c.txt" in said, "NVDA read c.txt")
    expect_selection(app, ["a.txt", "b.txt", "c.txt"], "c.txt", "Shift+Down twice")

    print("\n## 2. Ctrl+Down, Ctrl+Space")
    press(VK_CONTROL, VK_DOWN)
    said = joined(app.step("Ctrl+Down", 1.5))
    check("f.txt" in said and UNSELECTED in said, "NVDA read f.txt «не виділено»")
    expect_selection(app, ["a.txt", "b.txt", "c.txt"], "f.txt", "Ctrl+Down")
    press(VK_CONTROL, VK_SPACE)
    said = joined(app.step("Ctrl+Space", 1.5))
    check(SELECTED in said.replace(UNSELECTED, ""), "NVDA said «виділено»")
    expect_selection(app, ["a.txt", "b.txt", "c.txt", "f.txt"], "f.txt", "Ctrl+Space")

    print("\n## 3. Ctrl+Shift+C")
    four = [path(n) for n in ("a.txt", "b.txt", "c.txt", "f.txt")]
    copy(app, "Ctrl+Shift+C: four", four, "Скопійовано шляхів: 4.")
    expect_selection(app, ["a.txt", "b.txt", "c.txt", "f.txt"], "f.txt", "after the copy")

    print("\n## 4. sorting")
    check(menu_command(app, "За спаданням"), "«За спаданням» in the menu")
    app.step("За спаданням", 2)
    expect_selection(app, ["f.txt", "c.txt", "b.txt", "a.txt"], "f.txt", "sorted descending")
    copy(app, "Ctrl+Shift+C: in the new order", four[::-1], "Скопійовано шляхів: 4.")
    check(menu_command(app, "За зростанням"), "«За зростанням» in the menu")
    app.step("За зростанням", 2)
    expect_selection(app, ["a.txt", "b.txt", "c.txt", "f.txt"], "f.txt", "sorted back")

    print("\n## 5. details")
    press(VK_RETURN)
    up = wait_for(lambda: not foreground_title().endswith("revisor"), 5, 0.05)
    app.step("Enter: the details", 1.5)
    check(up and "f.txt" in foreground_title(), f"the details of f.txt: {foreground_title()!r}")
    if up:
        key(VK_ESCAPE)
        wait_for(lambda: foreground_title().endswith("revisor"), 5, 0.05)
        app.step("Esc: back", 1.5)
    expect_selection(app, ["a.txt", "b.txt", "c.txt", "f.txt"], "f.txt", "after the details")

    print("\n## 6. Ctrl+A")
    press(VK_CONTROL, vk("A"))
    app.step("Ctrl+A", 1.5)
    all_five = ["a.txt", "b.txt", "c.txt", "f.txt", "g.txt"]
    expect_selection(app, all_five, "f.txt", "Ctrl+A")
    copy(app, "Ctrl+Shift+C: five", [path(n) for n in all_five], "Скопійовано шляхів: 5.")

    print("\n## 7. new columns at OK")
    press(VK_HOME)
    app.step("Home", 1.2)
    press(VK_CONTROL, VK_DOWN)
    app.step("Ctrl+Down", 1.2)
    press(VK_CONTROL, VK_DOWN)
    app.step("Ctrl+Down", 1.2)
    press(VK_CONTROL, VK_SPACE)
    app.step("Ctrl+Space", 1.2)
    expect_selection(app, ["a.txt", "c.txt"], "c.txt", "before the settings")
    old = difference_list(app)
    press(VK_MENU, vk("F"))
    time.sleep(0.4)
    key(vk("S"))
    if check(wait_for(lambda: foreground_title() == SETTINGS, 5, 0.05), "settings open"):
        app.step("Alt+F, S: settings", 1)
        key(VK_CONTROL, VK_TAB)
        app.step("Ctrl+Tab", 1)
        key(VK_CONTROL, VK_TAB)
        app.step("Ctrl+Tab: «Різниця»", 1.2)
        key(VK_MENU, vk("C"))
        app.step("Alt+C: the list of columns", 1.5)
        key(VK_DOWN)
        app.step("Down: «Зміна»", 1.2)
        key(VK_SPACE)
        app.step("Space: «Зміна» hidden", 1.2)
        key(VK_RETURN)
        wait_for(lambda: foreground_title().endswith("revisor"), 5, 0.05)
        app.step("Enter: OK", 2.5)
        check(difference_list(app) != old, "the list was rebuilt")
        check(focused_window() == difference_list(app), "the focus is in the new list")
        expect_selection(app, ["a.txt", "c.txt"], "c.txt", "after OK")

    print("\n## 8. the filter")
    press(VK_CONTROL, vk("F"))
    app.step("Ctrl+F: the filter", 1.2)
    # Typed as text: in the Ukrainian layout the key T is «е».
    type_text("t")
    app.step("t: the filter «t»", 2)
    expect_selection(app, ["c.txt"], "c.txt", "a new filter")
    key(VK_ESCAPE)
    app.step("Esc: the filter cleared", 2)
    press(VK_F6)
    app.step("F6", 1)
    press(VK_F6)
    app.step("F6: the difference list", 1.5)
    check(focused_window() == difference_list(app), "the focus is back in the list")

    print("\n## 9. the tree")
    press(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the tree", 2.5)
    check(focused_class() == "SysTreeView32", "the focus is in the tree")
    press(VK_CONTROL, vk("A"))
    app.step("Ctrl+A in the tree", 1.2)
    check(focused_class() == "SysTreeView32", "Ctrl+A left the tree as it was")
    copy(app, "Ctrl+Shift+C in the tree", [path("c.txt")], COPIED_ONE)
    press(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the list", 2.5)
    expect_selection(app, ["c.txt"], "c.txt", "back from the tree")

    print("\n## 10. one change, then none")
    copy(app, "Ctrl+Shift+C: one", [path("c.txt")], COPIED_ONE)
    press(VK_CONTROL, VK_SPACE)
    app.step("Ctrl+Space: nothing selected", 1.5)
    expect_selection(app, [], "c.txt", "Ctrl+Space")
    copy(app, "Ctrl+Shift+C: none selected", [path("c.txt")], COPIED_ONE)


def main():
    paths = prepare({"Small": ([TINY], [])}, exe=EXE)
    os.makedirs(TINY)
    for name in ("a.txt", "b.txt", "c.txt", "d.txt", "e.txt"):
        write(name, name)
    make_settings(SCRATCH, language="uk")
    with App(paths, paths["Small"]) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start: Small", 1.5)
        run(app)
        u32.SetForegroundWindow(app.main)
        time.sleep(0.5)
        app.speech.new()


main()
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
