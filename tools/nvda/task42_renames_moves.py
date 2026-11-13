"""Live NVDA check of TASK-42: the difference recognises renamed and moved entries.

    python -X utf8 tools\\nvda\\task42_renames_moves.py <uk|en> [parts]

``parts`` is a comma-separated choice of ``small`` and ``upgrade`` (both by default).

«Small» has two folders of a scratch folder, ``tree`` and ``other``, on one volume. Between
two snapshots the script renames a file, renames one in case only, moves one to another folder
and writes to it, moves one to the other folder of the profile, renames a folder that holds
files — and in it renames a file, deletes one and adds one — and saves a file with a backup:
the file takes the backup's name and a new one takes its own.

Part ``small``: the summary names the renamed or moved after the modified (#10); every row
says what happened and where the entry was (#1, #4–#7); the details have the path before and
after, or the name where only its case changed (#8); Ctrl+4 and Alt+V show and hide the
renamed and moved, Ctrl+5 the inaccessible, the check boxes follow, and Ctrl+4 works from the
field «Фільтр:» (#11); the field finds an entry by the name it had (#9); in the tree the node
of a renamed folder stands where the folder is now, with what changed inside it (#4, #9). Part
``upgrade``: settings stored by 0.3 with other types hidden start with the renamed and moved
shown (#11). The last line says ``all heard`` or names what is missing.

The state of a check box is what NVDA says of it when the script gives it the focus: in the
dark theme wxWidgets draws the check boxes itself and tells screen readers their state through
its own accessible object, so ``BM_GETCHECK`` answers 0 whatever the state is.
"""

import ctypes
import os
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    ES_MULTILINE, ES_READONLY, SCRATCH, VK_DOWN, VK_ESCAPE, VK_F5, VK_HOME, VK_MENU, VK_RETURN,
    VK_RIGHT, VK_UP, App, children, class_name, control_text, edit_style, focused_class,
    focused_window, foreground_title, item_count, key, make_settings, prepare, snapshot_list,
    type_text, u32, visible_controls, vk, wait_for,
)

VK_CONTROL, VK_F6, VK_F7 = 0x11, 0x75, 0x76
# NVDA says the state of a check box in its own language, whatever revisor speaks.
CHECKED, UNCHECKED = ("позначено", "checked"), ("не позначено", "not checked")

lang = sys.argv[1] if len(sys.argv) > 1 else "uk"
parts = set(sys.argv[2].split(",")) if len(sys.argv) > 2 else {"small", "upgrade"}
tree = os.path.join(SCRATCH, "tree")
other = os.path.join(SCRATCH, "other")

WORDS = {
    "uk": {
        "summary": "перейменовано чи переміщено 7",
        "renamed": "Перейменовано",
        "moved": "Переміщено",
        "folder_renamed": "Перейменовано теку",
        "replaced": "Замінено",
        "deleted": "Видалено",
        "added": "Додано",
        "from": "з ",
        "fields": "розмір, дата зміни",
        "showing": "Показано {} із 10.",
        "box": "Перейменовані чи переміщені",
        "inaccessible_box": "Недоступні",
        "path_line": "Шлях: було ",
        "after": ", стало ",
        "name_line": "Ім'я: було k.txt, стало K.TXT",
        "change_renamed": "Зміна: перейменовано",
        "change_moved": "Зміна: переміщено; змінилося: розмір, дата зміни",
        "node_file": "b.txt — перейменовано: з a.txt",
        "node_folder": "Pictures — перейменовано теку: з Photos; 3 зміни",
        "node_inside": "first.jpg — перейменовано: з p1.jpg",
        "node_deleted": "p2.jpg — видалено",
    },
    "en": {
        "summary": "7 renamed or moved",
        "renamed": "Renamed",
        "moved": "Moved",
        "folder_renamed": "Folder renamed",
        "replaced": "Replaced",
        "deleted": "Deleted",
        "added": "Added",
        "from": "from ",
        "fields": "size, date modified",
        "showing": "Showing {} of 10.",
        "box": "Renamed or moved",
        "inaccessible_box": "Inaccessible",
        "path_line": "Path: before ",
        "after": ", after ",
        "name_line": "Name: before k.txt, after K.TXT",
        "change_renamed": "Change: renamed",
        "change_moved": "Change: moved; what changed: size, date modified",
        "node_file": "b.txt — renamed: from a.txt",
        "node_folder": "Pictures — folder renamed: from Photos; 3 changes",
        "node_inside": "first.jpg — renamed: from p1.jpg",
        "node_deleted": "p2.jpg — deleted",
    },
}[lang]

