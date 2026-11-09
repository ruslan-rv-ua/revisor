"""Live NVDA check of TASK-37: the difference tree shows a big folder a page at a time.

    python -X utf8 tools\\nvda\\task37_tree_pages.py <uk|en> [files in a] [files in b]

The profile's folder has two folders, ``a`` and ``b``, empty at the first snapshot; ``a`` gets
60 000 empty files and ``b`` 20 000 before the second (the counts can be given). The difference
arrives as a tree under the probe: the window answers within half a second (#1), the focus is
on the first change, named with the tree (#2), and the tree has one page of ``a``. Then ``b``
is expanded by hand under the probe (#3); its node «Ще…» says how many nodes are left (#5);
on it «Показати в Провіднику» says why it is unavailable and the context menu does not open
(#7); Enter, «Подробиці» (Alt+Enter) and a double click each bring the next page, with the
focus on its first node (#5). From the list, Ctrl+T on the last row shows the last page under
a node «Ще… вище», and Enter on that one brings the page before, with the focus on its last
node (#6); how long the window takes there, with four pages to take out, is only printed. A text filter leaves one page, that of the node with the focus (#8). The text of
the nodes «Ще…» is read through MSAA too: the speech log writes the no-break space of a
number as an escape. The last line says ``all heard`` or names what is missing.
"""

import ctypes
import ctypes.wintypes as wt
import os
import shutil
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    VK_APPS, VK_DOWN, VK_END, VK_ESCAPE, VK_F5, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, App,
    focused_class, focused_window, foreground_title, item_count, key, make_settings, prepare,
    snapshot_list, type_text, u32, visible_controls, vk, wait_for,
)
from probe import Probe  # noqa: E402

VK_CONTROL, VK_F6, VK_F7 = 0x11, 0x75, 0x76
TVM_GETCOUNT = 0x1105
MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP = 0x0002, 0x0004
PAGE = 1000
# How long the window may take to answer (#1, #3).
LIMIT = 0.5

u32.FindWindowW.restype = wt.HWND
u32.SendMessageW.restype = ctypes.c_ssize_t

lang = sys.argv[1] if len(sys.argv) > 1 else "uk"
in_a = int(sys.argv[2]) if len(sys.argv) > 2 else 60000
in_b = int(sys.argv[3]) if len(sys.argv) > 3 else 20000
assert in_b >= 5 * PAGE and in_b % PAGE == 0, "b needs whole pages, five at least"
source = os.path.join(tempfile.gettempdir(), "revisor-nvda-task37")

WORDS = {
    "uk": {
        "added": " — додано",
        "below": "Ще {} — клавіша Enter покаже наступні 1000",
        "above": "Ще {} вище — клавіша Enter покаже попередні 1000",
        "next": "клавіша Enter покаже наступні 1000",
        "previous": "вище — клавіша Enter покаже попередні 1000",
        "no_entry": "вузол «Ще…» — не файл і не тека",
        "no_actions": "Тут зараз немає доступних дій.",
        "thousands": " ",
    },
    "en": {
        "added": " — added",
        "below": "{} more — press Enter to show the next 1,000",
        "above": "{} more above — press Enter to show the previous 1,000",
        "next": "press Enter to show the next 1,000",
        "previous": "more above — press Enter to show the previous 1,000",
        "no_entry": "a “more” node is not a file or a folder",
        "no_actions": "No actions are available here now.",
        "thousands": ",",
    },
}[lang]

# NVDA says the state of a tree item in its own language, whatever revisor speaks.
EXPANDED = ("розгорнуто", "expanded")

missing = []

# --- the focused tree item, through MSAA ------------------------------------------------------
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


