"""Live NVDA check of TASK-12: «Знімок + порівняння» takes a snapshot and shows its difference at once.

    python -X utf8 tools\\nvda\\task12_snapshot_and_compare.py "<folder>;<folder>" "<exclusion>;<exclusion>" [parts]

``parts`` is a comma-separated choice of ``small``, ``big`` and ``profiles`` (all by default).

Two profiles, as in task11_compare.py: «Small» (a scratch folder with a few files, so snapshots
and comparisons take a moment and the rows are known) and «Big» (the folders given and the
scratch folder: about 800k entries, so a snapshot takes seconds and its comparison more than
one; see README.md). In the profile box «Big» comes before «Small». On «Small»: the command on a
profile without snapshots (#4), the usual run with the focus moving to the difference (#1, #5,
#6, #8), the comparison with the newest snapshot rather than the current one (#1), «Ніколи» (#5,
#6), a snapshot that fails on a busy database (#10), and the restart offer after the difference
(#8). On «Big» (part ``big``): the refusal during a snapshot in the menu and by the key (#3), the
progress row from the snapshot to its comparison without a gap (#5, #9), the cancels by the
button and by the keys in both stages (#9, #13), the focus on the progress row (#5), and a
comparison started during the snapshot (#11). Part ``profiles``: another profile opened during
the command with «Завжди» and with the default (#7), and closing in both stages (#12). A warm
snapshot of «Big» takes a second or two, so the keys of the snapshot stage follow Ctrl+F5 at once.
"""

import os
import sqlite3
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_DOWN, VK_END, VK_ESCAPE, VK_F4, VK_F5, VK_HOME, VK_INSERT, VK_MENU, VK_RETURN,
    VK_UP, App, args_folders, class_name, focused_window, foreground_title, key, prepare,
    progress_text, u32, visible_controls, vk, wait_for,
)

VK_SHIFT, VK_CONTROL, VK_F6, VK_F7 = 0x10, 0x11, 0x75, 0x76
LVM_GETITEMCOUNT, LVM_GETNEXTITEM, LVNI_FOCUSED = 0x1004, 0x100C, 0x0001
COMMAND = "Знімок + порівняння"

folders, exclusions = args_folders()
parts = set(sys.argv[3].split(",")) if len(sys.argv) > 3 else {"small", "big", "profiles"}
tree = os.path.join(SCRATCH, "tree")
paths = prepare({"Small": ([tree], []), "Big": (folders + [tree], exclusions)})


def write(rel, text):
    path = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


class Locked:
    """Holds the profile database exclusively for `seconds`: no other connection can read it
    meanwhile, as none can while another revisor compacts it."""

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


def deny(folder):
    """Closes `folder` to listing for this user, so a snapshot finds it denied by rights."""
    subprocess.run(["icacls", folder, "/deny", f"{os.environ['USERNAME']}:(RD)"],
                   check=True, capture_output=True)


def allow(folder):
    subprocess.run(["icacls", folder, "/remove:d", os.environ["USERNAME"]],
                   check=True, capture_output=True)


def lists(main):
    """The visible list views of the window: the snapshot list, then the difference list shown."""
    return visible_controls(main, "SysListView32")


def rows(h):
    return u32.SendMessageW(h, LVM_GETITEMCOUNT, 0, 0) if h else None


def focused_row(h):
    return u32.SendMessageW(h, LVM_GETNEXTITEM, -1, LVNI_FOCUSED) if h else None


def where(main):
    """Where the keyboard focus is, in the window's terms."""
    h = focused_window()
    shown = lists(main)
    if shown and h == shown[0]:
        return "snapshot list"
    if h in shown[1:]:
        return "difference list"
    return class_name(h) if h else "nothing"


