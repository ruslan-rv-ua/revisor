"""Live NVDA check of TASK-21: a snapshot of another profile ends under the deletion confirmation.

    python -X utf8 tools\\nvda\\task21_delete.py "<folder>;<folder>" "<exclusion>;<exclusion>"

Deleting is refused while a snapshot of the same profile runs, so two profiles are used: «Small»
(this folder, a snapshot in a second) and «Windows» (the folders given, a snapshot that takes
long enough to press keys into).
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    VK_DELETE, VK_DOWN, VK_F5, VK_MENU, VK_TAB, VK_UP, App, args_folders, focused_class, key,
    prepare, vk, wait_for,
)

folders, exclusions = args_folders()
paths = prepare({
    "Small": ([os.path.dirname(os.path.abspath(__file__))], []),
    "Windows": (folders, exclusions),
})

with App(paths, paths["Windows"]) as app:
    app.step("start", 2.5)

    # Snapshot of «Small», then a snapshot of «Windows» runs while «Small» is open again and
    # its snapshot is deleted: the confirmation is up when the other snapshot ends.
    key(VK_MENU, vk("R")); time.sleep(0.5); key(VK_UP)  # Alt+R, Up: «Small»
    app.step("Alt+R, Up: profile Small", 2)
    key(VK_F5)
    assert wait_for(app.snapshot_done, 60, 0.1)
    app.step("F5: Small snapshot ended", 2.5)
    key(VK_MENU, vk("R")); time.sleep(0.5); key(VK_DOWN)  # Alt+R, Down: «Windows»
    app.step("Alt+R, Down: profile Windows", 2)
    key(VK_F5)
    t_start = time.time()
    time.sleep(0.3)
    key(VK_MENU, vk("R")); time.sleep(0.3); key(VK_UP)  # back to «Small»
    time.sleep(0.5)
    for _ in range(5):
        if focused_class() == "SysListView32":
            break
        key(VK_TAB)
        time.sleep(0.3)
    key(VK_DELETE)
    print(f"   Delete pressed {time.time() - t_start:.1f} s after F5; progress: {app.progress()!r}")
    app.step("F5 on Windows, Alt+R, Up, Tab, Delete: confirmation", 1.5)
    # Under the modal confirmation the timer sleeps and the progress field stays, so wait by time.
    time.sleep(20)
    app.step("waiting under the confirmation", 0.5)
    key(vk("D"))  # «Видалити», Alt released
    app.step("D after the snapshot ended under the confirmation", 4)
    app.repeat_last_message("NVDA+End", 3)
    assert wait_for(app.snapshot_done, 120, 0.1)
    app.step("compaction ended", 3)
