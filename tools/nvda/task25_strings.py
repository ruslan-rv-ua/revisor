"""Live NVDA check of TASK-25: the strings the review of the proofreading changed.

    python -X utf8 tools\\nvda\\task25_strings.py uk|en

The settings file of the scratch folder holds the language. The folder of profiles has two
files that are no profiles: «Junk.db», not a database, says so with its advice after a dash,
and «Damaged.db», whose profile name is not text, says the error was unexpected. On the
profile «Gone», whose folder is on a drive letter no volume has, a snapshot names the reason
«диск теки не підключено», and a comparison of two such snapshots warns with it. On «Small»
the script closes a subfolder to listing between two snapshots (icacls, opened again at the
end), so the details of that folder say where it was inaccessible, after a dash. The
«Keyboard shortcuts» window names the key of the context menu.
"""

import ctypes
import os
import sqlite3
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    ES_MULTILINE, ES_READONLY, SCRATCH, VK_DOWN, VK_ESCAPE, VK_F1, VK_F5, VK_F7, VK_MENU,
    VK_RETURN, VK_TAB, App, control_text, edit_style, focused_class, focused_window,
    foreground_title, item_count, key, make_profile, make_settings, prepare, snapshot_list, vk,
    visible_controls, wait_for,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), LANG


def missing_drive():
    """A drive letter no volume has, as a root: Z:\\, or lower when Z: is taken."""
    used = ctypes.windll.kernel32.GetLogicalDrives()
    letter = next(chr(c) for c in range(ord("Z"), ord("A") - 1, -1)
                  if not used & (1 << (c - ord("A"))))
    return f"{letter}:\\"


tree = os.path.join(SCRATCH, "tree")
closed = os.path.join(tree, "closed")
gone = missing_drive() + ("Фото" if LANG == "uk" else "Photos")
paths = prepare({"Gone": ([gone], []), "Small": ([tree], [])})
scratch = os.path.dirname(paths["exe"])
make_settings(scratch, language=LANG)
os.makedirs(closed)
for rel in ("a.txt", r"closed\b.txt"):
    with open(os.path.join(tree, rel), "w", encoding="utf-8") as f:
        f.write(rel)
profiles = os.path.join(scratch, "data", "profiles")
with open(os.path.join(profiles, "Junk.db"), "w", encoding="utf-8") as f:
    f.write("not a database at all")
damaged = make_profile(scratch, "Damaged", [tree])
conn = sqlite3.connect(damaged)
conn.execute("UPDATE profile SET name = X'4142' WHERE id = 1")
conn.commit()
conn.close()

EXPECT = {
    "uk": {
        "junk": "«Junk.db» не вдалося відкрити: це не файл профілю revisor — перенесіть його "
                "з теки профілів.",
        "damaged": "«Damaged.db» не вдалося відкрити: сталася непередбачена помилка (Invalid "
                   "column type",
        # The log writes speech as Python's repr, a path's \ doubled, so no path here.
        "no_drive": "(диск теки не підключено).",
        "warning": f"Теку «{gone}» не вдалося прочитати в ранішому знімку (диск теки не "
                   "підключено), тож її вміст не порівнювали.",
        "details": "Недоступно: у пізнішому знімку — вміст теки не вдалося прочитати.",
        "shortcut": "Контекстне меню — Shift+F10, Клавіша контекстного меню",
        "take_snapshot": "Зробити знімок",
    },
    "en": {
        "junk": "“Junk.db” cannot be opened: this is not a revisor profile file—move it out of "
                "the folder of profiles.",
        "damaged": "“Damaged.db” cannot be opened: an unexpected error occurred (Invalid column "
                   "type",
        "no_drive": "(the folder's drive is not connected).",
        "warning": f"The folder “{gone}” could not be read in the earlier snapshot (the "
                   "folder's drive is not connected), so its contents were not compared.",
        "details": "Inaccessible: in the later snapshot—the contents of the folder could not be "
                   "read.",
        "shortcut": "Context menu — Shift+F10, Application key",
        "take_snapshot": "Take snapshot",
    },
}[LANG]

failures = []


def check(what, where, text, expected):
    """Note `what` as missing unless `text` — what NVDA said, or a window's text — has
    `expected`."""
    found = expected in text
    print(f"   {'OK' if found else 'MISSING'} {where}: {expected}")
    if not found:
        failures.append(what)


def heard(lines):
    return "\n".join(lines)


def comparison_field(main):
    """The field «Порівняння»: the one multiline read-only field of the main window."""
    for h in visible_controls(main, "Edit"):
        style = edit_style(h)
        if style & ES_READONLY and style & ES_MULTILINE:
            return h
    return None


def snapshot(app, count, name):
    """F5, then wait until the list has `count` snapshots; returns what NVDA said."""
    key(VK_F5)
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == count, 60), \
        f"snapshot {count} was not saved"
    return app.step(name, 2.5)


def icacls(*args):
    subprocess.run(["icacls", closed, *args], check=True, capture_output=True)


with App(paths, paths["Gone"]) as app:
    spoken = heard(app.step(f"start ({LANG}) on «Gone»", 3))
    # The message may come before NVDA is in the window: NVDA+End says it again.
    spoken += heard(app.repeat_last_message())
    check("not a profile", "heard", spoken, EXPECT["junk"])
    check("damaged profile", "heard", spoken, EXPECT["damaged"])

    check("no drive", "heard", heard(snapshot(app, 1, "F5: a folder on no drive")),
          EXPECT["no_drive"])
    snapshot(app, 2, "F5 again")
    key(VK_F7)
    app.step("F7: compare", 3)
    check("warning", "in the field «Порівняння»", control_text(comparison_field(app.main)),
          EXPECT["warning"])

    key(VK_F1)
    app.step("F1: keyboard shortcuts", 2)
    check("shortcuts", "in the window text", control_text(focused_window()), EXPECT["shortcut"])
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

    # To «Small»: the profile box lists «Gone» and then «Small».
    key(VK_MENU, vk("R"))
    time.sleep(0.5)
    key(VK_DOWN)
    app.step("Alt+R, Down: «Small»", 2)
    key(VK_TAB)
    assert wait_for(lambda: focused_class() == "SysListView32", 3), "not in the snapshot list"
    snapshot(app, 1, "F5: «Small»")
    icacls("/deny", f"{os.environ['USERNAME']}:(RD)")
    try:
        key(VK_F5)
        # Rights closed a folder, so revisor offers to restart as administrator.
        assert wait_for(lambda: foreground_title() == EXPECT["take_snapshot"], 60), \
            "no restart question"
        app.step("F5 with the folder closed", 1.5)
        key(VK_ESCAPE)
        assert wait_for(lambda: item_count(snapshot_list(app.main)) == 2, 10), \
            "snapshot 2 was not saved"
        app.step("Esc", 2)
        key(VK_F7)
        app.step("F7: compare, the focus in the difference list", 3)
        key(VK_MENU, VK_RETURN)
        app.step("Alt+Enter: details of the closed folder", 2)
        check("details", "in the window text", control_text(focused_window()),
              EXPECT["details"])
        key(VK_ESCAPE)
        app.step("Esc", 1.5)
    finally:
        icacls("/remove:d", os.environ["USERNAME"])

print("\nall heard" if not failures else f"\nMISSING: {failures}")