# The check boxes of the types, in their order.
BOXES = {
    "uk": ["Додані", "Видалені", "Змінені", "Перейменовані чи переміщені", "Недоступні"],
    "en": ["Added", "Deleted", "Modified", "Renamed or moved", "Inaccessible"],
}[lang]

missing = []


def check(what, found):
    print(f"   {'OK' if found else 'MISSING'}: {what}")
    if not found:
        missing.append(what)


def said(lines, text):
    return text in " ".join(lines)


def spoken(path):
    """A path as the speech log writes it: the log holds Python reprs, with `\\` doubled."""
    return path.replace("\\", "\\\\")


def write(root, rel, text):
    full = os.path.join(root, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


def fill():
    for rel, text in [
        ("a.txt", "a"),
        ("notes.txt", "notes"),
        ("report.doc", "the first version"),
        (r"keep\k.txt", "k"),
        (r"docs\one.txt", "1"),
        (r"docs\two.txt", "2"),
        (r"Photos\p1.jpg", "1"),
        (r"Photos\p2.jpg", "2"),
    ]:
        write(tree, rel, text)
    write(other, "o.txt", "o")


def change():
    """Ten rows: seven renamed or moved, one deleted, one added, one replaced."""
    rename = lambda was, now: os.rename(os.path.join(tree, was), os.path.join(tree, now))  # noqa: E731
    rename("a.txt", "b.txt")
    rename(r"keep\k.txt", r"keep\K.TXT")
    rename("notes.txt", r"docs\notes.txt")
    write(tree, r"docs\notes.txt", "notes, and more of them")
    os.rename(os.path.join(tree, r"docs\two.txt"), os.path.join(other, "two.txt"))
    rename("Photos", "Pictures")
    rename(r"Pictures\p1.jpg", r"Pictures\first.jpg")
    os.remove(os.path.join(tree, r"Pictures\p2.jpg"))
    write(tree, r"Pictures\p3.jpg", "3")
    rename("report.doc", "report.bak")
    write(tree, "report.doc", "the second version, longer")


def difference_list(main):
    shown = visible_controls(main, "SysListView32")
    return shown[1] if len(shown) > 1 else None


def filter_field(main):
    for h in visible_controls(main, "Edit"):
        if not edit_style(h) & (ES_MULTILINE | ES_READONLY):
            return h
    return None


def box(main, words):
    """The check box of the main window whose label starts with `words`."""
    for h in children(main):
        if class_name(h) == "Button" and control_text(h).replace("&", "").startswith(words):
            return h
    return None


def focus(h):
    """Gives the window `h` of revisor the focus, as a click on it would."""
    target = u32.GetWindowThreadProcessId(h, None)
    me = ctypes.windll.kernel32.GetCurrentThreadId()
    u32.AttachThreadInput(me, target, True)
    u32.SetFocus(h)
    u32.AttachThreadInput(me, target, False)


def checked(app, words):
    """Whether NVDA says that the check box whose label starts with `words` is checked, once
    the box gets the focus; None when it says neither."""
    h = box(app.main, words)
    if not h:
        return None
    if focused_window() == h:
        # Already there: NVDA names a control when the focus comes to it.
        focus(difference_list(app.main) or snapshot_list(app.main))
        time.sleep(0.8)
    app.speech.new()
    focus(h)
    heard = " ".join(app.step(f"the focus on the check box «{words}»", 1.3))
    if any(word in heard for word in UNCHECKED):
        return False
    return True if any(word in heard for word in CHECKED) else None


def snapshots(app, count):
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == count, 120), "no snapshot"
    assert wait_for(app.snapshot_done, 120, 0.02), "the progress row never went"


def compared(app, timeout=120):
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


def to_main_control(app, cls):
    for _ in range(3):
        if focused_class() == cls and focused_window() != snapshot_list(app.main):
            return
        key(VK_F6)
        time.sleep(0.7)
    assert focused_class() == cls, focused_class()


def details(app, name):
    """Opens the details of the row in focus, prints and returns their text, and closes them."""
    key(VK_RETURN)
    app.step(f"Enter: the details of {name}", 2)
    title = foreground_title()
    text = control_text(focused_window())
    print(f"   title: {title}\n   " + "\n   ".join(text.splitlines()[:4]))
    key(VK_ESCAPE)
    app.step("Esc", 1.5)
    return text


