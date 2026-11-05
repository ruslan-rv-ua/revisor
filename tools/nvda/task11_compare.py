"""Live NVDA check of TASK-11: comparing the current snapshot with the previous or a chosen one.

    python -X utf8 tools\\nvda\\task11_compare.py "<folder>;<folder>" "<exclusion>;<exclusion>" [parts]

``parts`` is a comma-separated choice of ``commands``, ``busy`` and ``big`` (all by default).

Two profiles: «Small» (a scratch folder with a few files, so comparisons are instant and the
rows are known) and «Big» (the folders given: snapshots of about 800k entries, so a comparison
runs for more than a second and a save holds the database for a while; see README.md). The
checks on «Small» cover the commands, the refusals, the window «Порівняти з», the three
values of «Фокус після порівняння», the re-announced list, a busy database (held from here),
the cancel and the close during a comparison; the checks on «Big» cover the progress row, a
comparison that waits for a snapshot being saved, and a deletion during a comparison.
"""

import os
import sqlite3
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F4, VK_F5, VK_HOME, VK_MENU, VK_RETURN,
    VK_TAB, App, args_folders, focused_class, foreground_title, key, prepare, progress_text,
    u32, visible_controls, vk, wait_for,
)

VK_SHIFT, VK_CONTROL, VK_F6, VK_F7, VK_F10 = 0x10, 0x11, 0x75, 0x76, 0x79
LVM_GETITEMCOUNT = 0x1004

folders, exclusions = args_folders()
parts = set(sys.argv[3].split(",")) if len(sys.argv) > 3 else {"commands", "busy", "big"}
tree = os.path.join(SCRATCH, "tree")
paths = prepare({"Small": ([tree], []), "Big": (folders, exclusions)})


def write(rel, text):
    path = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


class Locked:
    """Holds the profile database exclusively for `seconds`, as a snapshot being committed or
    a compaction by another revisor does: no other connection can read it meanwhile."""

    def __init__(self, db, seconds):
        # Released from the timer's thread.
        self.conn = sqlite3.connect(db, isolation_level=None, check_same_thread=False)
        self.conn.execute("BEGIN EXCLUSIVE")
        self.timer = threading.Timer(seconds, self.release)
        self.timer.start()

    def release(self):
        try:
            self.conn.execute("ROLLBACK")
        except sqlite3.Error:
            pass
        self.conn.close()

    def wait(self):
        self.timer.join()


def difference_rows(main):
    """How many rows the difference list (the last list view of the window) has."""
    lists = visible_controls(main, "SysListView32")
    return u32.SendMessageW(lists[-1], LVM_GETITEMCOUNT, 0, 0) if lists else None


def gauge_shown(main):
    return bool(visible_controls(main, "msctls_progress32"))


def settings(*keys):
    """Opens the settings window, presses `keys` and confirms."""
    key(VK_MENU, vk("F"))
    time.sleep(0.5)
    key(vk("S"))
    assert wait_for(lambda: foreground_title() == "Налаштування", 5, 0.05), "no settings window"
    time.sleep(0.5)
    for k in keys:
        key(*k) if isinstance(k, tuple) else key(k)
        time.sleep(0.4)
    key(VK_RETURN)
    time.sleep(1)


write("a.txt", "a")
write("b.txt", "b")
write(r"sub\c.txt", "c")


def two_small_snapshots(app):
    """Two snapshots of «Small», with a file added and one changed between them."""
    key(VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.05)
    app.step("F5: snapshot 1", 2)
    write("d.txt", "d")
    write("a.txt", "aa")
    key(VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.05)
    app.step("F5: snapshot 2", 2)


def check_commands(app):
    # 2. Quick compare with the focus in the snapshot list: the focus moves to the
    # difference (#1, #12), whose list NVDA reads with its first row.
    key(VK_F7)
    app.step("F7: compare with previous", 3)
    print("   focused:", focused_class(), "| rows:", difference_rows(app.main))
    key(VK_DOWN)
    app.step("Down: the next row", 1.5)

    # 3. F7 with the focus in the difference list: the first row of the new difference, and
    # the list announced again (#21).
    key(VK_F7)
    app.step("F7 in the difference list: refocus", 3)

    # 4. On the oldest snapshot there is no previous one (#4).
    key(VK_F6)
    app.step("F6: back to the snapshot list", 1.5)
    key(VK_HOME)
    key(VK_F7)
    app.step("Home, F7: refused", 2)

    # 5. «Порівняти з…» on the oldest: the window starts on the next snapshot; Enter compares.
    key(VK_CONTROL, VK_F7)
    app.step("Ctrl+F7: the window", 2.5)
    assert foreground_title() == "Порівняти з", foreground_title()
    key(VK_RETURN)
    app.step("Enter: compares with the chosen snapshot", 3)
    print("   focused:", focused_class(), "| rows:", difference_rows(app.main))

    # 6. Esc closes the window without a comparison; the context menu puts the Enter action
    # first (#22); Shift+F7 with no comparison is refused.
    key(VK_F6)
    key(VK_END)
    key(VK_CONTROL, VK_F7)
    app.step("Ctrl+F7 on the newest: the window", 2)
    key(VK_ESCAPE)
    app.step("Esc: closed", 1.5)
    key(VK_SHIFT, VK_F10)
    app.step("Shift+F10: the context menu", 2)
    key(VK_ESCAPE)
    time.sleep(0.5)
    key(VK_SHIFT, VK_F7)
    app.step("Shift+F7 with no comparison: refused", 2)

    # 7. «Ніколи»: the difference arrives without the focus, and NVDA hears the end (#13, #14).
    settings((VK_MENU, vk("M")), VK_END)
    app.step("settings: Never", 2)
    key(VK_F7)
    app.step("F7 with Never: finished, the focus stays", 3)
    print("   focused:", focused_class())

    # 8. «Завжди»: the focus comes to the difference from the profile box (#13).
    settings((VK_MENU, vk("M")), VK_HOME, VK_DOWN)
    app.step("settings: Always", 2)
    key(VK_MENU, vk("R"))
    app.step("Alt+R: the profile box", 1.5)
    key(VK_F7)
    app.step("F7 with Always from the profile box", 3)
    print("   focused:", focused_class())

    # 9. Back to the default; the focus that stayed in the profile box has not moved, so
    # it goes to the difference (#12).
    settings((VK_MENU, vk("M")), VK_HOME)
    app.step("settings: default", 2)
    key(VK_MENU, vk("R"))
    time.sleep(0.5)
    key(VK_F7)
    app.step("F7 from the profile box with the default: unmoved, so to the difference", 3)
    print("   focused:", focused_class())