def report(app):
    shown = lists(app.main)
    snapshots = shown[0] if shown else None
    difference = shown[-1] if len(shown) > 1 else None
    print(f"   focus: {where(app.main)} | snapshots: {rows(snapshots)} "
          f"(focused row {focused_row(snapshots)}) | difference rows: {rows(difference)} "
          f"| title: {foreground_title()}")


def phase(text):
    if text is None:
        return None
    if text.startswith("Порівняння"):
        return "comparison"
    if text.startswith("Чекаю"):
        return "waiting"
    if text.startswith("Збереження"):
        return "saving"
    return "scanning"


def watch_progress(app, timeout=600, every=0.01):
    """The progress row's phases and the focus, as they change, until the row goes: a gap
    between the snapshot and its comparison would show as None between them."""
    seen = []
    end = time.time() + timeout
    while time.time() < end:
        state = (phase(progress_text(app.main)), where(app.main))
        if not seen or seen[-1] != state:
            seen.append(state)
        if state[0] is None and any(s[0] for s in seen):
            return seen
        time.sleep(every)
    raise AssertionError(f"the progress row never went: {seen}")


def comparing(app):
    return phase(progress_text(app.main)) == "comparison"


def finished(app, timeout=600):
    """Waits for the jobs the progress row shows to end: for the row to come, unless it has come
    and gone already, and to go."""
    wait_for(lambda: app.progress() is not None, 1, 0.01)
    assert wait_for(app.snapshot_done, timeout, 0.02), "the progress row never went"


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


def snapshot_list(app):
    """Puts the focus in the snapshot list."""
    for _ in range(2):
        if where(app.main) == "snapshot list":
            return
        key(VK_F6)
        time.sleep(0.5)
    assert where(app.main) == "snapshot list", where(app.main)


write("a.txt", "a")
write("b.txt", "b")
write(r"sub\c.txt", "c")


def check_small(app):
    app.step("start: Small, without snapshots", 2.5)

    # 1. Without snapshots: the snapshot, and nothing to compare it with (#4); the focus stays
    # in the list (#6).
    key(VK_CONTROL, VK_F5)
    finished(app)
    app.step("Ctrl+F5 on the profile without snapshots", 2)
    report(app)

    # 2. A file added and one changed: the difference of the newest snapshot and the new one,
    # with the focus on it, then the report (#1, #5, #8); the new snapshot is current (#6).
    write("d.txt", "d")
    write("a.txt", "aa")
    key(VK_CONTROL, VK_F5)
    finished(app)
    app.step("Ctrl+F5: the snapshot and its difference", 3)
    report(app)
    key(VK_DOWN)
    app.step("Down: the next row", 1.5)

    # 3. The newest snapshot, not the current one: from the oldest snapshot the difference
    # holds only what changed since the newest (#1).
    snapshot_list(app)
    key(VK_HOME)
    app.step("F6, Home: the oldest snapshot current", 1.5)
    write("e.txt", "e")
    key(VK_CONTROL, VK_F5)
    finished(app)
    app.step("Ctrl+F5 on the oldest snapshot: e.txt only", 3)
    report(app)

    # 4. «Ніколи»: the difference arrives without the focus, which stays on its row in the list;
    # the new snapshot is added but not made current (#5, #6).
    settings((VK_MENU, vk("M")), VK_END)
    app.step("settings: Never", 2)
    snapshot_list(app)
    key(VK_HOME)
    time.sleep(0.5)
    write("f.txt", "f")
    key(VK_CONTROL, VK_F5)
    finished(app)
    app.step("Ctrl+F5 with Never, on the first row", 3)
    report(app)
    settings((VK_MENU, vk("M")), VK_HOME)
    app.step("settings: default", 2)

    # 5. A snapshot that fails (#10): the database held longer than a snapshot waits for it
    # (5 s); the reason in the status bar and a message box, and no comparison.
    lock = Locked(paths["Small"], 8)
    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: foreground_title() == COMMAND, 20, 0.05), foreground_title()
    app.step("Ctrl+F5 on a held database: the message box", 1)
    key(VK_INSERT, vk("B"))
    app.step("NVDA+B: its text", 3)
    key(VK_RETURN)
    app.step("Enter: closed", 2)
    lock.wait()
    report(app)

    # 6. The restart offer after the difference (#8): a folder closed to listing.
    locked = os.path.join(tree, "locked")
    write(r"locked\x.txt", "x")
    deny(locked)
    try:
        snapshot_list(app)
        key(VK_CONTROL, VK_F5)
        assert wait_for(lambda: foreground_title() == COMMAND, 60, 0.05), foreground_title()
        app.step("Ctrl+F5 with a closed folder: the difference, then the question", 1.5)
        key(VK_INSERT, vk("B"))
        app.step("NVDA+B: the question", 4)
        key(VK_ESCAPE)
        app.step("Esc: not now; the report", 3)
        report(app)
    finally:
        allow(locked)