def focused_item():
    """The name of the focused item of the focused tree and the middle of its rectangle on
    screen, through MSAA: ``(name, (x, y))``, or ``(None, None)``."""
    h = focused_window()
    acc = ctypes.c_void_p()
    if oleacc.AccessibleObjectFromWindow(h, OBJID_CLIENT, ctypes.byref(IID_IACCESSIBLE),
                                         ctypes.byref(acc)) != 0 or not acc:
        return None, None
    vtbl = ctypes.cast(ctypes.cast(acc, ctypes.POINTER(ctypes.c_void_p))[0],
                       ctypes.POINTER(ctypes.c_void_p))
    get_focus = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p,
                                   ctypes.POINTER(VARIANT))(vtbl[18])
    get_name = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, VARIANT,
                                  ctypes.POINTER(ctypes.c_void_p))(vtbl[10])
    location = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(ctypes.c_long),
                                  ctypes.POINTER(ctypes.c_long), ctypes.POINTER(ctypes.c_long),
                                  ctypes.POINTER(ctypes.c_long), VARIANT)(vtbl[22])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(vtbl[2])
    try:
        child = VARIANT()
        if get_focus(acc, ctypes.byref(child)) != 0 or child.vt != VT_I4:
            return None, None
        name, text = ctypes.c_void_p(), None
        if get_name(acc, child, ctypes.byref(name)) == 0 and name:
            text = ctypes.wstring_at(name)
            ctypes.windll.oleaut32.SysFreeString(name)
        left, top, width, height = (ctypes.c_long() for _ in range(4))
        point = None
        if location(acc, ctypes.byref(left), ctypes.byref(top), ctypes.byref(width),
                    ctypes.byref(height), child) == 0:
            point = (left.value + width.value // 2, top.value + height.value // 2)
        return text, point
    finally:
        release(acc)


def focused_name():
    """The text of the focused tree item, its no-break spaces made plain ones."""
    name = focused_item()[0] or ""
    return name.replace(" ", " ").replace(" ", " ")


def double_click_focused():
    """A double click on the focused tree item with the mouse itself, which then goes back
    where it was: wx asks where the cursor is, so posted clicks do not do."""
    in_revisor()
    _, point = focused_item()
    assert point, "no rectangle for the focused item"
    was = wt.POINT()
    u32.GetCursorPos(ctypes.byref(was))
    u32.SetCursorPos(*point)
    time.sleep(0.2)
    for _ in range(2):
        u32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
        u32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)
        time.sleep(0.08)
    time.sleep(0.3)
    u32.SetCursorPos(was.x, was.y)


def press(*vks):
    """Presses the keys, in revisor only: a step gone wrong must not type into another
    window."""
    in_revisor()
    key(*vks)


def in_revisor():
    title = foreground_title()
    if not title.endswith("revisor"):
        sys.exit(f"the focus left revisor for «{title}»: stopped; missing so far: {missing}")


def check(what, found):
    print(f"   {'OK' if found else 'MISSING'}: {what}")
    if not found:
        missing.append(what)


def said(lines, text):
    """Whether NVDA said `text`, or one of the texts of a tuple."""
    heard = " ".join(lines)
    return any(t in heard for t in text) if isinstance(text, tuple) else text in heard


def count(n):
    """`n` as revisor writes it in the interface's language, with a plain space."""
    return f"{n:,}".replace(",", WORDS["thousands"])


def tree_items(main):
    """How many items the shown tree has."""
    shown = visible_controls(main, "SysTreeView32")
    return u32.SendMessageW(shown[0], TVM_GETCOUNT, 0, 0) if shown else None


def worst(probe):
    return max((d for _, d in probe.latencies), default=0)


def answered(probe, what):
    slowest = worst(probe)
    print(f"   the window's slowest answer: {slowest:.3f} s")
    check(f"{what}: the window answers within {LIMIT} s", slowest <= LIMIT)


def snapshots(app, wanted):
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == wanted, 300), "no snapshot"
    assert wait_for(app.snapshot_done, 300, 0.02), "the progress row never went"


def compared(app, timeout=120):
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


def to_tree():
    for _ in range(3):
        if focused_class() == "SysTreeView32":
            return
        press(VK_F6)
        time.sleep(0.7)
    assert focused_class() == "SysTreeView32", focused_class()


