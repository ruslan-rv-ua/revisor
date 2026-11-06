"""Live NVDA check of TASK-17: the proofread strings, in the language given.

    python -X utf8 tools\\nvda\\task17_strings.py uk|en

The settings file of the scratch folder holds the language, so no restart is needed. On the
profile «Small» (this folder), without snapshots: the «Keyboard shortcuts» window names
«Порівняти з…» with its ellipsis; Ctrl+F7 says why not, again with the ellipsis; Ctrl+F5 saves
the first snapshot and says it was not compared; Ctrl+F7 then says there is no other snapshot;
«About revisor» names the license.
"""

import os
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    VK_ESCAPE, VK_F1, VK_F5, VK_MENU, App, control_text, focused_window, key, prepare, vk,
    wait_for,
)

VK_CONTROL, VK_F7 = 0x11, 0x76
LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), LANG
# PRAGMA application_id of the settings database, "RVST" (settings.rs APPLICATION_ID).
SETTINGS_ID = 0x52565354

paths = prepare({"Small": ([os.path.dirname(os.path.abspath(__file__))], [])})
db = sqlite3.connect(os.path.join(os.path.dirname(paths["exe"]), "data", "settings.db"))
db.executescript("CREATE TABLE setting (key TEXT PRIMARY KEY, value) WITHOUT ROWID;")
db.execute(f"PRAGMA application_id = {SETTINGS_ID}")
db.execute("PRAGMA user_version = 1")
db.execute("INSERT INTO setting VALUES ('language', ?)", (LANG,))
db.commit()
db.close()

EXPECT = {
    "uk": {
        "shortcut": "Порівняти з… — Ctrl+F7",
        "no_snapshots": "Порівняти з…: знімків ще немає.",
        "not_compared": "Знімок не порівняно: іншого знімка в профілі немає.",
        "no_other": "Порівняти з…: іншого знімка в профілі немає.",
        "license": "Ліцензія: MIT",
    },
    "en": {
        "shortcut": "Compare with… — Ctrl+F7",
        "no_snapshots": "Compare with…: there are no snapshots yet.",
        "not_compared": "The snapshot was not compared: the profile has no other snapshot.",
        "no_other": "Compare with…: the profile has no other snapshot.",
        "license": "License: MIT",
    },
}[LANG]

failures = []


def check(what, spoken, expected):
    heard = any(expected in line for line in spoken)
    print(f"   {'OK' if heard else 'MISSING'}: {expected}")
    if not heard:
        failures.append(what)


def check_text(what, text, expected):
    found = expected in text
    print(f"   {'OK' if found else 'MISSING'} in the window text: {expected}")
    if not found:
        failures.append(what)


with App(paths, paths["Small"]) as app:
    app.step(f"start ({LANG})", 2.5)

    key(VK_F1)
    app.step("F1: keyboard shortcuts", 2)
    check_text("shortcuts", control_text(focused_window()), EXPECT["shortcut"])
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

    key(VK_CONTROL, VK_F7)
    check("why not, no snapshots", app.step("Ctrl+F7 without snapshots", 2),
          EXPECT["no_snapshots"])

    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: app.progress() is not None, 5) or True
    assert wait_for(app.snapshot_done, 60), "the snapshot did not end"
    check("snapshot and compare", app.step("Ctrl+F5: the first snapshot", 3),
          EXPECT["not_compared"])

    key(VK_CONTROL, VK_F7)
    check("why not, one snapshot", app.step("Ctrl+F7 with one snapshot", 2), EXPECT["no_other"])

    key(VK_MENU, vk("H"))
    time.sleep(0.5)
    key(vk("A"))
    app.step("Alt+H, A: about", 2)
    check_text("about", control_text(focused_window()), EXPECT["license"])
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

print("\nall heard" if not failures else f"\nMISSING: {failures}")
