"""Live NVDA check of TASK-24: a comment waits for a busy profile database, visibly and
cancellably, and the start waits for a database another process holds.

    python -X utf8 tools\\nvda\\task24_comment_wait.py "<folder>;<folder>" "<exclusion>;<exclusion>" [parts]

``parts`` is a comma-separated choice of ``held``, ``cancel``, ``close``, ``start`` and ``big``
(all by default). «Small» is a scratch folder with a few files; its database is held from this
process, as another revisor compacting it would (``BEGIN EXCLUSIVE``). «Big» is the folders
given, whose snapshot takes a second or more to save (about 800k entries, see README.md): the
comment there waits for the save of this revisor's own snapshot.
"""

import os
import sqlite3
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_DOWN, VK_F2, VK_F4, VK_F5, VK_MENU, VK_RETURN, VK_UP, App, args_folders,
    foreground_title, key, prepare, type_text, vk, wait_for,
)

VK_SHIFT = 0x10
folders, exclusions = args_folders()
parts = set(sys.argv[3].split(",")) if len(sys.argv) > 3 else {
    "held", "cancel", "close", "start", "big"}
tree = os.path.join(SCRATCH, "tree")
paths = prepare({"Small": ([tree], []), "Big": (folders, exclusions)})
# After prepare, which empties the scratch folder.
os.makedirs(tree, exist_ok=True)
for name in ("a.txt", "b.txt"):
    with open(os.path.join(tree, name), "w", encoding="utf-8") as f:
        f.write(name)
SMALL, BIG = paths["Small"], paths["Big"]


class Locked:
    """Holds a profile database exclusively, as another revisor compacting it does: no other
    connection can read it until ``release`` (or after ``seconds``, from a timer)."""

    def __init__(self, db, seconds=None):
        self.conn = sqlite3.connect(db, isolation_level=None, check_same_thread=False)
        self.conn.execute("BEGIN EXCLUSIVE")
        self.timer = threading.Timer(seconds, self.release) if seconds else None
        if self.timer:
            self.timer.start()

    def release(self):
        try:
            self.conn.execute("ROLLBACK")
            self.conn.close()
        except sqlite3.Error:
            pass


def comments(db):
    """The comments of the snapshots in `db`, oldest first."""
    conn = sqlite3.connect(db)
    try:
        return [r[0] for r in conn.execute("SELECT comment FROM snapshot ORDER BY id")]
    finally:
        conn.close()


def comment(app, text):
    """F2, `text`, Enter: the comment window closes at once, whatever the database."""
    key(VK_F2)
    assert wait_for(lambda: foreground_title() == "Коментар", 5, 0.02), "no comment window"
    time.sleep(0.3)
    type_text(text)
    time.sleep(0.3)
    key(VK_RETURN)


def small_snapshot(app):
    key(VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.05), "the snapshot did not end"
    app.step("F5: a snapshot of «Small»", 2)


if parts & {"held", "cancel"}:
    with App(paths, SMALL) as app:
        app.step("start on «Small»", 2.5)
        # Two snapshots, so that the arrows in the list have a row to go to.
        small_snapshot(app)
        small_snapshot(app)
        if "held" in parts:
            # 1. Another process holds the database for 5 s: the comment waits, the window
            # answers, then it is saved (#1, #2).
            lock = Locked(SMALL, 5)
            comment(app, "чекав")
            app.step("Enter while the database is held: waiting", 1.5)
            print("   progress:", app.progress())
            key(VK_UP)
            app.step("Up in the snapshot list while it waits: the window answers", 1)
            key(VK_DOWN)
            app.step("Down", 1)
            assert wait_for(app.snapshot_done, 30, 0.05), "the comment never ended"
            app.step("the database is free: saved", 2)
            print("   comments:", comments(SMALL))
        if "cancel" in parts:
            # 2. Cancelled with Shift+F2: not saved, the earlier comment stays (#3).
            lock = Locked(SMALL)
            comment(app, "скасований")
            app.step("Enter while held: waiting", 1.5)
            key(VK_SHIFT, VK_F2)
            app.step("Shift+F2: cancelled", 2)
            # 3. Cancelled with the row's button, Alt+N; F2 meanwhile is refused (one at a
            # time), and so is the deletion of the profile's snapshots.
            comment(app, "теж скасований")
            app.step("Enter while held: waiting again", 1.5)
            key(VK_F2)
            app.step("F2 while one waits: refused", 1.5)
            key(VK_MENU, vk("N"))
            app.step("Alt+N: cancelled by the button", 2)
            lock.release()
            print("   comments:", comments(SMALL))

if "close" in parts:
    # 4. Closing while a comment waits: no question, revisor closes at once, nothing saved
    # (#4).
    with App(paths, SMALL) as app:
        app.step("start on «Small»", 2.5)
        before = comments(SMALL)
        lock = Locked(SMALL)
        comment(app, "при закритті")
        app.step("Enter while held: waiting", 1.5)
        t0 = time.time()
        key(VK_MENU, VK_F4)
        closed = wait_for(lambda: app.proc.poll() is not None, 10, 0.02)
        print(f"   closed: {closed} after {time.time() - t0:.2f} s")
        # What NVDA says now is the window behind revisor: not read (privacy).
        app.speech.new()
        lock.release()
        print("   comments before:", before, "| after:", comments(SMALL))

if "start" in parts:
    # 5. Started while another process holds the database: the window shows once it is free
    # (#5).
    lock = Locked(SMALL, 4)
    t0 = time.time()
    with App(paths, SMALL) as app:
        print(f"   the window showed after {time.time() - t0:.2f} s (held for 4 s)")
        app.step("start while held", 2.5)

if "big" in parts:
    # 6. A comment typed while this revisor's snapshot is saved: it waits for the save and is
    # said with the snapshot's end (#1, TASK-21).
    with App(paths, BIG) as app:
        app.step("start on «Big»", 2.5)
        # The first snapshot gives the comment a snapshot to go on.
        key(VK_F5)
        assert wait_for(app.snapshot_done, 600, 0.05), "the first snapshot did not end"
        app.dismiss_restart_question()
        app.step("F5: the first snapshot of «Big»", 3)
        for attempt in range(2):
            key(VK_F5)
            app.step(f"F5: snapshot {attempt + 2} of «Big»", 1)
            assert wait_for(app.saving, 600, 0.02), "no save"
            t_save = time.time()
            comment(app, f"під час запису {attempt + 1}")
            t0 = time.time()
            assert wait_for(app.snapshot_done, 120, 0.02), "the snapshot did not end"
            print(f"   Enter {t0 - t_save:.2f} s into the save; the end {time.time() - t0:.2f} s later")
            app.dismiss_restart_question()
            app.step("the save is over: the comment and the snapshot", 4)
            app.repeat_last_message("NVDA+End", 3)
        print("   comments:", comments(BIG))
