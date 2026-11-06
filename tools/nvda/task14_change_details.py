"""Live NVDA check of TASK-14: the details of a change and the change in File Explorer.

    python -X utf8 tools\\nvda\\task14_change_details.py [parts]

``parts`` is a comma-separated choice of ``small``, ``restart`` and ``drive`` (all by default);
``restart`` goes on from where ``small`` left the settings, so it runs after it.

«Small» is a scratch folder whose files change between two snapshots in every way the details
tell apart: an exe replaced by another one (sizes, dates and versions, #3, #6, #10), a file
hidden (#4, #8), a file rewritten whose archive attribute was cleared (#5), a file whose last
write moved within one second (#9), a name whose case changed (#4), a junction pointed
elsewhere (#3), a folder closed by rights (#3), files deleted and added. Part ``small``: the
commands refused without a difference (#14), the details opened by Enter, the menu and
Alt+Enter, read line by line and copied, and closed back onto the same change (#1, #2); the
setting «Атрибути» (#7); File Explorer on an entry on disk (#11), on a deleted one and on one
whose folder is gone, with the setting «Попереджати…» on (#12, #13). Part ``restart``: the
settings kept (#7, #13). Part ``drive``: a profile on a ``subst`` drive that is then removed:
nothing opens, and the status bar says why (#12).

Explorer windows the check opens are found and closed through Shell.Application, only those
under the scratch folder.
"""

import ctypes
import ctypes.wintypes as wt
import os
import shutil
import sqlite3
import string
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    ES_MULTILINE, ES_READONLY, SCRATCH, VK_DOWN, VK_ESCAPE, VK_F5, VK_HOME, VK_MENU,
    VK_RETURN, VK_TAB, App, children, class_name, control_text, edit_style,
    focused_window, foreground_title, k32, key, prepare, type_text, u32, visible_controls, vk,
    wait_for,
)

VK_SHIFT, VK_CONTROL, VK_F6, VK_F7, VK_F10 = 0x10, 0x11, 0x75, 0x76, 0x79
LVM_GETITEMCOUNT, LVM_GETNEXTITEM = 0x1004, 0x100C
LVNI_FOCUSED = 0x0001
WM_CLOSE = 0x0010
CF_UNICODETEXT = 13

parts = set(sys.argv[1].split(",")) if len(sys.argv) > 1 else {"small", "restart", "drive"}
tree = os.path.join(SCRATCH, "tree")
targets = os.path.join(SCRATCH, "targets")
user = os.environ.get("USERNAME", "")

u32.GetClipboardData.restype = ctypes.c_void_p
k32.GlobalLock.restype = ctypes.c_void_p
k32.GlobalLock.argtypes = [ctypes.c_void_p]
k32.GlobalUnlock.argtypes = [ctypes.c_void_p]


def path(rel):
    return os.path.join(tree, rel)


def write(rel, text):
    os.makedirs(os.path.dirname(path(rel)), exist_ok=True)
    with open(path(rel), "w", encoding="utf-8") as f:
        f.write(text)


def run(*cmd):
    subprocess.run(list(cmd), check=True, capture_output=True)


def lists(main):
    return visible_controls(main, "SysListView32")


def difference(main):
    shown = lists(main)
    return shown[-1] if len(shown) > 1 else None


def rows(h):
    return u32.SendMessageW(h, LVM_GETITEMCOUNT, 0, 0) if h else None


def focused_row(h):
    return u32.SendMessageW(h, LVM_GETNEXTITEM, -1, LVNI_FOCUSED) if h else None


def filter_field(main):
    for h in visible_controls(main, "Edit"):
        if not edit_style(h) & (ES_MULTILINE | ES_READONLY):
            return h
    return None


def where(main):
    h = focused_window()
    shown = lists(main)
    if shown and h == shown[0]:
        return "snapshot list"
    if h in shown[1:]:
        return "difference list"
    if h == filter_field(main):
        return "filter field"
    return class_name(h) if h else "nothing"


def report(app, note=""):
    d = difference(app.main)
    print(f"   focus: {where(app.main)} | difference rows: {rows(d)} (focused {focused_row(d)})"
          f" | title: {foreground_title()} {note}")


def finished(app, timeout=60):
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


def menu(*letters):
    key(VK_MENU, vk(letters[0]))
    time.sleep(0.6)
    for letter in letters[1:]:
        key(vk(letter))
        time.sleep(0.5)


def to_difference_list(app):
    for _ in range(3):
        if where(app.main) == "difference list":
            return
        key(VK_F6)
        time.sleep(0.6)
    assert where(app.main) == "difference list", where(app.main)


