"""Live NVDA check of TASK-17: the proofread strings, in the language given.

    python -X utf8 tools\\nvda\\task17_strings.py uk|en

The settings file of the scratch folder holds the language, so no restart is needed. On the
profile «Small» (this folder), without snapshots: the «Keyboard shortcuts» window names
«Порівняти з…» with its ellipsis; Ctrl+F7 says why not, again with the ellipsis; Ctrl+F5 saves
the first snapshot and says it was not compared; Ctrl+F7 then says there is no other snapshot;
«About revisor» names the license.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    VK_CONTROL, VK_ESCAPE, VK_F1, VK_F5, VK_F7, VK_MENU, App, control_text, focused_window,
    item_count, key, make_settings, prepare, snapshot_list, vk, wait_for,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), LANG

paths = prepare({"Small": ([os.path.dirname(os.path.abspath(__file__))], [])})
make_settings(os.path.dirname(paths["exe"]), language=LANG)

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


def check(what, where, text, expected):
    """Note `what` as missing unless `text` — what NVDA said, or a window's text — has
    `expected`."""
    found = expected in text
    print(f"   {'OK' if found else 'MISSING'} {where}: {expected}")
    if not found:
        failures.append(what)


def heard(lines):
    return "\n".join(lines)


with App(paths, paths["Small"]) as app:
    app.step(f"start ({LANG})", 2.5)

    key(VK_F1)
    app.step("F1: keyboard shortcuts", 2)
    check("shortcuts", "in the window text", control_text(focused_window()), EXPECT["shortcut"])
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

    key(VK_CONTROL, VK_F7)
    check("why not, no snapshots", "heard",
          heard(app.step("Ctrl+F7 without snapshots", 2)), EXPECT["no_snapshots"])

    key(VK_CONTROL, VK_F5)
    # The first snapshot is in the list once it is saved; the speech follows it.
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == 1, 60), \
        "the first snapshot was not saved"
    check("snapshot and compare", "heard",
          heard(app.step("Ctrl+F5: the first snapshot", 3)), EXPECT["not_compared"])

    key(VK_CONTROL, VK_F7)
    check("why not, one snapshot", "heard",
          heard(app.step("Ctrl+F7 with one snapshot", 2)), EXPECT["no_other"])

    key(VK_MENU, vk("H"))
    time.sleep(0.5)
    key(vk("A"))
    app.step("Alt+H, A: about", 2)
    check("about", "in the window text", control_text(focused_window()), EXPECT["license"])
    key(VK_ESCAPE)
    app.step("Esc", 1.5)

print("\nall heard" if not failures else f"\nMISSING: {failures}")
