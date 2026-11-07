"""Live NVDA check of TASK-33: the difference can be seen as a tree.

    python -X utf8 tools\\nvda\\task33_difference_tree.py <uk|en> [parts] ["<big folder>"]

``parts`` is a comma-separated choice of ``small``, ``restart`` and ``big`` (``small,restart``
by default); ``restart`` goes on from the settings ``small`` left. ``big`` takes the folder given
third (``C:\\dev`` by default) as the folder of its profile between two snapshots, so the
difference has every entry of it (TASK-13), and the difference arrives as a tree.

«Small» is a scratch folder with a few files, so the nodes are known. Part ``small``: the
difference as a list, then Ctrl+T — the tree, named, on the same entry (#7); moving, expanding
and collapsing, the level and the state said (#11); the details of a folder that is no change
and of a node that is (#9); the context menu, its first item bold (#9); the menu «Різниця» with
its view items and Ctrl+T, and an item of «Сортувати за», unavailable, saying why (#5); the
type filter and the text filter in the tree, the focus kept or moved (#4); Ctrl+T back to the
list on the same entry (#7); the short text of the nodes from the settings (#6); Ctrl+T with
the focus elsewhere, said in the status bar; a new comparison with the focus in the tree, named
again (#8). Part ``restart``: the tree kept for the next start (#3) and a difference arriving in
it. Part ``big``: a difference of every entry of the big folder arriving in the tree under the
probe (#2), the time to switch the view and to filter. The last line says ``all heard`` or names
what is missing.
"""

import ctypes
import ctypes.wintypes as wt
import os
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    ES_MULTILINE, ES_READONLY, SCRATCH, VK_APPS, VK_DOWN, VK_ESCAPE, VK_F5, VK_LEFT, VK_MENU,
    VK_RETURN, VK_RIGHT, VK_UP, App, control_text, edit_style, focused_class, focused_window,
    foreground_title, item_count, key, make_settings, prepare, snapshot_list, type_text, u32,
    visible_controls, vk, wait_for,
)
from probe import Probe  # noqa: E402

VK_SHIFT, VK_CONTROL, VK_F6, VK_F7 = 0x10, 0x11, 0x75, 0x76
TVM_GETCOUNT = 0x1105
MN_GETHMENU = 0x01E1

u32.FindWindowW.restype = wt.HWND
u32.SendMessageW.restype = ctypes.c_ssize_t
u32.GetMenuDefaultItem.restype = wt.UINT
u32.GetMenuDefaultItem.argtypes = [ctypes.c_void_p, wt.UINT, wt.UINT]

lang = sys.argv[1] if len(sys.argv) > 1 else "uk"
parts = set(sys.argv[2].split(",")) if len(sys.argv) > 2 else {"small", "restart"}
big_source = sys.argv[3] if len(sys.argv) > 3 else r"C:\dev"
tree = os.path.join(SCRATCH, "tree")
big = os.path.join(SCRATCH, "big")

WORDS = {
    "uk": {
        "a_long": "a.txt — змінено: розмір, дата зміни",
        "a_short": "a.txt — змінено",
        "b": "b.txt — видалено",
        "docs": "docs — 3 зміни",
        "sub": "sub — додано теку, 1 зміна",
        "sub_short": "sub — додано теку",
        "new": "new — додано теку, 2 зміни",
        "deep": "deep.txt — додано",
        "level": "рівень",
        "showing": "Показано",
        "inside": "Усередині:",
        "row_change": "Зміна: додано",
        "details": "Подробиці: ",
        "settings": "Налаштування",
        "sorted": "у дереві вузли завжди впорядковано за ім'ям",
        "as_list": "Різницю показано списком.",
        "as_tree": "Різницю показано деревом.",
        "view_tree": "Деревом",
        "view_list": "Списком",
        "no_difference": "Різниці ще немає",
    },
    "en": {
        "a_long": "a.txt — modified: size, date modified",
        "a_short": "a.txt — modified",
        "b": "b.txt — deleted",
        "docs": "docs — 3 changes",
        "sub": "sub — folder added, 1 change",
        "sub_short": "sub — folder added",
        "new": "new — folder added, 2 changes",
        "deep": "deep.txt — added",
        "level": "level",
        "showing": "Showing",
        "inside": "Inside:",
        "row_change": "Change: added",
        "details": "Details: ",
        "settings": "Settings",
        "sorted": "the tree is always sorted by name",
        "as_list": "The difference is shown as a list.",
        "as_tree": "The difference is shown as a tree.",
        "view_tree": "Tree",
        "view_list": "List",
        "no_difference": "No difference yet",
    },
}[lang]

