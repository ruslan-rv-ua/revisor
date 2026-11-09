"""Live NVDA check of TASK-34: the theme of the interface — light, dark, as Windows — heard with
NVDA and seen in pictures of revisor's own windows.

    python -X utf8 tools\\nvda\\task34_dark_theme.py uk|en [parts] ["<big folder>"]

``parts`` is a comma-separated choice (``walk,switch`` by default):

- ``walk``: the same walk through every window of revisor, once light and once dark: the main
  window with the snapshot list, the difference list and tree with the focus in them and in
  the field «Фільтр:», Tab round the window, the menu «Файл», the settings, the profile window
  and the folder picker, «Порівняти з», «Коментар», «Подробиці», «Гарячі клавіші», a question
  and «Про revisor». Each window gets a picture (PrintWindow: revisor's window only, nothing
  else on the screen) in ``%TEMP%\\revisor-task34``. What NVDA says on each step, without the
  numbers and the names of the themes, must be the same in both (#4); the lists get dark (#2).
- ``switch``: dark; «Тема» changed to «Світла» and back in «Налаштування»: the colours change at
  once, the focus stays on the difference list, and NVDA says what OK says without a change (#3).
- ``contrast``: dark; the script waits while the author switches a contrast theme of Windows on
  (Left Alt+Left Shift+PrtScn) and off again, at most five minutes each. While it is on, the
  lists take its colours; after, they are dark again (#6). The script never switches it itself:
  it is a setting of Windows.
- ``windows``: «Як у Windows»; the script waits while the author switches the mode of Windows
  (Settings, Personalization, Colors) and back: revisor follows it at once (#3).
- ``big``: a difference of every entry of the big folder (``C:\\dev`` by default) in the list;
  the theme changed both ways under the probe: how long the window does not answer.

The last line is ``all heard`` or what was missing.
"""

import os
import re
import subprocess
import sys
import tempfile
import time
import winreg

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from probe import Probe  # noqa: E402
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_CONTROL, VK_DELETE, VK_DOWN, VK_ESCAPE, VK_F1, VK_F2, VK_F5, VK_F6, VK_F7,
    VK_HOME, VK_MENU, VK_RETURN, VK_SHIFT, VK_SPACE, VK_TAB, App, colour, focused_class,
    focused_window, foreground_title, high_contrast, is_dark, item_count, key, list_colours,
    make_settings, popup_menu, prepare, screenshot, snapshot_list, u32, visible_controls, vk,
    wait_for,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"
PARTS = (sys.argv[2] if len(sys.argv) > 2 else "walk,switch").split(",")
BIG_SOURCE = sys.argv[3] if len(sys.argv) > 3 else r"C:\dev"
SHOTS = os.path.join(tempfile.gettempdir(), "revisor-task34")

# The values of «Тема» in the order of the list, as the settings store them and as they read.
THEMES = ("windows", "light", "dark")
THEME_NAMES = {"uk": ("Як у Windows", "Світла", "Темна"),
               "en": ("Same as Windows", "Light", "Dark")}[LANG]
WORDS = {
    "uk": {"saved": "Налаштування збережено", "picker": "Виберіть теку для профілю",
           "theme": "Тема", "snapshots": "Знімки"},
    "en": {"saved": "Settings saved", "picker": "Choose a folder for the profile",
           "theme": "Theme", "snapshots": "Snapshots"},
}[LANG]
COLOR_WINDOW = 5

missing = []
tree = os.path.join(SCRATCH, "tree")
big = os.path.join(SCRATCH, "big")


def check(what, found):
    print(f"   {'OK' if found else 'MISSING'}: {what}")
    if not found:
        missing.append(what)


def normal(lines):
    """What NVDA said, without what differs between two runs of the same walk: the numbers
    (times, sizes, counts), the names of the themes, and an utterance said twice in a row, as
    NVDA sometimes says the focused row again whatever the theme."""
    kept = [line for i, line in enumerate(lines) if i == 0 or line != lines[i - 1]]
    text = " | ".join(kept)
    for name in THEME_NAMES:
        text = text.replace(name, "<theme>")
    return re.sub(r"\d+", "#", text)


def write(rel, text):
    full = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


def fill_tree():
    write("changed.txt", "before")
    write("removed.txt", "goes away")
    write(r"docs\kept.txt", "stays")


def change_tree():
    write("changed.txt", "after, longer")
    os.remove(os.path.join(tree, "removed.txt"))
    write(r"docs\added.txt", "new")


def menu(*letters):
    key(VK_MENU, vk(letters[0]))
    time.sleep(0.6)
    for letter in letters[1:]:
        key(vk(letter))
        time.sleep(0.5)


