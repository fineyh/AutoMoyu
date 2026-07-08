"""屏幕截图（基于 mss）。

mss 的实例不是线程安全的：谁用谁在自己线程里创建。
本模块提供一个轻封装，供钓鱼工作线程使用。
"""
from __future__ import annotations

from typing import Optional

import mss
import numpy as np

# mss 10 起 mss.mss() 被弃用，改用 mss.MSS（旧版无此名则回退）。
_MSS = getattr(mss, "MSS", None) or mss.mss


class Capture:
    def __init__(self) -> None:
        self._sct = None

    def _ensure(self):
        if self._sct is None:
            self._sct = _MSS()
        return self._sct

    def grab(self, region: dict) -> np.ndarray:
        """region: {left, top, width, height}，返回 HxWx4 的 BGRA uint8 数组。"""
        sct = self._ensure()
        img = sct.grab({
            "left": int(region["left"]),
            "top": int(region["top"]),
            "width": int(region["width"]),
            "height": int(region["height"]),
        })
        return np.asarray(img, dtype=np.uint8)  # BGRA

    def grab_screen(self) -> np.ndarray:
        """抓取主显示器整屏，返回 BGRA。"""
        sct = self._ensure()
        mon = sct.monitors[1]  # 1 = 主显示器
        img = sct.grab(mon)
        return np.asarray(img, dtype=np.uint8)

    def screen_size(self) -> tuple[int, int]:
        sct = self._ensure()
        mon = sct.monitors[1]
        return mon["width"], mon["height"]

    def close(self) -> None:
        if self._sct is not None:
            try:
                self._sct.close()
            except Exception:
                pass
            self._sct = None
