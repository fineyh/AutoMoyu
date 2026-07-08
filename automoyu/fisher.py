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
from .detector import (auto_locate_bobber, frame_mad, make_detector,
                       render_bobber_debug)


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
        self._last_frame_emit = 0.0
        self._active_region = None  # 自动定位浮漂时，当前这一竿贴出的判定小框

    # ---------- 对外控制 ----------
    @property
    def running(self) -> bool:
        return self._thread is not None and self._thread.is_alive()

    def start(self) -> bool:
        if self.running:
            return False
        if self.cfg.get("auto_bobber"):
            win = str(self.cfg.get("target_window", "Minecraft"))
            if winio.find_window_rect(win) is None:
                self._emit({"type": "log", "level": "warn",
                            "msg": f"没找到游戏窗口「{win}」，请先启动并保持它可见。"})
                return False
        elif not self.cfg.get("region"):
            self._emit({"type": "log", "level": "warn", "msg": "未设置监控区域，请先选择或自动定位。"})
            return False
        # 收掉可能残留的上一个线程
        if self._thread is not None:
            self._thread.join(timeout=0.5)
        self._stop.clear()
        self._grab_warned = False
        self._detector = make_detector(self.cfg)
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
        auto_bobber = bool(cfg.get("auto_bobber"))
        self.stats.start_session(mode, target)
        if auto_bobber:
            target_name = "浮漂(自动定位)"
        else:
            target_name = {"xp": "经验条", "hookstate": "手持竿钩"}.get(target, "鱼钩")
        self._emit({"type": "state", "running": True, "phase": "运行中"})
        self._emit({"type": "log", "level": "info",
                    "msg": f"开始（{'全自动' if mode == 'full' else '半自动'} / {target_name}）"})
        try:
            while not self._stop.is_set():
                if self._duration_reached():
                    self._emit({"type": "log", "level": "info", "msg": "已达到设定时长，自动停止。"})
                    break
                if auto_bobber:
                    self._cycle_auto_bobber(mode)
                elif target == "hookstate":
                    self._cycle_hook_state()
                elif mode == "full":
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
        base = self._wait_settle()
        if base is None:
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
        base = self._wait_settle()
        if base is None:
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

    # ---------- 手持竿钩：看"状态"（有钩=没在钓 -> 甩竿） ----------
    def _cycle_hook_state(self) -> None:
        # 参考照 = 手持鱼竿、钩可见、未甩出的画面。
        # 首次进入时抓取；之后每次钓上后刷新，自动适应昼夜/天气光照变化。
        if not self._detector.has_baseline:
            ref = self._grab()
            if ref is None:
                self._isleep(0.3)
                return
            self._detector.set_baseline(ref)
            self._emit({"type": "log", "level": "info",
                        "msg": "已记住『有钩』参考画面（请确保此刻手持鱼竿、未甩出）。"})

        if not self._do_cast():
            return

        # 阶段一：确认真的甩出去了（钩离开参考状态）。超时未离开 = 可能没甩出去，返回重甩。
        self._emit({"type": "state", "running": True, "phase": "确认甩出"})
        cast_ok = self._wait_hook(present=False, max_wait=float(self.cfg.get("cast_confirm_s", 3)))
        if self._stop.is_set():
            return
        if not cast_ok:
            self._emit({"type": "log", "level": "warn", "msg": "钩仍在手上，疑似没甩出去，重试。"})
            self._isleep(0.2)
            return

        # 阶段二：等钩重新出现（钓上、收线回来）。
        self._emit({"type": "state", "running": True, "phase": "等钓上"})
        if self._wait_hook(present=True, max_wait=float(self.cfg.get("max_wait_s", 45))):
            cur = self._grab()  # 刷新参考，适应光照/天气变化
            if cur is not None:
                self._detector.set_baseline(cur)
            self._on_catch()
            self._isleep(self.cfg.get("recast_delay_ms", 900) / 1000.0)

    # ---------- 自动定位浮漂：找窗口 -> 甩竿 -> 定位 -> 贴小框判定 ----------
    def _cycle_auto_bobber(self, mode: str) -> None:
        self._active_region = None
        search = winio.find_window_rect(str(self.cfg.get("target_window", "Minecraft")))
        if not search:
            self._emit({"type": "log", "level": "warn", "msg": "没找到游戏窗口，等待…"})
            self._isleep(0.5)
            return
        search = self._shrink_search(search)

        # 甩竿前抓一张「无浮漂」参考（上一竿已收线，水里没浮漂）。
        before = self._grab(search)
        if before is None:
            self._isleep(0.3)
            return
        if not self._do_cast():
            return

        # 等浮漂飞出、落水、水花散开，再抓「有浮漂」的一帧。
        self._emit({"type": "state", "running": True, "phase": "定位浮漂"})
        if not self._isleep(self.cfg.get("settle_ms", 1500) / 1000.0):
            return
        after = self._grab(search)
        if after is None:
            self._isleep(0.3)
            return

        dbg = {} if self.cfg.get("bobber_debug") else None
        box = auto_locate_bobber(
            before, after,
            origin=(search["left"], search["top"]),
            box=int(self.cfg.get("bobber_box", 64)),
            hand_frac_x=float(self.cfg.get("bobber_hand_frac_x", 0.66)),
            hand_frac_y=float(self.cfg.get("bobber_hand_frac_y", 0.5)),
            debug=dbg,
        )
        if not box:
            self._dump_bobber_debug(before, after, dbg, "FAIL")
            self._emit({"type": "log", "level": "warn",
                        "msg": "没定位到浮漂（水面无明显新目标），重甩。"})
            self._isleep(0.3)
            return
        self._dump_bobber_debug(before, after, dbg, "OK")
        self._active_region = box
        self._emit({"type": "log", "level": "info",
                    "msg": f"已定位浮漂 @ {box['left']},{box['top']}（框 {box['width']}px）"})
        self._emit_frame(self._grab(box), False)

        # 在这一小框上等画面稳定、取基准，再监视浮漂下沉。
        base = self._wait_settle()
        if base is None:
            return
        self._detector.set_baseline(base)
        self._emit({"type": "state", "running": True,
                    "phase": "等咬钩" if mode == "full" else "监视中"})

        hit = self._watch_for_change("bite")
        if hit:
            if mode == "full":
                self._isleep(self.cfg.get("bite_reel_delay_ms", 60) / 1000.0)
                if self._stop.is_set():
                    return
                self._click_action("收竿")
                self._on_catch()
                self._isleep(self.cfg.get("post_reel_delay_ms", 1200) / 1000.0)
            else:
                self._on_catch()
                self._isleep(self.cfg.get("recast_delay_ms", 900) / 1000.0)

    def _dump_bobber_debug(self, before, after, dbg, tag: str) -> None:
        """把这一竿的定位依据落盘：甩前/甩后原图 + 打分热力图(叠上选中框)。

        tag: "OK"=定位成功、"FAIL"=被 min_ratio 否决(没找到)。文件名带 ratio，
        方便对着热力图判断阈值该松还是紧。仅在 bobber_debug 开启且传入 dbg 时执行。
        """
        if dbg is None or before is None or after is None:
            return
        try:
            import os
            from . import paths
            from .region_select import save_bmp
            folder = os.path.join(paths.DATA_DIR, "bobber_debug")
            os.makedirs(folder, exist_ok=True)
            stamp = time.strftime("%H%M%S")
            ratio = dbg.get("ratio", 0.0)
            save_bmp(os.path.join(folder, f"{stamp}_1_before.bmp"), before)
            save_bmp(os.path.join(folder, f"{stamp}_2_after.bmp"), after)
            overlay = render_bobber_debug(after, dbg)
            save_bmp(os.path.join(folder,
                     f"{stamp}_3_score_{tag}_r{ratio:.2f}.bmp"), overlay)
            self._emit({"type": "log", "level": "info",
                        "msg": f"已存定位依据({tag} 比值{ratio:.2f}) -> {folder}"})
        except Exception as e:
            self._emit({"type": "log", "level": "warn",
                        "msg": f"保存定位依据失败：{e!r}"})

    def _shrink_search(self, r: dict) -> dict:
        """把窗口客户区往里缩，避开顶部/两侧和底部 HUD（物品栏/经验/饥饿），
        只在中间这片水域里找浮漂，减少误锁到界面元素。"""
        mx = int(r["width"] * 0.08)
        top = int(r["height"] * 0.06)
        bot = int(r["height"] * 0.20)
        return {
            "left": r["left"] + mx,
            "top": r["top"] + top,
            "width": max(10, r["width"] - 2 * mx),
            "height": max(10, r["height"] - top - bot),
        }

    def _wait_hook(self, present: bool, max_wait: float) -> bool:
        """轮询，直到"钩在(present=True)/钩不在(present=False)"稳定成立(True)、
        超时(False) 或被停止(False)。"""
        poll_hz = max(2, int(self.cfg.get("poll_hz", 15)))
        interval = 1.0 / poll_hz
        confirm = max(1, int(self.cfg.get("confirm_frames", 2)))
        t0 = time.time()
        consec = 0
        while not self._stop.is_set():
            if time.time() - t0 > max_wait:
                return False
            frame = self._grab()
            if frame is None:
                self._isleep(interval)
                continue
            val, thr, is_present = self._detector.measure(frame)
            self._emit({"type": "metric", "value": val, "thr": thr, "trig": is_present})
            self._emit_frame(frame, is_present)
            if is_present == present:
                consec += 1
                if consec >= confirm:
                    return True
            else:
                consec = 0
            self._isleep(interval)
        return False

    def _wait_settle(self):
        """甩竿后等浮漂真正落水、画面稳定下来，再取基准。

        固定等待不可靠（落点远近、水花大小不同耗时不同），会把"运动中的一帧"
        当基准，导致刚甩出就误判变化。这里先等一个最小时间跳过甩竿/入水动画，
        再持续对比相邻两帧，直到连续 confirm 帧几乎不动（帧间差 ≤ 安静阈值）才
        认定稳定，用这一稳定帧做基准；最长不超过 settle_max_ms，超时用最后一帧兜底。

        返回稳定后的画面帧；被停止或首帧截图失败时返回 None。
        """
        # 最小等待：跳过甩竿飞行 + 入水那段剧烈变化。
        if not self._isleep(self.cfg.get("settle_ms", 1500) / 1000.0):
            return None

        last = self._grab()
        if last is None:
            self._isleep(0.3)
            return None
        if not self.cfg.get("settle_stabilize", True):
            return last

        self._emit({"type": "state", "running": True, "phase": "等待稳定"})
        poll_hz = max(2, int(self.cfg.get("poll_hz", 15)))
        interval = 1.0 / poll_hz
        max_wait = float(self.cfg.get("settle_max_ms", 5000)) / 1000.0
        confirm = max(1, int(self.cfg.get("confirm_frames", 2)))
        quiet = float(self.cfg.get("settle_quiet_mad", 3.0))
        t0 = time.time()
        prev = last
        consec = 0
        while not self._stop.is_set():
            if not self._isleep(interval):
                return None
            frame = self._grab()
            if frame is None:
                continue
            last = frame
            diff = frame_mad(prev, frame)
            self._emit({"type": "metric", "value": diff, "thr": quiet, "trig": False})
            self._emit_frame(frame, False)
            prev = frame
            if diff <= quiet:
                consec += 1
                if consec >= confirm:
                    return frame
            else:
                consec = 0
            if time.time() - t0 > max_wait:
                return last  # 超时兜底：水面可能一直在动，用最后一帧
        return None

    def _watch_for_change(self, kind: str) -> bool:
        """轮询区域，直到检测到变化(返回 True)、超时(False) 或被停止(False)。

        取基准后先"预热" watch_warmup_ms：这段时间照常喂帧给检测器(让自适应基准
        贴合水面晃动)，但不允许触发。这样刚甩出去、浮漂还在晃/水花未散那阵不会被
        误判成"变化"。预热结束才开始真正判定。
        """
        poll_hz = max(2, int(self.cfg.get("poll_hz", 15)))
        interval = 1.0 / poll_hz
        max_wait = float(self.cfg.get("max_wait_s", 45))
        confirm = max(1, int(self.cfg.get("confirm_frames", 2)))
        warmup = max(0.0, float(self.cfg.get("watch_warmup_ms", 600)) / 1000.0)
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
            in_warmup = (time.time() - t0) < warmup
            shown = trig and not in_warmup
            self._emit({"type": "metric", "value": val, "thr": thr,
                        "trig": shown, "warmup": in_warmup})
            self._emit_frame(frame, shown)
            if shown:
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
        winio.right_click(hold_s=self.cfg.get("click_hold_ms", 90) / 1000.0)
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
    def _emit_frame(self, frame, trig: bool = False) -> None:
        """把当前判定用的画面丢给 GUI 实时显示（限流，最多 ~6fps，省得刷屏）。"""
        if frame is None:
            return
        now = time.time()
        if now - self._last_frame_emit < 0.15:
            return
        self._last_frame_emit = now
        # mss 会复用内部缓冲，必须拷一份再跨线程传给 GUI。
        self._emit({"type": "frame", "img": frame.copy(), "trig": bool(trig)})

    def _grab(self, region: Optional[dict] = None):
        region = region or self._active_region or self.cfg.get("region")
        if not region:
            return None
        try:
            return self._cap.grab(region)
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