def done(app, timeout=120):
    """Wait for the snapshot or comparison just started to end."""
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


def difference_list(main):
    lists = visible_controls(main, "SysListView32")
    return lists[1] if len(lists) > 1 else None


def to_difference_list(app):
    for _ in range(4):
        shown = difference_list(app.main)
        if shown and focused_window() == shown:
            return
        key(VK_F6)
        time.sleep(0.7)
    assert focused_window() == difference_list(app.main), focused_class()


def colours(app, label):
    """Print the colours of the snapshot list and of the tree, if one shows; True when the
    list is dark."""
    background, text = list_colours(snapshot_list(app.main))
    trees = visible_controls(app.main, "SysTreeView32")
    tree_bk = colour(u32.SendMessageW(trees[0], 0x111F, 0, 0)) if trees else "no tree"
    print(f"   {label}: list background {background}, text {text}, tree background {tree_bk}, "
          f"window colour of Windows {colour(u32.GetSysColor(COLOR_WINDOW))}")
    return is_dark(background)


def picture(name, h=None, from_screen=False):
    if h is None:
        h = u32.GetForegroundWindow()
    path = screenshot(h, os.path.join(SHOTS, f"{LANG}-{name}.png"), from_screen)
    print(f"   picture: {path}")


def close_gently(app):
    """Close revisor with WM_CLOSE to its window rather than Alt+F4: while the author switches
    a setting of Windows, their own window is in front, and Alt+F4 would close that."""
    app.speech.new()  # not printed: NVDA spoke for the author's windows meanwhile
    u32.PostMessageW(app.main, 0x0010, 0, 0)  # WM_CLOSE
    wait_for(lambda: app.proc.poll() is not None, 10)


def apps_light():
    """Whether Windows is in light mode for apps (AppsUseLightTheme), as the author sets it."""
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize") as k:
        return bool(winreg.QueryValueEx(k, "AppsUseLightTheme")[0])


# --- walk -------------------------------------------------------------------------------------
def walk(app, theme):
    """The walk; returns what NVDA said on each step, without numbers and theme names."""
    said = {}

    def step(name, wait=1.5):
        said[name] = normal(app.step(f"{theme}: {name}", wait))

    def shot(name, h=None, from_screen=False):
        picture(f"{theme}-{name}", h, from_screen)

    # A cold start is slower: wait until NVDA has named the snapshot list, at most 10 s.
    heard = []
    end = time.time() + 10
    while time.time() < end and not any(WORDS["snapshots"] in line for line in heard):
        time.sleep(0.5)
        heard += app.speech.new()
    print(f"\n== {theme}: start, until NVDA names the snapshot list")
    for line in heard:
        print("   ", line)
    heard += app.step(f"{theme}: start, a second more", 1)
    said["start"] = normal(heard)
    shot("main-no-snapshots")
    key(VK_F5)
    done(app)
    step("F5: the first snapshot", 2.5)
    change_tree()
    key(VK_CONTROL, VK_F5)
    done(app)
    step("Ctrl+F5: the snapshot and the difference", 3)
    check(f"{theme}: the difference has rows", item_count(difference_list(app.main) or 0) > 0)
    shot("main-list-focused")
    key(VK_DOWN)
    step("Down: the next change")
    key(VK_MENU, VK_RETURN)
    step("Alt+Enter: details", 2)
    shot("details")
    key(VK_ESCAPE)
    step("Esc: back to the list")
    key(VK_CONTROL, vk("F"))
    step("Ctrl+F: the filter field")
    shot("main-list-unfocused")
    key(VK_SHIFT, VK_TAB)
    step("Shift+Tab: the list")
    key(VK_CONTROL, vk("T"))
    step("Ctrl+T: the tree", 2)
    shot("main-tree-focused")
    key(VK_CONTROL, vk("F"))
    step("Ctrl+F: the filter field, the tree behind it")
    shot("main-tree-unfocused")
    key(VK_SHIFT, VK_TAB)
    step("Shift+Tab: the tree")
    key(VK_CONTROL, vk("T"))
    step("Ctrl+T: the list", 2)
    for i in range(1, 9):
        key(VK_TAB)
        step(f"Tab {i} in the main window", 1)

    key(VK_MENU, vk("F"))
    step("Alt+F: the menu «Файл»", 1.5)
    menu_window = popup_menu()
    if menu_window:
        shot("menu-file", menu_window, from_screen=True)
    key(VK_DOWN)
    step("Down in the menu", 1)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    step("Esc, Esc: out of the menu", 1)

    menu("F", "S")
    step("Alt+F, S: the settings", 2.5)
    shot("settings")
    for i in range(1, 12):
        key(VK_TAB)
        step(f"settings: Tab {i}", 1)
        if i == 7:  # «Підтверджувати видалення знімків»: its state both ways
            key(VK_SPACE)
            step("settings: Space on the check box", 1)
            key(VK_SPACE)
            step("settings: Space again", 1)
    key(VK_ESCAPE)
    step("Esc: the settings closed", 1.5)

    menu("F", "E")
    step("Alt+F, E: the profile window", 2.5)
    shot("profile")
    key(VK_MENU, vk("A"))
    if wait_for(lambda: foreground_title() == WORDS["picker"], 10):
        step("Alt+A: the folder picker", 2.5)
        shot("folder-picker")
        key(VK_ESCAPE)
        step("Esc: the picker closed", 2)
    else:
        check(f"{theme}: the folder picker opens", False)
    key(VK_ESCAPE)
    step("Esc: the profile window closed", 2)

    key(VK_CONTROL, VK_F7)
    step("Ctrl+F7: compare with", 2)
    shot("compare-with")
    key(VK_ESCAPE)
    step("Esc: closed", 1.5)
    key(VK_F2)
    step("F2: the comment", 2)
    shot("comment")
    key(VK_ESCAPE)
    step("Esc: closed", 1.5)
    key(VK_F1)
    step("F1: keyboard shortcuts", 2)
    shot("shortcuts")
    key(VK_ESCAPE)
    step("Esc: closed", 1.5)

    # A question: Delete in the snapshot list, answered No with Esc.
    for _ in range(4):
        if focused_window() == snapshot_list(app.main):
            break
        key(VK_F6)
        time.sleep(0.7)
    key(VK_DELETE)
    step("Delete: the question", 2.5)
    shot("question")
    key(VK_ESCAPE)
    step("Esc: nothing deleted", 1.5)

    menu("H", "A")
    step("Alt+H, A: about revisor", 2.5)
    shot("about")
    key(VK_ESCAPE)
    step("Esc: closed", 1.5)
    return said


