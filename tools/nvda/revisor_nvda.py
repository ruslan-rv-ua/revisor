"""The shared part of revisor's live checks with NVDA (TASK-22).

A check script runs a copy of revisor.exe in a scratch folder with its own ``data\\profiles``,
drives it with injected keys and reads what NVDA said from NVDA's own log. This module holds
everything the scripts share; a scenario script imports it and writes only its steps. See
README.md next to this file for how to run a check and what NVDA needs (loggingLevel = IO).

Standard library only: ctypes, sqlite3, subprocess. Windows only, like revisor itself.
"""

import ctypes
import ctypes.wintypes as wt
import os
import re
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import zlib

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
EXE_SRC = os.path.join(REPO, "target", "release", "revisor.exe")
STORE_RS = os.path.join(REPO, "crates", "revisor-core", "src", "store.rs")
SETTINGS_RS = os.path.join(REPO, "crates", "revisor-core", "src", "settings.rs")
LOG = r"C:\Temp\nvda.log"
# The scratch folder: never the folder of the real exe, whose data\profiles holds real profiles.
SCRATCH = os.path.join(tempfile.gettempdir(), "revisor-nvda")
# PRAGMA application_id of a profile database, "RVPR" (store.rs APPLICATION_ID).
APPLICATION_ID = 0x52565052
SCHEMA_VERSION = 1

u32 = ctypes.windll.user32
k32 = ctypes.windll.kernel32


# --- preparation ------------------------------------------------------------------------
def rust_const(path, name):
    """The value of ``const NAME`` in a Rust source of revisor, read so it never drifts from
    the app: the text of a ``&str``, or an integer, such as ``0x5256_5354``."""
    with open(path, encoding="utf-8") as f:
        src = f.read()
    value = re.search(rf'const {name}: [^=]+= ("(?:.*?)"|[0-9A-Fa-fx_]+);', src, re.S).group(1)
    return value[1:-1] if value.startswith('"') else int(value.replace("_", ""), 0)


def schema():
    """The profile database schema, taken from store.rs so it never drifts from the app."""
    return rust_const(STORE_RS, "SCHEMA")


def make_settings(scratch, **values):
    """Create ``data\\settings.db`` in the scratch folder holding ``values``, such as
    ``language="en"``; its schema and marks come from settings.rs, as a profile's come from
    store.rs. Returns its path."""
    db = os.path.join(scratch, "data", "settings.db")
    conn = sqlite3.connect(db)
    conn.executescript(rust_const(SETTINGS_RS, "SCHEMA"))
    conn.execute(f"PRAGMA application_id = {rust_const(SETTINGS_RS, 'APPLICATION_ID')}")
    conn.execute(f"PRAGMA user_version = {rust_const(SETTINGS_RS, 'SCHEMA_VERSION')}")
    for key_name, value in values.items():
        conn.execute("INSERT INTO setting VALUES (?, ?)", (key_name, value))
    conn.commit()
    conn.close()
    return db


def make_profile(scratch, name, folders, exclusions=()):
    """Create ``data\\profiles\\<name>.db`` in the scratch folder; returns its path."""
    db = os.path.join(scratch, "data", "profiles", name + ".db")
    conn = sqlite3.connect(db)
    conn.executescript(schema())
    conn.execute(f"PRAGMA application_id = {APPLICATION_ID}")
    conn.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
    conn.execute("INSERT INTO profile (id, name) VALUES (1, ?)", (name,))
    for i, folder in enumerate(folders):
        conn.execute("INSERT INTO profile_folder VALUES (?, ?)", (i, folder))
    for i, pattern in enumerate(exclusions):
        conn.execute("INSERT INTO profile_exclusion VALUES (?, ?)", (i, pattern))
    conn.commit()
    conn.close()
    return db


def prepare(profiles, scratch=SCRATCH, exe=EXE_SRC):
    """Rebuild the scratch folder: a copy of the exe and the given profiles.

    ``profiles`` maps a profile name to ``(folders, exclusions)``. Returns a dict of the
    profile database paths by name, plus the key ``"exe"`` for the copied exe.
    """
    if not os.path.exists(exe):
        sys.exit(f"no exe at {exe}: build it first (cargo build --release)")
    if os.path.exists(scratch):
        shutil.rmtree(scratch)
    os.makedirs(os.path.join(scratch, "data", "profiles"))
    out = {"exe": shutil.copy(exe, os.path.join(scratch, "revisor.exe"))}
    for name, (folders, exclusions) in profiles.items():
        out[name] = make_profile(scratch, name, folders, exclusions)
    return out


