"""Live NVDA check of TASK-16: the language setting and the restart it offers.

    python -X utf8 tools\\nvda\\task16_language.py ["<folder>;<folder>" ["<exclusion>;…"]]

On a Ukrainian Windows the interface starts Ukrainian. The steps: the settings window names
«Мова (L):» with «Як у Windows»; «English» + OK says the language changes after a restart and
«Пізніше» (Esc) keeps the window; «Українська» + OK changes nothing and asks nothing; during a
snapshot of the folders given (default C:\\Windows) «Перезапустити зараз» first asks the same
question as closing revisor; and finally «Перезапустити зараз» without a snapshot restarts
revisor, whose settings window then reads «Language:».
"""

import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    VK_DOWN, VK_ESCAPE, VK_F4, VK_F5, VK_INSERT, VK_MENU, VK_RETURN, App, args_folders,
    find_main, foreground_title, key, prepare, u32, vk, wait_for,
)

folders, exclusions = args_folders()
paths = prepare({
    "Small": ([os.path.dirname(os.path.abspath(__file__))], []),
    "Windows": (folders, exclusions),
})


def open_settings(app, name):
    key(VK_MENU, vk("F"))
    time.sleep(0.4)
    key(vk("S"))
    app.step(name, 2)
    assert foreground_title() == "Налаштування", foreground_title()


def choose_english(app):
    """In the language box (focused when the window opens): E picks «English»."""
    key(vk("E"))
    app.step("E: English", 1)


def read_dialog(app, name):
    """NVDA+B reads the dialog's text (object descriptions are off)."""
    key(VK_INSERT, vk("B"))
    return app.step(name, 3)


with App(paths, paths["Small"]) as app:
    app.step("start (Ukrainian Windows: Ukrainian interface)", 2.5)

    # 1. The language box comes first and holds «Як у Windows».
    open_settings(app, "Alt+F, S: settings window, focus on «Мова (L):»")

    # 2. «English», OK: the settings are saved, the question is up; Esc = «Пізніше».
    key(VK_DOWN)
    app.step("Down: English", 1.5)
    key(VK_RETURN)
    app.step("Enter (OK): «Налаштування збережено», then the restart question", 2.5)
    assert foreground_title() == "Налаштування", foreground_title()
    read_dialog(app, "NVDA+B: the question's text and buttons")
    key(VK_ESCAPE)
    app.step("Esc = «Пізніше»: back in the main window, still Ukrainian", 2)
    assert foreground_title().endswith("revisor"), foreground_title()

    # 3. The setting is kept: the box opens on «English». «Українська» + OK asks nothing, as
    # the interface is Ukrainian already.
    open_settings(app, "Alt+F, S again: the box says English")
    key(VK_DOWN)
    app.step("Down: Українська", 1.5)
    key(VK_RETURN)
    app.step("Enter (OK): saved, no question", 2.5)
    assert foreground_title().endswith("revisor"), foreground_title()

    # 4. During a snapshot: «Перезапустити зараз» first asks the closing question.
    key(VK_MENU, vk("R")); time.sleep(0.5); key(VK_DOWN)  # Alt+R, Down: «Windows»
    app.step("Alt+R, Down: profile Windows", 2)
    # A warm-cache snapshot of ~830k entries ends in a few seconds, so the keys go fast and
    # the speech is read once at the end.
    key(VK_F5)
    time.sleep(0.3)
    key(VK_MENU, vk("F")); time.sleep(0.3); key(vk("S")); time.sleep(0.4)
    key(vk("E")); time.sleep(0.2)
    key(VK_RETURN); time.sleep(0.5)
    key(VK_RETURN)
    app.step("F5, Alt+F, S, E, Enter, Enter = «Перезапустити зараз» during the snapshot", 2.5)
    assert foreground_title() == "Вихід", foreground_title()
    read_dialog(app, "NVDA+B: «Триває знімок…»")
    key(VK_ESCAPE)
    app.step("Esc: cancelled, revisor and the snapshot go on", 2)
    assert foreground_title().endswith("revisor"), foreground_title()
    # The snapshot goes on, or ended under the question and was saved; either way it is
    # not lost. One still running is cancelled so the restart below is a plain one.
    if app.progress() is not None:
        key(VK_MENU, vk("N"))  # «Скасувати (N)»
        app.step("Alt+N: snapshot cancelled", 2.5)
    app.dismiss_restart_question()

    # 5. Without a snapshot: «Перезапустити зараз» restarts revisor on the same profile; the
    # new one speaks English.
    old_main, old_proc = app.main, app.proc
    open_settings(app, "Alt+F, S: English is kept")
    choose_english(app)
    key(VK_RETURN)
    app.step("Enter (OK): the restart question", 2.5)
    key(VK_RETURN)
    print("   (stop reading speech while the windows change)")
    assert wait_for(lambda: old_proc.poll() is not None, 10), "the old revisor did not exit"
    assert wait_for(lambda: find_main() not in (None, old_main), 15), "no new main window"
    app.main = find_main()
    time.sleep(1.5)
    u32.SetForegroundWindow(app.main)
    app.speech.new()  # drop what NVDA read while the windows changed
    app.step("the restarted revisor", 2.5)
    key(VK_MENU, vk("F"))
    time.sleep(0.4)
    key(vk("S"))
    app.step("Alt+F, S in the restarted revisor: «Language:», English", 2.5)
    assert foreground_title() == "Settings", foreground_title()
    key(VK_ESCAPE)
    time.sleep(1)
    # The restarted revisor is not the process App started: close it by its window.
    print("\n(stop reading speech) closing the restarted revisor")
    u32.SetForegroundWindow(app.main)
    time.sleep(0.5)
    key(VK_MENU, VK_F4)
    assert wait_for(lambda: find_main() is None, 10), "the restarted revisor did not close"
    app.proc = subprocess.Popen(["cmd", "/c", "exit"])  # a finished stand-in for __exit__
