"""Live NVDA check of TASK-13: the difference can be seen as a list.

    python -X utf8 tools\\nvda\\task13_difference_list.py "<folder>" [parts]

``parts`` is a comma-separated choice of ``small``, ``restart`` and ``big`` (all by default);
``restart`` goes on from where ``small`` left the settings, so it runs after it.

«Small» is a scratch folder with a few files, so the rows are known. Part ``small``: the field
«Порівняння» and the empty list before the first comparison and the refusals of the filter's
commands (#11, #30), the order of Tab (#2); then two snapshots with a comment and a comparison
whose list is named by its summary (#3, #12–#16), the field read line by line (#1, #4), the
text filter while typing, with «Показано N із M» after the pause and Esc (#24–#26), a check box
and its menu item (#22, #26), a key of a type from the field (#30), sorting by a column from the
menu with the focus on its change (#21), one row selected after Ctrl+A (#19), a new comparison
with the focus in the list (#3), the earlier snapshot deleted (#5) and the setting «Розмір»
(#17). Part ``restart``: a restart keeps the types and the order (#28) and starts with nothing
compared (#11). Part ``big``: the folder given, missing in the first snapshot and there in the
second, gives a difference of all its entries with a warning (#9, #29): its typing and sorting
are timed.
"""

import os
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    ES_MULTILINE, ES_READONLY, SCRATCH, VK_DELETE, VK_DOWN, VK_ESCAPE, VK_F2, VK_F5, VK_HOME,
    VK_MENU, VK_RETURN, VK_SPACE, VK_TAB, App, class_name,
    control_text, edit_style, focused_window, foreground_title, key, prepare, type_text, u32,
    visible_controls, vk, wait_for,
)

VK_SHIFT, VK_CONTROL, VK_F6, VK_F7 = 0x10, 0x11, 0x75, 0x76
LVM_GETITEMCOUNT, LVM_GETNEXTITEM, LVM_GETSELECTEDCOUNT = 0x1004, 0x100C, 0x1032
LVNI_FOCUSED = 0x0001

parts = set(sys.argv[2].split(",")) if len(sys.argv) > 2 else {"small", "restart", "big"}
big_source = sys.argv[1] if len(sys.argv) > 1 else r"C:\dev"
tree = os.path.join(SCRATCH, "tree")
big = os.path.join(SCRATCH, "big")


def write(rel, text):
    path = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def lists(main):
    """The visible list views of the window: the snapshot list, then the difference list shown."""
    return visible_controls(main, "SysListView32")


def rows(h):
    return u32.SendMessageW(h, LVM_GETITEMCOUNT, 0, 0) if h else None


def focused_row(h):
    return u32.SendMessageW(h, LVM_GETNEXTITEM, -1, LVNI_FOCUSED) if h else None


def selected(h):
    return u32.SendMessageW(h, LVM_GETSELECTEDCOUNT, 0, 0) if h else None


def difference(main):
    shown = lists(main)
    return shown[-1] if len(shown) > 1 else None


def comparison_field(main):
    for h in visible_controls(main, "Edit"):
        if edit_style(h) & ES_MULTILINE:
            return h
    return None


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
    if h == comparison_field(main):
        return "comparison field"
    if h == filter_field(main):
        return "filter field"
    return class_name(h) if h else "nothing"


def report(app):
    d = difference(app.main)
    field = comparison_field(app.main)
    print(f"   focus: {where(app.main)} | difference rows: {rows(d)} (focused {focused_row(d)}, "
          f"selected {selected(d)}) | title: {foreground_title()}")
    if field:
        for line in control_text(field).splitlines():
            print(f"   field: {line}")


def finished(app, timeout=120):
    """Waits for the progress row, if it comes, to go."""
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


def menu(*letters):
    """Opens a menu of the menu bar by its letters with Alt, as the keyboard does."""
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


def timed(act, done, timeout=10):
    """Seconds from `act` until `done()` holds. The window answers WM_GETTEXT and the list's
    messages only between its events, so a field that holds the typed text has filtered."""
    started = time.time()
    act()
    assert wait_for(done, timeout, 0.002), "the window did not answer"
    return time.time() - started


