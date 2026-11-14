"""Live NVDA check of TASK-49: the columns of the difference list can be hidden and reordered.

    python -X utf8 tools\\nvda\\task49_difference_columns.py [exe]

The exe goes first (the debug build by default). The interface is Ukrainian, the Ukrainian
layout is on. One profile, «Small», over a scratch folder of three files: F5 takes a
snapshot, the script deletes a.txt, grows b.txt and adds d.txt, and Ctrl+F5 shows the
difference with the focus on its first row.

1. The row is read with all six columns, the name first.
2. Settings, Ctrl+Tab twice to «Різниця», Alt+C: the list of columns, on «Ім'я (завжди
   видно)», checked. Space leaves it checked.
3. Down to «Зміна», Alt+U: the focus stays in the list, on «Зміна», now first.
4. Down to «Дата зміни» and «Тека», Space on each: unchecked. Alt+N on «Тека», the last:
   nothing moves.
5. Enter (OK): the difference list shows «Зміна» first, without the date and the folder, the
   same row in focus; ``difference_columns`` is stored.
6. Sorting by the hidden «Тека» from the menu (WM_COMMAND) works; the focus stays on its row.
7. A double click on the divider after the first column (the real mouse, revisor in front)
   fits it to its text: the new width is stored at once.
8. A restart: the list, empty, has the same columns, the first as wide as stored.
9. Settings again, Alt+C, Alt+D (Типові), Enter: all six back, in their order and widths.

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import ctypes
import ctypes.wintypes as wt
import os
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    REPO, SCRATCH, VK_CONTROL, VK_DOWN, VK_F5, VK_MENU, VK_RETURN, VK_SPACE, VK_TAB,
    App, children, class_name, focused_class, focused_window, foreground_title, key,
    make_settings, prepare, u32, ukrainian_layout, vk, wait_for,
)

EXE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "target", "debug", "revisor.exe")
TINY = os.path.join(SCRATCH, "tiny")
SETTINGS = "Налаштування"
DEFAULT = "name:220,change:140,what_changed:200,size:90,date_modified:160,folder:420"
CHOSEN = "change:140,name:220,what_changed:200,size:90,-date_modified:160,-folder:420"
missing = []
results = {}

k32 = ctypes.windll.kernel32
k32.OpenProcess.restype = wt.HANDLE
k32.VirtualAllocEx.restype = ctypes.c_void_p
k32.VirtualAllocEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, wt.DWORD]
k32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                  ctypes.c_void_p]
k32.VirtualFreeEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD]
u32.SendMessageW.argtypes = [wt.HWND, wt.UINT, ctypes.c_size_t, ctypes.c_size_t]
u32.SendMessageW.restype = ctypes.c_ssize_t
u32.PostMessageW.argtypes = [wt.HWND, wt.UINT, ctypes.c_size_t, ctypes.c_size_t]
u32.GetMenu.restype = ctypes.c_void_p
u32.GetSubMenu.restype = ctypes.c_void_p
u32.GetSubMenu.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemCount.argtypes = [ctypes.c_void_p]
u32.GetMenuItemID.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemID.restype = wt.UINT
u32.GetMenuStringW.argtypes = [ctypes.c_void_p, wt.UINT, wt.LPWSTR, ctypes.c_int, wt.UINT]
WM_COMMAND = 0x0111
LVM_GETHEADER, LVM_GETCOLUMNWIDTH = 0x101F, 0x101D
HDM_GETITEMCOUNT, HDM_GETITEMRECT = 0x1200, 0x1207


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)
    return ok


def verdict(step, *oks):
    results[step] = all(oks)


def setting(name):
    conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
    row = conn.execute("SELECT value FROM setting WHERE key = ?", (name,)).fetchone()
    conn.close()
    return row[0] if row else None


def write(name, text):
    with open(os.path.join(TINY, name), "w", encoding="utf-8") as f:
        f.write(text)


def joined(lines):
    return " ".join(lines)


def app_lines(said):
    """The utterances of `said`, joined by `joined`, one per line NVDA spoke."""
    return said.split("CancellableSpeech")


def difference_list(app):
    """The difference list shown: the enabled list with a size after the snapshot list."""
    lists = [c for c in children(app.main) if class_name(c) == "SysListView32"
             and u32.IsWindowEnabled(c) and u32.IsWindowVisible(c)]
    return lists[-1] if lists else None


def columns_of(hwnd):
    header = u32.SendMessageW(hwnd, LVM_GETHEADER, 0, 0)
    count = u32.SendMessageW(header, HDM_GETITEMCOUNT, 0, 0)
    return header, count, [u32.SendMessageW(hwnd, LVM_GETCOLUMNWIDTH, i, 0) for i in range(count)]


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


def divider_point(app, hwnd, column):
    """The screen point of the divider after `column` in the header of the list `hwnd`."""
    header = u32.SendMessageW(hwnd, LVM_GETHEADER, 0, 0)
    pid = wt.DWORD()
    u32.GetWindowThreadProcessId(app.main, ctypes.byref(pid))
    proc = k32.OpenProcess(0x0008 | 0x0010 | 0x0020, False, pid.value)
    mem = k32.VirtualAllocEx(proc, None, 4096, 0x3000, 0x04)
    u32.SendMessageW(header, HDM_GETITEMRECT, column, mem)
    rect = wt.RECT()
    k32.ReadProcessMemory(proc, mem, ctypes.byref(rect), ctypes.sizeof(rect), None)
    k32.VirtualFreeEx(proc, mem, 0, 0x8000)
    k32.CloseHandle(proc)
    point = wt.POINT(rect.right, (rect.top + rect.bottom) // 2)
    u32.ClientToScreen(header, ctypes.byref(point))
    return point


def double_click(point):
    u32.SetCursorPos(point.x, point.y)
    time.sleep(0.2)
    for _ in range(2):
        u32.mouse_event(0x0002, 0, 0, 0, 0)
        u32.mouse_event(0x0004, 0, 0, 0, 0)
        time.sleep(0.05)


def open_columns(app):
    key(VK_MENU, vk("F"))
    time.sleep(0.4)
    key(vk("S"))
    up = check(wait_for(lambda: foreground_title() == SETTINGS, 5, 0.05), "settings open")
    app.step("Alt+F, S: settings", 1)
    key(VK_CONTROL, VK_TAB)
    app.step("Ctrl+Tab", 1)
    key(VK_CONTROL, VK_TAB)
    app.step("Ctrl+Tab: «Різниця»", 1.2)
    key(VK_MENU, vk("C"))
    return up, joined(app.step("Alt+C: the list of columns", 1.5))


def start(paths):
    return App(paths, paths["Small"])


def main():
    paths = prepare({"Small": ([TINY], [])}, exe=EXE)
    os.makedirs(TINY)
    for name in ("a.txt", "b.txt", "c.txt"):
        write(name, name)
    make_settings(SCRATCH, language="uk")
    with start(paths) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start: Small", 1.5)
        first_run(app)
    print("\n#### restart")
    with start(paths) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        second_run(app)


def first_run(app):
    # Step 1.
    key(VK_F5)
    app.step("F5: a snapshot", 4)
    os.remove(os.path.join(TINY, "a.txt"))
    write("b.txt", "b" * 5000)
    write("d.txt", "d")
    key(VK_CONTROL, VK_F5)
    said = joined(app.step("Ctrl+F5: the difference", 5))
    oks = [check(focused_class() == "SysListView32", "the focus in the difference list")]
    hwnd = difference_list(app)
    _, count, _ = columns_of(hwnd)
    oks.append(check(count == 6, f"six columns ({count})"))
    # NVDA reads a row's cells after the first with their headers, and skips empty ones.
    oks.append(check("a.txt; Зміна: Видалено" in said and "Тека: " in said,
                     "the row read with the name first, the folder too"))
    verdict(1, *oks)

    # Step 2.
    up, said = open_columns(app)
    oks = [up, check("Колонки списку різниці" in said, "the list is named"),
           check("Ім'я (завжди видно)" in said, "on «Ім'я (завжди видно)»"),
           check("не позначено" not in said, "it is checked")]
    key(VK_SPACE)
    said = joined(app.step("Space on «Ім'я»", 1.2))
    oks.append(check("не позначено" not in said, "Space leaves it checked"))
    verdict(2, *oks)

    # Step 3.
    key(VK_DOWN)
    said = joined(app.step("Down: «Зміна»", 1.2))
    oks = [check("Зміна" in said, "on «Зміна»")]
    key(VK_MENU, vk("U"))
    said = joined(app.step("Alt+U: moved up", 1.5))
    oks.append(check("Зміна" in said, "NVDA reads «Зміна» in its new place"))
    oks.append(check(not said.startswith("[\"Ім'я"), "not the text the row had before"))
    oks.append(check(focused_class() == "SysListView32", "the focus stays in the list"))
    verdict(3, *oks)

    # Step 4.
    for _ in range(4):
        key(VK_DOWN)
        said = joined(app.step("Down", 1))
    oks = [check("Дата зміни" in said, "on «Дата зміни»")]
    key(VK_SPACE)
    said = joined(app.step("Space: unchecked", 1.2))
    oks.append(check("не позначено" in said, "«Дата зміни» unchecked"))
    key(VK_DOWN)
    app.step("Down: «Тека»", 1)
    key(VK_SPACE)
    said = joined(app.step("Space: unchecked", 1.2))
    oks.append(check("не позначено" in said, "«Тека» unchecked"))
    key(VK_MENU, vk("N"))
    said = joined(app.step("Alt+N on the last: nothing moves", 1.2))
    oks.append(check(focused_class() == "SysListView32", "the focus stays in the list"))
    verdict(4, *oks)

    # Step 5.
    key(VK_RETURN)
    said = joined(app.step("Enter: OK", 3))
    oks = [check(wait_for(lambda: u32.GetForegroundWindow() == app.main, 5, 0.05),
                 "the settings closed")]
    oks.append(check(setting("difference_columns") == CHOSEN,
                     f"stored: {setting('difference_columns')}"))
    hwnd = difference_list(app)
    _, count, widths = columns_of(hwnd)
    oks.append(check(count == 4, f"four columns ({count}, widths {widths})"))
    oks.append(check(focused_class() == "SysListView32" and focused_window() == hwnd,
                     "the focus in the new list"))
    rows = [line for line in app_lines(said) if "a.txt" in line]
    oks.append(check(len(rows) == 1, f"the row read once ({len(rows)})"))
    oks.append(check("Видалено; Ім'я: a.txt" in said, "the row read with the change first"))
    oks.append(check("Тека: " not in said and "Дата зміни: " not in said,
                     "no folder and no date in the row"))
    verdict(5, *oks)

    # Step 6.
    key(VK_DOWN)
    app.step("Down: the next row", 1.2)
    sorted_ = menu_command(app, "Тека")
    said = joined(app.step("Sort by «Тека» (hidden)", 2))
    oks = [check(sorted_, "the menu item «Тека»"),
           check(setting("difference_sort") == "folder", "sorted by the folder"),
           check(focused_class() == "SysListView32", "the focus stays in the list")]
    verdict(6, *oks)

    # Step 7.
    before = columns_of(hwnd)[2][0]
    point = divider_point(app, hwnd, 0)
    in_front = u32.GetForegroundWindow() == app.main
    if in_front:
        double_click(point)
    app.step("double click on the divider", 1.5)
    after = columns_of(hwnd)[2][0]
    stored = setting("difference_columns") or ""
    oks = [check(in_front, "revisor in front for the mouse"),
           check(after != before, f"the first column fitted: {before} → {after}"),
           check(stored.startswith(f"change:{after},"), f"stored at once: {stored}")]
    verdict(7, *oks)
    results["width"] = after


def second_run(app):
    # Step 8.
    app.step("start again", 1.5)
    hwnd = difference_list(app)
    _, count, widths = columns_of(hwnd)
    oks = [check(count == 4, f"four columns after the restart ({count})"),
           check(widths[0] == results.get("width"), f"the first as wide as stored: {widths}")]
    verdict(8, *oks)

    # Step 9.
    up, said = open_columns(app)
    key(VK_MENU, vk("D"))
    said = joined(app.step("Alt+D: defaults", 1.2))
    oks = [up, check(focused_class() == "SysListView32", "the focus stays in the list")]
    key(VK_RETURN)
    said = joined(app.step("Enter: OK", 3))
    oks.append(check(setting("difference_columns") == DEFAULT,
                     f"the defaults stored: {setting('difference_columns')}"))
    hwnd = difference_list(app)
    _, count, widths = columns_of(hwnd)
    oks.append(check(count == 6 and widths == [220, 140, 200, 90, 160, 420],
                     f"six columns in their widths: {widths}"))
    verdict(9, *oks)


main()
print("\nsteps:", ", ".join(f"{n} {'PASS' if ok else 'FAIL'}" for n, ok in sorted(
    (k, v) for k, v in results.items() if isinstance(k, int))))
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