def args_folders(default=(r"C:\Windows",)):
    """``sys.argv[1]``: profile folders separated by ';'; ``sys.argv[2]``: exclusions."""
    folders = sys.argv[1].split(";") if len(sys.argv) > 1 else list(default)
    exclusions = sys.argv[2].split(";") if len(sys.argv) > 2 else []
    return folders, exclusions


# --- NVDA speech log --------------------------------------------------------------------
class Speech:
    """New ``Speaking [...]`` lines of NVDA's log since the last read.

    The log holds whatever the author was reading before the check, so only the bytes written
    after the object was made are ever read (privacy).
    """

    def __init__(self, log=LOG):
        if not os.path.exists(log):
            sys.exit(f"no NVDA log at {log}: set loggingLevel = IO in nvda.ini (see README.md)")
        self.log = log
        self.offset = os.path.getsize(log)

    def new(self):
        with open(self.log, "rb") as f:
            f.seek(self.offset)
            data = f.read()
        self.offset += len(data)
        text = data.decode("utf-8", "replace")
        return [m.group(1) for m in re.finditer(r"Speaking (\[.*?\])\r?\n", text)]

    def step(self, name, wait=1.5):
        """Wait, then print and return what NVDA said meanwhile under the step's name."""
        time.sleep(wait)
        lines = self.new()
        print(f"\n== {name}")
        for line in lines:
            print("   ", line)
        sys.stdout.flush()
        return lines


# --- windows and controls -----------------------------------------------------------------
EnumWindowsProc = ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
WM_GETTEXT, WM_GETTEXTLENGTH = 0x000D, 0x000E


def window_text(h):
    n = u32.GetWindowTextLengthW(h)
    buf = ctypes.create_unicode_buffer(n + 1)
    u32.GetWindowTextW(h, buf, n + 1)
    return buf.value


def class_name(h):
    buf = ctypes.create_unicode_buffer(256)
    u32.GetClassNameW(h, buf, 256)
    return buf.value


def find_main(suffix="revisor"):
    """The visible top-level window whose title ends with ``suffix``, or None."""
    found = []

    def cb(h, _):
        if u32.IsWindowVisible(h) and window_text(h).endswith(suffix):
            found.append(h)
        return True

    u32.EnumWindows(EnumWindowsProc(cb), 0)
    return found[0] if found else None


def children(h):
    out = []

    def cb(c, _):
        out.append(c)
        return True

    u32.EnumChildWindows(h, EnumWindowsProc(cb), 0)
    return out


def control_text(h):
    """The text of a control through WM_GETTEXT: what the field holds, not what was said."""
    n = u32.SendMessageW(h, WM_GETTEXTLENGTH, 0, 0)
    buf = ctypes.create_unicode_buffer(n + 1)
    u32.SendMessageW(h, WM_GETTEXT, n + 1, buf)
    return buf.value


def has_size(h):
    rect = wt.RECT()
    u32.GetWindowRect(h, ctypes.byref(rect))
    return rect.right > rect.left and rect.bottom > rect.top


def visible_controls(main, cls):
    """The controls of class `cls` on screen: shown and of some size. The difference list not
    shown is disabled and has no size rather than hidden (TASK-13)."""
    return [c for c in children(main)
            if class_name(c) == cls and u32.IsWindowVisible(c) and has_size(c)]


LVM_GETITEMCOUNT = 0x1004


def item_count(h):
    """How many rows the list view `h` has."""
    return u32.SendMessageW(h, LVM_GETITEMCOUNT, 0, 0)


def snapshot_list(main):
    """The snapshot list of the main window: the first list view on screen."""
    lists = visible_controls(main, "SysListView32")
    return lists[0] if lists else None


GWL_STYLE, ES_MULTILINE, ES_READONLY = -16, 0x0004, 0x0800


def edit_style(h):
    return u32.GetWindowLongW(h, GWL_STYLE)