write("a.txt", "a")
write("b.txt", "b")
write(r"old\gone.txt", "gone")
write(r"sub\c.txt", "c")


def check_small(app):
    app.step("start: Small, without snapshots", 2.5)
    report(app)

    # 1. Before the first comparison (#11): the field and the empty list; Tab skips the
    # filter's controls, which wait for a difference (#2).
    for i in range(3):
        key(VK_TAB)
        app.step(f"Tab {i + 1}", 1.5)
    key(VK_SHIFT, VK_TAB)
    app.step("Shift+Tab", 1.5)
    # The filter's commands say why they do not run (#30).
    key(VK_CONTROL, vk("F"))
    app.step("Ctrl+F without a difference", 1.5)
    key(VK_CONTROL, vk("1"))
    app.step("Ctrl+1 without a difference", 1.5)
    key(VK_MENU, vk("L"))
    app.step("Alt+L without a difference", 1.5)

    # 2. Two snapshots, the first with a comment, and the files changed between them.
    key(VK_F6)
    time.sleep(0.5)
    key(VK_F5)
    finished(app)
    app.step("F5: the first snapshot", 1.5)
    key(VK_F2)
    time.sleep(1)
    type_text("перед встановленням")
    time.sleep(0.5)
    key(VK_RETURN)
    app.step("F2: the comment", 1.5)
    write("a.txt", "aaaa")
    write("new.txt", "new")
    write(r"dir\inside.txt", "inside")
    os.remove(os.path.join(tree, "b.txt"))
    shutil.rmtree(os.path.join(tree, "old"))
    key(VK_F5)
    finished(app)
    app.step("F5: the second snapshot", 1.5)

    # 3. F7 from the snapshot list: the focus goes to the difference, named by its summary,
    # then its first row (#3); the rows, column by column (#12–#14).
    key(VK_F7)
    finished(app)
    app.step("F7: the difference", 2.5)
    report(app)
    for i in range(3):
        key(VK_DOWN)
        app.step(f"Down {i + 1}", 1.5)

    # 4. The field «Порівняння», line by line (#1, #4).
    key(VK_SHIFT, VK_TAB)
    app.step("Shift+Tab: the field", 2)
    key(VK_DOWN)
    app.step("Down: the summary line", 1.5)
    key(VK_TAB)
    app.step("Tab: the list again", 2)

    # 5. The text filter (#2, #24–#26): typing, the pause, Esc.
    key(VK_TAB)
    app.step("Tab: the field «Фільтр:»", 1.5)
    type_text("txt")
    app.step("typed «txt», then the pause", 2.5)
    report(app)
    key(VK_ESCAPE)
    app.step("Esc", 1.5)
    report(app)

    # 6. A check box and its menu item (#22, #26), a type's key from a check box (#30).
    key(VK_TAB)
    app.step("Tab: the first check box", 1.5)
    key(VK_SPACE)
    app.step("Space: added hidden", 1.5)
    report(app)
    menu("D")
    for i in range(5):
        key(VK_DOWN)
        app.step(f"menu Різниця, Down {i + 1}", 1)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    time.sleep(0.5)
    key(VK_CONTROL, vk("1"))
    app.step("Ctrl+1: added shown again", 1.5)
    key(VK_CONTROL, vk("2"))
    app.step("Ctrl+2: deleted hidden", 1.5)
    report(app)
    key(VK_CONTROL, vk("2"))
    app.step("Ctrl+2: deleted shown again", 1.5)

    # 7. Sorting from the menu: the focus stays on its change (#21); one row selected (#19).
    to_difference_list(app)
    app.step("F6: the list", 2)
    key(VK_DOWN)
    key(VK_DOWN)
    app.step("Down, Down", 1.5)
    menu("D", "S", "N")
    app.step("Sort by name", 2)
    report(app)
    menu("D", "S", "E")
    app.step("Descending", 2)
    report(app)
    menu("D", "S", "A")
    menu("D", "S", "T")
    app.step("Ascending, by path", 2)
    key(VK_CONTROL, vk("A"))
    app.step("Ctrl+A", 1)
    report(app)

    # 8. A new comparison while the focus is in the list: named again (#3, TASK-11 #21).
    key(VK_F7)
    finished(app)
    app.step("F7 with the focus in the list", 2.5)

    # 9. The earlier snapshot deleted: the pair says so (#5).
    key(VK_F6)
    time.sleep(0.5)
    key(VK_HOME)
    time.sleep(0.5)
    key(VK_DELETE)
    time.sleep(1)
    key(vk("D"))  # «Видалити», Alt released
    finished(app)
    app.step("the earlier snapshot deleted", 3)
    report(app)

    # 10. The setting «Розмір»: «У байтах» (#17).
    menu("F", "S")
    assert wait_for(lambda: foreground_title() == "Налаштування", 5, 0.05), "no settings"
    time.sleep(0.5)
    key(VK_MENU, vk("S"))
    app.step("Alt+S in the settings", 1.5)
    key(VK_DOWN)
    key(VK_DOWN)
    app.step("Down, Down", 1.5)
    key(VK_RETURN)
    time.sleep(1)
    to_difference_list(app)
    app.step("the list, sizes in bytes", 2)
    report(app)

    # Left for the restart: deleted hidden, by size descending.
    key(VK_CONTROL, vk("2"))
    menu("D", "S", "S")
    menu("D", "S", "E")
    app.step("deleted hidden, by size, descending", 2)


