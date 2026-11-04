"""Live NVDA check of TASK-21: the messages of one moment are said together.

    python -X utf8 tools\\nvda\\task21_messages.py "<folder>;<folder>" "<exclusion>;<exclusion>"

The profile should take a second or more to save (about 800k entries) and have no denied
entries, or the restart question gets in the way: see README.md.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    VK_DELETE, VK_F2, VK_F5, VK_RETURN, App, args_folders, foreground_title, key, prepare,
    type_text, vk, wait_for,
)

folders, exclusions = args_folders()
paths = prepare({"Windows": (folders, exclusions)})

with App(paths, paths["Windows"]) as app:
    app.step("start", 2.5)

    # 1. A plain snapshot: one message, as before.
    key(VK_F5)
    t_start = time.time()
    app.step("F5: snapshot started", 1.5)
    assert wait_for(app.saving, 600, 0.02), "no save"
    t_save = time.time()
    assert wait_for(app.snapshot_done, 600, 0.02), "snapshot 1 did not end"
    t_snap = time.time() - t_start
    print(f"   snapshot 1 took {t_snap:.1f} s, its save {time.time() - t_save:.2f} s")
    app.dismiss_restart_question()
    app.step("snapshot 1 ended", 3)
    app.repeat_last_message("NVDA+End after snapshot 1")

    # 2. A plain comment: one message.
    key(VK_F2)
    app.step("F2: comment window", 1.5)
    type_text("звичайний")
    time.sleep(0.5)
    key(VK_RETURN)
    app.step("Enter: plain comment", 2.5)
    app.repeat_last_message("NVDA+End after the plain comment")

    # 3. F2 while the snapshot is being saved: the write waits for the commit.
    key(VK_F5)
    app.step("F5: snapshot 2 started", 1)
    saving = wait_for(app.saving, 600, 0.02)
    print("   saving seen:", saving, "|", app.progress())
    sys.stdout.flush()
    t_save = time.time()
    key(VK_F2)
    assert wait_for(lambda: foreground_title() == "Коментар", 5, 0.01), "no comment window"
    type_text("ab")
    t0 = time.time()
    key(VK_RETURN)
    assert wait_for(app.snapshot_done, 120, 0.01), "snapshot 2 did not end"
    print(f"   F2 at {t0 - t_save:.2f} s after the save began; Enter → end took {time.time() - t0:.2f} s")
    app.dismiss_restart_question()
    app.step("Enter during the save: comment + snapshot 2", 4)
    app.repeat_last_message("NVDA+End after the joined message", 3)

    # 4. The snapshot ends while the comment window is open.
    key(VK_F5)
    app.step("F5: snapshot 3 started", 1)
    key(VK_F2)
    app.step("F2 during the scan: comment window", 1.5)
    type_text("під вікном")
    # Wait long enough for the snapshot to end under the dialog (the timer sleeps there).
    time.sleep(t_snap * 1.5 + 5)
    app.step("waiting under the window", 0.5)
    key(VK_RETURN)
    assert wait_for(app.snapshot_done, 120, 0.05), "snapshot 3 did not end"
    app.dismiss_restart_question()
    app.step("Enter after the snapshot ended under the window", 4)
    app.repeat_last_message("NVDA+End", 3)

    # 5. The snapshot ends while the deletion confirmation is open.
    key(VK_F5)
    app.step("F5: snapshot 4 started", 1)
    key(VK_DELETE)
    app.step("Delete during the scan: confirmation", 1.5)
    time.sleep(t_snap * 1.5 + 5)
    app.step("waiting under the confirmation", 0.5)
    key(vk("D"))  # «Видалити», Alt released
    assert wait_for(app.snapshot_done, 120, 0.05), "snapshot 4 did not end"
    app.step("D after the snapshot ended under the confirmation", 4)
    app.repeat_last_message("NVDA+End", 3)
    assert wait_for(app.snapshot_done, 120, 0.1)
    app.step("compaction ended", 3)
