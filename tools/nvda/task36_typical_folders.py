"""Live NVDA check of TASK-36: typical folders in the profile window.

    python -X utf8 tools\\nvda\\task36_typical_folders.py uk|en

The language goes into the settings file of the scratch folder. The profile «Small» has the
folders ``C:\\Program Files\\Common Files`` (inside Program Files) and ``C:\\Users\\Public``
(which covers the public desktop). The steps, with the Ukrainian layout on:

- «Змінити профіль…», Alt+T: the list of typical folders opens with the focus on its first
  row; every row is checked (read through LVM_GETITEMSTATE) and the public desktop is not
  among them; the arrows read each row with its name, path and check state;
- Esc closes the list and adds nothing;
- Alt+T again, Space on «Windows» unchecks it, Enter adds the rest: Program Files replaces
  Common Files without a question, the focus is on Program Files in the folder list;
- Alt+T again: only Windows is left; Enter adds it;
- Alt+T once more: the message that all typical folders are in the profile (NVDA+B reads it);
- Esc closes the profile window without saving.

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_DOWN, VK_ESCAPE, VK_INSERT, VK_MENU, VK_RETURN, VK_SPACE, VK_UP, App, class_name,
    focused_window, foreground_title, item_count, key, make_settings, prepare, u32,
    ukrainian_layout, vk,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"

WORDS = {
    "uk": {
        "edit": "Змінити профіль", "title": "Додати типові теки", "folders": "Теки",
        "checked": "позначено", "unchecked": "не позначено", "checkbox": "прапорець",
        "program_files": "Program Files (C:\\Program Files)", "x86": "Program Files (x86)",
        "program_data": "ProgramData", "app_data": "AppData користувача", "windows": "Windows (",
        "desktop": "Робочий стіл користувача", "public": "Загальний робочий стіл",
        "added": "C:\\Program Files", "all_in": "Усі типові теки вже є в профілі",
    },
    "en": {
        "edit": "Edit profile", "title": "Add typical folders", "folders": "Folders",
        "checked": "позначено", "unchecked": "не позначено", "checkbox": "прапорець",
        "program_files": "Program Files (C:\\Program Files)", "x86": "Program Files (x86)",
        "program_data": "ProgramData", "app_data": "User AppData", "windows": "Windows (",
        "desktop": "User desktop", "public": "Public desktop",
        "added": "C:\\Program Files", "all_in": "All typical folders are in the profile already",
    },
}[LANG]
missing = []

LVIS_STATEIMAGEMASK, LVM_GETITEMSTATE, LB_GETCOUNT = 0xF000, 0x102C, 0x018B


def heard(lines, *names):
    """Note each word of `names` (keys of WORDS) that no line of `lines` has."""
    # The log writes a backslash doubled, as Python's repr does.
    text = " ".join(lines).replace("\\\\", "\\")
    for name in names:
        if WORDS[name] not in text:
            missing.append(WORDS[name])
            print(f"   !! not heard: {WORDS[name]!r}")
    return text


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)


def alt(letter):
    key(VK_MENU, vk(letter))


def checked_rows(h):
    return [(u32.SendMessageW(h, LVM_GETITEMSTATE, i, LVIS_STATEIMAGEMASK) >> 12) == 2
            for i in range(item_count(h))]


def folder_count():
    """How many folders the profile window lists: the focus is in that list box."""
    h = focused_window()
    return u32.SendMessageW(h, LB_GETCOUNT, 0, 0) if class_name(h) == "ListBox" else None


paths = prepare({"Small": ([r"C:\Program Files\Common Files", r"C:\Users\Public"], [])})
make_settings(SCRATCH, language=LANG)

with App(paths, paths["Small"]) as app:
    app.step("start", 3)
    ukrainian_layout(app.main)
    app.step("Ukrainian layout on", 1)

    alt("F")
    time.sleep(0.4)
    key(vk("E"))
    heard(app.step("Alt+F, E: «Змінити профіль…»", 2.5), "edit")

    # --- the list opens, all checked -----------------------------------------------------------
    alt("T")
    text = heard(app.step("Alt+T: the list of typical folders", 3),
                 "title", "folders", "program_files", "checkbox", "checked")
    rows = focused_window()
    check(class_name(rows) == "SysListView32", "the focus is in the list")
    states = checked_rows(rows)
    print("   rows checked:", states)
    check(len(states) == 6, "six rows: the public desktop is covered by C:\\Users\\Public")
    check(all(states), "every row is checked")
    walk = []
    for _ in range(len(states) - 1):
        key(VK_DOWN)
        walk += app.step("Down", 1.2)
    text = heard(walk, "x86", "program_data", "app_data", "windows", "desktop")
    check(WORDS["public"] not in text, "the public desktop is not offered")
    alt("F")
    app.step("Alt+F: the focus stays in the list (NVDA says nothing)", 1.2)
    check(focused_window() == rows, "Alt+F keeps the focus in the list")

    key(VK_ESCAPE)
    app.step("Esc: closed", 1.5)
    alt("F")
    app.step("Alt+F: the folder list", 1.2)
    check(folder_count() == 2, "Esc added nothing")

    # --- uncheck Windows, add the rest -----------------------------------------------------------
    alt("T")
    app.step("Alt+T: the list again", 2.5)
    for _ in range(4):
        key(VK_DOWN)
    heard(app.step("Down ×4: Windows", 1.5), "windows")
    key(VK_SPACE)
    heard(app.step("Space: unchecked", 1.5), "unchecked")
    check(foreground_title() == WORDS["title"], "Space leaves the list open")
    key(VK_UP)
    key(VK_DOWN)
    heard(app.step("Up, Down: Windows unchecked", 1.5), "windows", "unchecked")
    key(VK_RETURN)
    heard(app.step("Enter: added, the focus on Program Files", 2.5), "added")
    # Users\Public, then Program Files, Program Files (x86), ProgramData, AppData, Desktop;
    # Common Files went with no question.
    check(folder_count() == 6, f"six folders in the profile (got {folder_count()})")
    check(foreground_title() == WORDS["edit"], "no question was asked")

    # --- only Windows is left -------------------------------------------------------------------
    alt("T")
    heard(app.step("Alt+T: only Windows left", 2.5), "windows", "checked")
    check(item_count(focused_window()) == 1, "one row")
    key(VK_RETURN)
    app.step("Enter: Windows added", 2)
    check(folder_count() == 7, f"seven folders in the profile (got {folder_count()})")

    # --- nothing is left -----------------------------------------------------------------------
    alt("T")
    app.step("Alt+T: nothing left", 2.5)
    key(VK_INSERT, vk("B"))
    heard(app.step("NVDA+B: the message", 2.5), "all_in")
    key(VK_RETURN)
    app.step("Enter: the message closed", 1.5)

    key(VK_ESCAPE)
    app.step("Esc: the profile window closed, nothing saved", 2)

print()
print("all heard" if not missing else "not heard or wrong: " + "; ".join(missing))
sys.exit(1 if missing else 0)
