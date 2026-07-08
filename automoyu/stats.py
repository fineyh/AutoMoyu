"""统计数据：生涯累计 + 本次会话 + 历史记录，JSON 持久化。

线程模型：add_fish() 由钓鱼工作线程调用，读取由 GUI 线程调用，用锁保护。
"""
from __future__ import annotations

import json
import os
import threading
import time
from datetime import datetime
from typing import Optional

from . import paths


def _now() -> float:
    return time.time()


def fmt_hms(seconds: float) -> str:
    seconds = int(max(0, seconds))
    h, rem = divmod(seconds, 3600)
    m, s = divmod(rem, 60)
    if h:
        return f"{h}:{m:02d}:{s:02d}"
    return f"{m:02d}:{s:02d}"


class Stats:
    def __init__(self) -> None:
        self._lock = threading.RLock()
        self.career = {"fish": 0, "seconds": 0.0, "sessions": 0, "since": None}
        self.history: list[dict] = []
        # 会话状态
        self._active = False
        self._sess_fish = 0
        self._sess_start = 0.0
        self._sess_mode = ""
        self._sess_target = ""
        self._load()

    # ---------- 持久化 ----------
    def _load(self) -> None:
        try:
            with open(paths.STATS_PATH, "r", encoding="utf-8") as f:
                data = json.load(f)
            c = data.get("career", {})
            self.career["fish"] = int(c.get("fish", 0))
            self.career["seconds"] = float(c.get("seconds", 0.0))
            self.career["sessions"] = int(c.get("sessions", 0))
            self.career["since"] = c.get("since")
            h = data.get("history", [])
            if isinstance(h, list):
                self.history = h[-500:]
        except FileNotFoundError:
            pass
        except Exception:
            pass
        if not self.career.get("since"):
            self.career["since"] = datetime.now().strftime("%Y-%m-%d")

    def _save_locked(self) -> None:
        paths.ensure_data_dir()
        data = {"career": self.career, "history": self.history[-500:]}
        tmp = paths.STATS_PATH + ".tmp"
        try:
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump(data, f, ensure_ascii=False, indent=2)
            os.replace(tmp, paths.STATS_PATH)
        except Exception:
            pass

    # ---------- 会话 ----------
    def start_session(self, mode: str, target: str) -> None:
        with self._lock:
            self._active = True
            self._sess_fish = 0
            self._sess_start = _now()
            self._sess_mode = mode
            self._sess_target = target

    def add_fish(self, n: int = 1) -> None:
        with self._lock:
            if not self._active:
                return
            self._sess_fish += n
            self.career["fish"] += n

    def end_session(self) -> Optional[dict]:
        """结束会话，累加进生涯并写入历史。返回本次会话小结。"""
        with self._lock:
            if not self._active:
                return None
            elapsed = _now() - self._sess_start
            self._active = False
            record = {
                "start": datetime.fromtimestamp(self._sess_start).strftime("%Y-%m-%d %H:%M"),
                "seconds": round(elapsed, 1),
                "fish": self._sess_fish,
                "mode": self._sess_mode,
                "target": self._sess_target,
            }
            self.career["seconds"] += elapsed
            self.career["sessions"] += 1
            self.history.append(record)
            self._save_locked()
            return record

    # ---------- 只读快照 ----------
    def session_snapshot(self) -> dict:
        with self._lock:
            elapsed = (_now() - self._sess_start) if self._active else 0.0
            fish = self._sess_fish
            rate = (fish / elapsed * 3600.0) if elapsed > 1 else 0.0
            return {
                "active": self._active,
                "fish": fish,
                "seconds": elapsed,
                "rate_per_hour": rate,
            }

    def career_snapshot(self) -> dict:
        with self._lock:
            return dict(self.career)

    def recent_history(self, n: int = 30) -> list[dict]:
        with self._lock:
            return list(reversed(self.history[-n:]))

    def reset_career(self) -> None:
        with self._lock:
            self.career = {
                "fish": 0, "seconds": 0.0, "sessions": 0,
                "since": datetime.now().strftime("%Y-%m-%d"),
            }
            self.history = []
            self._save_locked()
