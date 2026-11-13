"""Live NVDA check of TASK-44: the settings window laid out on tabs.

    python -X utf8 tools\\nvda\\task44_settings_tabs.py uk|en [walk,look]

The language goes into the settings file of the scratch folder; the Ukrainian layout is on.

- ``walk``: «Налаштування» opens on «Загальні» with the focus on «Мова»; Ctrl+Tab, Ctrl+Tab,
  Ctrl+Tab go round the tabs, Ctrl+Shift+Tab back, Ctrl+PgUp and Ctrl+PgDn too, and NVDA names
  each tab; on «Різниця» Alt+L (a key of «Загальні») does nothing, Alt+S moves to «Розмір»;
  Shift+Tab from the first list reaches the tab names, where the arrows switch tabs; Alt+C on
  «Знімки» toggles the check box, Esc saves nothing (reopened, it is as before), then Alt+C and
  Enter save it (reopened, it is toggled), and Alt+C, Enter put it back.
- ``look``: each theme («Як у Windows», «Світла», «Темна») in turn, each tab pictured in
  ``%TEMP%\\revisor-task44``, with the size of the window and of each tab printed.

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import ctypes
import ctypes.wintypes
import os
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import revisor_nvda  # noqa: E402
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_CONTROL, VK_ESCAPE, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB,
    App, children, class_name, control_text, focused_window, foreground_title, key,
    make_settings, prepare, screenshot, u32, ukrainian_layout, vk, wait_for, window_text,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"
PARTS = (sys.argv[2] if len(sys.argv) > 2 else "walk,look").split(",")
SHOTS = os.path.join(tempfile.gettempdir(), "revisor-task44")

VK_PRIOR, VK_NEXT = 0x21, 0x22
revisor_nvda.EXT.update({VK_PRIOR, VK_NEXT})
TCM_GETCURSEL = 0x130B

WORDS = {
    "uk": {"title": "Налаштування", "tabs": ("Загальні", "Знімки", "Різниця"),
           "language": "Мова", "size": "Розмір", "confirm": "Підтверджувати видалення знімків",
           "focus": "Фокус після знімка", "comparison": "Фокус після порівняння",
           "tab": "вкладка"},
    "en": {"title": "Settings", "tabs": ("General", "Snapshots", "Difference"),
           "language": "Language", "size": "Size", "confirm": "Confirm snapshot deletion",
           "focus": "Focus after a snapshot", "comparison": "Focus after a comparison",
           "tab": "вкладка"},
}[LANG]
missing = []


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)


def heard(lines, *words):
    text = " ".join(lines)
    for word in words:
        check(word in text, f"heard {word!r}")
    return text


def alt(letter):
    key(VK_MENU, vk(letter))


def ctrl(*vks):
    key(VK_CONTROL, *vks)


def dialog():
    h = u32.GetForegroundWindow()
    return h if window_text(h) == WORDS["title"] else None


def tabs(h):
    return next(c for c in children(h) if class_name(c) == "_wx_SysTabCtl32")


def tab_shown(h):
    return u32.SendMessageW(tabs(h), TCM_GETCURSEL, 0, 0)


def open_settings(app, name="Alt+F, S: «Налаштування»"):
    alt("F")
    time.sleep(0.4)
    key(vk("S"))
    said = app.step(name, 2.5)
    assert wait_for(dialog, 3), "the settings window did not open"
    return said, dialog()


def focused_text():
    h = focused_window()
    return f"{class_name(h)} {control_text(h)!r}"


def walk():
    paths = prepare({})
    make_settings(SCRATCH, language=LANG)
    with App(paths) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start", 1)
        said, h = open_settings(app)
        heard(said, WORDS["title"], WORDS["language"])
        check(tab_shown(h) == 0, "opens on the first tab (#1)")
        check(class_name(focused_window()) == "ComboBox", f"focus on «Мова»: {focused_text()}")

        names = WORDS["tabs"]
        for index, keys, label in [
            (1, (VK_TAB,), "Ctrl+Tab"),
            (2, (VK_TAB,), "Ctrl+Tab"),
            (0, (VK_TAB,), "Ctrl+Tab round"),
            (2, (VK_SHIFT, VK_TAB), "Ctrl+Shift+Tab round back"),
            (1, (VK_PRIOR,), "Ctrl+PgUp"),
            (2, (VK_NEXT,), "Ctrl+PgDn"),
        ]:
            ctrl(*keys)
            said = app.step(f"{label}: «{names[index]}»", 1.5)
            heard(said, names[index])
            check(tab_shown(h) == index, f"{label}: tab {index} shown, {tab_shown(h)}")
            print("    focus:", focused_text())

        # #3: the keys of the tab shown only.
        alt("L")
        app.step("Alt+L on «Різниця»: a key of «Загальні», nothing happens", 1.5)
        check(tab_shown(h) == 2, "Alt+L leaves «Різниця» shown")
        alt("S")
        heard(app.step("Alt+S: «Розмір»", 1.5), WORDS["size"])
        print("    focus:", focused_text())

        # The arrows on the tab names.
        alt("M")
        app.step("Alt+M: the first list of «Різниця»", 1.5)
        key(VK_SHIFT, VK_TAB)
        said = app.step("Shift+Tab: the tab names", 1.5)
        heard(said, names[2])
        check(class_name(focused_window()) == "_wx_SysTabCtl32", f"focus on tabs: {focused_text()}")
        key(VK_LEFT)
        heard(app.step("Left: «Знімки»", 1.5), names[1])
        check(tab_shown(h) == 1, "Left shows «Знімки»")
        key(VK_RIGHT)
        heard(app.step("Right: «Різниця»", 1.5), names[2])
        key(VK_LEFT)
        app.step("Left: «Знімки»", 1.5)
        key(VK_TAB)
        heard(app.step("Tab: «Фокус після знімка»", 1.5), WORDS["focus"])

        # #4: Cancel keeps nothing, OK keeps every tab. The state is read from the speech:
        # BM_GETCHECK does not see wx's check box.
        def state(lines):
            text = " ".join(lines)
            return "не позначено" not in text

        alt("C")
        said = app.step("Alt+C: the check box toggled", 2)
        heard(said, WORDS["confirm"])
        before = not state(said)
        key(VK_ESCAPE)
        app.step("Esc: closed", 1.5)
        open_settings(app, "reopened after Esc")
        ctrl(VK_TAB)
        app.step("Ctrl+Tab: «Знімки»", 1.5)
        alt("C")
        # Esc kept `before`, so Alt+C toggles it away from it.
        check(state(app.step("Alt+C: toggled from what Esc left", 2)) != before,
              "Esc kept nothing")
        key(VK_RETURN)
        app.step("Enter: saved", 2)
        open_settings(app, "reopened after Enter")
        ctrl(VK_TAB)
        app.step("Ctrl+Tab", 1)
        alt("C")
        # OK kept `not before`, so Alt+C toggles it back to `before`.
        check(state(app.step("Alt+C: toggled from what OK saved", 2)) == before,
              "OK kept the check box")
        key(VK_RETURN)
        app.step("Enter: saved again", 2)
        print("   foreground:", foreground_title())


def rect_of(h):
    r = ctypes.wintypes.RECT()
    u32.GetWindowRect(h, ctypes.byref(r))
    return r.right - r.left, r.bottom - r.top


def look():
    for theme in ("windows", "light", "dark"):
        paths = prepare({})
        make_settings(SCRATCH, language=LANG, theme=theme)
        with App(paths) as app:
            ukrainian_layout(app.main)
            time.sleep(0.5)
            _, h = open_settings(app, f"{theme}: the settings")
            print(f"    window {rect_of(h)}")
            for index in range(3):
                time.sleep(0.5)
                path = screenshot(h, os.path.join(SHOTS, f"{LANG}-{theme}-{index}.png"))
                pages = [c for c in children(h) if class_name(c).startswith("wxWindow")
                         and u32.IsWindowVisible(c)]
                print(f"    tab {index}: {path}, visible panels {[rect_of(p) for p in pages]}")
                ctrl(VK_TAB)
                app.step(f"{theme}: Ctrl+Tab", 1)
            key(VK_ESCAPE)
            app.step(f"{theme}: Esc", 1)


if "walk" in PARTS:
    walk()
if "look" in PARTS:
    look()
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
