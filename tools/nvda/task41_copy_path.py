"""Live NVDA check of TASK-41: the path of the entry in focus can be copied.

    python -X utf8 tools\\nvda\\task41_copy_path.py uk|en

The language goes into the settings file of the scratch folder; the Ukrainian layout is on.
The profile «Програми» watches a small folder of the scratch, which the script changes
between snapshots. The check puts paths of the scratch on the clipboard, so what was there
before is gone afterwards. The script never reads the clipboard before a copy of its own: it
watches the clipboard's sequence number and reads the text only once it changed.

1. The menu «Різниця» holds «Копіювати шлях (O)» with Ctrl+Shift+C right after «Показати в
   Провіднику», unavailable while there is no difference (#1); Ctrl+Shift+C then says why and
   leaves the clipboard alone (#4).
2. F5, changes — a file deleted, one modified, one renamed, one added in a folder —, Ctrl+F5:
   on every row of the list Ctrl+Shift+C puts the row's whole path on the clipboard, with no
   quotes; the status bar says «Шлях скопійовано.» and the focus stays (#2, #3).
3. Alt+D, O copies too, and so does «Копіювати шлях (O)» of the context menu, which holds
   «Подробиці», «Показати в Провіднику» and «Копіювати шлях» in that order (#1).
4. Settings, tab «Різниця»: Alt+H ticks «Показувати скопійований шлях у рядку стану», Enter
   saves it; the status bar then names the path (#8).
5. Ctrl+T: in the tree, the folder of the profile and a folder that is no change itself copy
   their own paths (#2).
6. A new folder of 1,050 files, Ctrl+F5: on its node «Ще…» Ctrl+Shift+C says why it cannot
   copy, and the clipboard stays as it was (#4).
7. revisor closes: the path copied last is still on the clipboard, as it was flushed.

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
    SCRATCH, VK_APPS, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE, VK_F5, VK_F6, VK_HOME, VK_MENU,
    VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB, App, children, class_name, control_text,
    focused_class, focused_window, foreground_title, item_count, key, make_settings, prepare,
    snapshot_list, u32, ukrainian_layout, vk, wait_for,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"
WATCHED = os.path.join(SCRATCH, "watched")
BIG_FILES = 1050

CF_UNICODETEXT = 13
MN_GETHMENU = 0x01E1
MF_BYPOSITION, MF_GRAYED = 0x400, 0x1

k32 = ctypes.windll.kernel32
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
u32.GetMenuStringW.argtypes = [ctypes.c_void_p, wt.UINT, wt.LPWSTR, ctypes.c_int, wt.UINT]
u32.GetMenuState.restype = wt.UINT
u32.GetMenuState.argtypes = [ctypes.c_void_p, wt.UINT, wt.UINT]
u32.FindWindowW.restype = wt.HWND
u32.SendMessageW.restype = ctypes.c_ssize_t

WORDS = {
    "uk": {
        "menu": ["Подробиці (&D)", "Показати в Провіднику (&E)", "Копіювати шлях (&O)"],
        "no_difference": "Копіювати шлях: різниці ще немає.",
        "copied": "Шлях скопійовано.",
        "copied_path": "Шлях скопійовано: ",
        "more": "Копіювати шлях: вузол «Ще…» — не файл і не тека.",
        "next": "клавіша Enter покаже наступні 1000",
        "settings": "Налаштування",
        "tab": "Різниця",
        "check_box": "Показувати скопійований шлях у рядку стану",
        "saved": "Налаштування збережено.",
    },
    "en": {
        "menu": ["&Details", "Show in File &Explorer", "C&opy path"],
        "no_difference": "Copy path: there is no difference yet.",
        "copied": "Path copied.",
        "copied_path": "Path copied: ",
        "more": "Copy path: a “more” node is not a file or a folder.",
        "next": "press Enter to show the next 1,000",
        "settings": "Settings",
        "tab": "Difference",
        "check_box": "Show the copied path in the status bar",
        "saved": "Settings saved.",
    },
}[LANG]
# NVDA says the state of a check box in its own language, whatever revisor speaks.
CHECKED, UNCHECKED = "позначено", "не позначено"
missing = []


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)


def heard(lines, *words):
    text = " ".join(lines)
    for word in words:
        check(word in text, f"heard {word!r}")
    return text


def in_revisor():
    title = foreground_title()
    if not title.endswith("revisor"):
        sys.exit(f"the focus left revisor for «{title}»: stopped; missing so far: {missing}")


def press(*vks):
    """Presses keys in revisor's main window only: a step gone wrong must not type into
    another window."""
    in_revisor()
    key(*vks)


def write(rel, text):
    full = os.path.join(WATCHED, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


def snapshots(app, count):
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == count, 60), "no snapshot"
    assert wait_for(app.snapshot_done, 60, 0.02), "the progress row never went"


# --- the clipboard ------------------------------------------------------------------------------
def sequence():
    return u32.GetClipboardSequenceNumber()


def clipboard_text():
    """The text on the clipboard. Called only once a copy of revisor changed it."""
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


last_copied = []


def copied(app, name, act, wait=1.5):
    """Runs `act` and returns what NVDA said and the clipboard's text, or None when the
    clipboard did not change."""
    before = sequence()
    act()
    lines = app.step(name, wait)
    if not wait_for(lambda: sequence() != before, 2):
        return lines, None
    text = clipboard_text()
    last_copied.append(text)
    return lines, text


def status_text(main):
    """The text of the status bar, through WM_GETTEXT: its first part, where messages go."""
    for h in children(main):
        if class_name(h) == "msctls_statusbar32":
            return control_text(h)
    return None


# --- menus --------------------------------------------------------------------------------------
def menu_items(menu):
    """The texts of a menu's items and whether each is greyed out."""
    items = []
    for i in range(u32.GetMenuItemCount(menu)):
        buf = ctypes.create_unicode_buffer(256)
        u32.GetMenuStringW(menu, i, buf, 256, MF_BYPOSITION)
        greyed = bool(u32.GetMenuState(menu, i, MF_BYPOSITION) & MF_GRAYED)
        items.append((buf.value, greyed))
    return items


