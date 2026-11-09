"""Live NVDA check of TASK-32: win32 split by concern, formatting given its locale — nothing
the user hears or sees has changed.

    python -X utf8 tools\\nvda\\task32_win32_split.py

In the Ukrainian interface: the empty lists — their names heard and their texts drawn (a
screenshot of the window goes to ``%TEMP%\\revisor-nvda-task32.png``); the snapshot list's rows
and the difference list's rows with their dates and sizes; the details of a change; the context
menu of the difference list with its first item the bold default, and the menu «Файл» opened
after it — by «а», the F key of the Ukrainian layout — without one; the shield on «Перезапустити
як адміністратор» before any menu opens (wx takes it off as «Файл» opens, in develop
too). The last line says ``all heard`` or names what is missing.
"""

import ctypes
import ctypes.wintypes as wt
import os
import re
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from revisor_nvda import (  # noqa: E402
    SCRATCH, VK_DOWN, VK_ESCAPE, VK_F5, VK_MENU, VK_RETURN, App, control_text, focused_window,
    foreground_title, item_count, key, make_settings, prepare, snapshot_list, u32,
    ukrainian_layout, visible_controls, vk, wait_for,
)

VK_SHIFT, VK_CONTROL, VK_F6, VK_F10 = 0x10, 0x11, 0x75, 0x79
MN_GETHMENU = 0x01E1
MIIM_ID, MIIM_BITMAP = 0x2, 0x80
MF_BYPOSITION = 0x400
SHOT = os.path.join(tempfile.gettempdir(), "revisor-nvda-task32.png")
DATE = re.compile(r"\d{2}\.\d{2}\.\d{4} \d{1,2}:\d{2}:\d{2}")

u32.GetMenu.restype = ctypes.c_void_p
u32.GetMenu.argtypes = [wt.HWND]
u32.GetSubMenu.restype = ctypes.c_void_p
u32.GetSubMenu.argtypes = [ctypes.c_void_p, ctypes.c_int]
u32.GetMenuItemCount.argtypes = [ctypes.c_void_p]
u32.GetMenuDefaultItem.restype = wt.UINT
u32.GetMenuDefaultItem.argtypes = [ctypes.c_void_p, wt.UINT, wt.UINT]
u32.GetMenuStringW.argtypes = [ctypes.c_void_p, wt.UINT, wt.LPWSTR, ctypes.c_int, wt.UINT]
u32.FindWindowW.restype = wt.HWND
u32.SendMessageW.restype = ctypes.c_ssize_t


class MENUITEMINFOW(ctypes.Structure):
    _fields_ = [("cbSize", wt.UINT), ("fMask", wt.UINT), ("fType", wt.UINT),
                ("fState", wt.UINT), ("wID", wt.UINT), ("hSubMenu", ctypes.c_void_p),
                ("hbmpChecked", ctypes.c_void_p), ("hbmpUnchecked", ctypes.c_void_p),
                ("dwItemData", ctypes.c_size_t), ("dwTypeData", ctypes.c_void_p), ("cch", wt.UINT),
                ("hbmpItem", ctypes.c_void_p)]


u32.GetMenuItemInfoW.argtypes = [ctypes.c_void_p, wt.UINT, wt.BOOL, ctypes.POINTER(MENUITEMINFOW)]

missing = []
tree = os.path.join(SCRATCH, "tree")


def check(what, found):
    print(f"   {'OK' if found else 'MISSING'}: {what}")
    if not found:
        missing.append(what)


def menu_text(menu, position):
    buf = ctypes.create_unicode_buffer(256)
    u32.GetMenuStringW(menu, position, buf, 256, MF_BYPOSITION)
    return buf.value


def open_popup():
    """The menu of the popup open now, or None."""
    if not wait_for(lambda: u32.FindWindowW("#32768", None), 3):
        return None
    popup = u32.FindWindowW("#32768", None)
    return u32.SendMessageW(popup, MN_GETHMENU, 0, 0) or None


def default_item(menu):
    item = u32.GetMenuDefaultItem(menu, 1, 0)
    return None if item == 0xFFFFFFFF else item


def screenshot(main, path):
    rect = wt.RECT()
    u32.GetWindowRect(main, ctypes.byref(rect))
    ps = ("Add-Type -AssemblyName System.Drawing;"
          f"$b=New-Object System.Drawing.Bitmap {rect.right - rect.left},{rect.bottom - rect.top};"
          "$g=[System.Drawing.Graphics]::FromImage($b);"
          f"$g.CopyFromScreen({rect.left},{rect.top},0,0,$b.Size);"
          f"$b.Save('{path}',[System.Drawing.Imaging.ImageFormat]::Png)")
    subprocess.run(["powershell", "-NoProfile", "-Command", ps], check=True)
    print(f"   screenshot: {path}")