def compare_walks(light, dark):
    """#4: the same steps say the same in both themes."""
    print("\n== the walks compared, light and dark")
    for name, heard in light.items():
        other = dark.get(name)
        if other != heard:
            print(f"   differs at «{name}»:\n     light: {heard}\n     dark:  {other}")
        check(f"the same speech light and dark: {name}", other == heard)


# --- switch -----------------------------------------------------------------------------------
def choose_theme(app, index):
    """Open the settings, put «Тема» on THEMES[index] and confirm with Enter; returns what
    NVDA said after Enter."""
    menu("F", "S")
    app.step("Alt+F, S: the settings", 2.5)
    key(VK_MENU, vk("H"))
    app.step("Alt+H: «Тема»", 1.5)
    key(VK_HOME)
    for _ in range(index):
        key(VK_DOWN)
    app.step(f"the theme: {THEME_NAMES[index]}", 1)
    key(VK_RETURN)
    return normal(app.step(f"Enter: {THEME_NAMES[index]}", 2.5))


def switch(app):
    to_difference_list(app)
    before = focused_window()
    menu("F", "S")
    app.step("Alt+F, S: the settings", 2.5)
    key(VK_RETURN)
    baseline = normal(app.step("Enter without a change", 2.5))
    check("OK without a change says the settings are saved", WORDS["saved"] in baseline)
    for index, dark in ((1, False), (2, True)):
        with Probe(app.proc.pid, app.main) as probe:
            heard = choose_theme(app, index)
        probe.report(f"the theme {THEME_NAMES[index]}")
        name = THEME_NAMES[index]
        check(f"{name}: the focus stays on the difference list", focused_window() == before)
        if heard != baseline:
            print(f"     without a change: {baseline}\n     {name}: {heard}")
        check(f"{name}: NVDA says what OK says without a change", heard == baseline)
        check(f"{name}: the colours change at once", colours(app, name) == dark)
        picture(f"switch-{THEMES[index]}")


# --- contrast and windows -----------------------------------------------------------------------
# How long the script waits for the author to switch a setting of Windows: as long as they
# need, since they do it between other things; the script is stopped by hand if they cannot.
AUTHOR_WAIT = 6 * 3600


