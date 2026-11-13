"""Live NVDA check of TASK-40: the export of the difference as CSV.

    python -X utf8 tools\\nvda\\task40_csv.py uk|en

The language goes into the settings file of the scratch folder; the Ukrainian layout is on.
The profile «Програми» watches a small folder of the scratch, which the script changes
between two snapshots, Cyrillic names among them. Nothing is written outside the scratch.

1. F5, a change, Ctrl+F5: a difference. Alt+D, X opens the save dialog with the text report
   first; Tab goes to the list of types, which NVDA reads, and Down chooses «CSV (*.csv)»,
   second (#1). The full path of a file in the scratch without an extension, Enter: the
   status bar says the difference is exported, and the file is CSV: UTF-8 with its mark,
   CRLF, the line of the columns, a line per change, separated by the list separator of
   Windows (#2, #5, #6).
2. The dialog again: it offers CSV, the type chosen last, and a name ending in .csv (#1);
   Esc.
3. Settings: the tab «Експорт» opens on «Роздільник у CSV»; from the check box, Alt+D takes
   the focus back to it, which NVDA reads with its value «Як у Windows»; Down chooses «Кома», Enter saves (#5). An export
   then separates the values by commas.
4. The dialog once more: the text report chosen as the type (Alt+Down opens the list, an
   arrow alone only opens it too), the name of a CSV file typed: the file is the text
   report, as the type says (decided 2026-11-14).

The last line is ``all heard`` or what NVDA did not say or did not happen.
"""

import os
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_F5, VK_MENU, VK_RETURN, VK_SHIFT, VK_TAB, VK_UP,
    App, control_text, focused_class, focused_window, foreground_title, item_count, key,
    make_settings, prepare, snapshot_list, type_text, u32, ukrainian_layout, vk, wait_for,
    window_text,
)

LANG = sys.argv[1] if len(sys.argv) > 1 else "uk"
assert LANG in ("uk", "en"), "the first argument is uk or en"
WATCHED = os.path.join(SCRATCH, "watched")
OUT = os.path.join(SCRATCH, "out")
COLUMNS = (
    "path,change,kind,what_changed,path_before,size_before,size_after,created_before,"
    "created_after,modified_before,modified_after,attributes_before,attributes_after,"
    "file_version_before,file_version_after,product_version_before,product_version_after,"
    "link_target_before,link_target_after,inaccessible_in"
)