def difference_menu(main):
    """The items of the menu «Різниця», the fourth of the menu bar."""
    return menu_items(u32.GetSubMenu(u32.GetMenu(main), 3))


def open_popup():
    if not wait_for(lambda: u32.FindWindowW("#32768", None), 3):
        return None
    return u32.SendMessageW(u32.FindWindowW("#32768", None), MN_GETHMENU, 0, 0) or None


# --- the focused item of a list or a tree, through MSAA -----------------------------------------
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


def focused_name():
    """The name of the focused item of the focused list or tree: a row's name, a node's
    text, its no-break spaces made plain ones; '' when there is none."""
    acc = ctypes.c_void_p()
    if oleacc.AccessibleObjectFromWindow(focused_window(), OBJID_CLIENT,
                                         ctypes.byref(IID_IACCESSIBLE),
                                         ctypes.byref(acc)) != 0 or not acc:
        return ""
    vtbl = ctypes.cast(ctypes.cast(acc, ctypes.POINTER(ctypes.c_void_p))[0],
                       ctypes.POINTER(ctypes.c_void_p))
    get_focus = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p,
                                   ctypes.POINTER(VARIANT))(vtbl[18])
    get_name = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, VARIANT,
                                  ctypes.POINTER(ctypes.c_void_p))(vtbl[10])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(vtbl[2])
    try:
        child = VARIANT()
        if get_focus(acc, ctypes.byref(child)) != 0 or child.vt != VT_I4:
            return ""
        name = ctypes.c_void_p()
        if get_name(acc, child, ctypes.byref(name)) != 0 or not name:
            return ""
        text = ctypes.wstring_at(name)
        ctypes.windll.oleaut32.SysFreeString(name)
        return text.replace("\xa0", " ").replace(" ", " ")
    finally:
        release(acc)


# --- the steps ----------------------------------------------------------------------------------
def before_any_difference(app):
    items = difference_menu(app.main)
    print("   the menu «Різниця»:", items)
    names = [text.split("\t")[0] for text, _ in items]
    check(names[:3] == WORDS["menu"], "Подробиці, Показати в Провіднику, Копіювати шлях first")
    copy = items[2] if len(items) > 2 else ("", False)
    check(copy[0].endswith("\tCtrl+Shift+C"), f"the item shows Ctrl+Shift+C: {copy[0]!r}")
    check(copy[1], "the item is unavailable without a difference")
    lines, text = copied(app, "Ctrl+Shift+C: no difference yet",
                         lambda: press(VK_CONTROL, VK_SHIFT, vk("C")))
    heard(lines, WORDS["no_difference"])
    check(text is None, "the clipboard is left alone")


