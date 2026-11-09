"""Live NVDA check of TASK-30: access keys, read from the English names, and the one dispatcher.

    python -X utf8 tools\\nvda\\task30_access_keys.py uk|en

The language goes into the settings file of the scratch folder. The steps, each with the
Ukrainian layout on in the main window so the keys work by their position:

- main window: Alt+R (profile box), Alt+M (field «Порівняння»), Alt+L before a difference (why
  not), Alt+F opens the menu «Файл»; after «Знімок + порівняння» Alt+E presses the check box
  «Змінені» once Alt is up, and F6 after it leads to the snapshot list, so no key is lost in
  the window menu;
  during a snapshot of C:\\Windows Alt+N presses «Скасувати (N)»;
- profile window (Редагувати профіль…): Alt+F, Alt+E, Alt+N move the focus; Alt+A opens the
  folder picker, Esc closes it, and Alt+R removes the folder;
- settings: Alt+F moves the focus, Alt+C toggles the check box twice;
- «Порівняти з…»: from its button, Alt+W puts the focus back in the list;
- «Коментар»: Alt+C puts the focus in the field;
- a question: Delete in the snapshot list asks «Видалити знімки», Alt+D deletes, and Alt+R
  after it is heard.

The last line is ``all heard`` or what NVDA did not say.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_DELETE, VK_ESCAPE, VK_F2, VK_F5, VK_F6, VK_F7, VK_MENU, App, foreground_title,
    key, make_settings, prepare, ukrainian_layout, vk, wait_for,
)

VK_CONTROL = 0x11
LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"

# What NVDA should say, by language.
WORDS = {
    "uk": {
        "profile": "Профіль", "comparison": "Порівняння", "no_difference": "різниці ще немає",
        "file_menu": "Файл", "modified": "Змінені", "snapshots": "Знімки", "cancelled": "скасовано",
        "edit": "Редагувати профіль", "folders": "Теки", "exclusions": "Винятки", "name": "Назва",
        "picker": "Виберіть теку", "add": "Додати теку", "settings": "Налаштування",
        "focus": "Фокус після знімка", "confirm": "Підтверджувати видалення знімків",
        "compare_with": "Порівняти з", "compare_label": "зі знімком (W)", "comment": "Коментар до знімка",
        "delete_question": "Видалити знімки",
    },
    "en": {
        "profile": "Profile", "comparison": "Comparison", "no_difference": "no difference yet",
        "file_menu": "File", "modified": "Modified", "snapshots": "Snapshots", "cancelled": "anceled",
        "edit": "Edit profile", "folders": "Folders", "exclusions": "Exclusions", "name": "Name",
        "picker": "Choose a folder", "add": "Add folder", "settings": "Settings",
        "focus": "Focus after a snapshot", "confirm": "Confirm snapshot deletion",
        "compare_with": "Compare with", "compare_label": "with:", "comment": "Comment on the snapshot",
        "delete_question": "Delete snapshots",
    },
}[LANG]
missing = []


def heard(lines, *names):
    """Note each word of `names` (keys of WORDS) that no line of `lines` has."""
    text = " ".join(lines)
    for name in names:
        if WORDS[name] not in text:
            missing.append(WORDS[name])
            print(f"   !! not heard: {WORDS[name]!r}")


def alt(letter):
    key(VK_MENU, vk(letter))


tree = os.path.join(SCRATCH, "tree")
changed = os.path.join(tree, "changed.txt")


def write(text):
    os.makedirs(tree, exist_ok=True)
    with open(changed, "w", encoding="utf-8") as f:
        f.write(text)


paths = prepare({"Small": ([tree], []), "Windows": ([r"C:\Windows"], [])})
make_settings(SCRATCH, language=LANG)
write("before")

with App(paths, paths["Small"]) as app:
    app.step("start", 3)
    ukrainian_layout(app.main)
    app.step("Ukrainian layout on", 1)

    # --- the main window: a focus at once --------------------------------------------------
    alt("R")
    heard(app.step("Alt+R: the profile box", 1.5), "profile")
    alt("M")
    heard(app.step("Alt+M: the field «Порівняння»", 1.5), "comparison")
    alt("L")
    heard(app.step("Alt+L before a difference: why not", 1.5), "no_difference")
    alt("F")
    heard(app.step("Alt+F: the menu «Файл»", 1.5), "file_menu")
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    app.step("Esc, Esc: out of the menu", 1.5)

    # Two snapshots and a difference.
    key(VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.1), "snapshot 1 did not end"
    app.step("F5: snapshot 1", 2.5)
    write("after, longer")
    key(VK_CONTROL, VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.1), "«Знімок + порівняння» did not end"
    app.step("Ctrl+F5: snapshot 2 and its difference", 3)

    # --- the main window: a check box is pressed once Alt is up -------------------------------
    alt("E")
    heard(app.step("Alt+E: «Змінені» toggled and focused", 2), "modified")
    key(VK_F6)
    heard(app.step("F6 right after: the snapshot list, not lost in the window menu", 2), "snapshots")
    alt("E")
    app.step("Alt+E: «Змінені» back on", 2)

    # --- «Порівняти з…» -----------------------------------------------------------------------
    key(VK_CONTROL, VK_F7)
    heard(app.step("Ctrl+F7: «Порівняти з…»", 2), "compare_with")
    key(0x09)  # Tab: away from the list
    app.step("Tab: on «Порівняти»", 1)
    alt("W")
    heard(app.step("Alt+W: the focus in the list", 1.5), "compare_label")
    key(VK_ESCAPE)
    app.step("Esc: closed", 1.5)

    # --- «Коментар» -----------------------------------------------------------------------------
    key(VK_F2)
    heard(app.step("F2: «Коментар»", 2), "comment")
    key(0x09)  # Tab to OK
    app.step("Tab: away from the field", 1)
    alt("C")
    heard(app.step("Alt+C: back in the field", 1.5), "comment")
    key(VK_ESCAPE)
    app.step("Esc: closed", 1.5)

    # --- the profile window -------------------------------------------------------------------
    alt("F")
    time.sleep(0.4)
    key(vk("E"))
    heard(app.step("Alt+F, E: «Редагувати профіль…»", 2.5), "edit")
    alt("F")
    heard(app.step("Alt+F: the folders", 1.5), "folders")
    alt("E")
    heard(app.step("Alt+E: the exclusions", 1.5), "exclusions")
    alt("N")
    heard(app.step("Alt+N: the name", 1.5), "name")
    alt("A")
    heard(app.step("Alt+A: the folder picker, once Alt is up", 3), "picker")
    key(VK_ESCAPE)
    app.step("Esc: the picker closed", 2)
    alt("R")
    heard(app.step("Alt+R: the folder removed, the focus on «Додати теку»", 2), "add")
    key(VK_ESCAPE)
    app.step("Esc: the profile window closed, nothing saved", 2)

    # --- settings -----------------------------------------------------------------------------
    alt("F")
    time.sleep(0.4)
    key(vk("S"))
    heard(app.step("Alt+F, S: «Налаштування»", 2.5), "settings")
    alt("F")
    heard(app.step("Alt+F: «Фокус після знімка»", 1.5), "focus")
    alt("C")
    heard(app.step("Alt+C: the check box toggled", 2), "confirm")
    alt("C")
    app.step("Alt+C: toggled back", 2)
    key(VK_ESCAPE)
    app.step("Esc: closed, nothing saved", 2)

    # --- a question ---------------------------------------------------------------------------
    key(VK_F6)
    app.step("F6: the snapshot list", 1.5)
    key(VK_DELETE)
    heard(app.step("Delete: the question «Видалити знімки»", 2.5), "delete_question")
    alt("D")
    app.step("Alt+D: deleted once Alt is up", 3)
    alt("R")
    heard(app.step("Alt+R right after: not lost in the window menu", 2), "profile")

    # --- «Скасувати (N)» during a snapshot -------------------------------------------------------
    key(0x28)  # Down in the profile box: «Windows»
    app.step("Down: the profile Windows", 2.5)
    key(VK_F5)
    time.sleep(1.5)
    app.step("F5: a snapshot of C:\\Windows", 0.5)
    alt("N")
    heard(app.step("Alt+N: «Скасувати (N)», once Alt is up", 3), "cancelled")
    print("   foreground:", foreground_title())

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
