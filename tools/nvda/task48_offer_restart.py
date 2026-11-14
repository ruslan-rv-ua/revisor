"""Live NVDA check of TASK-48: the offer to restart as administrator can be turned off.

    python -X utf8 tools\\nvda\\task48_offer_restart.py [exe]

The exe goes first (the debug build by default). The interface is Ukrainian, the Ukrainian
layout is on. One profile, «Small»: a scratch folder of three files and a subfolder the script
closes with ``icacls /deny`` (opened again at the end), so every snapshot has denied entries
and revisor, run as a standard user, offers a restart.

1. F5: the question «Зробити знімок» comes; NVDA+B reads its text; Tab goes from «Скасувати»
   to the check box «Більше не пропонувати», unticked, then to «Перезапустити», «Скасувати».
2. Alt+D ticks the check box and the question stays; Esc closes it; nothing more is said of
   it, and ``offer_restart = 0`` is stored.
3. F5: no question; NVDA says the snapshot with its inaccessible entries.
4. Settings, «Знімки»: Tab to the last check box, «Пропонувати перезапуск від імені
   адміністратора після знімка», unticked; Alt+R ticks it; Enter stores ``offer_restart = 1``.
5. Ctrl+F5: the question «Знімок + порівняння» comes; Esc without the check box keeps the
   setting on.
6. Ctrl+F5: the question again; Alt+D, then Enter (the default «Скасувати») turns it off.
7. Ctrl+F5: no question.

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import os
import sqlite3
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    REPO, SCRATCH, VK_CONTROL, VK_END, VK_ESCAPE, VK_F5, VK_INSERT, VK_MENU, VK_RETURN, VK_TAB,
    App, children, class_name, foreground_title, key, make_settings, prepare, u32,
    ukrainian_layout, vk, wait_for, window_text,
)

EXE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "target", "debug", "revisor.exe")
TINY = os.path.join(SCRATCH, "tiny")
CLOSED = os.path.join(TINY, "closed")
SNAPSHOT = "Зробити знімок"
SNAPSHOT_AND_COMPARE = "Знімок + порівняння"
SETTINGS = "Налаштування"
DONT = "Більше не пропонувати"
OFFER = "Пропонувати перезапуск від імені адміністратора після знімка"
missing = []
results = {}


def check(ok, what):
    print(f"   {'ok' if ok else '!!'}: {what}")
    if not ok:
        missing.append(what)
    return ok


def verdict(step, *oks):
    results[step] = all(oks)


def setting(name):
    conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
    row = conn.execute("SELECT value FROM setting WHERE key = ?", (name,)).fetchone()
    conn.close()
    return row[0] if row else None


def deny(folder, on):
    user = os.environ["USERNAME"]
    args = ["/deny", f"{user}:(RD)"] if on else ["/remove:d", user]
    subprocess.run(["icacls", folder, *args], check=True, capture_output=True)


def question_up(title, timeout=30):
    return wait_for(lambda: foreground_title() == title, timeout, 0.05)


def check_box_of(dialog, label):
    return next((c for c in children(dialog)
                 if class_name(c) == "Button" and window_text(c).startswith(label)), None)


def main_in_front(app):
    return wait_for(lambda: u32.GetForegroundWindow() == app.main, 5, 0.05)


def main():
    paths = prepare({"Small": ([TINY], [])}, exe=EXE)
    os.makedirs(CLOSED)
    for folder in (TINY, CLOSED):
        for name in ("a.txt", "b.txt", "c.txt"):
            with open(os.path.join(folder, name), "w", encoding="utf-8") as f:
                f.write(name)
    deny(CLOSED, True)
    make_settings(SCRATCH, language="uk")
    try:
        with App(paths, paths["Small"]) as app:
            ukrainian_layout(app.main)
            time.sleep(0.5)
            app.step("start: Small", 1.5)
            run(app)
    finally:
        deny(CLOSED, False)


def run(app):
    # Step 1.
    key(VK_F5)
    up = check(question_up(SNAPSHOT), f"F5: the question «{SNAPSHOT}» is up")
    said = " ".join(app.step("F5: the question", 1.5))
    dialog = u32.GetForegroundWindow()
    box = check_box_of(dialog, DONT)
    oks = [up, check(box is not None, "the question has the check box"),
           check("Скасувати" in said, "the focus on «Скасувати»")]
    key(VK_INSERT, vk("B"))
    said = " ".join(" ".join(app.step("NVDA+B: the text", 7)).split())
    oks.append(check("Записів, недоступних через права: 1. Перезапустити revisor" in said,
                     "the text unchanged"))
    key(VK_TAB)
    said = " ".join(app.step("Tab: the check box", 1.2))
    oks.append(check(DONT in said and "не позначено" in said, f"{DONT!r} said unchecked"))
    key(VK_TAB)
    said = " ".join(app.step("Tab: «Перезапустити»", 1.2))
    oks.append(check("Перезапустити" in said, "then «Перезапустити»"))
    key(VK_TAB)
    said = " ".join(app.step("Tab: «Скасувати»", 1.2))
    oks.append(check("Скасувати" in said, "then «Скасувати»"))
    verdict(1, *oks)

    # Step 2.
    key(VK_MENU, vk("D"))
    said = " ".join(app.step("Alt+D: ticked", 1.2))
    oks = [check("позначено" in said and "не позначено" not in said, "Alt+D: said checked"),
           check(foreground_title() == SNAPSHOT, "the question stays up")]
    key(VK_ESCAPE)
    oks.append(check(main_in_front(app), "Esc closed the question"))
    said = app.step("Esc: closed", 2)
    oks.append(check(not any("Налаштування" in s for s in said), "nothing said of the setting"))
    oks.append(check(setting("offer_restart") == 0, "offer_restart = 0 stored"))
    verdict(2, *oks)

    # Step 3.
    key(VK_F5)
    said = " ".join(app.step("F5: no question", 4))
    oks = [check(u32.GetForegroundWindow() == app.main, "no question came"),
           check("Знімок збережено" in said, "heard «Знімок збережено»"),
           check("Недоступних записів: 1" in said, "the inaccessible entries said")]
    verdict(3, *oks)

    # Step 4.
    key(VK_MENU, vk("F"))
    time.sleep(0.4)
    key(vk("S"))
    oks = [check(question_up(SETTINGS, 5), "settings open")]
    app.step("Alt+F, S: settings", 1)
    key(VK_CONTROL, VK_TAB)
    app.step("Ctrl+Tab: «Знімки»", 1.5)
    for _ in range(3):
        key(VK_TAB)
        said = " ".join(app.step("Tab", 1.2))
    oks.append(check(OFFER in said and "не позначено" in said, f"last: {OFFER!r}, unchecked"))
    key(VK_MENU, vk("R"))
    said = " ".join(app.step("Alt+R: ticked", 1.2))
    oks.append(check("позначено" in said and "не позначено" not in said, "Alt+R: said checked"))
    key(VK_RETURN)
    app.step("Enter: saved", 1.5)
    oks.append(check(setting("offer_restart") == 1, "offer_restart = 1 stored"))
    verdict(4, *oks)

    # Step 5.
    key(VK_CONTROL, VK_F5)
    up = check(question_up(SNAPSHOT_AND_COMPARE), f"Ctrl+F5: «{SNAPSHOT_AND_COMPARE}» is up")
    said = " ".join(app.step("Ctrl+F5: the question", 1.5))
    key(VK_ESCAPE)
    oks = [up, check(main_in_front(app), "Esc closed it")]
    app.step("Esc: closed", 3)
    oks.append(check(setting("offer_restart") == 1, "offer_restart still 1"))
    verdict(5, *oks)

    # Step 6.
    key(VK_CONTROL, VK_F5)
    up = check(question_up(SNAPSHOT_AND_COMPARE), "Ctrl+F5: the question again")
    app.step("Ctrl+F5: the question", 1.5)
    key(VK_MENU, vk("D"))
    app.step("Alt+D: ticked", 1.2)
    key(VK_RETURN)
    oks = [up, check(main_in_front(app), "Enter (Скасувати) closed it")]
    app.step("Enter: closed", 3)
    oks.append(check(setting("offer_restart") == 0, "offer_restart = 0 stored"))
    verdict(6, *oks)

    # Step 7.
    key(VK_CONTROL, VK_F5)
    said = " ".join(app.step("Ctrl+F5: no question", 5))
    verdict(7, check(u32.GetForegroundWindow() == app.main, "no question came"),
            check("Недоступних записів: 1" in said, "the inaccessible entries said"))
    key(VK_INSERT, VK_END)
    app.step("NVDA+End", 2)


main()
print("\nsteps:", ", ".join(f"{n} {'PASS' if ok else 'FAIL'}" for n, ok in sorted(results.items())))
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