# NVDA says the state of a tree item in its own language, whatever revisor speaks.
COLLAPSED, EXPANDED = ("згорнуто", "collapsed"), ("розгорнуто", "expanded")

missing = []

# --- the level of the focused tree item, through MSAA ----------------------------------------
# The author's NVDA has reportObjectPositionInformation off, so the level is not spoken; the
# tree item's level is its MSAA value, which a screen reader reads when that is on.
oleacc = ctypes.windll.oleacc
OBJID_CLIENT, VT_I4, VT_BSTR = -4, 3, 8


class VARIANT(ctypes.Structure):
    _fields_ = [("vt", ctypes.c_ushort), ("r1", ctypes.c_ushort), ("r2", ctypes.c_ushort),
                ("r3", ctypes.c_ushort), ("value", ctypes.c_longlong), ("extra", ctypes.c_longlong)]


class GUID(ctypes.Structure):
    _fields_ = [("a", ctypes.c_ulong), ("b", ctypes.c_ushort), ("c", ctypes.c_ushort),
                ("d", ctypes.c_ubyte * 8)]


IID_IACCESSIBLE = GUID(0x618736E0, 0x3C3D, 0x11CF,
                       (ctypes.c_ubyte * 8)(0x81, 0x0C, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71))
ctypes.windll.oleaut32.SysFreeString.argtypes = [ctypes.c_void_p]


def focused_item_level():
    """The MSAA value of the tree's focused item — its level — or None."""
    h = focused_window()
    acc = ctypes.c_void_p()
    if oleacc.AccessibleObjectFromWindow(h, OBJID_CLIENT, ctypes.byref(IID_IACCESSIBLE),
                                         ctypes.byref(acc)) != 0 or not acc:
        return None
    vtbl = ctypes.cast(ctypes.cast(acc, ctypes.POINTER(ctypes.c_void_p))[0],
                       ctypes.POINTER(ctypes.c_void_p))
    get_focus = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p,
                                   ctypes.POINTER(VARIANT))(vtbl[18])
    get_value = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, VARIANT,
                                   ctypes.POINTER(ctypes.c_void_p))(vtbl[11])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(vtbl[2])
    try:
        child = VARIANT()
        if get_focus(acc, ctypes.byref(child)) != 0 or child.vt != VT_I4:
            return None
        value = ctypes.c_void_p()
        if get_value(acc, child, ctypes.byref(value)) != 0 or not value:
            return None
        text = ctypes.wstring_at(value)
        ctypes.windll.oleaut32.SysFreeString(value)
        return text
    finally:
        release(acc)


def position_reported():
    """Whether the author's NVDA speaks position information, levels among it."""
    ini = os.path.join(os.environ.get("APPDATA", ""), "nvda", "nvda.ini")
    try:
        with open(ini, encoding="utf-8") as f:
            return "reportObjectPositionInformation = False" not in f.read()
    except OSError:
        return True


def check(what, found):
    print(f"   {'OK' if found else 'MISSING'}: {what}")
    if not found:
        missing.append(what)


def said(lines, text):
    """Whether NVDA said `text`, or one of the texts of a tuple."""
    heard = " ".join(lines)
    return any(t in heard for t in text) if isinstance(text, tuple) else text in heard


def write(rel, text):
    full = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


def trees(main):
    """The difference trees on screen: the one shown, or none."""
    return visible_controls(main, "SysTreeView32")


def tree_items(main):
    """How many items the shown tree has: only those of the folders opened so far."""
    shown = trees(main)
    return u32.SendMessageW(shown[0], TVM_GETCOUNT, 0, 0) if shown else None


def difference_list(main):
    shown = visible_controls(main, "SysListView32")
    return shown[1] if len(shown) > 1 else None


def filter_field(main):
    for h in visible_controls(main, "Edit"):
        if not edit_style(h) & (ES_MULTILINE | ES_READONLY):
            return h
    return None


def report(app):
    print(f"   focus: {focused_class()} | trees shown: {len(trees(app.main))} "
          f"| tree items: {tree_items(app.main)} | list shown: "
          f"{difference_list(app.main) is not None} | title: {foreground_title()}")