def check_small(app):
    app.step("start: Small", 2.5)
    key(VK_F5)
    snapshots(app, 1)
    app.step("F5: the snapshot before", 1.5)
    change()
    time.sleep(1)
    key(VK_F5)
    snapshots(app, 2)
    app.step("F5: the snapshot after", 1.5)

    # 1. The difference: the summary, and every row with what happened and where it was.
    key(VK_F7)
    compared(app)
    heard = app.step("F7: the difference", 3)
    check("the list has ten rows", item_count(difference_list(app.main)) == 10)
    check("the summary names the renamed or moved (#10)", said(heard, WORDS["summary"]))
    rows = [heard]
    for i in range(9):
        key(VK_DOWN)
        rows.append(app.step(f"Down {i + 1}", 1.4))
    two, b, notes, k, pictures, first, p2, p3, bak, report = rows
    docs = os.path.join(tree, "docs")
    check("moved between the folders of the profile, from its folder (#1)",
          said(two, "two.txt") and said(two, WORDS["moved"])
          and said(two, spoken(WORDS["from"] + docs)))
    check("renamed, from its name (#6)",
          said(b, "b.txt") and said(b, WORDS["renamed"]) and said(b, WORDS["from"] + "a.txt"))
    check("moved and modified: the folder, then the other fields (#7)",
          said(notes, WORDS["moved"]) and said(notes, spoken(WORDS["from"] + tree) + "; ")
          and said(notes, WORDS["fields"]))
    check("renamed in case only (#6)",
          said(k, "K.TXT") and said(k, WORDS["renamed"]) and said(k, WORDS["from"] + "k.txt"))
    check("a folder renamed is one row (#4)",
          said(pictures, WORDS["folder_renamed"]) and said(pictures, WORDS["from"] + "Photos"))
    check("renamed inside the renamed folder, not moved (#4)",
          said(first, WORDS["renamed"]) and said(first, WORDS["from"] + "p1.jpg")
          and not said(first, WORDS["moved"]))
    check("deleted and added inside it (#4)",
          said(p2, "p2.jpg") and said(p2, WORDS["deleted"])
          and said(p3, "p3.jpg") and said(p3, WORDS["added"]))
    check("the file that left is renamed, its old place replaced (#5)",
          said(bak, "report.bak") and said(bak, WORDS["from"] + "report.doc")
          and said(report, "report.doc") and said(report, WORDS["replaced"]))

    # 2. The details (#8): the path before and after; the name where only its case changed.
    key(VK_UP)
    app.step("Up: report.bak", 1.4)
    text = details(app, "report.bak")
    was, now = os.path.join(tree, "report.doc"), os.path.join(tree, "report.bak")
    check("the details have the path before and after",
          text.startswith(WORDS["path_line"] + was + WORDS["after"] + now))
    check("and the change", WORDS["change_renamed"] in text)
    for _ in range(5):
        key(VK_UP)
        time.sleep(0.4)
    app.step("Up ×5: K.TXT", 1.5)
    text = details(app, "K.TXT")
    check("a name of another case has its own line, and the path as it is",
          WORDS["name_line"] in text and WORDS["after"] not in text.splitlines()[0])
    key(VK_UP)
    app.step("Up: notes.txt", 1.4)
    text = details(app, "notes.txt")
    check("moved and modified: what else changed", WORDS["change_moved"] in text)

    # 3. The fourth type (#11): Ctrl+4, the check box and its letter, Ctrl+5.
    showing = WORDS["showing"].format
    key(VK_CONTROL, vk("4"))
    lines = app.step("Ctrl+4: the renamed and moved hidden", 2)
    check("Ctrl+4 hides them and says how many rows show", said(lines, showing(3)))
    check("the check box follows", checked(app, WORDS["box"]) is False)
    key(VK_MENU, vk("D"))
    time.sleep(0.6)
    heard = []
    for i in range(7):
        key(VK_DOWN)
        heard += app.step(f"menu, Down {i + 1}", 1)
    check("the menu item stands with Ctrl+4, and the inaccessible with Ctrl+5",
          said(heard, WORDS["box"]) and said(heard, "Ctrl+4") and said(heard, "Ctrl+5"))
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    time.sleep(0.5)
    key(VK_CONTROL, vk("4"))
    lines = app.step("Ctrl+4: shown again", 2)
    check("Ctrl+4 shows them again",
          said(lines, showing(10)) and checked(app, WORDS["box"]) is True)
    key(VK_MENU, vk("V"))
    lines = app.step("Alt+V: the check box", 2)
    check("Alt+V presses the check box",
          said(lines, showing(3)) and checked(app, WORDS["box"]) is False)
    key(VK_MENU, vk("V"))
    lines = app.step("Alt+V again", 2)
    check("and again", said(lines, showing(10)))
    key(VK_CONTROL, vk("5"))
    app.step("Ctrl+5: the inaccessible hidden", 2)
    check("Ctrl+5 is the inaccessible now",
          checked(app, WORDS["inaccessible_box"]) is False
          and checked(app, WORDS["box"]) is True)
    key(VK_CONTROL, vk("5"))
    app.step("Ctrl+5 again", 2)

    # 4. From the field «Фільтр:»: the keys work, and the text finds the name an entry had.
    key(VK_CONTROL, vk("F"))
    time.sleep(0.5)
    key(VK_CONTROL, vk("4"))
    lines = app.step("Ctrl+F, Ctrl+4 in the field", 2)
    check("Ctrl+4 works from the field", said(lines, showing(3)))
    key(VK_CONTROL, vk("4"))
    app.step("Ctrl+4 again", 2)
    type_text("a.txt")
    lines = app.step("typed «a.txt», the pause", 2.5)
    check("the text finds an entry by the name it had (#9)", said(lines, showing(1)))
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

    # 5. The tree (#9): the node of a renamed folder is where the folder is now.
    to_main_control(app, "SysListView32")
    key(VK_HOME)
    app.step("F6, Home: the first row", 1.5)
    key(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the tree", 2.5)
    check("Ctrl+T moves the focus to the tree", focused_class() == "SysTreeView32")
    key(VK_DOWN)
    app.step("Down: the folder «tree»", 1.5)
    key(VK_RIGHT)
    app.step("Right: expanded", 1.5)
    heard = []
    for i in range(4):
        key(VK_DOWN)
        heard += app.step(f"Down {i + 1}", 1.4)
    check("a renamed file says where it was", said(heard, WORDS["node_file"]))
    check("a renamed folder is one node with what changed inside it",
          said(heard, WORDS["node_folder"]))
    key(VK_RIGHT)
    app.step("Right: the folder opens", 1.5)
    inside = []
    for i in range(2):
        key(VK_DOWN)
        inside += app.step(f"Down {i + 1}", 1.4)
    check("inside it: the file renamed there, and the one deleted there",
          said(inside, WORDS["node_inside"]) and said(inside, WORDS["node_deleted"]))
    key(VK_CONTROL, vk("T"))
    app.step("Ctrl+T: the list again", 2)


def check_upgrade(app):
    app.step("start: settings of 0.3 with only the added and the modified shown", 2.5)
    key(VK_F5)
    snapshots(app, 1)
    app.step("F5: the snapshot before", 1.5)
    os.rename(os.path.join(tree, "a.txt"), os.path.join(tree, "b.txt"))
    time.sleep(1)
    key(VK_F5)
    snapshots(app, 2)
    app.step("F5: the snapshot after", 1.5)
    key(VK_F7)
    compared(app)
    lines = app.step("F7: the difference", 3)
    check("the renamed file shows, though the user never chose to show its type",
          said(lines, "b.txt") and said(lines, WORDS["renamed"]))
    states = [checked(app, words) for words in BOXES]
    print(f"   check boxes: {states}")
    check("the renamed or moved are shown, the other types as they were stored (#11)",
          states == [True, False, True, True, False])


try:
    if "small" in parts:
        paths = prepare({"Small": ([tree, other], [])})
        make_settings(os.path.dirname(paths["exe"]), language=lang)
        fill()
        with App(paths, paths["Small"]) as app:
            check_small(app)
    if "upgrade" in parts:
        paths = prepare({"Small": ([tree, other], [])})
        make_settings(os.path.dirname(paths["exe"]), language=lang,
                      difference_types="added,modified")
        fill()
        with App(paths, paths["Small"]) as app:
            check_upgrade(app)
finally:
    for folder in (tree, other):
        if os.path.exists(folder):
            shutil.rmtree(folder, ignore_errors=True)

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