def write(rel, text):
    full = os.path.join(tree, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as f:
        f.write(text)


def differences(main):
    shown = visible_controls(main, "SysListView32")
    return shown[1] if len(shown) > 1 else None


# 1. The empty lists: no profiles yet.
paths = prepare({})
make_settings(os.path.dirname(paths["exe"]), language="uk")
with App(paths) as app:
    check("the empty snapshot list's name",
          "Профілів ще немає" in " ".join(app.step("start: no profiles", 3)))
    screenshot(app.main, SHOT)

# 2. A profile with a difference.
paths = prepare({"Small": ([tree], [])})
make_settings(os.path.dirname(paths["exe"]), language="uk")
write("keep.txt", "stays")
write("change.txt", "before")
write("remove.txt", "goes away")
with App(paths, paths["Small"]) as app:
    ukrainian_layout(app.main)
    # The shield on «Перезапустити як адміністратор», before any menu has opened: wx takes
    # the item's bitmap off as «Файл» opens, so the shield is never drawn — in develop too
    # (found in TASK-32, not caused by it).
    file_menu = u32.GetSubMenu(u32.GetMenu(app.main), 0)
    shielded = []
    for i in range(u32.GetMenuItemCount(file_menu)):
        info = MENUITEMINFOW(cbSize=ctypes.sizeof(MENUITEMINFOW), fMask=MIIM_BITMAP | MIIM_ID)
        u32.GetMenuItemInfoW(file_menu, i, True, ctypes.byref(info))
        if info.hbmpItem:
            shielded.append(menu_text(file_menu, i))
    print(f"   items with a bitmap: {shielded}")
    check("the shield on the restart as administrator",
          any("адміністратор" in item for item in shielded))
    check("the empty snapshot list's name, with a profile",
          "Знімків ще немає" in " ".join(app.step("start: a profile without snapshots", 3)))

    key(VK_F5)
    assert wait_for(lambda: item_count(snapshot_list(app.main)) == 1, 60), "no snapshot"
    lines = " ".join(app.step("F5: the snapshot before", 2.5))
    check("a snapshot's name is the regional date and time", bool(DATE.search(lines)))

    write("change.txt", "after the program ran, longer")
    os.remove(os.path.join(tree, "remove.txt"))
    write("new.txt", "x" * 2000)
    time.sleep(1)
    key(VK_CONTROL, VK_F5)
    assert wait_for(lambda: (item_count(differences(app.main)) or 0) > 0, 60), "no difference"
    app.step("Ctrl+F5: snapshot and compare", 3)

    # The rows of the difference list, with the size and the date modified.
    heard = []
    for i in range(3):
        key(VK_DOWN)
        heard += app.step(f"Down {i + 1}", 1.5)
    text = " ".join(heard)
    check("a difference row's size", bool(re.search(r"\d+(,\d+)? (байт|байти|байтів|КБ)", text)))
    check("a difference row's date", bool(DATE.search(text)))

    key(VK_MENU, VK_RETURN)
    app.step("Alt+Enter: the details", 2)
    details = control_text(focused_window())
    print("   " + "\n   ".join(line for line in details.splitlines()
                               if line.startswith(("Розмір", "Дата", "Змінено", "Створено"))))
    check("the details' size", "Розмір:" in details)
    check("the details' date", bool(DATE.search(details)))
    key(VK_ESCAPE)
    app.step("Esc: back to the list", 1.5)

    # The context menu: its first item is the bold default.
    key(VK_SHIFT, VK_F10)
    menu = open_popup()
    app.step("Shift+F10: the context menu", 1.5)
    check("a context menu opened", menu is not None)
    if menu:
        print(f"   first item: {menu_text(menu, 0)!r}, default: {default_item(menu)}")
        check("the context menu's first item is bold", default_item(menu) == 0)
    key(VK_ESCAPE)
    app.step("Esc: closed", 1.5)

    # «Файл» by «а», the F key of the Ukrainian layout, has no bold item.
    key(VK_MENU, vk("F"))
    menu = open_popup()
    app.step("Alt+F in the Ukrainian layout: «Файл»", 1.5)
    check("«Файл» opened by the key's position", menu is not None)
    if menu:
        print(f"   first item: {menu_text(menu, 0)!r}, default: {default_item(menu)}")
        check("«Файл» has no bold item", default_item(menu) is None)
    key(VK_ESCAPE)
    key(VK_ESCAPE)
    app.step("Esc, Esc", 1.5)

    print(f"   foreground: {foreground_title()}")

print("\nall heard" if not missing else f"\nmissing: {missing}")
sys.exit(1 if missing else 0)