def contrast(app):
    """#6: revisor dark, whatever the contrast theme is at the start; the author switches it
    until revisor has been seen with it on and off, and it ends off."""
    on = high_contrast()
    seen = set()
    while True:
        name = f"contrast-{'on' if on else 'off'}-{len(seen)}"
        background, _ = list_colours(snapshot_list(app.main))
        colours(app, name)
        if on:
            check(f"{name}: the list has the window colour of the contrast theme",
                  background == colour(u32.GetSysColor(COLOR_WINDOW)))
        else:
            check(f"{name}: dark", is_dark(background))
        picture(name, app.main)
        seen.add(on)
        if len(seen) == 2 and not on:
            return
        print(f"\n>>> Now switch the contrast theme of Windows {'OFF' if on else 'ON'}: "
              "Left Alt + Left Shift + PrtScn, or Settings.")
        sys.stdout.flush()
        if not wait_for(lambda: high_contrast() != on, AUTHOR_WAIT, 0.5):
            check(f"the contrast theme was switched {'off' if on else 'on'}", False)
            return
        on = not on
        time.sleep(4)
        # Not printed: NVDA spoke for the author's own windows while they switched it.
        app.speech.new()


def windows_mode(app):
    """#3: «Як у Windows»; the author switches the mode of Windows for apps and back, and
    revisor follows at once."""
    original = light = apps_light()
    seen = set()
    while True:
        name = f"windows-{'light' if light else 'dark'}-{len(seen)}"
        check(f"{name}: revisor follows Windows", colours(app, name) == (not light))
        picture(name, app.main)
        seen.add(light)
        if len(seen) == 2 and light == original:
            return
        print(f"\n>>> Now switch Windows to {'DARK' if light else 'LIGHT'} mode for apps "
              "(Settings, Personalization, Colors).")
        sys.stdout.flush()
        if not wait_for(lambda: apps_light() != light, AUTHOR_WAIT, 0.5):
            check("Windows switched its mode", False)
            return
        light = not light
        time.sleep(4)
        # Not printed: NVDA spoke for the author's Settings window while they switched it.
        app.speech.new()


# --- big --------------------------------------------------------------------------------------
def check_big(app):
    app.step("start: Big", 2.5)
    key(VK_F5)
    done(app, 600)
    app.step("F5: the folder missing", 1.5)
    subprocess.run(["cmd", "/c", "mklink", "/J", big, BIG_SOURCE], check=True,
                   capture_output=True)
    key(VK_F5)
    done(app, 600)
    app.step("F5: the folder there", 2)
    key(VK_F7)
    done(app, 600)
    app.step("F7: the difference of every entry", 3)
    print(f"   difference rows: {item_count(difference_list(app.main) or 0)}")
    to_difference_list(app)
    before = focused_window()
    for index in (1, 2):
        with Probe(app.proc.pid, app.main) as probe:
            choose_theme(app, index)
        probe.report(f"the theme {THEME_NAMES[index]} over a big difference")
        check(f"big, {THEME_NAMES[index]}: the focus stays", focused_window() == before)


# --- the runs -----------------------------------------------------------------------------------
def session(theme):
    paths = prepare({"Small": ([tree], [])})
    make_settings(os.path.dirname(paths["exe"]), language=LANG, theme=theme)
    fill_tree()
    return paths


try:
    walks = {}
    if "walk" in PARTS:
        for theme in ("light", "dark"):
            paths = session(theme)
            with App(paths, paths["Small"]) as app:
                walks[theme] = walk(app, theme)
                check(f"{theme}: the lists are {theme}",
                      colours(app, theme) == (theme == "dark"))
                if theme == "dark" and "switch" in PARTS:
                    switch(app)
        compare_walks(walks["light"], walks["dark"])
    elif "switch" in PARTS:
        paths = session("dark")
        with App(paths, paths["Small"]) as app:
            app.step("start", 3)
            key(VK_F5)
            done(app)
            change_tree()
            key(VK_CONTROL, VK_F5)
            done(app)
            app.step("a difference", 3)
            switch(app)
    if "contrast" in PARTS:
        paths = session("dark")
        with App(paths, paths["Small"]) as app:
            app.step("start, dark", 3)
            try:
                contrast(app)
            finally:
                close_gently(app)
    if "windows" in PARTS:
        paths = session("windows")
        with App(paths, paths["Small"]) as app:
            app.step("start, as Windows", 3)
            try:
                windows_mode(app)
            finally:
                close_gently(app)
    if "big" in PARTS:
        if os.path.lexists(big):
            os.rmdir(big)
        paths = prepare({"Big": ([big], [])})
        make_settings(os.path.dirname(paths["exe"]), language=LANG, theme="dark")
        with App(paths, paths["Big"]) as app:
            check_big(app)
finally:
    if os.path.lexists(big):
        os.rmdir(big)

print(f"\npictures: {SHOTS}")
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