def check_busy(app):
    # 10. A busy database: the comparison waits, says so after a second and gets its bar
    # after five (#7, #8, #16); then the database is freed and the comparison ends.
    key(VK_F6)
    time.sleep(0.5)
    lock = Locked(paths["Small"], 7)
    key(VK_F7)
    app.step("F7 while the database is held: nothing for a second", 1.5)
    print("   progress:", progress_text(app.main), "| gauge:", gauge_shown(app.main))
    time.sleep(4.2)
    app.step("after 5 s", 0.5)
    print("   progress:", progress_text(app.main), "| gauge:", gauge_shown(app.main))
    lock.wait()
    app.step("the database freed: the comparison ends", 3)
    print("   progress:", progress_text(app.main), "| focused:", focused_class())

    # 11. Cancel while waiting (#9, #10): the difference shown stays.
    rows_before = difference_rows(app.main)
    lock = Locked(paths["Small"], 6)
    key(VK_F7)
    time.sleep(2)
    app.step("F7 while the database is held, 2 s", 0.5)
    key(VK_SHIFT, VK_F7)
    app.step("Shift+F7: cancelled", 3)
    print("   rows before:", rows_before, "| after:", difference_rows(app.main))
    lock.wait()

    # 12. Closing during a comparison: at once, without a question (#18).
    lock = Locked(paths["Small"], 6)
    key(VK_F7)
    time.sleep(1.5)
    app.step("F7 while the database is held", 0.5)
    t0 = time.time()
    key(VK_MENU, VK_F4)
    assert wait_for(lambda: app.proc.poll() is not None, 10, 0.02), "revisor did not close"
    print(f"   closed {time.time() - t0:.2f} s after Alt+F4, exit code {app.proc.poll()}")
    lock.wait()


def check_big(app):
    app.step("start on Big", 2.5)
    for n in (1, 2):
        key(VK_F5)
        assert wait_for(app.snapshot_done, 600, 0.05), f"snapshot {n} did not end"
        app.dismiss_restart_question()
        app.step(f"F5: Big snapshot {n}", 2.5)

    # 13. A comparison of two large snapshots: the progress row after a second (#7), the
    # difference with the focus (#12).
    key(VK_F7)
    t0 = time.time()
    app.step("F7: comparing (1.5 s)", 1.5)
    print("   progress at 1.5 s:", progress_text(app.main))
    assert wait_for(lambda: progress_text(app.main) is None, 60, 0.02)
    print(f"   the row went after {time.time() - t0:.2f} s")
    app.step("the comparison ended", 3)
    print("   focused:", focused_class(), "| rows:", difference_rows(app.main))

    # 14. A comparison started while a snapshot is being saved waits for the database (#16).
    key(VK_F6)
    time.sleep(0.5)
    key(VK_F5)
    assert wait_for(app.saving, 600, 0.02), "no save"
    key(VK_F7)
    app.step("F7 during the save", 1.5)
    print("   progress at 1.5 s:", progress_text(app.main))
    assert wait_for(app.snapshot_done, 120, 0.02)
    app.dismiss_restart_question()
    app.step("the snapshot and the comparison ended", 4)
    print("   focused:", focused_class(), "| rows:", difference_rows(app.main))

    # 15. Deleting the current snapshot right after a comparison started: the deletion waits
    # for the trees to load, and the difference arrives all the same (#17). Without the
    # confirmation, which would wait longer than the loading does.
    settings((VK_MENU, vk("C")))
    app.step("settings: no confirmation", 2)
    key(VK_F6)
    key(VK_END)
    time.sleep(0.5)
    key(VK_F7)
    t0 = time.time()
    time.sleep(0.2)
    key(VK_DELETE)
    app.step("F7, then Delete 0.2 s later", 4)
    print(f"   {time.time() - t0:.2f} s; focused: {focused_class()} | rows: {difference_rows(app.main)}")
    assert wait_for(app.snapshot_done, 120, 0.1)
    app.step("the compaction ended", 3)
    settings((VK_MENU, vk("C")))
    app.step("settings: confirmation back", 2)


if parts & {"commands", "busy"}:
    with App(paths, paths["Small"]) as app:
        app.step("start", 2.5)
        two_small_snapshots(app)
        if "commands" in parts:
            check_commands(app)
        if "busy" in parts:
            check_busy(app)

if "big" in parts:
    with App(paths, paths["Big"]) as app:
        check_big(app)