def open_popup():
    if not wait_for(lambda: u32.FindWindowW("#32768", None), 3):
        return None
    popup = u32.FindWindowW("#32768", None)
    return u32.SendMessageW(popup, MN_GETHMENU, 0, 0) or None


def snapshots(app, count):
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == count, 120), "no snapshot"
    assert wait_for(app.snapshot_done, 120, 0.02), "the progress row never went"


def compared(app, timeout=120):
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


def menu(*letters):
    key(VK_MENU, vk(letters[0]))
    time.sleep(0.6)
    for letter in letters[1:]:
        key(vk(letter))
        time.sleep(0.5)


def to_tree(app):
    for _ in range(3):
        if focused_class() == "SysTreeView32":
            return
        key(VK_F6)
        time.sleep(0.7)
    assert focused_class() == "SysTreeView32", focused_class()


def fill_tree():
    write("a.txt", "a")
    write("b.txt", "b")
    write(r"docs\one.txt", "1")
    write(r"docs\two.txt", "2")
    write(r"keep\k.txt", "k")


def change_tree():
    write("a.txt", "aaaa")
    os.remove(os.path.join(tree, "b.txt"))
    write(r"docs\one.txt", "1111")
    write(r"docs\sub\deep.txt", "deep")
    write(r"new\n1.txt", "n1")
    write(r"new\n2.txt", "n2")


