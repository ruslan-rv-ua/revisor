"""Live NVDA check of TASK-27: the snapshot list, now a module of its own, behaves as before.

    python -X utf8 tools\\nvda\\task27_snapshot_list.py [parts]

``parts`` is a comma-separated choice of ``empty`` and ``list`` (both by default).

``empty``: no profiles, then a profile without snapshots — what the empty list says in its name.
``list``: the profile «Small» (a scratch folder with a few files, so a snapshot takes a second):
F5 with the focus in the list and outside it, a comment, deleting one snapshot and then two,
each refused first and then agreed to, and «Порівняти з…» with the rows as the list shows them.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_F2, VK_F5, VK_F7, VK_MENU, VK_RETURN,
    VK_SHIFT, VK_TAB, VK_UP, App, focused_class, foreground_title, item_count, key, prepare,
    snapshot_list, type_text, vk, wait_for,
)

parts = set(sys.argv[1].split(",")) if len(sys.argv) > 1 else {"empty", "list"}
tree = os.path.join(SCRATCH, "tree")
missing = []


def heard(lines, *words):
    """Note each of `words` that no line of `lines` has."""
    text = " ".join(lines)
    for w in words:
        if w not in text:
            missing.append(w)
            print(f"   !! not heard: {w!r}")


def make_tree():
    os.makedirs(tree, exist_ok=True)
    for i in range(3):
        with open(os.path.join(tree, f"file{i}.txt"), "w", encoding="utf-8") as f:
            f.write(str(i))


def rows(app):
    return item_count(snapshot_list(app.main))


def snapshot(app, name):
    key(VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.1), "the snapshot did not end"
    return app.step(name, 2.5)


if "empty" in parts:
    paths = prepare({})
    with App(paths) as app:
        heard(app.step("start without profiles", 3), "Профілів ще немає")
    paths = prepare({"Small": ([SCRATCH], [])})
    with App(paths, paths["Small"]) as app:
        heard(app.step("start on a profile without snapshots", 3), "Знімків ще немає")

if "list" in parts:
    paths = prepare({"Small": ([tree], [])})
    make_tree()
    with App(paths, paths["Small"]) as app:
        app.step("start", 3)
        print("   focused:", focused_class())

        # F5 with the focus in the list: the new row is current and has the focus.
        for n in range(1, 5):
            lines = snapshot(app, f"F5 in the list: snapshot {n}")
            heard(lines, "Знімок збережено:")
            print("   rows:", rows(app), "| focused:", focused_class())

        # A comment on the current (newest) snapshot: the row says it.
        key(VK_F2)
        app.step("F2: the comment window", 2)
        type_text("перед оновленням")
        time.sleep(0.3)
        key(VK_RETURN)
        heard(app.step("Enter: comment saved", 2), "Коментар збережено")
        key(VK_UP)
        app.step("Up", 1.5)
        key(0x28)  # Down, back to the newest row
        heard(app.step("Down: the row with its comment", 1.5), "перед оновленням")

        # F5 with the focus outside the list (the profile box): «Фокус після знімка» is
        # «якщо в переліку», so the focus stays and the new row only becomes current.
        key(VK_MENU, vk("R"))
        app.step("Alt+R: the profile box", 1.5)
        lines = snapshot(app, "F5 outside the list: snapshot 5")
        heard(lines, "Знімок збережено:")
        print("   rows:", rows(app), "| focused:", focused_class())
        key(VK_TAB)
        app.step("Tab: back to the list, on the new current row", 2)
        print("   focused:", focused_class())

        # Delete one: refused, then agreed to. After the refusal the focus rectangle is back
        # on the same row; after the deletion it is on the row that followed.
        key(VK_UP)
        key(VK_UP)
        app.step("Up, Up: the second snapshot", 1.5)
        key(VK_DELETE)
        app.step("Delete: the question", 2)
        print("   foreground:", foreground_title())
        key(VK_ESCAPE)
        app.step("Esc: refused, the same row", 2)
        print("   rows:", rows(app))
        key(VK_DELETE)
        app.step("Delete again", 2)
        key(vk("D"))
        heard(app.step("D: deleted, the row that followed", 3), "Видалено")
        print("   rows:", rows(app))

        # Delete two: the current row and the one above it.
        key(VK_SHIFT, VK_UP)
        app.step("Shift+Up: two selected", 1.5)
        key(VK_DELETE)
        app.step("Delete: the question for two", 2)
        key(VK_ESCAPE)
        app.step("Esc: refused, the same row", 2)
        key(VK_DELETE)
        app.step("Delete again", 2)
        key(vk("D"))
        heard(app.step("D: two deleted", 3), "Видалено")
        print("   rows:", rows(app))
        assert wait_for(app.snapshot_done, 60, 0.1), "the compaction did not end"
        app.step("compaction ended", 2)

        # «Порівняти з…»: the other snapshot, its row as the list shows it.
        key(VK_CONTROL, VK_F7)
        app.step("Ctrl+F7: the window", 2.5)
        print("   foreground:", foreground_title())
        key(VK_ESCAPE)
        app.step("Esc: closed", 2)
        print("   focused:", focused_class())

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