WORDS = {
    "uk": {"export": "Експортувати", "exported": "Різницю експортовано: ",
           "text_type": "Текстовий звіт (*.txt)", "csv_type": "CSV (*.csv)",
           "tab": "Експорт", "delimiter": "Роздільник у CSV", "windows": "Як у Windows",
           "comma": "Кома", "title": "Звіт про різницю, revisor "},
    "en": {"export": "Export", "exported": "Difference exported: ",
           "text_type": "Text report (*.txt)", "csv_type": "CSV (*.csv)",
           "tab": "Export", "delimiter": "CSV delimiter", "windows": "Same as Windows",
           "comma": "Comma", "title": "Difference report, revisor "},
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


def type_list(app, name):
    """Tab from the name to the list of types; what NVDA said of it."""
    key(VK_TAB)
    return heard(app.step(name, 1.5))


def choose_type(app, arrow, name):
    """Chooses the type next to the one shown in the list of types, which an arrow only
    opens: open it, move, Enter; what NVDA said."""
    key(VK_MENU, VK_DOWN)
    app.step("Alt+Down: the list open", 1)
    key(arrow)
    said = app.step(name, 1.5)
    key(VK_RETURN)
    said += app.step("Enter: chosen", 1.5)
    return said


def back_to_name(app):
    key(VK_SHIFT, VK_TAB)
    app.step("Shift+Tab: the name", 1)


def export_to(app, path):
    """Types `path` into the name and saves; what NVDA said after."""
    key(VK_CONTROL, vk("A"))
    time.sleep(0.3)
    type_text(path)
    time.sleep(0.5)
    app.speech.new()
    key(VK_RETURN)


def read(path):
    with open(path, "rb") as f:
        return f.read()


def check_csv(data, delimiter):
    check(data.startswith(b"\xef\xbb\xbf"), "UTF-8 with its mark")
    check(b"\r\n" in data and b"\n" not in data.replace(b"\r\n", b""), "CRLF lines")
    lines = data.decode("utf-8-sig").rstrip("\r\n").split("\r\n")
    print("   " + "\n   ".join(lines))
    check(lines[0] == COLUMNS.replace(",", delimiter), f"the columns by {delimiter!r}")
    check(any("звіт.txt" in line and "added" in line for line in lines), "Cyrillic kept")
    check(any(line.split(delimiter)[1:3] == ["deleted", "file"] for line in lines[1:]),
          "a deleted file")
    check(len(lines) == 5, f"a line per change: {len(lines) - 1}")


def stored(key_name):
    conn = sqlite3.connect(os.path.join(SCRATCH, "data", "settings.db"))
    row = conn.execute("SELECT value FROM setting WHERE key = ?", (key_name,)).fetchone()
    conn.close()
    return row


def main():
    paths = prepare({"Програми": ([WATCHED], [])})
    os.makedirs(WATCHED)
    os.makedirs(OUT)
    write("a.txt", "a")
    write("b.txt", "b")
    make_settings(SCRATCH, language=LANG)
    with App(paths) as app:
        ukrainian_layout(app.main)
        time.sleep(0.5)
        app.step("start", 2)

        # 1. A difference, CSV chosen in the dialog.
        key(VK_F5)
        snapshots(app, 1)
        app.step("F5: the snapshot before", 1.5)
        write("b.txt", "bbbbbbbb")
        write("звіт.txt", "c")
        os.remove(os.path.join(WATCHED, "a.txt"))
        os.makedirs(os.path.join(WATCHED, "нова тека"))
        time.sleep(1)
        key(VK_CONTROL, VK_F5)
        snapshots(app, 2)
        app.step("Ctrl+F5: the difference", 3)
        open_export(app, "Alt+D, X: the save dialog")
        offered = file_name_field()
        print("   file name offered:", offered)
        check(offered.endswith(".txt"), "the text report's name first")
        heard([type_list(app, "Tab: the list of types")], WORDS["text_type"])
        heard(choose_type(app, VK_DOWN, "Down: CSV"), WORDS["csv_type"])
        back_to_name(app)
        print("   file name now:", file_name_field())
        first = os.path.join(OUT, "різниця")
        export_to(app, first)
        written = first + ".csv"
        assert wait_for(lambda: os.path.exists(written), 10), f"no CSV: {os.listdir(OUT)}"
        heard(app.step("Enter: exported", 2), WORDS["exported"] + "різниця.csv.")
        check_csv(read(written), ";")
        check(stored("export_format") == ("csv",), f"CSV remembered: {stored('export_format')}")

        # 2. The type chosen last comes first.
        open_export(app, "Alt+D, X: CSV first now")
        offered = file_name_field()
        print("   file name offered:", offered)
        check(offered.endswith(".csv"), "a name ending in .csv")
        heard([type_list(app, "Tab: the list of types")], WORDS["csv_type"])
        key(VK_ESCAPE)
        app.step("Esc: no export", 1.5)

        # 3. The setting «Роздільник у CSV».
        alt("F")
        time.sleep(0.4)
        key(vk("S"))
        app.step("Alt+F, S: settings", 2.5)
        key(VK_CONTROL, VK_SHIFT, VK_TAB)
        # The tab's first control, the list, takes the focus with it.
        heard(app.step("Ctrl+Shift+Tab: the last tab", 1.5), WORDS["tab"], WORDS["delimiter"])
        key(VK_TAB)
        app.step("Tab: the check box", 1)
        alt("D")
        heard(app.step("Alt+D: the delimiter", 1.5), WORDS["delimiter"], WORDS["windows"])
        key(VK_DOWN)
        heard(app.step("Down: comma", 1.5), WORDS["comma"])
        key(VK_RETURN)
        app.step("Enter: saved", 2)
        check(stored("csv_delimiter") == ("comma",), f"comma saved: {stored('csv_delimiter')}")
        open_export(app, "Alt+D, X: with commas")
        comma = os.path.join(OUT, "кома.csv")
        export_to(app, comma)
        assert wait_for(lambda: os.path.exists(comma), 10), f"no CSV: {os.listdir(OUT)}"
        app.step("Enter: exported", 2)
        check_csv(read(comma), ",")

        # 4. The type says the format, whatever the name.
        open_export(app, "Alt+D, X: the text report by its type")
        type_list(app, "Tab: the list of types")
        heard(choose_type(app, VK_UP, "Up: the text report"), WORDS["text_type"])
        back_to_name(app)
        named = os.path.join(OUT, "звіт.csv")
        export_to(app, named)
        found = wait_for(lambda: any(n.startswith("звіт") for n in os.listdir(OUT)), 10)
        app.step("Enter: exported", 2)
        names = sorted(os.listdir(OUT))
        print("   files:", names)
        report = [n for n in names if n.startswith("звіт")]
        check(found and report, "a file of the name typed")
        if report:
            text = read(os.path.join(OUT, report[0])).decode("utf-8-sig")
            check(text.startswith(WORDS["title"]), f"the text report: {text[:40]!r}")
        check(stored("export_format") == ("text",), "the text report remembered")
        u32.SetForegroundWindow(app.main)
        time.sleep(1)
        app.speech.new()


main()
print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
