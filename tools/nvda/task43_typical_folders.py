"""Live NVDA check of TASK-43: the user folder and other users' folders among the typical ones.

    python -X utf8 tools\\nvda\\task43_typical_folders.py uk|en

The language goes into the settings file of the scratch folder; the Ukrainian layout is on.
The profile «Small» has ``C:\\Users\\Public``, which covers the public desktop. The steps:

- «Редагувати профіль…», Alt+T: the list opens; the user folder and Windows are unchecked,
  the rest checked (read through LVM_GETITEMSTATE); the arrows read each row with its name,
  path and check state; no line about other users is in the window;
- Space on the user folder checks it, Enter adds the checked folders: the user folder comes
  without AppData and the desktop inside it, Windows is not added;
- Esc closes the profile window without saving;
- «Налаштування», Ctrl+Tab to «Знімки», Alt+O checks «Показувати теки інших користувачів у
  переліку типових тек»; Enter saves it (read back from settings.db);
- the list again: when revisor runs without administrator's rights and another user's folder
  cannot be opened, the line «Теки інших користувачів видно лише від імені адміністратора.»
  stands above the list (NVDA+B reads it), and no other user's row is offered; run as
  administrator, the other users' rows are at the end, unchecked, and the arrows read them.

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import ctypes
import os
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_INSERT, VK_MENU, VK_RETURN, VK_SPACE, VK_TAB, VK_UP,
    App, children, class_name, control_text, focused_window, foreground_title, item_count, key,
    make_settings, prepare, u32, ukrainian_layout, vk, wait_for,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"
ADMIN = bool(ctypes.windll.shell32.IsUserAnAdmin())

WORDS = {
    "uk": {
        "edit": "Редагувати профіль", "title": "Додати типові теки", "folders": "Теки",
        "settings": "Налаштування", "snapshots": "Знімки",
        "checked": "позначено", "unchecked": "не позначено",
        "program_files": "Program Files (C:\\Program Files)", "windows": "Windows (",
        "user": "Тека користувача (", "app_data": "AppData користувача (",
        "desktop": "Робочий стіл користувача (",
        "other_user": "Тека користувача ", "other_app_data": "AppData користувача ",
        "other_desktop": "Робочий стіл користувача ",
        "setting": "Показувати теки інших користувачів у переліку типових тек",
        "note": "Теки інших користувачів видно лише від імені адміністратора.",
    },
    "en": {
        "edit": "Edit profile", "title": "Add typical folders", "folders": "Folders",
        "settings": "Settings", "snapshots": "Snapshots",
        "checked": "позначено", "unchecked": "не позначено",
        "program_files": "Program Files (C:\\Program Files)", "windows": "Windows (",
        "user": "User folder (", "app_data": "User AppData (", "desktop": "User desktop (",
        "other_user": "User folder of ", "other_app_data": "AppData of ",
        "other_desktop": "Desktop of ",
        "setting": "Show other users' folders in the typical folders list",
        "note": "Other users' folders are shown only when revisor runs as administrator.",
    },
}[LANG]
missing = []

LVIS_STATEIMAGEMASK, LVM_GETITEMSTATE, LB_GETCOUNT = 0xF000, 0x102C, 0x018B


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)


def heard(lines, *names):
    """Note each word of `names` (keys of WORDS) that no line of `lines` has."""
    # The log writes a backslash doubled, as Python's repr does.
    text = " ".join(lines).replace("\\\\", "\\")
    for name in names:
        check(WORDS[name] in text, f"heard {WORDS[name]!r}")
    return text


def alt(letter):
    key(VK_MENU, vk(letter))


def checked_rows(h):
    return [(u32.SendMessageW(h, LVM_GETITEMSTATE, i, LVIS_STATEIMAGEMASK) >> 12) == 2
            for i in range(item_count(h))]


def folder_count():
    """How many folders the profile window lists: the focus is in that list box."""
    h = focused_window()
    return u32.SendMessageW(h, LB_GETCOUNT, 0, 0) if class_name(h) == "ListBox" else None


def note_shown():
    texts = [control_text(c) for c in children(u32.GetForegroundWindow())
             if class_name(c) == "Static"]
    return WORDS["note"] in texts


def open_list(app, name):
    alt("T")
    said = app.step(name, 3)
    rows = focused_window()
    check(class_name(rows) == "SysListView32", "the focus is in the list")
    return said, rows


def walk(app, rows, first):
    """The arrows over every row; returns what NVDA said for each, `first` for the first row,
    which the list said when it got the focus."""
    said = [" ".join(first)]
    for i in range(1, item_count(rows)):
        key(VK_DOWN)
        said.append(" ".join(app.step(f"row {i + 1}", 1.2)))
    # The log writes a backslash doubled, as Python's repr does.
    return [s.replace("\\\\", "\\") for s in said]


def is_row(row, name):
    """Whether NVDA said the row of `name` (a key of WORDS): the log line is a list's repr."""
    return f"'{WORDS[name]}" in row


