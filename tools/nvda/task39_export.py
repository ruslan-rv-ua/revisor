"""Live NVDA check of TASK-39: the export of the difference to a text report.

    python -X utf8 tools\\nvda\\task39_export.py uk|en

The language goes into the settings file of the scratch folder; the Ukrainian layout is on.
The profile «Програми» watches a small folder of the scratch, which the script changes
between two snapshots. Nothing is written outside the scratch: the first save dialog, which
starts in «Документи», is only looked at and closed.

1. Before any comparison, the item «Експортувати…» highlighted in the menu «Різниця» says
   why it is unavailable (#1).
2. F5, a change, Ctrl+F5: a difference. Alt+D, X opens the save dialog titled after the
   command, with the name of the profile and both snapshots (#2); Esc closes it.
3. The dialog again: the full path of a file in the scratch, Enter: the status bar says the
   difference is exported and names the file, the focus stays where it was (#9). The file is
   UTF-8 with its mark, lines end in CRLF, and it starts with the version line (#4).
4. The dialog again, now in the scratch folder: Enter on the same name asks before it
   overwrites (#2); «Ні», Esc, and the file is as it was.
5. Settings: the tab «Експорт» with «Відкривати файл після експорту», Alt+O toggles it,
   Enter saves; an export then opens the file in its program, which the script closes (#9).

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import os
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_CONTROL, VK_ESCAPE, VK_F5, VK_MENU, VK_RETURN, VK_SHIFT, VK_TAB, App,
    class_name, control_text, focused_class, focused_window, foreground_title, key,
    make_settings, prepare, snapshot_list, item_count, type_text, u32, ukrainian_layout, vk,
    wait_for, window_text,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"
WATCHED = os.path.join(SCRATCH, "watched")
OUT = os.path.join(SCRATCH, "out")
WM_CLOSE = 0x0010

WORDS = {
    "uk": {"export": "Експортувати", "unavailable": "Експортувати…: різниці ще немає.",
           "exported": "Різницю експортовано: ", "title": "Звіт про різницю, revisor ",
           "settings": "Налаштування", "tab": "Експорт",
           "open": "Відкривати файл після експорту", "group": "Додано: "},
    "en": {"export": "Export", "unavailable": "Export…: there is no difference yet.",
           "exported": "Difference exported: ", "title": "Difference report, revisor ",
           "settings": "Settings", "tab": "Export",
           "open": "Open the file after export", "group": "Added: "},
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


def write(name, text):
    with open(os.path.join(WATCHED, name), "w", encoding="utf-8") as f:
        f.write(text)


def snapshots(app, count):
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == count, 60), "no snapshot"
    assert wait_for(app.snapshot_done, 60, 0.02), "the progress row never went"


def export_dialog():
    h = u32.GetForegroundWindow()
    return h if window_text(h) == WORDS["export"] else None


def open_export(app, name):
    alt("D")
    time.sleep(0.5)
    key(vk("X"))
    said = app.step(name, 2.5)
    check(wait_for(export_dialog, 3), "the save dialog opened, titled after the command")
    return said


def where():
    print(f"   foreground {foreground_title()!r}, focus {focused_class()}")


def file_name_field():
    return control_text(focused_window())


def main():
    paths = prepare({"Програми": ([WATCHED], [])})
    os.makedirs(WATCHED)
    os.makedirs(OUT)
    write("a.txt", "a")
    write("b.txt", "b")
    make_settings(SCRATCH, language=LANG)
    report = os.path.join(OUT, "report.txt")
    with App(paths) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start", 2)

        # 1. Unavailable before a comparison: the highlighted item says why.
        alt("D")
        app.step("Alt+D: the menu «Різниця»", 1.5)
        for _ in range(30):
            key(0x26)  # Up, from the top round to the bottom group
            time.sleep(0.15)
            if WORDS["unavailable"] in " ".join(app.speech.new()):
                break
        app.step("Up to «Експортувати…»", 1.5)
        key(VK_ESCAPE)
        key(VK_ESCAPE)
        app.step("Esc, Esc", 1)
        # The search above consumed the speech; say again what the status bar holds.
        key(0x2D, 0x23)  # NVDA+End
        heard(app.step("NVDA+End: the last message", 2), WORDS["unavailable"])

        # 2. A difference, and the dialog with the name offered.
        key(VK_F5)
        snapshots(app, 1)
        app.step("F5: the snapshot before", 1.5)
        write("b.txt", "bbbbbbbb")
        write("c.txt", "c")
        os.remove(os.path.join(WATCHED, "a.txt"))
        os.makedirs(os.path.join(WATCHED, "new folder"))
        time.sleep(1)
        key(VK_CONTROL, VK_F5)
        snapshots(app, 2)
        app.step("Ctrl+F5: the difference", 3)
        before = focused_window()
        print("   focus before the export:", class_name(before))
        said = open_export(app, "Alt+D, X: the save dialog")
        heard(said, WORDS["export"])
        offered = file_name_field()
        print("   file name offered:", offered)
        check(offered.startswith("Програми ") and " — " in offered and ":" not in offered,
              "the name holds the profile and both moments without colons")
        key(VK_ESCAPE)
        app.step("Esc: no export", 1.5)
        check(not os.listdir(OUT), "nothing written")

        # 3. Export to the scratch.
        open_export(app, "Alt+D, X again")
        key(VK_CONTROL, vk("A"))
        time.sleep(0.3)
        type_text(report)
        time.sleep(0.5)
        app.speech.new()
        key(VK_RETURN)
        assert wait_for(lambda: os.path.exists(report), 10), "no report written"
        said = app.step("Enter: exported", 2)
        heard(said, WORDS["exported"] + "report.txt.")
        check(focused_window() == before, f"the focus stayed: {focused_class()}")
        with open(report, "rb") as f:
            data = f.read()
        check(data.startswith(b"\xef\xbb\xbf"), "UTF-8 with its mark")
        check(b"\r\n" in data and b"\n" not in data.replace(b"\r\n", b""), "CRLF lines")
        text = data.decode("utf-8-sig")
        lines = text.split("\r\n")
        check(lines[0].startswith(WORDS["title"]), f"the first line: {lines[0]}")
        check(any(line.startswith(WORDS["group"]) for line in lines), "a group of added")
        print("   " + "\n   ".join(lines))
        check(sorted(os.listdir(OUT)) == ["report.txt"], f"only the report: {os.listdir(OUT)}")

        # 4. The same name again: the dialog asks before it overwrites.
        open_export(app, "Alt+D, X: in the folder of the last export")
        print("   file name offered:", file_name_field())
        key(VK_CONTROL, vk("A"))
        time.sleep(0.3)
        type_text("report.txt")
        time.sleep(0.5)
        key(VK_RETURN)
        said = app.step("Enter on report.txt: the question", 2.5)
        print("   foreground:", foreground_title())
        check(not export_dialog(), "a question over the dialog")
        # The question has Yes and No only, so Esc does nothing there; the focus is on No.
        key(VK_RETURN)
        app.step("Enter on «Ні»: back to the dialog", 1.5)
        where()
        key(VK_ESCAPE)
        app.step("Esc: closed", 1.5)
        where()
        with open(report, "rb") as f:
            check(f.read() == data, "the report is as it was")

        # 5. The setting, and the file opened after the export.
        alt("F")
        time.sleep(0.4)
        key(vk("S"))
        app.step("Alt+F, S: «Налаштування»", 2.5)
        where()
        key(VK_CONTROL, VK_SHIFT, VK_TAB)
        heard(app.step("Ctrl+Shift+Tab: the last tab", 1.5), WORDS["tab"])
        alt("O")
        said = " ".join(app.step("Alt+O: the check box", 1.5))
        check("не позначено" not in said and "позначено" in said, "Alt+O ticked the check box")
        key(VK_RETURN)
        app.step("Enter: saved", 2)
        conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
        stored = conn.execute(
            "SELECT value FROM setting WHERE key = 'open_after_export'"
        ).fetchone()
        conn.close()
        check(stored == (1,), f"the setting is saved on: {stored}")
        second = os.path.join(OUT, "opened.txt")
        open_export(app, "Alt+D, X: to be opened")
        key(VK_CONTROL, vk("A"))
        time.sleep(0.3)
        type_text(second)
        time.sleep(0.5)
        key(VK_RETURN)
        assert wait_for(lambda: os.path.exists(second), 10), "no second report"
        opened = wait_for(lambda: "opened" in foreground_title(), 10)
        title = foreground_title()
        app.step("the file opened", 2)
        check(opened, f"the file opened in its program: {title!r}")
        if opened:
            u32.PostMessageW(u32.GetForegroundWindow(), WM_CLOSE, 0, 0)
            time.sleep(1.5)
        u32.SetForegroundWindow(app.main)
        time.sleep(1)
        app.speech.new()


main()
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
