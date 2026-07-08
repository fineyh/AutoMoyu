"""Windows 底层输入/窗口层。

- 用 SendInput 从驱动层注入鼠标点击（和 AutoHotkey 相同的路径，能打进 UWP/Bedrock）。
- 读取前台窗口标题，用于"只在 Minecraft 聚焦时点击"的防误触。
- 用 RegisterHotKey 注册全局热键（即使 Minecraft 聚焦也能收到）。
- 进程 DPI 感知，保证 mss 截图坐标与真实像素一致。
"""
from __future__ import annotations

import ctypes
import threading
import time
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

# ---- ULONG_PTR（指针宽度整数）----
if ctypes.sizeof(ctypes.c_void_p) == 8:
    ULONG_PTR = ctypes.c_ulonglong
else:
    ULONG_PTR = ctypes.c_ulong


# ---- SendInput 结构体 ----
class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class _INPUTunion(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("u", _INPUTunion)]


INPUT_MOUSE = 0
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
MOUSEEVENTF_RIGHTDOWN = 0x0008
MOUSEEVENTF_RIGHTUP = 0x0010

# ---- 函数签名（64 位下必须显式声明，避免句柄/指针被截断）----
user32.SendInput.argtypes = (wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int)
user32.SendInput.restype = wintypes.UINT

user32.GetForegroundWindow.restype = wintypes.HWND
user32.GetWindowTextLengthW.argtypes = (wintypes.HWND,)
user32.GetWindowTextLengthW.restype = ctypes.c_int
user32.GetWindowTextW.argtypes = (wintypes.HWND, wintypes.LPWSTR, ctypes.c_int)
user32.GetWindowTextW.restype = ctypes.c_int

user32.RegisterHotKey.argtypes = (wintypes.HWND, ctypes.c_int, wintypes.UINT, wintypes.UINT)
user32.RegisterHotKey.restype = wintypes.BOOL
user32.UnregisterHotKey.argtypes = (wintypes.HWND, ctypes.c_int)
user32.UnregisterHotKey.restype = wintypes.BOOL
user32.GetMessageW.argtypes = (ctypes.POINTER(wintypes.MSG), wintypes.HWND, wintypes.UINT, wintypes.UINT)
user32.GetMessageW.restype = ctypes.c_int
user32.PeekMessageW.argtypes = (
    ctypes.POINTER(wintypes.MSG), wintypes.HWND, wintypes.UINT, wintypes.UINT, wintypes.UINT)
user32.PeekMessageW.restype = wintypes.BOOL
user32.PostThreadMessageW.argtypes = (wintypes.DWORD, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
user32.PostThreadMessageW.restype = wintypes.BOOL
kernel32.GetCurrentThreadId.restype = wintypes.DWORD


def set_dpi_awareness() -> None:
    """让进程 DPI 感知，保证截图坐标 == 屏幕物理像素。"""
    try:
        # PROCESS_SYSTEM_DPI_AWARE = 1，对 tkinter 较友好
        ctypes.windll.shcore.SetProcessDpiAwareness(1)
        return
    except Exception:
        pass
    try:
        user32.SetProcessDPIAware()
    except Exception:
        pass


def _send(*inputs: INPUT) -> int:
    n = len(inputs)
    arr = (INPUT * n)(*inputs)
    return user32.SendInput(n, arr, ctypes.sizeof(INPUT))


def _mouse_input(flags: int) -> INPUT:
    return INPUT(type=INPUT_MOUSE, u=_INPUTunion(mi=MOUSEINPUT(0, 0, 0, flags, 0, 0)))


def right_click(hold_s: float = 0.04) -> None:
    """在当前光标位置按下并松开右键（第一人称下与坐标无关）。"""
    _send(_mouse_input(MOUSEEVENTF_RIGHTDOWN))
    time.sleep(max(0.0, hold_s))
    _send(_mouse_input(MOUSEEVENTF_RIGHTUP))


def left_click(hold_s: float = 0.04) -> None:
    _send(_mouse_input(MOUSEEVENTF_LEFTDOWN))
    time.sleep(max(0.0, hold_s))
    _send(_mouse_input(MOUSEEVENTF_LEFTUP))


def get_foreground_title() -> str:
    hwnd = user32.GetForegroundWindow()
    if not hwnd:
        return ""
    length = user32.GetWindowTextLengthW(hwnd)
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buf, length + 1)
    return buf.value


# ---- 虚拟键码 ----
VK_CODES = {
    "F1": 0x70, "F2": 0x71, "F3": 0x72, "F4": 0x73, "F5": 0x74, "F6": 0x75,
    "F7": 0x76, "F8": 0x77, "F9": 0x78, "F10": 0x79, "F11": 0x7A, "F12": 0x7B,
    "HOME": 0x24, "END": 0x23, "INSERT": 0x2D, "PAGEUP": 0x21, "PAGEDOWN": 0x22,
    "PAUSE": 0x13, "SCROLL": 0x91,
}
AVAILABLE_HOTKEYS = list(VK_CODES.keys())

WM_HOTKEY = 0x0312
WM_QUIT = 0x0012
MOD_NOREPEAT = 0x4000
PM_NOREMOVE = 0x0000


class HotkeyManager:
    """在独立线程里跑消息循环，注册全局热键。

    bindings: list[(name:str, callback:callable)]，name 是 VK_CODES 里的键名。
    回调在热键线程执行，必须线程安全（内部只做置位/入队）。
    """

    def __init__(self) -> None:
        self._thread: threading.Thread | None = None
        self._tid: int = 0
        self._bindings: list[tuple[str, object]] = []
        self._ready = threading.Event()

    def start(self, bindings: list[tuple[str, object]]) -> list[str]:
        """返回注册失败的键名列表（可能被其它程序占用）。"""
        self.stop()
        self._bindings = [b for b in bindings if b[0].upper() in VK_CODES]
        self._ready.clear()
        self._failed: list[str] = []
        self._thread = threading.Thread(target=self._run, name="HotkeyLoop", daemon=True)
        self._thread.start()
        self._ready.wait(timeout=2.0)
        return list(self._failed)

    def _run(self) -> None:
        self._tid = kernel32.GetCurrentThreadId()
        # 强制创建消息队列，保证之后 PostThreadMessage 一定送达
        msg = wintypes.MSG()
        user32.PeekMessageW(ctypes.byref(msg), None, 0, 0, PM_NOREMOVE)

        registered: list[int] = []
        for i, (name, _cb) in enumerate(self._bindings, start=1):
            vk = VK_CODES[name.upper()]
            if user32.RegisterHotKey(None, i, MOD_NOREPEAT, vk):
                registered.append(i)
            else:
                self._failed.append(name)
        self._ready.set()

        while True:
            ret = user32.GetMessageW(ctypes.byref(msg), None, 0, 0)
            if ret in (0, -1):
                break
            if msg.message == WM_HOTKEY:
                idx = int(msg.wParam)
                if 1 <= idx <= len(self._bindings):
                    cb = self._bindings[idx - 1][1]
                    try:
                        cb()
                    except Exception:
                        pass

        for i in registered:
            user32.UnregisterHotKey(None, i)

    def stop(self) -> None:
        if self._thread and self._tid:
            user32.PostThreadMessageW(self._tid, WM_QUIT, 0, 0)
            self._thread.join(timeout=1.5)
        self._thread = None
        self._tid = 0