def check_restart(app):
    # #28: the types and the order kept; #11: nothing compared yet.
    app.step("restarted", 2.5)
    report(app)
    menu("D")
    for i in range(5):
        key(VK_DOWN)
        app.step(f"menu Різниця, Down {i + 1}", 1)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    time.sleep(0.5)
    menu("D", "S")
    for i in range(9):
        key(VK_DOWN)
        app.step(f"Sort by, Down {i + 1}", 1)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    time.sleep(0.5)
    # The settings back to the defaults, for the next runs.
    key(VK_CONTROL, vk("2"))
    menu("D", "S", "A")
    menu("D", "S", "T")
    app.step("Ctrl+2, ascending, by path", 2)


def check_big(app):
    # #9, #29: a folder missing in the first snapshot and there in the second.
    app.step("start: Big", 2.5)
    key(VK_F5)
    finished(app, 600)
    app.step("F5: the folder missing", 1.5)
    subprocess.run(["cmd", "/c", "mklink", "/J", big, big_source], check=True,
                   capture_output=True)
    key(VK_F5)
    finished(app, 600)
    app.step("F5: the folder there", 2)
    key(VK_F7)
    finished(app, 600)
    app.step("F7: the difference of every entry", 3)
    report(app)
    d = difference(app.main)
    total = rows(d)
    key(VK_TAB)
    time.sleep(0.5)
    field = filter_field(app.main)
    assert where(app.main) == "filter field", where(app.main)
    typed = ""
    for letter in "cargo":
        typed += letter
        took = timed(lambda: type_text(letter), lambda: control_text(field) == typed, 5)
        print(f"   «{typed}»: {took:.3f} s, {rows(d)} of {total} rows")
    app.step("typed «cargo»", 2)
    took = timed(lambda: key(VK_ESCAPE), lambda: rows(d) == total)
    print(f"   Esc: {took:.3f} s")
    app.step("Esc", 1.5)
    took = timed(lambda: key(VK_CONTROL, vk("1")), lambda: rows(d) == 0)
    print(f"   Ctrl+1 hides every added row: {took:.3f} s")
    key(VK_CONTROL, vk("1"))
    time.sleep(1)
    to_difference_list(app)
    menu("D", "S", "N")
    app.step("sorted by name", 2)
    menu("D", "S", "T")
    app.step("by path", 2)


try:
    if "small" in parts or "restart" in parts:
        paths = prepare({"Small": ([tree], [])})
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
        with App(paths, paths["Big"]) as app:
            check_big(app)
finally:
    if os.path.lexists(big):
        os.rmdir(big)