def edit_profile(app, name):
    alt("F")
    time.sleep(0.4)
    key(vk("E"))
    return heard(app.step(name, 2.5), "edit")


def setting_saved():
    conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
    row = conn.execute("SELECT value FROM setting WHERE key = 'other_users_folders'").fetchone()
    conn.close()
    return row and row[0]


paths = prepare({"Small": ([r"C:\Users\Public"], [])})
make_settings(SCRATCH, language=LANG)
print("running as administrator:", ADMIN)

with App(paths, paths["Small"]) as app:
    app.step("start", 3)
    ukrainian_layout(app.main)
    app.step("Ukrainian layout on", 1)

    # --- #1, #2: the user folder offered, it and Windows unchecked ---------------------------
    edit_profile(app, "Alt+F, E: «Редагувати профіль…»")
    said, rows = open_list(app, "Alt+T: the list of typical folders")
    heard(said, "title", "folders", "program_files", "checked")
    states = checked_rows(rows)
    print("   rows checked:", states)
    rows_said = walk(app, rows, said)
    check(len(rows_said) == len(states), "a row said for every row")
    own = len(states)
    for row, state in zip(rows_said, states):
        large = is_row(row, "windows") or is_row(row, "user")
        check(state == (not large), f"checked {state}: {row[:60]!r}")
        check((WORDS["unchecked"] in row) == (not state), "the state is heard")
    names = " | ".join(rows_said)
    for word in ("windows", "user", "app_data", "desktop"):
        check(WORDS[word] in names, f"a row {WORDS[word]!r}")
    order = [next((w for w in ("user", "app_data", "desktop") if is_row(r, w)), None)
             for r in rows_said[-3:]]
    check(order == ["user", "app_data", "desktop"], "the user's three come last, in order")
    check(not note_shown(), "no line about other users while the setting is off")

    # --- #3: the user folder with AppData and the desktop: only the user folder -------------
    key(VK_UP)
    key(VK_UP)
    said = heard(app.step("Up ×2: the user folder", 1.5), "user")
    key(VK_SPACE)
    heard(app.step("Space: checked", 1.5), "checked")
    check(checked_rows(rows)[own - 3], "the user folder is checked")
    key(VK_RETURN)
    app.step("Enter: added", 2.5)
    # Users\Public, Program Files, (x86), ProgramData, the user folder: not AppData, desktop.
    expected = 1 + sum(1 for r in rows_said[:-3] if not is_row(r, "windows")) + 1
    check(folder_count() == expected,
          f"{expected} folders in the profile (got {folder_count()})")
    check(foreground_title() == WORDS["edit"], "no question was asked")
    key(VK_ESCAPE)
    app.step("Esc: the profile window closed, nothing saved", 2)

    # --- #5: the setting -------------------------------------------------------------------
    alt("F")
    time.sleep(0.4)
    key(vk("S"))
    heard(app.step("Alt+F, S: «Налаштування»", 2.5), "settings")
    key(VK_CONTROL, VK_TAB)
    heard(app.step("Ctrl+Tab: «Знімки»", 1.5), "snapshots")
    alt("O")
    said = heard(app.step("Alt+O: the check box", 2), "setting", "checked")
    check(WORDS["unchecked"] not in " ".join(said), "the check box is checked now")
    key(VK_RETURN)
    app.step("Enter: saved", 2)
    check(setting_saved() == 1, "the setting is saved")

    # --- #6, #7: the list with the setting on ------------------------------------------------
    edit_profile(app, "Alt+F, E: «Редагувати профіль…» again")
    said, rows = open_list(app, "Alt+T: the list with other users")
    states = checked_rows(rows)
    print("   rows checked:", states)
    if ADMIN:
        others = len(states) - own
        check(others > 0 and others % 3 == 0, f"three rows for each other user ({others})")
        check(not any(states[own:]), "the other users' rows are unchecked")
        check(not note_shown(), "no line about rights as administrator")
        rows_said = walk(app, rows, said)
        for i, row in enumerate(rows_said[own:]):
            word = ("other_user", "other_app_data", "other_desktop")[i % 3]
            check(is_row(row, word) and WORDS["unchecked"] in row,
                  f"{word}: {row[:70]!r}")
    else:
        check(len(states) == own, "no other user's row without administrator's rights")
        check(note_shown(), "the line about rights is in the window")
        key(VK_INSERT, vk("B"))
        heard(app.step("NVDA+B: the window's text", 3), "note")
    key(VK_ESCAPE)
    app.step("Esc: the list closed", 1.5)
    key(VK_ESCAPE)
    app.step("Esc: the profile window closed", 2)

print()
print("all heard" if not missing else "not heard or wrong: " + "; ".join(missing))
sys.exit(1 if missing else 0)