def only(app, text):
    """Filters the difference list by `text` and puts the focus on its first row."""
    key(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    type_text(text)
    time.sleep(1.5)
    key(VK_SHIFT, VK_TAB)
    time.sleep(0.8)
    assert where(app.main) == "difference list", where(app.main)


def clear_filter(app):
    key(VK_CONTROL, vk("F"))
    time.sleep(0.4)
    key(VK_ESCAPE)
    time.sleep(0.6)


def details_dialog():
    """The open «Подробиці» window and its text field, or (None, None)."""
    h = u32.GetForegroundWindow()
    if not foreground_title().startswith("Подробиці: "):
        return None, None
    for c in children(h):
        if class_name(c) == "Edit":
            return h, c
    return h, None


def clipboard_text():
    if not u32.OpenClipboard(0):
        return None
    try:
        h = u32.GetClipboardData(CF_UNICODETEXT)
        if not h:
            return None
        p = k32.GlobalLock(h)
        try:
            return ctypes.wstring_at(p)
        finally:
            k32.GlobalUnlock(h)
    finally:
        u32.CloseClipboard()


def read_details(app, how, lines=3, copy=False):
    """The details window opened by `how`: its text, a few lines read with Down, a copy, Esc."""
    d = difference(app.main)
    row = focused_row(d)
    app.step(how, 2.5)
    assert wait_for(lambda: details_dialog()[1] is not None, 5), "no details window"
    _, field = details_dialog()
    print(f"   window: {foreground_title()}")
    for line in control_text(field).splitlines():
        print(f"   | {line}")
    for i in range(lines):
        key(VK_DOWN)
        app.step(f"Down {i + 1}", 1.2)
    if copy:
        key(VK_CONTROL, vk("A"))
        time.sleep(0.4)
        key(VK_CONTROL, vk("C"))
        time.sleep(0.6)
        copied = clipboard_text()
        print(f"   copied {len(copied or '')} characters, the same as the field: "
              f"{copied is not None and copied.replace(chr(13), '') == control_text(field).replace(chr(13), '')}")
        app.step("Ctrl+A, Ctrl+C", 1)
    key(VK_ESCAPE)
    app.step("Esc", 2)
    assert wait_for(lambda: details_dialog()[0] is None, 5), "the window did not close"
    print(f"   back on the same change: {focused_row(difference(app.main)) == row} "
          f"(focus: {where(app.main)})")


def explorer_windows():
    """File Explorer windows on the scratch folder: (hwnd, folder, selected paths)."""
    script = (
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8; "
        f"$scratch = '{SCRATCH}'; "
        "(New-Object -ComObject Shell.Application).Windows() | ForEach-Object { try { "
        "$folder = $_.Document.Folder.Self.Path; "
        "if ($folder -like \"$scratch*\") { "
        "$sel = ($_.Document.SelectedItems() | ForEach-Object { $_.Path }) -join ';'; "
        "Write-Output (\"{0}|{1}|{2}\" -f $_.HWND, $folder, $sel) } } catch {} }"
    )
    out = subprocess.run(["powershell", "-NoProfile", "-Command", script],
                         capture_output=True, encoding="utf-8").stdout
    found = []
    for line in out.splitlines():
        hwnd, folder, selected = (line.split("|") + ["", ""])[:3]
        if hwnd.strip():
            found.append((int(hwnd), folder, [s for s in selected.split(";") if s]))
    return found


def close_explorers(app):
    for hwnd, _, _ in explorer_windows():
        u32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
    time.sleep(1.5)
    u32.SetForegroundWindow(app.main)
    time.sleep(1)


def show_in_explorer(app, name, answer=None):
    """Ctrl+Enter on the change in focus; `answer` presses a key of the question, if any."""
    key(VK_CONTROL, VK_RETURN)
    if answer is not None:
        assert wait_for(lambda: foreground_title() == "Показати в Провіднику", 5), "no question"
        app.step(f"Ctrl+Enter on {name}: the question", 1.5)
        key(*answer)
    opened = wait_for(lambda: explorer_windows(), 8, 0.5)
    app.step(f"Ctrl+Enter on {name}", 2.5)
    for _, folder, selected in explorer_windows():
        print(f"   File Explorer: {folder} | selected: {selected}")
    if not opened:
        print("   File Explorer: nothing opened")
    close_explorers(app)


def fill_tree():
    """The files of «Small» before its first snapshot; `prepare` has just emptied the folder."""
    write("a.txt", "a")
    write("b.txt", "b")
    write("hidden.txt", "h")
    write("readme.txt", "r")
    write("precise.txt", "p")
    write(r"old\gone.txt", "gone")
    write(r"locked\inside.txt", "inside")
    shutil.copy(os.path.join(SCRATCH, "revisor.exe"), path("app.exe"))
    os.makedirs(os.path.join(targets, "one"), exist_ok=True)
    os.makedirs(os.path.join(targets, "two"), exist_ok=True)
    run("cmd", "/c", "mklink", "/J", path("link"), os.path.join(targets, "one"))
    # 2026-11-06 12:00:00.1 UTC as the last write of precise.txt.
    second = 1_793_793_600
    os.utime(path("precise.txt"), ns=(second * 10**9, second * 10**9 + 100_000_000))


def change_tree():
    """What changes between the two snapshots of «Small»."""
    shutil.copy(os.path.join(os.environ["SystemRoot"], "System32", "whoami.exe"), path("app.exe"))
    run("attrib", "+h", path("hidden.txt"))
    write("b.txt", "bbbb")
    run("attrib", "-a", path("b.txt"))
    second = 1_793_793_600
    os.utime(path("precise.txt"), ns=(second * 10**9, second * 10**9 + 900_000_000))
    os.rename(path("readme.txt"), path("README.txt"))
    os.rmdir(path("link"))
    run("cmd", "/c", "mklink", "/J", path("link"), os.path.join(targets, "two"))
    run("icacls", path("locked"), "/deny", f"{user}:(RD)")
    os.remove(path("a.txt"))
    shutil.rmtree(path("old"))
    write("new.txt", "new")


def unlock():
    """Opens the folder closed by rights again and takes the junction away, so the scratch
    folder can be removed."""
    if os.path.exists(path("locked")):
        subprocess.run(["icacls", path("locked"), "/remove:d", user], capture_output=True)
    if os.path.lexists(path("link")):
        os.rmdir(path("link"))


def check_small(app):
    app.step("start: Small, without snapshots", 2.5)
    # #14: the commands without a difference say why.
    key(VK_MENU, VK_RETURN)
    app.step("Alt+Enter without a difference", 1.5)
    key(VK_CONTROL, VK_RETURN)
    app.step("Ctrl+Enter without a difference", 1.5)

    key(VK_F5)
    finished(app)
    app.step("F5: the first snapshot", 1.5)
    change_tree()
    key(VK_F5)
    finished(app)
    app.step("F5: the second snapshot", 2)
    app.dismiss_restart_question()
    time.sleep(1)
    key(VK_F7)
    finished(app)
    app.step("F7: the difference", 2.5)
    report(app)

    # #14: the menu «Різниця» and the context menu of the list.
    menu("D")
    for i in range(2):
        key(VK_DOWN)
        app.step(f"menu Різниця, Down {i + 1}", 1)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    time.sleep(0.5)
    to_difference_list(app)
    key(VK_SHIFT, VK_F10)
    app.step("Shift+F10: the context menu", 1.5)
    key(VK_DOWN)
    app.step("Down", 1)
    key(VK_ESCAPE)
    time.sleep(0.8)

    # #1–#3, #6, #10: the exe, by Enter in the list, read and copied.
    only(app, "app.exe")
    report(app)
    key(VK_RETURN)
    read_details(app, "Enter on app.exe", lines=9, copy=True)

    # #4, #8: the file hidden, by the menu command.
    clear_filter(app)
    only(app, "hidden")
    menu("D", "D")
    read_details(app, "menu Різниця, Подробиці on hidden.txt", lines=0)

    # #5: rewritten, its archive attribute cleared: not counted.
    clear_filter(app)
    only(app, "b.txt")
    key(VK_MENU, VK_RETURN)
    read_details(app, "Alt+Enter on b.txt", lines=0)

    # #9: the last write moved within one second.
    clear_filter(app)
    only(app, "precise")
    key(VK_RETURN)
    read_details(app, "Enter on precise.txt", lines=0)

    # #4: the case of a name.
    clear_filter(app)
    only(app, "readme")
    key(VK_RETURN)
    read_details(app, "Enter on README.txt", lines=0)

    # #3: a link pointed elsewhere, and a folder closed by rights.
    clear_filter(app)
    only(app, "link")
    key(VK_RETURN)
    read_details(app, "Enter on link", lines=0)
    clear_filter(app)
    only(app, "locked")
    key(VK_RETURN)
    read_details(app, "Enter on locked", lines=0)

    # Alt+Enter from the field «Фільтр:» acts on the change in focus.
    key(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    key(VK_MENU, VK_RETURN)
    read_details(app, "Alt+Enter from the field «Фільтр:»", lines=0)
    # A filter that hides every row: nothing to act on (#14).
    key(VK_CONTROL, vk("F"))
    time.sleep(0.4)
    type_text("zzz")
    time.sleep(1.5)
    key(VK_MENU, VK_RETURN)
    app.step("Alt+Enter with every row hidden", 1.5)
    key(VK_CONTROL, VK_RETURN)
    app.step("Ctrl+Enter with every row hidden", 1.5)
    clear_filter(app)

    # #7: «Атрибути» → «Літерами», and #13: «Попереджати…» on.
    menu("F", "S")
    assert wait_for(lambda: foreground_title() == "Налаштування", 5), "no settings"
    time.sleep(0.5)
    key(VK_MENU, vk("A"))
    app.step("Alt+A in the settings", 1.5)
    key(VK_DOWN)
    app.step("Down: Літерами", 1.5)
    key(VK_MENU, vk("W"))
    app.step("Alt+W: Попереджати…", 1.5)
    key(VK_RETURN)
    time.sleep(1)
    to_difference_list(app)
    only(app, "hidden")
    key(VK_RETURN)
    read_details(app, "Enter on hidden.txt, attributes as letters", lines=0)

    # #11: an entry on disk; #12, #13: a deleted one, with the question; one whose folder is
    # gone, the question declined.
    clear_filter(app)
    only(app, "new.txt")
    show_in_explorer(app, "new.txt")
    clear_filter(app)
    only(app, "link")
    show_in_explorer(app, "link (the junction, not its target)")
    clear_filter(app)
    only(app, "a.txt")
    show_in_explorer(app, "a.txt (deleted)", answer=(VK_RETURN,))
    clear_filter(app)
    only(app, "gone.txt")
    show_in_explorer(app, "old\\gone.txt (its folder gone), declined", answer=(VK_ESCAPE,))
    clear_filter(app)
    only(app, "gone.txt")
    show_in_explorer(app, "old\\gone.txt (its folder gone)", answer=(VK_MENU, vk("O")))
    clear_filter(app)


def check_restart(app):
    app.step("restarted", 2.5)
    db = os.path.join(SCRATCH, "data", "settings.db")
    conn = sqlite3.connect(db)
    kept = dict(conn.execute(
        "SELECT key, value FROM setting WHERE key IN ('attribute_format', 'warn_missing_entry')"
    ))
    conn.close()
    print(f"   settings.db: {kept}")
    menu("F", "S")
    assert wait_for(lambda: foreground_title() == "Налаштування", 5), "no settings"
    time.sleep(0.5)
    for i in range(7):
        key(VK_TAB)
        app.step(f"settings, Tab {i + 1}", 1.2)
    key(VK_ESCAPE)
    time.sleep(1)


def free_letter():
    drives = k32.GetLogicalDrives()
    for i, letter in enumerate(string.ascii_uppercase):
        if letter >= "M" and not drives & (1 << i):
            return letter
    raise SystemExit("no free drive letter")


def check_drive(app, letter):
    app.step("start: Drive", 2.5)
    key(VK_F5)
    finished(app)
    app.step("F5: the first snapshot", 1.5)
    with open(f"{letter}:\\data\\new.txt", "w", encoding="utf-8") as f:
        f.write("new")
    key(VK_F5)
    finished(app)
    key(VK_F7)
    finished(app)
    app.step("F7: the difference", 2.5)
    run("subst", f"{letter}:", "/d")
    to_difference_list(app)
    key(VK_HOME)
    time.sleep(0.5)
    key(VK_CONTROL, VK_RETURN)
    app.step("Ctrl+Enter with the drive removed", 2)
    for _, folder, selected in explorer_windows():
        print(f"   File Explorer: {folder} | selected: {selected}")
    report(app)


try:
    unlock()
    if "small" in parts or "restart" in parts:
        paths = prepare({"Small": ([tree], [])})
        fill_tree()
        if "small" in parts:
            with App(paths, paths["Small"]) as app:
                check_small(app)
        if "restart" in parts:
            with App(paths, paths["Small"]) as app:
                check_restart(app)
        unlock()
    if "drive" in parts:
        unlock()
        letter = free_letter()
        base = os.path.join(SCRATCH + "-drive")
        shutil.rmtree(base, ignore_errors=True)
        os.makedirs(os.path.join(base, "data"))
        run("subst", f"{letter}:", base)
        try:
            paths = prepare({"Drive": ([f"{letter}:\\data"], [])})
            with App(paths, paths["Drive"]) as app:
                check_drive(app, letter)
        finally:
            subprocess.run(["subst", f"{letter}:", "/d"], capture_output=True)
            shutil.rmtree(base, ignore_errors=True)
finally:
    unlock()