def progress_field(main):
    """The progress field of the main window, if shown: its one single-line read-only field
    (the field «Порівняння» is read-only too but has many lines, TASK-13)."""
    for h in visible_controls(main, "Edit"):
        style = edit_style(h)
        if style & ES_READONLY and not style & ES_MULTILINE:
            return h
    return None


def progress_text(main):
    """The text of the progress field of the main window, or None when it is hidden.

    Under a modal dialog the jobs' timer sleeps and the field stays, so waiting for None there
    stalls forever: wait by time instead.
    """
    h = progress_field(main)
    return control_text(h) if h else None


class GUITHREADINFO(ctypes.Structure):
    _fields_ = [
        ("cbSize", wt.DWORD),
        ("flags", wt.DWORD),
        ("hwndActive", wt.HWND),
        ("hwndFocus", wt.HWND),
        ("hwndCapture", wt.HWND),
        ("hwndMenuOwner", wt.HWND),
        ("hwndMoveSize", wt.HWND),
        ("hwndCaret", wt.HWND),
        ("rc", wt.RECT),
    ]


def focused_window():
    info = GUITHREADINFO()
    info.cbSize = ctypes.sizeof(GUITHREADINFO)
    u32.GetGUIThreadInfo(0, ctypes.byref(info))
    return info.hwndFocus


def focused_class():
    h = focused_window()
    return class_name(h) if h else ""


def foreground_title():
    return window_text(u32.GetForegroundWindow())


def popup_menu():
    """The open popup menu (class ``#32768``) on screen, or None."""
    found = []

    def cb(h, _):
        if u32.IsWindowVisible(h) and class_name(h) == "#32768":
            found.append(h)
        return True

    u32.EnumWindows(EnumWindowsProc(cb), 0)
    return found[0] if found else None


# --- colours and screenshots (TASK-34) ------------------------------------------------------
LVM_GETBKCOLOR, LVM_GETTEXTCOLOR, TVM_GETBKCOLOR = 0x1000, 0x1023, 0x111F
SPI_GETHIGHCONTRAST, HCF_HIGHCONTRASTON = 0x0042, 0x1


def colour(value):
    """A COLORREF (0x00BBGGRR) as ``(r, g, b)``; None for -1, «the system colour»."""
    value &= 0xFFFFFFFF
    if value == 0xFFFFFFFF:
        return None
    return value & 0xFF, (value >> 8) & 0xFF, (value >> 16) & 0xFF


def list_colours(h):
    """The background and text colours of the list view `h`, as ``(r, g, b)`` each."""
    return (colour(u32.SendMessageW(h, LVM_GETBKCOLOR, 0, 0)),
            colour(u32.SendMessageW(h, LVM_GETTEXTCOLOR, 0, 0)))


def is_dark(rgb):
    """Whether a colour is dark: its relative luminance (WCAG) is under a half."""
    if rgb is None:
        return False

    def channel(c):
        c /= 255
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (channel(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5


class HIGHCONTRASTW(ctypes.Structure):
    _fields_ = [("cbSize", wt.UINT), ("dwFlags", wt.DWORD), ("lpszDefaultScheme", wt.LPWSTR)]


def high_contrast():
    """Whether a contrast theme of Windows is on."""
    hc = HIGHCONTRASTW(ctypes.sizeof(HIGHCONTRASTW), 0, None)
    u32.SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, ctypes.byref(hc), 0)
    return bool(hc.dwFlags & HCF_HIGHCONTRASTON)


gdi = ctypes.windll.gdi32
gdi.CreateCompatibleDC.restype = wt.HDC
gdi.CreateCompatibleDC.argtypes = [wt.HDC]
gdi.CreateCompatibleBitmap.restype = wt.HBITMAP
gdi.CreateCompatibleBitmap.argtypes = [wt.HDC, ctypes.c_int, ctypes.c_int]
gdi.SelectObject.restype = wt.HGDIOBJ
gdi.SelectObject.argtypes = [wt.HDC, wt.HGDIOBJ]
gdi.BitBlt.argtypes = [wt.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wt.HDC,
                       ctypes.c_int, ctypes.c_int, wt.DWORD]
gdi.GetDIBits.argtypes = [wt.HDC, wt.HBITMAP, wt.UINT, wt.UINT, ctypes.c_void_p, ctypes.c_void_p,
                          wt.UINT]
gdi.DeleteObject.argtypes = [wt.HGDIOBJ]
gdi.DeleteDC.argtypes = [wt.HDC]
u32.GetDC.restype = wt.HDC
u32.GetDC.argtypes = [wt.HWND]
u32.ReleaseDC.argtypes = [wt.HWND, wt.HDC]
u32.PrintWindow.argtypes = [wt.HWND, wt.HDC, wt.UINT]
u32.SetThreadDpiAwarenessContext.restype = ctypes.c_void_p
u32.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
PW_RENDERFULLCONTENT, SRCCOPY = 2, 0x00CC0020
PER_MONITOR_AWARE_V2 = ctypes.c_void_p(-4)


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wt.DWORD), ("biWidth", wt.LONG), ("biHeight", wt.LONG),
        ("biPlanes", wt.WORD), ("biBitCount", wt.WORD), ("biCompression", wt.DWORD),
        ("biSizeImage", wt.DWORD), ("biXPelsPerMeter", wt.LONG), ("biYPelsPerMeter", wt.LONG),
        ("biClrUsed", wt.DWORD), ("biClrImportant", wt.DWORD),
    ]