def fill(folder, letter, files):
    for i in range(files):
        open(os.path.join(source, folder, f"{letter}{i:06}.txt"), "w").close()


def file(letter, i):
    return f"{letter}{i:06}.txt{WORDS['added']}"


def check_pages(app):
    app.step("start: Big, as a tree", 2.5)
    press(VK_F5)
    snapshots(app, 1)
    app.step("F5: the snapshot before", 1.5)
    fill("a", "f", in_a)
    fill("b", "g", in_b)
    press(VK_F5)
    snapshots(app, 2)
    app.step("F5: the snapshot after", 2)

    # 1. The difference arrives: the window answers, the focus is on the first change, and
    # the big folder has one page (#1, #2, #4).
    with Probe(app.proc.pid, app.main) as probe:
        press(VK_F7)
        compared(app)
        assert wait_for(lambda: focused_class() == "SysTreeView32", 30, 0.005), "no tree"
        lines = app.step("F7: the difference arrives as a tree", 3)
    probe.report("F7: the difference arrives as a tree")
    answered(probe, "arrival")
    check("the first change has the focus", focused_name() == file("f", 0))
    check("NVDA names the tree and says the first change",
          len(lines) >= 2 and said(lines, file("f", 0)))
    # The folder of the difference, a, b, a page of a, and its node «Ще…».
    check("a has one page", tree_items(app.main) == 3 + PAGE + 1)

    # 2. Expanding the other big folder by hand (#3).
    press(VK_LEFT)
    app.step("Left: up to a", 1.5)
    press(VK_LEFT)
    app.step("Left: a collapsed", 1.5)
    press(VK_DOWN)
    app.step("Down: b", 1.5)
    with Probe(app.proc.pid, app.main) as probe:
        press(VK_RIGHT)
        lines = app.step("Right: b expands", 2.5)
    probe.report("Right: b expands")
    answered(probe, "expanding by hand")
    check("b is said expanded", said(lines, EXPANDED))
    check("b has one page too", tree_items(app.main) == 3 + 2 * (PAGE + 1))

    # 3. The node «Ще…» (#5), File Explorer and the context menu on it (#7).
    press(VK_END)
    lines = app.step("End: the node «Ще…» of b", 2)
    print(f"   the focused item: {focused_name()!r}")
    check("the node says how many are left",
          focused_name() == WORDS["below"].format(count(in_b - PAGE)))
    check("NVDA says what Enter does", said(lines, WORDS["next"]))
    if WORDS["next"] not in focused_name():
        # On a file or a folder the next keys would open File Explorer and its menu.
        sys.exit(f"the focus is not on a node «Ще…»: stopped; missing: {missing}")
    press(VK_CONTROL, VK_RETURN)
    lines = app.step("Ctrl+Enter: File Explorer on the node «Ще…»", 2)
    check("File Explorer says why it is unavailable", said(lines, WORDS["no_entry"]))
    press(VK_APPS)
    popup = wait_for(lambda: u32.FindWindowW("#32768", None), 2)
    lines = app.step("Application key on the node «Ще…»", 1)
    check("the context menu does not open", not popup)
    check("the status bar says there are no actions", said(lines, WORDS["no_actions"]))
    if popup:
        press(VK_ESCAPE)
        app.step("Esc", 1)

    # 4. Enter, «Подробиці» and a double click turn the page (#5).
    turns = (("Enter", lambda: press(VK_RETURN)),
             ("Alt+Enter, «Подробиці»", lambda: press(VK_MENU, VK_RETURN)),
             ("a double click", double_click_focused))
    for page, (name, act) in enumerate(turns, start=1):
        if page > 1:
            press(VK_END)
            app.step("End: the node «Ще…» again", 1.5)
            check(f"the node says {count(in_b - page * PAGE)} are left",
                  focused_name() == WORDS["below"].format(count(in_b - page * PAGE)))
        with Probe(app.proc.pid, app.main) as probe:
            act()
            lines = app.step(f"{name} on the node «Ще…»", 2.5)
        probe.report(f"{name}: the next page")
        answered(probe, name)
        first = file("g", page * PAGE)
        check(f"{name}: the focus is on the first node of the next page",
              focused_name() == first and said(lines, first))
        check(f"{name}: a page more",
              tree_items(app.main) == 3 + PAGE + 1 + (page + 1) * PAGE + 1)
        check(f"{name}: no window opened", foreground_title().endswith("revisor"))

    # 5. Ctrl+T from the last row of the list: the last page, under a node «Ще… вище» (#6).
    press(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the list", 2)
    check("Ctrl+T moves the focus to the list", focused_class() == "SysListView32")
    press(VK_END)
    app.step("End: the last row", 1.5)
    with Probe(app.proc.pid, app.main) as probe:
        press(VK_CONTROL, vk("T"))
        lines = app.step("Ctrl+T: the tree, on the last file", 2.5)
    probe.report("Ctrl+T: the tree, on the last file")
    # Not a criterion: the four pages shown before go, an item at a time, and that takes
    # the window about a second with a screen reader.
    print(f"   the window's slowest answer: {worst(probe):.3f} s")
    last = file("g", in_b - 1)
    check("the focus is on the last file", focused_name() == last and said(lines, last))
    print(f"   NOTE: NVDA said the node {' '.join(lines).count(last)} time(s)")
    # A page of a, and of b its last page under the node «Ще… вище».
    check("b shows one page, the last", tree_items(app.main) == 3 + 2 * (PAGE + 1))
    press(VK_LEFT)
    app.step("Left: up to b", 1.5)
    press(VK_DOWN)
    lines = app.step("Down: the node «Ще… вище»", 2)
    print(f"   the focused item: {focused_name()!r}")
    check("the node says how many are above",
          focused_name() == WORDS["above"].format(count(in_b - PAGE)))
    check("NVDA says what Enter does there", said(lines, WORDS["previous"]))
    with Probe(app.proc.pid, app.main) as probe:
        press(VK_RETURN)
        lines = app.step("Enter on the node «Ще… вище»", 2.5)
    probe.report("Enter: the page before")
    answered(probe, "Enter on the node «Ще… вище»")
    before = file("g", in_b - PAGE - 1)
    check("the focus is on the last node of the page before",
          focused_name() == before and said(lines, before))
    check("a page more above", tree_items(app.main) == 3 + PAGE + 1 + 2 * PAGE + 1)

    # 6. The filter leaves one page, that of the node with the focus (#8).
    with Probe(app.proc.pid, app.main) as probe:
        press(VK_CONTROL, vk("F"))
        time.sleep(0.5)
        # The second half of b: the node with the focus is on its page before the last.
        in_revisor()
        type_text(f"g{in_b // 2:06}"[:3])
        app.step("Ctrl+F, the files of the second half of b, the pause", 3)
    probe.report("the filter")
    answered(probe, "the filter")
    # The folder of the difference and b; a page between its two nodes «Ще…».
    check("the filter leaves one page of b", tree_items(app.main) == 2 + PAGE + 2)
    to_tree()
    lines = app.step("F6: back in the tree", 2)
    check("the focus stays on its node", focused_name() == before and said(lines, before))
    press(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    press(VK_ESCAPE)
    app.step("Ctrl+F, Esc: the filter cleared", 2.5)
    check("without the filter b shows one page again",
          tree_items(app.main) == 3 + PAGE + 2)
    to_tree()
    lines = app.step("F6: back in the tree", 2)
    check("the focus is still on its node", focused_name() == before)


try:
    shutil.rmtree(source, ignore_errors=True)
    os.makedirs(os.path.join(source, "a"))
    os.makedirs(os.path.join(source, "b"))
    paths = prepare({"Big": ([source], [])})
    make_settings(os.path.dirname(paths["exe"]), language=lang, difference_view="tree")
    with App(paths, paths["Big"]) as app:
        check_pages(app)
finally:
    shutil.rmtree(source, ignore_errors=True)

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