def check_small(app):
    app.step("start: Small", 2.5)
    key(VK_F5)
    snapshots(app, 1)
    app.step("F5: the snapshot before", 1.5)
    change_tree()
    time.sleep(1)
    key(VK_F5)
    snapshots(app, 2)
    app.step("F5: the snapshot after", 1.5)

    # 1. The difference as a list, then as a tree on the same entry (#7).
    key(VK_F7)
    compared(app)
    app.step("F7: the difference as a list", 2.5)
    report(app)
    lines = app.step("(the list)", 0.5)
    with Probe(app.proc.pid, app.main) as probe:
        key(VK_CONTROL, vk("T"))
        lines = app.step("Ctrl+T: the tree", 2.5)
    probe.report("Ctrl+T: the events of the switch")
    report(app)
    check("Ctrl+T moves the focus to the tree", focused_class() == "SysTreeView32")
    check("the tree's first change is the list's first row", said(lines, WORDS["a_long"]))
    first_level = focused_item_level()
    print(f"   MSAA value (level) of the focused item: {first_level!r}")
    if position_reported():
        check("the level is said", said(lines, WORDS["level"]))
    else:
        print("   (NVDA reports no position information here, so the level is not spoken)")
        check("the level is exposed to screen readers", (first_level or "").isdigit())
    check("only the way to the first change has items",
          (tree_items(app.main) or 0) <= 6)

    # 2. Moving, expanding, collapsing (#11).
    key(VK_DOWN)
    check("Down: the deleted file", said(app.step("Down", 1.5), WORDS["b"]))
    key(VK_DOWN)
    lines = app.step("Down: the folder", 1.5)
    check("a folder says how many changes it holds", said(lines, WORDS["docs"]))
    check("a closed folder is said collapsed", said(lines, COLLAPSED))
    key(VK_RIGHT)
    check("Right expands it", said(app.step("Right", 1.5), EXPANDED))
    report(app)
    key(VK_DOWN)
    app.step("Down: inside the folder", 1.5)
    level = focused_item_level()
    print(f"   MSAA value (level) inside the folder: {level!r}")
    check("a level deeper inside the folder",
          (level or "").isdigit() and (first_level or "").isdigit()
          and int(level) == int(first_level) + 1)
    key(VK_DOWN)
    check("a folder that is a change says so",
          said(app.step("Down: the folder added", 1.5), WORDS["sub"]))
    key(VK_LEFT)
    app.step("Left: up to the folder", 1.5)
    key(VK_LEFT)
    check("Left collapses it", said(app.step("Left", 1.5), COLLAPSED))

    # 3. The details of a folder that is no change, and of a node that is one (#9).
    key(VK_MENU, VK_RETURN)
    app.step("Alt+Enter: the details of the folder", 2)
    title = foreground_title()
    text = control_text(focused_window())
    print(f"   title: {title}\n   " + "\n   ".join(text.splitlines()))
    check("the folder's details name it", title == WORDS["details"] + "docs")
    check("the folder's details sum up what is inside", WORDS["inside"] in text)
    key(VK_ESCAPE)
    app.step("Esc", 1.5)
    key(VK_DOWN)
    check("Down: the folder added with files", said(app.step("Down", 1.5), WORDS["new"]))
    key(VK_RETURN)
    app.step("Enter: the details of the node", 2)
    title = foreground_title()
    text = control_text(focused_window())
    print(f"   title: {title}\n   " + "\n   ".join(text.splitlines()[:4]))
    check("Enter opens the details of a node that is a change", WORDS["row_change"] in text)
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

    # 4. The context menu, its first item bold (#9).
    key(VK_APPS)
    popup = open_popup()
    app.step("Application key: the context menu", 1.5)
    check("the tree's context menu opens", popup is not None)
    if popup:
        check("its first item is bold", u32.GetMenuDefaultItem(popup, 1, 0) == 0)
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

    # 5. The menu «Різниця»: the view items with Ctrl+T, and «Сортувати за» unavailable,
    # saying why (#5, #7).
    menu("D")
    heard = []
    for i in range(9):
        key(VK_DOWN)
        heard += app.step(f"menu, Down {i + 1}", 1)
    check("the view items show Ctrl+T", said(heard, WORDS["view_tree"]) and said(heard, "Ctrl+T"))
    key(VK_RIGHT)
    lines = app.step("Right: «Сортувати за» in the tree", 2)
    check("a sort item says why it is unavailable", said(lines, WORDS["sorted"]))
    key(VK_DOWN)
    app.step("Down", 1.5)
    for _ in range(3):
        key(VK_ESCAPE)
        time.sleep(0.3)
    app.step("Esc ×3", 1)

    # 6. The filter in the tree (#4).
    key(VK_CONTROL, vk("2"))
    lines = app.step("Ctrl+2: deleted hidden", 2)
    check("the type filter says how many rows show", said(lines, WORDS["showing"]))
    key(VK_CONTROL, vk("2"))
    app.step("Ctrl+2: shown again", 2)
    key(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    type_text("deep")
    app.step("Ctrl+F, «deep», the pause", 2.5)
    key(VK_F6)
    time.sleep(0.6)
    to_tree(app)
    lines = app.step("F6, F6: back in the tree", 2)
    check("the focus went to the entry the filter shows", said(lines, WORDS["deep"]))
    report(app)

    # 7. Back to the list on the same entry (#7).
    key(VK_CONTROL, vk("T"))
    lines = app.step("Ctrl+T: the list", 2.5)
    report(app)
    check("Ctrl+T back moves the focus to the list", focused_class() == "SysListView32")
    check("the list has the same entry", said(lines, "deep.txt"))
    key(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    key(VK_ESCAPE)
    app.step("Ctrl+F, Esc: the filter cleared", 2)

    # 8. Ctrl+T with the focus elsewhere: the status bar says it.
    key(VK_CONTROL, vk("T"))
    check("Ctrl+T from the filter field says the tree",
          said(app.step("Ctrl+T in the field", 2), WORDS["as_tree"]))
    key(VK_CONTROL, vk("T"))
    check("and the list", said(app.step("Ctrl+T in the field again", 2), WORDS["as_list"]))
    key(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the tree again", 1.5)

    # 9. The short text of the nodes (#6).
    menu("F", "S")
    assert wait_for(lambda: foreground_title() == WORDS["settings"], 5, 0.05), "no settings"
    time.sleep(0.5)
    key(VK_MENU, vk("T"))
    app.step("Alt+T in the settings: Tree nodes", 1.5)
    key(VK_DOWN)
    app.step("Down: the short text", 1.5)
    key(VK_RETURN)
    time.sleep(1)
    to_tree(app)
    lines = app.step("F6: the tree, the short text", 2)
    key(VK_UP)
    lines += app.step("Up", 1.5)
    check("the short text has the change without what changed or the count",
          said(lines, WORDS["sub_short"]) and not said(lines, WORDS["sub"]))

    # 10. A new comparison with the focus in the tree: named again, on its first change (#8).
    key(VK_F7)
    compared(app)
    lines = app.step("F7 with the focus in the tree", 3)
    report(app)
    check("the new difference has the focus in a tree", focused_class() == "SysTreeView32")
    check("on its first change", said(lines, WORDS["a_short"]))


def check_restart(app):
    # The tree is kept for the next start (#3); a difference arrives in it (#8).
    app.step("restarted", 2.5)
    to_tree(app)
    lines = app.step("F6: the empty tree", 2)
    check("the empty tree is named", said(lines, WORDS["no_difference"]))
    key(VK_F6)
    time.sleep(0.6)
    key(VK_F7)
    compared(app)
    lines = app.step("F7: the difference arrives as a tree", 3)
    report(app)
    check("a difference arrives in the tree", focused_class() == "SysTreeView32")
    check("on its first change", said(lines, WORDS["a_short"]))


def timed(act, done, timeout=30):
    started = time.time()
    act()
    assert wait_for(done, timeout, 0.002), "the window did not answer"
    return time.time() - started


def check_big(app):
    app.step("start: Big, as a tree", 2.5)
    key(VK_F5)
    compared(app, 600)
    app.step("F5: the folder missing", 1.5)
    subprocess.run(["cmd", "/c", "mklink", "/J", big, big_source], check=True,
                   capture_output=True)
    key(VK_F5)
    compared(app, 600)
    app.step("F5: the folder there", 2)
    with Probe(app.proc.pid, app.main) as probe:
        started = time.time()
        key(VK_F7)
        compared(app, 600)
        assert wait_for(lambda: focused_class() == "SysTreeView32", 30, 0.005), "no tree"
        print(f"   F7 until the tree has the focus: {time.time() - started:.3f} s")
        lines = app.step("F7: the difference of every entry, as a tree", 3)
    probe.report("F7: the difference arrives as a tree")
    report(app)
    check("the big difference arrives in the tree", focused_class() == "SysTreeView32")
    check("on its first change, named with the tree", len(lines) >= 2)
    print(f"   MSAA value (level) of the focused item: {focused_item_level()!r}")
    with Probe(app.proc.pid, app.main) as probe:
        took = timed(lambda: key(VK_CONTROL, vk("T")),
                     lambda: focused_class() == "SysListView32")
        print(f"   Ctrl+T to the list: {took:.3f} s")
        app.step("Ctrl+T: the list", 2)
    probe.report("Ctrl+T: to the list")
    with Probe(app.proc.pid, app.main) as probe:
        took = timed(lambda: key(VK_CONTROL, vk("T")),
                     lambda: focused_class() == "SysTreeView32")
        print(f"   Ctrl+T to the tree: {took:.3f} s")
        app.step("Ctrl+T: the tree", 2)
    probe.report("Ctrl+T: to the tree")
    key(VK_RIGHT)
    app.step("Right: the first folder opens", 2)
    print(f"   tree items: {tree_items(app.main)}")
    key(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    field = filter_field(app.main)
    typed = ""
    for letter in "cargo":
        typed += letter
        took = timed(lambda: type_text(letter), lambda: control_text(field) == typed, 10)
        print(f"   «{typed}»: {took:.3f} s, tree items {tree_items(app.main)}")
    app.step("typed «cargo»", 2.5)
    took = timed(lambda: key(VK_ESCAPE), lambda: control_text(field) == "", 10)
    print(f"   Esc: {took:.3f} s")
    app.step("Esc", 2)
    to_tree(app)
    with Probe(app.proc.pid, app.main) as probe:
        key(VK_F7)
        compared(app, 600)
        lines = app.step("F7 with the focus in the tree", 3)
    probe.report("F7: a new big difference in place of the old")
    check("a new big difference is named again in the tree",
          focused_class() == "SysTreeView32" and len(lines) >= 2)


try:
    if "small" in parts or "restart" in parts:
        paths = prepare({"Small": ([tree], [])})
        make_settings(os.path.dirname(paths["exe"]), language=lang)
        fill_tree()
        if "small" in parts:
            with App(paths, paths["Small"]) as app:
                check_small(app)
        if "restart" in parts:
            with App(paths, paths["Small"]) as app:
                check_restart(app)
    if "big" in parts:
        if os.path.lexists(big):
            os.rmdir(big)
        paths = prepare({"Big": ([big], [])})
        make_settings(os.path.dirname(paths["exe"]), language=lang, difference_view="tree")
        with App(paths, paths["Big"]) as app:
            check_big(app)
finally:
    if os.path.lexists(big):
        os.rmdir(big)
    if os.path.exists(tree):
        shutil.rmtree(tree, ignore_errors=True)

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