def write_png(path, width, height, bgra):
    """Write 32-bit BGRA pixels, top row first, as an RGB PNG, with the standard library."""
    rgb = bytearray(width * height * 3)
    rgb[0::3] = bgra[2::4]
    rgb[1::3] = bgra[1::4]
    rgb[2::3] = bgra[0::4]
    stride = width * 3
    raw = b"".join(b"\0" + bytes(rgb[y * stride:(y + 1) * stride]) for y in range(height))

    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data)))

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(raw, 6)))
        f.write(chunk(b"IEND", b""))


def screenshot(h, path, from_screen=False):
    """Save the window `h` as a PNG at `path` and return the path. The window draws itself
    (PrintWindow), so nothing else on the screen gets into the picture; `from_screen` copies
    its rectangle from the screen instead, for a popup menu, which does not draw itself so."""
    old = u32.SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2)
    try:
        rect = wt.RECT()
        u32.GetWindowRect(h, ctypes.byref(rect))
        width, height = rect.right - rect.left, rect.bottom - rect.top
        screen = u32.GetDC(None)
        mem = gdi.CreateCompatibleDC(screen)
        bitmap = gdi.CreateCompatibleBitmap(screen, width, height)
        previous = gdi.SelectObject(mem, bitmap)
        if from_screen:
            gdi.BitBlt(mem, 0, 0, width, height, screen, rect.left, rect.top, SRCCOPY)
        else:
            u32.PrintWindow(h, mem, PW_RENDERFULLCONTENT)
        info = BITMAPINFOHEADER(ctypes.sizeof(BITMAPINFOHEADER), width, -height, 1, 32, 0, 0, 0,
                                0, 0, 0)
        pixels = ctypes.create_string_buffer(width * height * 4)
        gdi.SelectObject(mem, previous)
        gdi.GetDIBits(mem, bitmap, 0, height, pixels, ctypes.byref(info), 0)
        gdi.DeleteObject(bitmap)
        gdi.DeleteDC(mem)
        u32.ReleaseDC(None, screen)
    finally:
        u32.SetThreadDpiAwarenessContext(old)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    write_png(path, width, height, pixels.raw)
    return path


# --- keys ---------------------------------------------------------------------------------
KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE = 1, 2, 4
VK_TAB, VK_RETURN, VK_ESCAPE, VK_MENU, VK_SPACE = 0x09, 0x0D, 0x1B, 0x12, 0x20
VK_SHIFT, VK_CONTROL = 0x10, 0x11
VK_END, VK_HOME, VK_LEFT, VK_UP, VK_RIGHT, VK_DOWN = 0x23, 0x24, 0x25, 0x26, 0x27, 0x28
VK_INSERT, VK_DELETE, VK_APPS = 0x2D, 0x2E, 0x5D
VK_F1, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6 = 0x70, 0x71, 0x72, 0x73, 0x74, 0x75
VK_F7, VK_F8, VK_F9, VK_F10, VK_F11, VK_F12 = 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B
# Keys that need the extended flag: without it wx sees the numpad keys, and NVDA takes numpad
# arrows as its review keys.
EXT = {VK_END, VK_HOME, VK_INSERT, VK_DELETE, VK_LEFT, VK_UP, VK_RIGHT, VK_DOWN, VK_APPS}


