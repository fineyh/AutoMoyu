"""钓鱼状态机（跑在独立工作线程里）。

半自动：甩竿 -> 等稳定 -> 取基准 -> 监视"变化"(钓上/经验变化) -> 重新甩竿。
全自动：甩竿 -> 等稳定 -> 取基准 -> 监视"咬钩"(瞬间变化) -> 收竿 -> 再甩竿。

工作线程不碰任何 GUI 控件，所有输出通过 emit(dict) 回调（GUI 侧入队处理）。
"""
from __future__ import annotations

import threading
import time
from typing import Callable, Optional

from . import winio
from .capture import Capture
from .detector import make_detector


class FishingController:
    def __init__(self, config: dict, stats, emit: Optional[Callable[[dict], None]] = None) -> None:
        self.cfg = config
        self.stats = stats
        self._emit = emit or (lambda ev: None)

        self._thread: Optional[threading.Thread] = None
        self._stop = threading.Event()
        self._detector = None
        self._session_start = 0.0
        self._grab_warned = False

    # ---------- 对外控制 ----------
    @property
    def running(self) -> bool:
        return self._thread is not None and self._thread.is_alive()

    def start(self) -> bool:
        if self.running:
            return False
        region = self.cfg.get("region")
        if not region:
            self._emit({"type": "log", "level": "warn", "msg": "未设置监控区域，请先选择或自动定位。"})
            return False
        # 收掉可能残留的上一个线程
        if self._thread is not None:
            self._thread.join(timeout=0.5)
        self._stop.clear()
        self._grab_warned = False
        self._detector = make_detector(self.cfg.get("target", "xp"), self.cfg.get("sensitivity", 5))
        self._thread = threading.Thread(target=self._loop, name="FisherLoop", daemon=True)
        self._thread.start()
        return True

    def stop(self) -> None:
        self._stop.set()

    def wait(self, timeout: float = 1.5) -> None:
        if self._thread is not None:
            self._thread.join(timeout=timeout)

    def toggle(self) -> None:
        if self.running:
            self.stop()
        else:
            self.start()

    def set_sensitivity(self, s: int) -> None:
        self.cfg["sensitivity"] = int(s)
        if self._detector is not None:
            self._detector.set_sensitivity(int(s))

    # ---------- 工作线程 ----------
    def _loop(self) -> None:
        cfg = self.cfg
        mode = cfg.get("mode", "semi")
        target = cfg.get("target", "xp")
        self._cap = Capture()
        self._session_start = time.time()
        self.stats.start_session(mode, target)
        self._emit({"type": "state", "running": True, "phase": "运行中"})
        self._emit({"type": "log", "level": "info",
                    "msg": f"开始（{'全自动' if mode == 'full' else '半自动'} / {'经验条' if target == 'xp' else '鱼钩'}）"})
        try:
            while not self._stop.is_set():
                if self._duration_reached():
                    self._emit({"type": "log", "level": "info", "msg": "已达到设定时长，自动停止。"})
                    break
                if mode == "full":
                    self._cycle_full()
                else:
                    self._cycle_semi()
        except Exception as e:  # 兜底，避免线程静默死掉
            self._emit({"type": "log", "level": "warn", "msg": f"运行异常：{e!r}"})
        finally:
            rec = self.stats.end_session()
            self._cap.close()
            self._emit({"type": "stats"})
            self._emit({"type": "state", "running": False, "phase": "待机"})
            if rec:
                self._emit({"type": "log", "level": "info",
                            "msg": f"结束：本次 {rec['fish']} 条 / {int(rec['seconds'])}s"})

    def _duration_reached(self) -> bool:
        dur = int(self.cfg.get("duration_min", 0) or 0)
        if dur <= 0:
            return False
        return (time.time() - self._session_start) >= dur * 60

    # ---------- 半自动：只甩竿，等"钓上/经验变化" ----------
    def _cycle_semi(self) -> None:
        if not self._do_cast():
            return
        if not self._isleep(self.cfg.get("settle_ms", 1500) / 1000.0):
            return
        base = self._grab()
        if base is None:
            self._isleep(0.3)
            return
        self._detector.set_baseline(base)
        self._emit({"type": "state", "running": True, "phase": "监视中"})

        caught = self._watch_for_change("catch")
        if caught:
            self._on_catch()
            self._isleep(self.cfg.get("recast_delay_ms", 900) / 1000.0)

    # ---------- 全自动：甩竿 -> 咬钩 -> 收竿 ----------
    def _cycle_full(self) -> None:
        if not self._do_cast():
            return
        if not self._isleep(self.cfg.get("settle_ms", 1500) / 1000.0):
            return
        base = self._grab()
        if base is None:
            self._isleep(0.3)
            return
        self._detector.set_baseline(base)
        self._emit({"type": "state", "running": True, "phase": "等咬钩"})

        bite = self._watch_for_change("bite")
        if bite:
            self._isleep(self.cfg.get("bite_reel_delay_ms", 60) / 1000.0)
            if self._stop.is_set():
                return
            self._click_action("收竿")
            self._on_catch()
            self._isleep(self.cfg.get("post_reel_delay_ms", 1200) / 1000.0)

    def _watch_for_change(self, kind: str) -> bool:
        """轮询区域，直到检测到变化(返回 True)、超时(False) 或被停止(False)。"""
        poll_hz = max(2, int(self.cfg.get("poll_hz", 15)))
        interval = 1.0 / poll_hz
        max_wait = float(self.cfg.get("max_wait_s", 45))
        confirm = max(1, int(self.cfg.get("confirm_frames", 2)))
        t0 = time.time()
        consec = 0
        while not self._stop.is_set():
            if time.time() - t0 > max_wait:
                self._emit({"type": "log", "level": "warn", "msg": "长时间无变化，保险重甩。"})
                return False
            frame = self._grab()
            if frame is None:
                self._isleep(interval)
                continue
            val, thr, trig = self._detector.measure(frame)
            self._emit({"type": "metric", "value": val, "thr": thr, "trig": trig})
            if trig:
                consec += 1
                if consec >= confirm:
                    return True
            else:
                consec = 0
            self._isleep(interval)
        return False

    # ---------- 动作 ----------
    def _do_cast(self) -> bool:
        return self._click_action("甩竿")

    def _click_action(self, label: str) -> bool:
        if not self._wait_focus():
            return False
        winio.right_click(hold_s=self.cfg.get("click_hold_ms", 40) / 1000.0)
        self._emit({"type": "log", "level": "info", "msg": label})
        return True

    def _wait_focus(self) -> bool:
        if not self.cfg.get("focus_guard", True):
            return True
        target = str(self.cfg.get("target_window", "Minecraft")).lower()
        warned = False
        while not self._stop.is_set():
            title = winio.get_foreground_title().lower()
            if target in title:
                if warned:
                    self._emit({"type": "state", "running": True, "phase": "运行中"})
                return True
            if not warned:
                self._emit({"type": "log", "level": "warn",
                            "msg": f"未聚焦「{self.cfg.get('target_window')}」，暂停点击，等待切回…"})
                self._emit({"type": "state", "running": True, "phase": "等待聚焦"})
                warned = True
            self._isleep(0.2)
        return False

    def _on_catch(self) -> None:
        self.stats.add_fish(1)
        self._emit({"type": "log", "level": "catch", "msg": "钓到一条！"})
        self._emit({"type": "stats"})

    # ---------- 工具 ----------
    def _grab(self):
        try:
            return self._cap.grab(self.cfg["region"])
        except Exception as e:
            if not self._grab_warned:
                self._emit({"type": "log", "level": "warn", "msg": f"截图失败：{e!r}"})
                self._grab_warned = True
            return None

    def _isleep(self, seconds: float) -> bool:
        """可被停止打断的 sleep；返回 True 表示正常睡完，False 表示被停止。"""
        end = time.time() + max(0.0, seconds)
        while True:
            remaining = end - time.time()
            if remaining <= 0:
                return True
            if self._stop.wait(min(remaining, 0.05)):
                return False
