"""The NVDA pass of the main scenario before a release (TASK-19, «Випуск релізу»).

    python -X utf8 tools\\nvda\\release_pass.py uk|en [exe]

``exe`` is the file being released: the one downloaded from the release page or the one scoop
installed (``~\\scoop\\apps\\revisor\\current\\revisor.exe``); without it, the local release
build. The script runs a copy of it in the scratch folder, with its own ``data`` and the
language given, so neither the real data nor the installed copy is touched. It prints the
SHA-256 of the exe first, so the record of the pass names the file it passed.

The main scenario of the brief, from the keyboard alone: no profiles yet; Ctrl+N, a name, a
folder through the folder picker, «Створити»; a snapshot before (F5); the script changes files
in the folder as an installer would; «Знімок + порівняння» (Ctrl+F5); the difference list, the
details of a change (Alt+Enter), the type filter (Ctrl+2), the text filter (Ctrl+F), F6 between
the lists, and the keyboard shortcuts (F1). After each step it prints what NVDA said and checks
the words that step must have; the last line says ``all heard`` or names what is missing, and
the exit code is not 0 when anything is.
"""

import hashlib
import os
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    EXE_SRC, SCRATCH, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_F1, VK_F5, VK_F6, VK_MENU, VK_RETURN,
    VK_SHIFT, App, control_text, focused_window, foreground_title, item_count, key,
    make_settings, prepare, snapshot_list, type_text, visible_controls, vk, wait_for,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), f"the language is uk or en, not {LANG!r}"
EXE = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else EXE_SRC

EXPECT = {
    "uk": {
        "no_profiles": "Профілів ще немає",
        "new_profile": "Новий профіль",
        "picker": "Виберіть теку для профілю",
        "name": "Перевірка релізу",
        "created": "Профіль «Перевірка релізу» створено.",
        "saved": "Знімок збережено:",
        "changes": ("додано", "видалено", "змінено"),
        "details": "Подробиці:",
        "change_property": "Зміна",
        "showing": "Показано",
        "shortcut": "Знімок + порівняння — Ctrl+F5",
    },
    "en": {
        "no_profiles": "No profiles yet",
        "new_profile": "New profile",
        "picker": "Choose a folder for the profile",
        "name": "Release check",
        "created": "Profile “Release check” created.",
        "saved": "Snapshot saved:",
        "changes": ("added", "deleted", "modified"),
        "details": "Details:",
        "change_property": "Change",
        "showing": "Showing",
        "shortcut": "Snapshot and compare — Ctrl+F5",
    },
}[LANG]

failures = []


def check(what, where, text, expected):
    """Note `what` as missing unless `text` (what NVDA said, or a window's text) has
    `expected`."""
    found = expected in text
    print(f"   {'OK' if found else 'MISSING'} {where}: {expected}")
    if not found:
        failures.append(what)


def heard(lines):
    return "\n".join(lines)


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


print(f"exe: {EXE}\nSHA-256: {sha256(EXE)}\nlanguage: {LANG}")
paths = prepare({}, exe=EXE)
scratch = os.path.dirname(paths["exe"])
make_settings(scratch, language=LANG)
# The folder the profile watches: what the "program" changes between the two snapshots.
tree = os.path.join(SCRATCH, "program")
write(os.path.join(tree, "keep.txt"), "stays as it is")
write(os.path.join(tree, "change.txt"), "before")
write(os.path.join(tree, "remove.txt"), "goes away")


def differences(main):
    """The difference list on screen, after the snapshot list."""
    lists = visible_controls(main, "SysListView32")
    return lists[1] if len(lists) > 1 else None


with App(paths) as app:
    check("no profiles", "heard", heard(app.step("start: no profiles yet", 2.5)),
          EXPECT["no_profiles"])

    key(VK_CONTROL, vk("N"))
    lines = app.step("Ctrl+N: the profile window", 2)
    check("profile window", "title", foreground_title(), EXPECT["new_profile"])
    check("profile window", "heard", heard(lines), EXPECT["new_profile"])

    type_text(EXPECT["name"])
    app.step("type the name", 1)

    # Alt+A, «Додати теку…»: Windows' own folder picker. A typed path and Enter first go into
    # the folder and then choose it, so Enter repeats until the picker has gone.
    key(VK_MENU, vk("A"))
    assert wait_for(lambda: foreground_title() == EXPECT["picker"], 10), "no folder picker"
    app.step("Alt+A: the folder picker", 1.5)
    type_text(tree)
    for _ in range(5):
        key(VK_RETURN)
        time.sleep(1.5)
        if foreground_title() == EXPECT["new_profile"]:
            break
    assert foreground_title() == EXPECT["new_profile"], "the folder picker did not close"
    app.step("the folder chosen", 1.5)

    # Back to «Назва», where Enter presses «Створити».
    key(VK_MENU, vk("N"))
    time.sleep(0.5)
    key(VK_RETURN)
    check("profile created", "heard",
          heard(app.step("Alt+N, Enter: create the profile", 2.5)), EXPECT["created"])

    key(VK_F5)
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == 1, 60), \
        "the snapshot before was not saved"
    check("snapshot before", "heard", heard(app.step("F5: the snapshot before", 2.5)),
          EXPECT["saved"])

    # The "program": one file changed, one deleted, one added, and a folder with a file.
    write(os.path.join(tree, "change.txt"), "after the program ran")
    os.remove(os.path.join(tree, "remove.txt"))
    write(os.path.join(tree, "new.txt"), "created by the program")
    write(os.path.join(tree, "settings", "program.ini"), "[program]")
    time.sleep(1)

    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == 2, 60), \
        "the snapshot after was not saved"
    assert wait_for(lambda: (item_count(differences(app.main)) or 0) > 0, 60), \
        "no difference appeared"
    lines = app.step("Ctrl+F5: snapshot and compare", 3)
    check("snapshot after", "heard", heard(lines), EXPECT["saved"])
    for word in EXPECT["changes"]:
        check("summary", "heard", heard(lines), word)
    print(f"   difference rows: {item_count(differences(app.main))}")

    key(VK_DOWN)
    app.step("Down arrow: the next change", 1.5)

    key(VK_MENU, VK_RETURN)
    lines = app.step("Alt+Enter: details", 2)
    check("details", "title", foreground_title(), EXPECT["details"])
    check("details", "in the window text", control_text(focused_window()),
          EXPECT["change_property"])
    key(VK_ESCAPE)
    app.step("Esc: back to the list", 1.5)

    key(VK_CONTROL, vk("2"))
    check("type filter", "heard", heard(app.step("Ctrl+2: hide the deleted", 2)),
          EXPECT["showing"])
    key(VK_CONTROL, vk("2"))
    app.step("Ctrl+2: show them again", 2)

    key(VK_CONTROL, vk("F"))
    app.step("Ctrl+F: the filter field", 1.5)
    type_text("new")
    # «Показано N із M» comes once the typing has paused for a second.
    check("text filter", "heard", heard(app.step("type «new»", 2.5)), EXPECT["showing"])
    key(VK_ESCAPE)
    app.step("Esc: clear the filter", 2)

    key(VK_F6)
    app.step("F6: the snapshot list", 1.5)
    key(VK_SHIFT, VK_F6)
    app.step("Shift+F6: the difference list", 1.5)

    key(VK_F1)
    app.step("F1: keyboard shortcuts", 2)
    check("shortcuts", "in the window text", control_text(focused_window()),
          EXPECT["shortcut"])
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

shutil.rmtree(SCRATCH, ignore_errors=True)
print("\nall heard" if not failures else f"\nMISSING: {failures}")
sys.exit(1 if failures else 0)