def vk(ch):
    """The virtual key of a Latin letter or digit, for a chord such as Alt+R."""
    return ord(ch.upper())


def key(*vks, hold=0.05):
    """Press the keys in order and release them in reverse: ``key(VK_MENU, vk("R"))``."""
    for k in vks:
        u32.keybd_event(k, 0, KEYEVENTF_EXTENDEDKEY if k in EXT else 0, 0)
        time.sleep(0.03)
    time.sleep(hold)
    for k in reversed(vks):
        u32.keybd_event(k, 0, (KEYEVENTF_EXTENDEDKEY if k in EXT else 0) | KEYEVENTF_KEYUP, 0)
        time.sleep(0.03)


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wt.WORD),
        ("wScan", wt.WORD),
        ("dwFlags", wt.DWORD),
        ("time", wt.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class INPUT(ctypes.Structure):
    class U(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("pad", ctypes.c_byte * 32)]

    _anonymous_ = ("u",)
    _fields_ = [("type", wt.DWORD), ("u", U)]


def type_text(text):
    """Type text as Unicode through SendInput, whatever the keyboard layout is."""
    for ch in text:
        for flags in (KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP):
            i = INPUT(type=1)
            i.ki = KEYBDINPUT(0, ord(ch), flags, 0, None)
            u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(INPUT))
        time.sleep(0.02)


WM_INPUTLANGCHANGEREQUEST = 0x0050


def ukrainian_layout(main):
    """Switch the app's window to the Ukrainian layout (for access keys typed as letters)."""
    layout = u32.LoadKeyboardLayoutW("00000422", 1)
    u32.PostMessageW(main, WM_INPUTLANGCHANGEREQUEST, 0, layout)


def wait_for(pred, timeout, every=0.03):
    end = time.time() + timeout
    while time.time() < end:
        if pred():
            return True
        time.sleep(every)
    return False


# --- the app --------------------------------------------------------------------------------
class App:
    """A running copy of revisor from the scratch folder.

    ``with App(paths, paths["Windows"]) as app:`` starts it on that profile, waits for its main
    window (``app.main``) and brings it to the foreground; the exit closes it with Alt+F4.
    """

    def __init__(self, paths, profile=None, speech=None):
        self.exe = paths["exe"]
        self.profile = profile
        self.speech = speech or Speech()
        self.proc = None
        self.main = None

    def __enter__(self):
        cmd = [self.exe] + (["--profile", self.profile] if self.profile else [])
        self.proc = subprocess.Popen(cmd, cwd=os.path.dirname(self.exe))
        assert wait_for(find_main, 15), "revisor's main window did not appear"
        self.main = find_main()
        time.sleep(1.0)
        u32.SetForegroundWindow(self.main)
        return self

    def __exit__(self, *_):
        # Stop reading speech before closing: focus returns to whatever was behind revisor and
        # NVDA reads it into the log.
        print("\n(stop reading speech) closing")
        sys.stdout.flush()
        if self.proc.poll() is None:
            key(VK_MENU, VK_F4)
            time.sleep(2)
        if self.proc.poll() is None:
            self.proc.terminate()

    def step(self, name, wait=1.5):
        return self.speech.step(name, wait)

    def progress(self):
        return progress_text(self.main)

    def snapshot_done(self):
        return self.progress() is None

    def saving(self):
        """True while the progress field says the snapshot is being saved (or is gone)."""
        return (self.progress() or "Збереження").startswith("Збереження")

    def dismiss_restart_question(self):
        """Close «Перезапустити від імені адміністратора?» if a folder had denied entries."""
        if foreground_title() == "Зробити знімок":
            print("   (restart question up: Esc)")
            key(VK_ESCAPE)
            return True
        return False

    def repeat_last_message(self, name="NVDA+End", wait=2):
        key(VK_INSERT, VK_END)
        return self.step(name, wait)