def the_list(app, expected):
    lists = [h for h in children(app.main) if class_name(h) == "SysListView32"]
    shown = focused_window()
    if shown == snapshot_list(app.main) or shown not in lists:
        press(VK_F6)
        app.step("F6: the difference list", 1.5)
    check(focused_class() == "SysListView32" and focused_window() != snapshot_list(app.main),
          "the focus is in the difference list")
    rows = item_count(focused_window())
    print(f"   rows: {rows}")
    press(VK_HOME)
    app.step("Home", 1.5)
    seen = set()
    for row in range(rows):
        if row:
            press(VK_DOWN)
            app.step("Down", 1.2)
        name = focused_name()
        before = focused_window()
        lines, text = copied(app, f"Ctrl+Shift+C on «{name}»",
                             lambda: press(VK_CONTROL, VK_SHIFT, vk("C")))
        heard(lines, WORDS["copied"])
        check(text == expected.get(name), f"the clipboard holds {expected.get(name)!r}: {text!r}")
        check(text is not None and '"' not in text, "no quotes")
        check(focused_window() == before and focused_name() == name, "the focus stayed")
        check(name not in " ".join(lines), "NVDA did not say the row again")
        seen.add(name)
    check({"a.txt", "b.txt", "e.txt", "new.txt"} <= seen,
          f"a deleted, a modified, a renamed and an added entry among the rows: {sorted(seen)}")


def menu_and_context_menu(app, expected):
    press(VK_HOME)
    app.step("Home", 1.2)
    name = focused_name()

    def menu_bar():
        press(VK_MENU, vk("D"))
        time.sleep(0.6)
        key(vk("O"))

    lines, text = copied(app, f"Alt+D, O on «{name}»", menu_bar)
    heard(lines, WORDS["copied"])
    check(text == expected.get(name), f"the menu item copied {expected.get(name)!r}: {text!r}")

    press(VK_DOWN)
    app.step("Down", 1.2)
    name = focused_name()
    press(VK_APPS)
    popup = open_popup()
    app.step("Application key: the context menu", 1.5)
    items = menu_items(popup) if popup else []
    print("   the context menu:", items)
    check([text for text, _ in items] == WORDS["menu"],
          "the context menu: Подробиці, Показати в Провіднику, Копіювати шлях")
    if not popup:
        return
    lines, text = copied(app, f"O in the context menu on «{name}»", lambda: key(vk("O")))
    heard(lines, WORDS["copied"])
    check(text == expected.get(name), f"the context menu copied {expected.get(name)!r}: {text!r}")
    check(not u32.FindWindowW("#32768", None), "the context menu closed")


def the_setting(app, expected):
    press(VK_MENU, vk("F"))
    time.sleep(0.5)
    key(vk("S"))
    assert wait_for(lambda: foreground_title() == WORDS["settings"], 5, 0.05), "no settings"
    app.step("Alt+F, S: the settings", 2)
    key(VK_CONTROL, VK_TAB)
    app.step("Ctrl+Tab", 1.2)
    key(VK_CONTROL, VK_TAB)
    heard(app.step("Ctrl+Tab: the tab «Різниця»", 1.5), WORDS["tab"])
    key(VK_MENU, vk("H"))
    said = " ".join(app.step("Alt+H: the check box", 1.5))
    check(WORDS["check_box"] in said, "Alt+H reached «Показувати скопійований шлях…»")
    check(UNCHECKED not in said and CHECKED in said, "Alt+H ticked it")
    key(VK_RETURN)
    heard(app.step("Enter: saved", 2), WORDS["saved"])
    conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
    stored = conn.execute("SELECT value FROM setting WHERE key = 'show_copied_path'").fetchone()
    conn.close()
    check(stored == (1,), f"the setting is saved on: {stored}")
    in_revisor()
    name = focused_name()
    lines, text = copied(app, f"Ctrl+Shift+C on «{name}», the path shown",
                         lambda: press(VK_CONTROL, VK_SHIFT, vk("C")), 2)
    heard(lines, WORDS["copied_path"], os.path.basename(expected.get(name, "?")) + ".")
    check(text == expected.get(name), f"the clipboard holds {expected.get(name)!r}: {text!r}")
    shown = status_text(app.main)
    print(f"   the status bar: {shown!r}")
    check(shown == f"{WORDS['copied_path']}{text}.", "the status bar names the whole path")


