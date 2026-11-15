"""Live NVDA check of TASK-46: a sound at the end of a snapshot, and a silent status bar.

    python -X utf8 tools\\nvda\\task46_sound.py [exe]

The exe goes first (the debug build by default). The interface is Ukrainian, the Ukrainian
layout is on. The script cannot hear the sounds: before each step that should sound (or stay
silent) it prints a line with the time, so the author, listening, can match what they heard.

Two profiles: «Small» (a scratch folder of three files, a snapshot ends in about a second) and
«Big» (``C:\\dev``, long enough to cancel). In the profile box «Big» comes before «Small».

1. The scratch settings hold ``sound_only_after_long = 0``, so short snapshots sound.
2. Settings, «Загальні»: Tab through the four new check boxes, each said with its name and
   «позначено»; Alt+S and Alt+C (both sounds off) make the threshold unavailable
   (``IsWindowEnabled``), Alt+S makes it available again; Esc keeps nothing.
3. SOUND «готово»: F5 with the window active.
4. SOUND «готово»: F5, the window minimized at once, restored after ~3 s.
5. SOUND «без результату»: on «Big», F5 and Shift+F5 at once; NVDA says «Знімок скасовано».
6. NO SOUND: the threshold back on (Alt+O, Enter), F5 on «Small».
7. Status bar silent: Alt+R (off), Enter; F5 says nothing of its end by itself, NVDA+End
   says «Знімок збережено».

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import ctypes
import os
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    REPO, SCRATCH, VK_DOWN, VK_END, VK_ESCAPE, VK_F5, VK_INSERT, VK_MENU, VK_RETURN, VK_SHIFT,
    VK_TAB, VK_UP, App, children, class_name, control_text, focused_window, foreground_title,
    key, make_settings, prepare, u32, ukrainian_layout, vk, wait_for, window_text,
)

EXE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "target", "debug", "revisor.exe")
BIG = r"C:\dev"
TINY = os.path.join(SCRATCH, "tiny")
SW_MINIMIZE, SW_RESTORE = 6, 9
SETTINGS = "Налаштування"
SAVED = "Знімок збережено"
BOXES = [
    ("S", "Відтворювати звук після знімка"),
    ("C", "Відтворювати звук після «Знімок + порівняння»"),
    ("O", "Лише після операцій, довших за 10 секунд"),
    ("R", "Передавати скрінрідеру повідомлення рядка стану"),
]
missing = []
results = {}


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)
    return ok


def verdict(step, *oks):
    results[step] = all(oks)


def stamp(text):
    print(f"\n>>> [{time.strftime('%H:%M:%S')}] {text}")
    sys.stdout.flush()


def wait_said(app, word, timeout, name):
    """Collects what NVDA says until `word` is in it or `timeout` passes; prints it."""
    said = []
    end = time.time() + timeout
    while time.time() < end:
        said += app.speech.new()
        if any(word in line for line in said):
            break
        time.sleep(0.1)
    time.sleep(0.8)
    said += app.speech.new()
    print(f"\n== {name}")
    for line in said:
        print("   ", line)
    return said


def setting(name):
    conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
    row = conn.execute("SELECT value FROM setting WHERE key = ?", (name,)).fetchone()
    conn.close()
    return row[0] if row else None


def open_settings(app):
    key(VK_MENU, vk("F"))
    time.sleep(0.4)
    key(vk("S"))
    assert wait_for(lambda: foreground_title() == SETTINGS, 5, 0.05), "no settings window"
    time.sleep(0.8)
    return u32.GetForegroundWindow()


def threshold_box(dialog):
    return next(c for c in children(dialog)
                if class_name(c) == "Button" and window_text(c).startswith(BOXES[2][1]))


def bring_back(main):
    """Restores the minimized window and puts it in front."""
    u32.ShowWindow(main, SW_RESTORE)
    time.sleep(0.3)
    if u32.GetForegroundWindow() != main:
        fg_thread = u32.GetWindowThreadProcessId(u32.GetForegroundWindow(), None)
        me = ctypes.windll.kernel32.GetCurrentThreadId()
        u32.AttachThreadInput(me, fg_thread, True)
        u32.SetForegroundWindow(main)
        u32.AttachThreadInput(me, fg_thread, False)
    u32.SetForegroundWindow(main)
    return wait_for(lambda: u32.GetForegroundWindow() == main, 3, 0.05)


def main():
    paths = prepare({"Big": ([BIG], []), "Small": ([TINY], [])}, exe=EXE)
    os.makedirs(TINY)
    for name in ("a.txt", "b.txt", "c.txt"):
        with open(os.path.join(TINY, name), "w", encoding="utf-8") as f:
            f.write(name)
    # Step 1.
    make_settings(SCRATCH, language="uk", sound_only_after_long=0)
    stamp("step 1: scratch settings hold sound_only_after_long = 0")
    verdict(1, check(setting("sound_only_after_long") == 0, "sound_only_after_long = 0 stored"))

    with App(paths, paths["Small"]) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start: Small", 1.5)
        print("   title:", foreground_title())

        # Step 2.
        stamp("step 2: settings, the four check boxes (no sound expected)")
        dialog = open_settings(app)
        app.step("Alt+F, S: «Налаштування»", 0.5)
        key(VK_TAB)
        app.step("Tab: «Тема»", 1.2)
        oks = []
        for letter, label in BOXES:
            key(VK_TAB)
            text = " ".join(app.step(f"Tab: «{label}»", 1.2))
            oks.append(check(label in text, f"heard {label!r}"))
            # The threshold is off: step 1 stored it so; the rest are on by default.
            if letter == "O":
                oks.append(check("не позначено" in text, f"{label!r} said unchecked"))
            else:
                oks.append(check("позначено" in text and "не позначено" not in text,
                                 f"{label!r} said checked"))
        box = threshold_box(dialog)
        key(VK_MENU, vk("S"))
        app.step("Alt+S: sound after a snapshot off", 1.2)
        key(VK_MENU, vk("C"))
        app.step("Alt+C: sound after «Знімок + порівняння» off", 1.2)
        oks.append(check(not u32.IsWindowEnabled(box), "both off: the threshold unavailable"))
        key(VK_MENU, vk("S"))
        app.step("Alt+S: sound after a snapshot on", 1.2)
        oks.append(check(bool(u32.IsWindowEnabled(box)), "one on: the threshold available"))
        key(VK_ESCAPE)
        app.step("Esc: closed, nothing saved", 1.5)
        oks.append(check(setting("sound_after_snapshot") is None
                         and setting("sound_after_snapshot_and_compare") is None,
                         "Esc stored nothing"))
        verdict(2, *oks)

        # Step 3.
        stamp("step 3: SOUND «готово» expected now (F5, window active)")
        key(VK_F5)
        said = wait_said(app, SAVED, 30, "F5: snapshot of Small")
        verdict(3, check(any(SAVED in s for s in said), f"heard {SAVED!r}"))
        app.dismiss_restart_question()

        # Step 4.
        stamp("step 4: SOUND «готово» expected now (F5, window minimized)")
        key(VK_F5)
        u32.ShowWindow(app.main, SW_MINIMIZE)
        time.sleep(3)
        # What NVDA said meanwhile is about whichever window came in front; only revisor's
        # own lines are printed.
        hidden = app.speech.new()
        print(f"   (minimized: {len(hidden)} speech lines, revisor's shown only)")
        for line in hidden:
            if "Знімок" in line:
                print("   ", line)
        back = bring_back(app.main)
        time.sleep(1)
        app.speech.new()  # dropped: the restore may read another window first
        check(back, "revisor back in front")
        print("   title:", foreground_title(), "| snapshot running:", app.progress())
        verdict(4, back, app.progress() is None)
        if not back:
            print("   revisor is not in front: stopping here")
            return

        # Step 5.
        key(VK_MENU, vk("R"))
        time.sleep(0.5)
        key(VK_UP)
        app.step("Alt+R, Up: profile Big", 2.5)
        on_big = check("Big" in foreground_title(), f"on Big: {foreground_title()}")
        stamp("step 5: SOUND «без результату» expected now (F5, Shift+F5 on Big)")
        key(VK_F5)
        time.sleep(0.2)
        key(VK_SHIFT, VK_F5)
        said = wait_said(app, "Знімок скасовано", 20, "F5, Shift+F5: cancelled")
        verdict(5, on_big, check(any("Знімок скасовано" in s for s in said),
                                 "heard «Знімок скасовано»"))
        key(VK_MENU, vk("R"))
        time.sleep(0.5)
        key(VK_DOWN)
        app.step("Alt+R, Down: profile Small", 2.5)
        on_small = check("Small" in foreground_title(), f"on Small: {foreground_title()}")

        # Step 6.
        open_settings(app)
        app.step("settings", 0.5)
        key(VK_MENU, vk("O"))
        text = " ".join(app.step("Alt+O: the threshold on", 1.2))
        key(VK_RETURN)
        app.step("Enter: saved", 1.5)
        stored = check(setting("sound_only_after_long") == 1, "sound_only_after_long = 1 stored")
        stamp("step 6: NO SOUND expected now (F5 on Small, threshold on)")
        key(VK_F5)
        said = wait_said(app, SAVED, 30, "F5: snapshot of Small, threshold on")
        verdict(6, on_small, stored, "Лише після" in text,
                check(any(SAVED in s for s in said), f"heard {SAVED!r}"))

        # Step 7.
        open_settings(app)
        app.step("settings", 0.5)
        key(VK_MENU, vk("R"))
        text = " ".join(app.step("Alt+R: speak status off", 1.2))
        oks = [check("не позначено" in text, "speak status said unchecked")]
        key(VK_RETURN)
        said = app.step("Enter: saved (status not spoken)", 2)
        oks.append(check(not any("Налаштування збережено" in s for s in said),
                         "«Налаштування збережено» not spoken"))
        oks.append(check(setting("speak_status") == 0, "speak_status = 0 stored"))
        stamp("step 7: status bar silent (threshold on, so no sound either)")
        key(VK_F5)
        said = app.step("F5: snapshot of Small, status silent", 3)
        oks.append(check(app.progress() is None, "snapshot ended"))
        oks.append(check(not any(SAVED in s for s in said), f"{SAVED!r} not spoken by itself"))
        key(VK_INSERT, VK_END)
        said = app.step("NVDA+End: the status bar", 2)
        oks.append(check(any(SAVED in s for s in said), f"NVDA+End says {SAVED!r}"))
        verdict(7, *oks)


main()
print("\nsteps:", ", ".join(f"{n} {'PASS' if ok else 'FAIL'}" for n, ok in sorted(results.items())))
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