def snapshot_before(app):
    """Starts on Big with a snapshot to compare the command's with."""
    app.step("start on Big", 2.5)
    snapshot_list(app)
    if rows(lists(app.main)[0]) == 0:
        key(VK_F5)
        finished(app)
        app.step("F5: the snapshot before", 2.5)


def stage(app):
    """The stage of the command the progress row shows now, for the record."""
    return phase(progress_text(app.main))


def check_big(app):
    snapshot_before(app)

    # 7. From the snapshot list: a second command is refused during the snapshot (#3), the
    # progress row goes from the snapshot to the comparison without a gap (#5, #9), and the
    # difference gets the focus, then the report is said (#1, #8).
    write("big1.txt", "1")
    key(VK_CONTROL, VK_F5)
    time.sleep(0.5)
    key(VK_CONTROL, VK_F5)
    app.step("Ctrl+F5 twice: refused during the snapshot", 0.5)
    print("   progress:", watch_progress(app))
    app.step("the snapshot and its comparison ended", 3)
    report(app)

    # 8. During a snapshot the menu item is unavailable (#3).
    snapshot_list(app)
    key(VK_CONTROL, VK_F5)
    time.sleep(0.2)
    key(VK_MENU, vk("S"))
    time.sleep(0.5)
    key(VK_DOWN)
    app.step("Alt+S, Down: the item during the snapshot", 1)
    key(VK_ESCAPE)
    time.sleep(0.3)
    key(VK_ESCAPE)
    finished(app)
    app.step("Esc, Esc: the command ended", 3)

    # 9. The button during the snapshot cancels the whole command: no row, no comparison (#9).
    snapshot_list(app)
    count = rows(lists(app.main)[0])
    key(VK_CONTROL, VK_F5)
    time.sleep(0.1)
    print("   stage at Alt+N:", stage(app))
    key(VK_MENU, vk("N"))
    finished(app)
    app.step("Ctrl+F5, Alt+N during the snapshot", 2)
    print("   snapshots:", count, "->", rows(lists(app.main)[0]))

    # 9a. Shift+F5 during the snapshot likewise (#13).
    key(VK_CONTROL, VK_F5)
    time.sleep(0.1)
    print("   stage at Shift+F5:", stage(app))
    key(VK_SHIFT, VK_F5)
    finished(app)
    app.step("Ctrl+F5, Shift+F5: the command cancelled", 2)
    print("   snapshots:", count, "->", rows(lists(app.main)[0]))

    # 10. The button during the comparison cancels it alone: the snapshot stays (#9).
    write("big2.txt", "2")
    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: comparing(app), 600, 0.005), "no comparison"
    key(VK_MENU, vk("N"))
    finished(app)
    app.step("Alt+N during the comparison", 2.5)
    print("   snapshots:", count, "->", rows(lists(app.main)[0]))
    report(app)

    # 11. Shift+F7 during the comparison likewise (#13).
    write("big3.txt", "3")
    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: comparing(app), 600, 0.005), "no comparison"
    key(VK_SHIFT, VK_F7)
    finished(app)
    app.step("Shift+F7 during the comparison", 2.5)
    report(app)

    # 12. On the progress row the focus stays from the snapshot to the comparison and goes to
    # the difference at the end (#5).
    snapshot_list(app)
    write("big4.txt", "4")
    key(VK_CONTROL, VK_F5)
    time.sleep(0.5)
    key(VK_MENU, vk("P"))
    app.step("Ctrl+F5, Alt+P: the progress field", 0.5)
    print("   progress:", watch_progress(app))
    app.step("ended: from the progress to the difference", 3)
    report(app)

    # 13. A comparison started during the snapshot: the snapshot ends as a regular one, by
    # «Фокус після знімка», and is not compared (#11).
    snapshot_list(app)
    key(VK_END)
    write("big5.txt", "5")
    key(VK_CONTROL, VK_F5)
    time.sleep(0.5)
    key(VK_F7)
    print("   progress:", watch_progress(app))
    app.step("Ctrl+F5, F7 during the snapshot", 4)
    report(app)



