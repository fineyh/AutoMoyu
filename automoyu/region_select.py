"""全屏拖拽选择监控区域。返回屏幕绝对坐标 {left, top, width, height}。"""
from __future__ import annotations

import tkinter as tk
from typing import Callable, Optional


class RegionSelector(tk.Toplevel):
    def __init__(self, master: tk.Misc, on_done: Callable[[Optional[dict]], None]) -> None:
        super().__init__(master)
        self.on_done = on_done
        self.overrideredirect(True)
        self.attributes("-topmost", True)
        try:
            self.attributes("-alpha", 0.30)
        except Exception:
            pass
        # 覆盖整个主显示器
        w = self.winfo_screenwidth()
        h = self.winfo_screenheight()
        self.geometry(f"{w}x{h}+0+0")
        self.configure(bg="black")
        self.canvas = tk.Canvas(self, bg="gray15", highlightthickness=0, cursor="crosshair")
        self.canvas.pack(fill="both", expand=True)
        self.canvas.create_text(
            w // 2, 40,
            text="拖拽框选要监控的区域（经验条：框住那条绿色横条；鱼钩：框住手持鱼竿/浮漂处）  ·  Esc 取消",
            fill="white", font=("Microsoft YaHei UI", 13),
        )

        self._start = None
        self._rect = None
        self.canvas.bind("<ButtonPress-1>", self._on_press)
        self.canvas.bind("<B1-Motion>", self._on_drag)
        self.canvas.bind("<ButtonRelease-1>", self._on_release)
        self.bind("<Escape>", lambda e: self._finish(None))
        self.focus_force()

    def _local(self, event) -> tuple[int, int]:
        return event.x_root - self.winfo_rootx(), event.y_root - self.winfo_rooty()

    def _on_press(self, event) -> None:
        self._start = self._local(event)
        if self._rect:
            self.canvas.delete(self._rect)
        self._rect = self.canvas.create_rectangle(
            *self._start, *self._start, outline="#39FF14", width=2)

    def _on_drag(self, event) -> None:
        if not self._start:
            return
        x, y = self._local(event)
        self.canvas.coords(self._rect, self._start[0], self._start[1], x, y)

    def _on_release(self, event) -> None:
        if not self._start:
            self._finish(None)
            return
        x0, y0 = self._start
        x1, y1 = self._local(event)
        left, top = min(x0, x1), min(y0, y1)
        width, height = abs(x1 - x0), abs(y1 - y0)
        if width < 5 or height < 3:
            self._finish(None)
            return
        # 加上窗口自身位置，转成屏幕绝对坐标
        left += self.winfo_rootx()
        top += self.winfo_rooty()
        self._finish({"left": int(left), "top": int(top),
                      "width": int(width), "height": int(height)})

    def _finish(self, region: Optional[dict]) -> None:
        try:
            self.destroy()
        finally:
            self.on_done(region)
