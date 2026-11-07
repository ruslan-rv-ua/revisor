"""Live NVDA check of TASK-28: a failure of «Показати в Провіднику» opens a window.

    python -X utf8 tools\\nvda\\task28_explorer_failure.py

The rest of what TASK-28 #8 checks — F5 in the list and outside it, «Знімок + порівняння»,
a comment while a snapshot is being saved, deleting snapshots under a question while a
snapshot runs — is heard with the scripts of the tasks that made it: task27_snapshot_list.py
(part ``list``), task12_snapshot_and_compare.py (part ``small``), task21_messages.py and
task21_delete.py.

«Small» is a scratch folder with a file in a subfolder. Between two snapshots the file
changes; after «Знімок + порівняння» the subfolder is closed by rights (``icacls /deny`` of
listing) and so are the file's attributes (opened again at the end), so File Explorer cannot
parse the path of the change, which is still on disk: Ctrl+Enter on it fails, and the window
«Показати в Провіднику» says why (ADR-0013). Esc closes it and the focus is back on the
change. With the listing closed alone, File Explorer still parses the path and asks for access
itself.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_ESCAPE, VK_F5, VK_INSERT, VK_RETURN, App, focused_class, foreground_title,
    key, prepare, vk, wait_for,
)

VK_CONTROL = 0x11
COMMAND = "Показати в Провіднику"

tree = os.path.join(SCRATCH, "tree")
locked = os.path.join(tree, "locked")
inside = os.path.join(locked, "inside.txt")
user = os.environ.get("USERNAME", "")
missing = []


def heard(lines, *words):
    """Note each of `words` that no line of `lines` has."""
    text = " ".join(lines)
    for w in words:
        if w not in text:
            missing.append(w)
            print(f"   !! not heard: {w!r}")


def write(text):
    os.makedirs(locked, exist_ok=True)
    with open(inside, "w", encoding="utf-8") as f:
        f.write(text)


def unlock():
    subprocess.run(["icacls", locked, "/remove:d", user], capture_output=True)
    subprocess.run(["icacls", inside, "/remove:d", user], capture_output=True)


paths = prepare({"Small": ([tree], [])})
write("before")
try:
    with App(paths, paths["Small"]) as app:
        app.step("start", 3)
        key(VK_F5)
        assert wait_for(app.snapshot_done, 60, 0.1), "snapshot 1 did not end"
        app.step("F5: snapshot 1", 2.5)

        write("after, longer")

        # «Знімок + порівняння»: the focus goes to the difference, on its one row.
        key(VK_CONTROL, VK_F5)
        assert wait_for(app.snapshot_done, 60, 0.1), "«Знімок + порівняння» did not end"
        heard(app.step("Ctrl+F5: snapshot 2 and its difference", 3), "inside.txt")
        print("   focused:", focused_class())

        # Now neither the folder of the change can be listed nor the file's attributes read:
        # the file is still on disk, but File Explorer cannot parse its path.
        subprocess.run(["icacls", inside, "/deny", f"{user}:(RA)"], check=True,
                       capture_output=True)
        subprocess.run(["icacls", locked, "/deny", f"{user}:(RD)"], check=True,
                       capture_output=True)

        key(VK_CONTROL, VK_RETURN)
        opened = wait_for(lambda: foreground_title() == COMMAND, 8, 0.1)
        print("   foreground:", foreground_title())
        if not opened:
            missing.append("the window of the failure")
            print("   !! no window: File Explorer may have opened instead")
        app.step("Ctrl+Enter: the failure", 2)
        # Object descriptions are off: NVDA+B reads the text of the window.
        key(VK_INSERT, vk("B"))
        heard(app.step("NVDA+B: the window's text", 3), "Провідник")
        if opened:
            key(VK_ESCAPE)
            app.step("Esc: closed, back on the change", 2)
            print("   focused:", focused_class(), "| foreground:", foreground_title())
finally:
    unlock()

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