def check_profiles(app):
    snapshot_before(app)

    # 14. Another profile opened during the command (#7): with «Завжди» revisor opens Big
    # again with the focus on the difference; with the default the difference arrives, and
    # Small stays open.
    settings((VK_MENU, vk("M")), VK_HOME, VK_DOWN)
    app.step("settings: Always", 2)
    for name in ("Always", "default"):
        snapshot_list(app)
        count = rows(lists(app.main)[0])
        write(f"big6-{name}.txt", name)
        key(VK_CONTROL, VK_F5)
        time.sleep(0.1)
        key(VK_MENU, vk("R"))
        time.sleep(0.3)
        key(VK_DOWN)
        time.sleep(0.2)
        print("   title:", foreground_title(), "| stage:", stage(app))
        app.step(f"Ctrl+F5, Alt+R, Down: Small open during the command ({name})", 0.2)
        print("   progress:", watch_progress(app))
        app.step(f"ended with {name}", 3)
        report(app)
        if name == "Always":
            print("   Big's snapshots:", count, "->", rows(lists(app.main)[0]))
            settings((VK_MENU, vk("M")), VK_HOME)
            app.step("settings: default", 2)
    key(VK_MENU, vk("R"))
    time.sleep(0.5)
    key(VK_UP)
    app.step("Alt+R, Up: Big again", 1.5)
    report(app)

    # 15. Closing during the snapshot asks first (TASK-8); kept, the command goes on (#12).
    snapshot_list(app)
    write("big7.txt", "7")
    key(VK_CONTROL, VK_F5)
    time.sleep(0.1)
    print("   stage at Alt+F4:", stage(app))
    key(VK_MENU, VK_F4)
    assert wait_for(lambda: foreground_title() == "Вихід", 5, 0.05), foreground_title()
    app.step("Alt+F4 during the snapshot: the question", 1)
    key(VK_ESCAPE)
    print("   progress:", watch_progress(app))
    app.step("Esc: kept, the command ended", 3)
    report(app)

    # 16. During the comparison revisor closes at once (#12).
    write("big8.txt", "8")
    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: comparing(app), 600, 0.005), "no comparison"
    print("   stage at Alt+F4:", stage(app))
    t0 = time.time()
    key(VK_MENU, VK_F4)
    assert wait_for(lambda: app.proc.poll() is not None, 10, 0.02), "revisor did not close"
    print(f"   closed {time.time() - t0:.2f} s after Alt+F4, exit code {app.proc.poll()}")


if "small" in parts:
    with App(paths, paths["Small"]) as app:
        check_small(app)

if "big" in parts:
    with App(paths, paths["Big"]) as app:
        check_big(app)

if "profiles" in parts:
    with App(paths, paths["Big"]) as app:
        check_profiles(app)