def the_tree(app):
    press(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the tree", 2.5)
    check(focused_class() == "SysTreeView32", "the focus is in the tree")
    press(VK_HOME)
    app.step("Home: the folder of the profile", 1.5)
    lines, text = copied(app, f"Ctrl+Shift+C on «{focused_name()}»",
                         lambda: press(VK_CONTROL, VK_SHIFT, vk("C")), 2)
    heard(lines, WORDS["copied_path"])
    check(text == WATCHED, f"the folder of the profile: {text!r}")
    docs = None
    for _ in range(8):
        press(VK_DOWN)
        app.step("Down", 1.2)
        if focused_name().startswith("docs"):
            docs = focused_name()
            break
    check(docs is not None, "the node of docs, a folder with a change inside")
    if docs is None:
        return
    lines, text = copied(app, f"Ctrl+Shift+C on «{docs}»",
                         lambda: press(VK_CONTROL, VK_SHIFT, vk("C")), 2)
    heard(lines, WORDS["copied_path"])
    check(text == os.path.join(WATCHED, "docs"), f"the folder docs: {text!r}")


def the_more_node(app):
    for i in range(BIG_FILES):
        write(rf"big\f{i:04}.txt", "")
    time.sleep(1)
    press(VK_CONTROL, VK_F5)
    snapshots(app, 3)
    app.step("Ctrl+F5: a folder of 1,050 files, in the tree", 3)
    print(f"   focused: {focused_class()} «{focused_name()}»")
    press(VK_RIGHT)
    app.step("Right: big expanded", 2)
    press(VK_END)
    lines = app.step("End: the node «Ще…»", 2)
    print(f"   focused: «{focused_name()}»")
    on_more = WORDS["next"] in focused_name()
    check(on_more, "the focus is on the node «Ще…»")
    if not on_more:
        return
    lines, text = copied(app, "Ctrl+Shift+C on the node «Ще…»",
                         lambda: press(VK_CONTROL, VK_SHIFT, vk("C")))
    heard(lines, WORDS["more"])
    check(text is None, "the clipboard stays as it was")


def main():
    paths = prepare({"Програми": ([WATCHED], [])})
    write("a.txt", "a")
    write("b.txt", "b")
    write("d.txt", "d")
    write(r"docs\keep.txt", "k")
    make_settings(SCRATCH, language=LANG)
    expected = {
        "a.txt": os.path.join(WATCHED, "a.txt"),
        "b.txt": os.path.join(WATCHED, "b.txt"),
        "e.txt": os.path.join(WATCHED, "e.txt"),
        "new.txt": os.path.join(WATCHED, "docs", "new.txt"),
        "docs": os.path.join(WATCHED, "docs"),
        "watched": WATCHED,
    }
    with App(paths) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start", 2)
        before_any_difference(app)

        press(VK_F5)
        snapshots(app, 1)
        app.step("F5: the snapshot before", 1.5)
        os.remove(os.path.join(WATCHED, "a.txt"))
        write("b.txt", "bbbbbbbb")
        os.rename(os.path.join(WATCHED, "d.txt"), os.path.join(WATCHED, "e.txt"))
        write(r"docs\new.txt", "n")
        time.sleep(1)
        press(VK_CONTROL, VK_F5)
        snapshots(app, 2)
        app.step("Ctrl+F5: the difference", 3)

        the_list(app, expected)
        menu_and_context_menu(app, expected)
        the_setting(app, expected)
        the_tree(app)
        the_more_node(app)
        u32.SetForegroundWindow(app.main)
        time.sleep(0.5)
        app.speech.new()
    # The copy was flushed: it outlives revisor.
    time.sleep(1)
    last = last_copied[-1] if last_copied else None
    print(f"\n   copied last: {last!r}")
    check(last is not None and clipboard_text() == last,
          "the path copied last stays on the clipboard after revisor closed")


main()
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
