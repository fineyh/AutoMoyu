"""AutoMoyu 主界面（tkinter）。

- 小窗、可置顶。
- 选择监控目标（经验条/鱼钩）与区域（拖拽 / 自动定位）。
- 半自动/全自动、灵敏度、时长、热键。
- 实时检测меter、本次/生涯数据、历史、日志。

线程规则：工作线程只通过 queue 发事件，GUI 用 after() 轮询；GUI 控件只在主线程碰。
"""
from __future__ import annotations

import os
import queue
import tkinter as tk
from tkinter import messagebox, ttk

from . import config as cfgmod
from . import detector as det
from . import winio
from .capture import Capture
from .fisher import FishingController
from .region_select import RegionSelector, screenshot_to_ppm
from .stats import Stats, fmt_hms

FONT = ("Microsoft YaHei UI", 9)
FONT_BIG = ("Microsoft YaHei UI", 15, "bold")
FONT_SMALL = ("Microsoft YaHei UI", 8)


class App:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.cfg = cfgmod.load()
        self.stats = Stats()
        self.events: "queue.Queue[dict]" = queue.Queue()
        self.controller = FishingController(self.cfg, self.stats, emit=self.events.put)
        self.hotkeys = winio.HotkeyManager()
        self._gui_cap = Capture()
        self._busy = False  # 自动定位倒计时期间
        self._last_metric = None

        self._build_ui()
        self._apply_loaded_config()
        self._register_hotkeys()
        self.root.after(50, self._pump_events)
        self.root.after(500, self._tick_stats)
        self.root.protocol("WM_DELETE_WINDOW", self._on_close)

    # ================= UI 构建 =================
    def _build_ui(self) -> None:
        self.root.title("AutoMoyu 🎣")
        self.root.geometry("390x860")
        self.root.minsize(370, 640)
        try:
            self.root.tk.call("tk", "scaling", 1.0)
        except Exception:
            pass

        style = ttk.Style()
        try:
            style.theme_use("clam")
        except Exception:
            pass

        pad = {"padx": 8, "pady": 3}

        # --- 顶部：标题 + 置顶 ---
        top = ttk.Frame(self.root)
        top.pack(fill="x", **pad)
        ttk.Label(top, text="AutoMoyu 摸鱼助手", font=("Microsoft YaHei UI", 12, "bold")).pack(side="left")
        self.var_top = tk.BooleanVar(value=self.cfg.get("always_on_top", True))
        ttk.Checkbutton(top, text="📌置顶", variable=self.var_top,
                        command=self._on_topmost).pack(side="right")

        # --- 状态灯 ---
        st = ttk.Frame(self.root)
        st.pack(fill="x", **pad)
        self.dot = tk.Canvas(st, width=14, height=14, highlightthickness=0)
        self.dot.pack(side="left")
        self._dot_id = self.dot.create_oval(2, 2, 12, 12, fill="#888", outline="")
        self.var_status = tk.StringVar(value="待机")
        ttk.Label(st, textvariable=self.var_status, font=FONT).pack(side="left", padx=6)

        # --- 模式 & 目标 ---
        mf = ttk.LabelFrame(self.root, text="模式 / 监控目标")
        mf.pack(fill="x", **pad)
        self.var_mode = tk.StringVar(value=self.cfg.get("mode", "semi"))
        row1 = ttk.Frame(mf); row1.pack(fill="x", padx=6, pady=2)
        ttk.Radiobutton(row1, text="半自动(只甩竿)", value="semi",
                        variable=self.var_mode, command=self._on_cfg_change).pack(side="left")
        ttk.Radiobutton(row1, text="全自动(甩+收)", value="full",
                        variable=self.var_mode, command=self._on_cfg_change).pack(side="left", padx=8)

        self.var_target = tk.StringVar(value=self.cfg.get("target", "xp"))
        row2 = ttk.Frame(mf); row2.pack(fill="x", padx=6, pady=2)
        ttk.Radiobutton(row2, text="经验条", value="xp",
                        variable=self.var_target, command=self._on_target_change).pack(side="left")
        ttk.Radiobutton(row2, text="鱼钩/浮漂", value="hook",
                        variable=self.var_target, command=self._on_target_change).pack(side="left", padx=8)
        ttk.Radiobutton(row2, text="手持竿钩", value="hookstate",
                        variable=self.var_target, command=self._on_target_change).pack(side="left")

        # 自动识别窗口 + 每竿自动定位浮漂：勾上就不用手动框选了。
        rowab = ttk.Frame(mf); rowab.pack(fill="x", padx=6, pady=(2, 0))
        self.var_autobobber = tk.BooleanVar(value=bool(self.cfg.get("auto_bobber", False)))
        ttk.Checkbutton(rowab, text="自动识别窗口并定位浮漂（无需框选）",
                        variable=self.var_autobobber,
                        command=self._on_autobobber_change).pack(side="left")

        rowdbg = ttk.Frame(mf); rowdbg.pack(fill="x", padx=6, pady=(0, 2))
        self.var_bobberdbg = tk.BooleanVar(value=bool(self.cfg.get("bobber_debug", False)))
        ttk.Checkbutton(rowdbg, text="保存定位依据截图（甩前/甩后/热力图）到 data\\bobber_debug",
                        variable=self.var_bobberdbg,
                        command=self._on_cfg_change).pack(side="left")

        row3 = ttk.Frame(mf); row3.pack(fill="x", padx=6, pady=3)
        self.btn_region = ttk.Button(row3, text="选择区域", width=9, command=self._select_region)
        self.btn_region.pack(side="left")
        self.btn_auto = ttk.Button(row3, text="自动定位经验条", command=self._auto_locate)
        self.btn_auto.pack(side="left", padx=4)
        self.var_region = tk.StringVar(value="区域：未设置")
        ttk.Label(mf, textvariable=self.var_region, font=FONT_SMALL, foreground="#555").pack(
            anchor="w", padx=6, pady=(0, 2))
        # 选完区域/自动定位后显示框到的画面；运行时这里会实时刷新"正在判定"的画面，
        # 外框绿=正常、红=判定为变化/触发，方便你一眼看出判定快了还是慢了。
        self._preview_photo = None
        self.preview_box = tk.Frame(mf, bg="#ccc", bd=0)
        self.preview_box.pack(padx=6, pady=(0, 4))
        self.lbl_preview = ttk.Label(
            self.preview_box, text="（选择区域或自动定位后，这里显示框到的画面）",
            font=FONT_SMALL, foreground="#888", anchor="center")
        self.lbl_preview.pack(padx=2, pady=2)

        # --- 灵敏度 + 实时меter ---
        sf = ttk.LabelFrame(self.root, text="灵敏度 & 实时检测")
        sf.pack(fill="x", **pad)
        srow = ttk.Frame(sf); srow.pack(fill="x", padx=6, pady=2)
        ttk.Label(srow, text="灵敏度", font=FONT).pack(side="left")
        self.var_sens = tk.IntVar(value=int(self.cfg.get("sensitivity", 5)))
        self.scale = ttk.Scale(srow, from_=1, to=10, orient="horizontal",
                               command=self._on_sens)
        self.scale.pack(side="left", fill="x", expand=True, padx=6)
        self.lbl_sens = ttk.Label(srow, text=str(self.var_sens.get()), width=2, font=FONT)
        self.lbl_sens.pack(side="left")
        self.scale.set(self.var_sens.get())  # 最后再 set，避免回调触发时控件还没建好

        self.meter = tk.Canvas(sf, height=16, bg="#eee", highlightthickness=1,
                               highlightbackground="#ccc")
        self.meter.pack(fill="x", padx=6, pady=2)
        self.meter.bind("<Configure>", lambda e: self._draw_meter())
        self.var_metric = tk.StringVar(value="变化 —  /  阈值 —")
        ttk.Label(sf, textvariable=self.var_metric, font=FONT_SMALL).pack(anchor="w", padx=6, pady=(0, 4))

        # --- 时序微调（甩出 → 判定）---
        tf = ttk.LabelFrame(self.root, text="时序微调（甩出 → 判定）")
        tf.pack(fill="x", **pad)
        self.var_adaptive = tk.BooleanVar(value=bool(self.cfg.get("hook_adaptive", True)))
        ck = ttk.Frame(tf); ck.pack(fill="x", padx=6, pady=(2, 0))
        ttk.Checkbutton(ck, text="自适应基准：只有浮漂突然下沉才触发（抗水面晃动）",
                        variable=self.var_adaptive, command=self._on_cfg_change).pack(side="left")
        self._timing_vars: dict = {}
        self._timing_entry(tf, "甩竿后先等(ms)", "settle_ms", "跳过飞行/入水动画")
        self._timing_entry(tf, "浮漂框大小(px)", "bobber_box", "自动定位浮漂时贴的小框边长")
        self._timing_entry(tf, "判定预热(ms)", "watch_warmup_ms", "这段时间不判定，调大治『太快』")
        self._timing_entry(tf, "静止阈值", "settle_quiet_mad", "水面老在动就调大")
        self._timing_entry(tf, "钓上后重甩(ms)", "recast_delay_ms", None)
        self._timing_entry(tf, "咬钩→收竿(ms)", "bite_reel_delay_ms", "仅全自动")
        self._timing_entry(tf, "收竿后再甩(ms)", "post_reel_delay_ms", "仅全自动")

        # --- 时长 & 热键 & 安全 ---
        of = ttk.LabelFrame(self.root, text="选项")
        of.pack(fill="x", **pad)
        r = ttk.Frame(of); r.pack(fill="x", padx=6, pady=2)
        ttk.Label(r, text="本次时长(分, 0=不限)", font=FONT).pack(side="left")
        self.var_dur = tk.StringVar(value=str(self.cfg.get("duration_min", 0)))
        e = ttk.Entry(r, textvariable=self.var_dur, width=6)
        e.pack(side="left", padx=6)
        e.bind("<FocusOut>", lambda ev: self._on_cfg_change())

        r2 = ttk.Frame(of); r2.pack(fill="x", padx=6, pady=2)
        ttk.Label(r2, text="开始/停止", font=FONT).pack(side="left")
        self.var_toggle = tk.StringVar(value=self.cfg.get("toggle_key", "F6"))
        ttk.Combobox(r2, textvariable=self.var_toggle, values=winio.AVAILABLE_HOTKEYS,
                     width=6, state="readonly").pack(side="left", padx=4)
        ttk.Label(r2, text="急停", font=FONT).pack(side="left", padx=(8, 0))
        self.var_stop = tk.StringVar(value=self.cfg.get("stop_key", "F8"))
        ttk.Combobox(r2, textvariable=self.var_stop, values=winio.AVAILABLE_HOTKEYS,
                     width=6, state="readonly").pack(side="left", padx=4)
        ttk.Button(r2, text="应用热键", command=self._register_hotkeys).pack(side="left", padx=4)

        r3 = ttk.Frame(of); r3.pack(fill="x", padx=6, pady=2)
        self.var_guard = tk.BooleanVar(value=self.cfg.get("focus_guard", True))
        ttk.Checkbutton(r3, text="仅当窗口聚焦才点击：", variable=self.var_guard,
                        command=self._on_cfg_change).pack(side="left")
        self.var_win = tk.StringVar(value=self.cfg.get("target_window", "Minecraft"))
        ew = ttk.Entry(r3, textvariable=self.var_win, width=12)
        ew.pack(side="left")
        ew.bind("<FocusOut>", lambda ev: self._on_cfg_change())

        r4 = ttk.Frame(of); r4.pack(fill="x", padx=6, pady=2)
        ttk.Label(r4, text="甩竿按住(ms)", font=FONT).pack(side="left")
        self.var_hold = tk.StringVar(value=str(self.cfg.get("click_hold_ms", 90)))
        eh = ttk.Entry(r4, textvariable=self.var_hold, width=6)
        eh.pack(side="left", padx=6)
        eh.bind("<FocusOut>", lambda ev: self._on_cfg_change())
        ttk.Label(r4, text="太快可能甩不出去；甩不出就调大(建议80–150)",
                  font=FONT_SMALL, foreground="#888").pack(side="left")

        # --- 开始/停止按钮 ---
        bf = ttk.Frame(self.root)
        bf.pack(fill="x", **pad)
        self.btn_start = ttk.Button(bf, text="▶ 开始", command=self.controller.start)
        self.btn_start.pack(side="left", fill="x", expand=True, padx=(0, 4))
        self.btn_stop = ttk.Button(bf, text="■ 停止", command=self.controller.stop)
        self.btn_stop.pack(side="left", fill="x", expand=True)

        # --- 数据 ---
        df = ttk.LabelFrame(self.root, text="数据")
        df.pack(fill="x", **pad)
        grid = ttk.Frame(df); grid.pack(fill="x", padx=6, pady=4)
        ttk.Label(grid, text="本次", font=("Microsoft YaHei UI", 9, "bold")).grid(row=0, column=0, sticky="w")
        ttk.Label(grid, text="生涯", font=("Microsoft YaHei UI", 9, "bold")).grid(row=0, column=1, sticky="w")
        self.var_sess = tk.StringVar(value="—")
        self.var_career = tk.StringVar(value="—")
        ttk.Label(grid, textvariable=self.var_sess, font=FONT, justify="left").grid(
            row=1, column=0, sticky="nw", padx=(0, 12))
        ttk.Label(grid, textvariable=self.var_career, font=FONT, justify="left").grid(
            row=1, column=1, sticky="nw")
        grid.columnconfigure(0, weight=1)
        grid.columnconfigure(1, weight=1)
        hb = ttk.Frame(df); hb.pack(fill="x", padx=6, pady=(0, 4))
        ttk.Button(hb, text="历史记录", command=self._show_history).pack(side="left")
        ttk.Button(hb, text="清空生涯", command=self._reset_career).pack(side="left", padx=6)

        # --- 日志 ---
        lf = ttk.LabelFrame(self.root, text="日志")
        lf.pack(fill="both", expand=True, **pad)
        self.log = tk.Text(lf, height=6, font=FONT_SMALL, state="disabled", wrap="word")
        self.log.pack(fill="both", expand=True, padx=4, pady=4)
        self.log.tag_config("warn", foreground="#c26a00")
        self.log.tag_config("catch", foreground="#0a7d12")

    def _timing_entry(self, parent, label: str, key: str, hint) -> None:
        row = ttk.Frame(parent); row.pack(fill="x", padx=6, pady=1)
        ttk.Label(row, text=label, font=FONT_SMALL, width=13, anchor="w").pack(side="left")
        var = tk.StringVar(value=str(self.cfg.get(key, cfgmod.DEFAULTS.get(key))))
        e = ttk.Entry(row, textvariable=var, width=7)
        e.pack(side="left", padx=4)
        e.bind("<FocusOut>", lambda ev: self._on_cfg_change())
        e.bind("<Return>", lambda ev: self._on_cfg_change())
        if hint:
            ttk.Label(row, text=hint, font=FONT_SMALL, foreground="#888").pack(side="left")
        self._timing_vars[key] = var

    # ================= 配置应用/保存 =================
    def _apply_loaded_config(self) -> None:
        self._on_topmost()
        self._refresh_region_label()
        self._refresh_stats()
        self._sync_region_buttons()
        self._log(f"数据目录：{self._data_hint()}", "info")

    def _data_hint(self) -> str:
        from . import paths
        return paths.DATA_DIR

    def _collect_config(self) -> None:
        self.cfg["mode"] = self.var_mode.get()
        self.cfg["target"] = self.var_target.get()
        self.cfg["sensitivity"] = int(self.var_sens.get())
        self.cfg["always_on_top"] = bool(self.var_top.get())
        self.cfg["focus_guard"] = bool(self.var_guard.get())
        self.cfg["target_window"] = self.var_win.get().strip() or "Minecraft"
        self.cfg["toggle_key"] = self.var_toggle.get()
        self.cfg["stop_key"] = self.var_stop.get()
        try:
            self.cfg["duration_min"] = max(0, int(float(self.var_dur.get())))
        except (ValueError, TypeError):
            self.cfg["duration_min"] = 0
            self.var_dur.set("0")
        try:
            self.cfg["click_hold_ms"] = max(10, int(float(self.var_hold.get())))
        except (ValueError, TypeError):
            self.cfg["click_hold_ms"] = 90
            self.var_hold.set("90")
        if hasattr(self, "var_adaptive"):
            self.cfg["hook_adaptive"] = bool(self.var_adaptive.get())
        if hasattr(self, "var_autobobber"):
            self.cfg["auto_bobber"] = bool(self.var_autobobber.get())
        if hasattr(self, "var_bobberdbg"):
            self.cfg["bobber_debug"] = bool(self.var_bobberdbg.get())
        for key, var in getattr(self, "_timing_vars", {}).items():
            default = cfgmod.DEFAULTS.get(key)
            is_float = key == "settle_quiet_mad"
            try:
                v = float(var.get())
                self.cfg[key] = v if is_float else max(0, int(v))
            except (ValueError, TypeError):
                self.cfg[key] = default
                var.set(str(default))

    def _save(self) -> None:
        cfgmod.save(self.cfg)

    def _on_cfg_change(self) -> None:
        self._collect_config()
        self._save()

    def _on_target_change(self) -> None:
        self._on_cfg_change()
        self._sync_region_buttons()

    def _on_autobobber_change(self) -> None:
        self._on_cfg_change()
        self._sync_region_buttons()
        if self.var_autobobber.get():
            self._log("已开启自动定位浮漂：无需框选，每甩一竿自动找浮漂贴小框判定。"
                      "请确保游戏窗口标题含「%s」。" % self.cfg.get("target_window", "Minecraft"), "info")

    def _sync_region_buttons(self) -> None:
        """自动定位浮漂时，手动框选/自动定位经验条都用不上，禁用以免误操作。"""
        auto_bobber = bool(self.var_autobobber.get()) if hasattr(self, "var_autobobber") else False
        if hasattr(self, "btn_region"):
            self.btn_region.state(["disabled"] if auto_bobber else ["!disabled"])
        # 自动定位经验条：仅经验条目标、且未开自动浮漂时可用。
        if auto_bobber or self.var_target.get() != "xp":
            self.btn_auto.state(["disabled"])
        else:
            self.btn_auto.state(["!disabled"])

    def _on_sens(self, _val: str) -> None:
        s = int(round(float(self.scale.get())))
        self.var_sens.set(s)
        if hasattr(self, "lbl_sens"):
            self.lbl_sens.config(text=str(s))
        self.controller.set_sensitivity(s)
        self.cfg["sensitivity"] = s
        self._save()

    def _on_topmost(self) -> None:
        on = bool(self.var_top.get())
        self.root.attributes("-topmost", on)
        self.cfg["always_on_top"] = on
        self._save()

    # ================= 热键 =================
    def _register_hotkeys(self) -> None:
        self._collect_config()
        self._save()
        toggle = self.cfg["toggle_key"]
        stop = self.cfg["stop_key"]
        bindings = [
            (toggle, lambda: self.events.put({"type": "hotkey", "action": "toggle"})),
        ]
        if stop and stop != toggle:
            bindings.append((stop, lambda: self.events.put({"type": "hotkey", "action": "stop"})))
        failed = self.hotkeys.start(bindings)
        if failed:
            self._log(f"热键注册失败(可能被占用)：{', '.join(failed)}", "warn")
        else:
            self._log(f"热键：{toggle}=开始/停止，{stop}=急停", "info")

    # ================= 区域选择 / 自动定位 =================
    def _select_region(self) -> None:
        if self.controller.running:
            messagebox.showinfo("提示", "请先停止再修改区域。")
            return
        if self._busy:
            return
        # 先倒计时让你切回 Minecraft（HUD/经验条可见），再截一张全屏静态图去框选。
        # 否则游戏一失焦就弹暂停菜单、经验条消失，根本框不到。
        self._busy = True
        self.btn_auto.state(["disabled"])
        self.root.attributes("-topmost", False)
        self._region_countdown(3)

    def _region_countdown(self, n: int) -> None:
        if n > 0:
            self.var_status.set(f"选择区域：{n} 秒后截图，请切到 Minecraft…")
            self.root.after(1000, lambda: self._region_countdown(n - 1))
        else:
            self._grab_then_select()

    def _grab_then_select(self) -> None:
        ppm_path = None
        screen = None
        try:
            screen = self._gui_cap.grab_screen()
            ppm_path = screenshot_to_ppm(screen)
        except Exception as e:
            self._log(f"截屏失败，改用实时框选（游戏可能已暂停）：{e!r}", "warn")
            ppm_path = None
        # 此刻 Minecraft 仍在前台且第一人称锁着鼠标，覆盖层会拿不到指针（要手动按 Esc 才动）。
        # 我们已拿到冻结截图，主动替用户按一次 Esc 让游戏松开光标，覆盖层就能立刻拖框。
        self._release_game_cursor()
        self.root.withdraw()

        def done(region):
            self.root.deiconify()
            self._busy = False
            self._sync_region_buttons()
            self._on_topmost()
            self.var_status.set("待机" if not self.controller.running else "运行中")
            if ppm_path:
                try:
                    os.remove(ppm_path)
                except OSError:
                    pass
            if region:
                self._apply_region(region, screen, "区域已设置")

        self.root.after(120, lambda: RegionSelector(self.root, done, bg_image_path=ppm_path))

    def _auto_locate(self) -> None:
        if self.var_target.get() != "xp":
            messagebox.showinfo("提示", "自动定位仅用于经验条。")
            return
        self._countdown_grab(self._do_auto_locate, "自动定位经验条")

    def _do_auto_locate(self) -> None:
        try:
            screen = self._gui_cap.grab_screen()
        except Exception as e:
            self._log(f"截屏失败：{e!r}", "warn")
            return
        self._release_game_cursor()
        region = det.auto_locate_xp_bar(screen)
        if region:
            self._apply_region(region, screen, "已自动定位经验条")
        else:
            self._log("没找到经验条绿色。请确保经验条有一点经验且未被遮挡，或手动框选。", "warn")
            messagebox.showwarning(
                "未找到", "没找到经验条的绿色。\n请确保：\n· Minecraft 在前台且经验条可见\n"
                "· 经验条里有一点经验(非空)\n否则请用「选择区域」手动框选。")

    def _apply_region(self, region: dict, frame_full, source: str) -> None:
        """保存区域，并直接用「刚才那张截图」裁出对应画面显示出来（不再二次截图）。"""
        self.cfg["region"] = region
        self._save()
        self._refresh_region_label()
        crop = None
        if frame_full is not None:
            try:
                t, l = int(region["top"]), int(region["left"])
                crop = frame_full[t:t + int(region["height"]), l:l + int(region["width"])]
            except Exception:
                crop = None
        self._show_region_preview(crop)
        self._log(f"{source}：区域 {region['width']}×{region['height']}", "info")
        if crop is not None and getattr(crop, "size", 0) and self.var_target.get() == "xp":
            try:
                g = int(det.green_mask(crop).sum())
                pct = g / (crop.shape[0] * crop.shape[1]) * 100
                hint = "  ← 偏低，可能没框到绿条" if pct < 1 else ""
                self._log(f"绿色像素 {g}（{pct:.1f}%）{hint}", "info")
            except Exception:
                pass

    def _show_live_frame(self, crop, trig: bool) -> None:
        """运行时实时显示"正在判定"的那块画面：小区域放大看清，触发时外框变红。"""
        if crop is None or getattr(crop, "size", 0) == 0:
            return
        ppm = None
        try:
            ppm = screenshot_to_ppm(crop)
            img = tk.PhotoImage(file=ppm)
        except Exception:
            return
        finally:
            if ppm:
                try:
                    os.remove(ppm)
                except OSError:
                    pass
        w = img.width()
        if w < 120:                       # 小区域(鱼钩/浮漂)放大到看得清
            img = img.zoom(max(1, 120 // max(1, w)))
        elif w > 340:                     # 大区域缩进窗口
            img = img.subsample((w // 340) + 1)
        self._preview_photo = img  # 保引用，否则被 GC 图就没了
        self.lbl_preview.config(image=img, text="")
        self.preview_box.config(bg="#e23b3b" if trig else "#3ba55d")

    def _show_region_preview(self, crop) -> None:
        if crop is None or getattr(crop, "size", 0) == 0:
            return
        ppm = None
        try:
            ppm = screenshot_to_ppm(crop)
            img = tk.PhotoImage(file=ppm)
        except Exception:
            return
        finally:
            if ppm:
                try:
                    os.remove(ppm)
                except OSError:
                    pass
        # tk 只能整数倍缩小；把宽度压到 ~340 以内适应窗口。
        maxw = 340
        if img.width() > maxw:
            factor = (img.width() // maxw) + 1
            img = img.subsample(factor, factor)
        self._preview_photo = img  # 保引用，否则被 GC 图就没了
        self.lbl_preview.config(image=img, text="")
        self.preview_box.config(bg="#ccc")

    def _release_game_cursor(self) -> None:
        """若目标游戏正在前台锁着鼠标，替用户按一次 Esc 让它松开光标。"""
        try:
            tgt = str(self.cfg.get("target_window", "Minecraft")).lower()
            if tgt and tgt in winio.get_foreground_title().lower():
                winio.tap_key(winio.VK_ESCAPE)
        except Exception:
            pass

    def _countdown_grab(self, action, label: str) -> None:
        """给用户几秒切回 Minecraft，再执行 action（截图类操作）。"""
        if self._busy:
            return
        self._busy = True
        self.btn_auto.state(["disabled"])
        self.root.attributes("-topmost", False)

        def step(n: int) -> None:
            if n > 0:
                self.var_status.set(f"{label}：{n} 秒后截图，请切到 Minecraft…")
                self.root.after(1000, lambda: step(n - 1))
            else:
                try:
                    action()
                finally:
                    self._busy = False
                    self._sync_region_buttons()
                    self._on_topmost()
                    self.var_status.set("待机" if not self.controller.running else "运行中")
        step(3)

    def _refresh_region_label(self) -> None:
        r = self.cfg.get("region")
        if r:
            self.var_region.set(f"区域：{r['left']},{r['top']}  {r['width']}×{r['height']}")
        else:
            self.var_region.set("区域：未设置")

    # ================= 事件泵 =================
    def _pump_events(self) -> None:
        latest_metric = None
        latest_frame = None
        try:
            while True:
                ev = self.events.get_nowait()
                t = ev.get("type")
                if t == "metric":
                    latest_metric = ev
                elif t == "frame":
                    latest_frame = ev
                elif t == "log":
                    self._log(ev.get("msg", ""), ev.get("level", "info"))
                elif t == "state":
                    self._set_state(ev)
                elif t == "stats":
                    self._refresh_stats()
                elif t == "hotkey":
                    self._handle_hotkey(ev.get("action"))
            # while 循环靠 Empty 退出
        except queue.Empty:
            pass
        if latest_metric is not None:
            self._update_meter(latest_metric)
        if latest_frame is not None:
            self._show_live_frame(latest_frame.get("img"), latest_frame.get("trig", False))
        self.root.after(50, self._pump_events)

    def _handle_hotkey(self, action: str) -> None:
        if action == "toggle":
            self.controller.toggle()
        elif action == "stop":
            self.controller.stop()

    def _set_state(self, ev: dict) -> None:
        running = ev.get("running", False)
        phase = ev.get("phase", "待机")
        self.var_status.set(phase)
        color = {"运行中": "#22aa22", "监视中": "#22aa22", "等咬钩": "#22aa22",
                 "等待聚焦": "#e08a00", "待机": "#888"}.get(phase, "#22aa22" if running else "#888")
        self.dot.itemconfig(self._dot_id, fill=color)
        if running:
            self.btn_start.state(["disabled"])
        else:
            self.btn_start.state(["!disabled"])
            self._update_meter(None)
            self.preview_box.config(bg="#ccc")  # 停止后外框恢复中性色

    def _update_meter(self, ev) -> None:
        self._last_metric = ev
        self._draw_meter()

    def _draw_meter(self) -> None:
        self.meter.delete("bar")
        ev = getattr(self, "_last_metric", None)
        if not ev:
            self.var_metric.set("变化 —  /  阈值 —")
            return
        w = self.meter.winfo_width()
        if w <= 1:
            w = 300
        val = ev.get("value", 0.0)
        thr = max(1e-6, ev.get("thr", 1.0))
        ratio = min(1.0, val / thr)
        fill_w = max(0, int(w * ratio))
        color = "#e23b3b" if ev.get("trig") else "#3b82e2"
        if fill_w > 0:
            self.meter.create_rectangle(0, 0, fill_w, 20, fill=color, outline="", tags="bar")
        if self.var_target.get() == "hookstate":
            # 状态匹配：差值≤阈值 = 判定"钩在"。
            self.var_metric.set(f"差值 {val:.1f}  /  阈值 {thr:.1f}"
                                + ("   ★钩在" if ev.get("trig") else "   （钩不在）"))
        elif ev.get("warmup"):
            self.var_metric.set(f"变化 {val:.1f}  /  阈值 {thr:.1f}   （预热中·暂不判定）")
        else:
            self.var_metric.set(f"变化 {val:.1f}  /  阈值 {thr:.1f}"
                                + ("   ★触发" if ev.get("trig") else ""))

    # ================= 数据 =================
    def _tick_stats(self) -> None:
        if self.controller.running:
            self._refresh_stats()
        self.root.after(1000, self._tick_stats)

    def _refresh_stats(self) -> None:
        s = self.stats.session_snapshot()
        c = self.stats.career_snapshot()
        self.var_sess.set(
            f"钓鱼：{s['fish']} 条\n"
            f"时长：{fmt_hms(s['seconds'])}\n"
            f"时速：{s['rate_per_hour']:.0f} 条/时")
        self.var_career.set(
            f"总计：{c['fish']} 条\n"
            f"时长：{fmt_hms(c['seconds'])}\n"
            f"场次：{c['sessions']}（自 {c.get('since', '—')}）")

    def _show_history(self) -> None:
        win = tk.Toplevel(self.root)
        win.title("历史记录")
        win.geometry("420x360")
        win.attributes("-topmost", bool(self.var_top.get()))
        cols = ("start", "dur", "fish", "mode")
        tv = ttk.Treeview(win, columns=cols, show="headings")
        for c, txt, w in (("start", "开始时间", 130), ("dur", "时长", 80),
                          ("fish", "条数", 60), ("mode", "模式", 80)):
            tv.heading(c, text=txt)
            tv.column(c, width=w, anchor="center")
        tv.pack(fill="both", expand=True, padx=6, pady=6)
        for rec in self.stats.recent_history(200):
            mode = "全自动" if rec.get("mode") == "full" else "半自动"
            tv.insert("", "end", values=(
                rec.get("start", ""), fmt_hms(rec.get("seconds", 0)),
                rec.get("fish", 0), mode))
        if not self.stats.recent_history(1):
            ttk.Label(win, text="暂无记录").pack()

    def _reset_career(self) -> None:
        if messagebox.askyesno("确认", "清空生涯累计和全部历史记录？此操作不可撤销。"):
            self.stats.reset_career()
            self._refresh_stats()
            self._log("已清空生涯数据。", "info")

    # ================= 日志 =================
    def _log(self, msg: str, level: str = "info") -> None:
        import time as _t
        ts = _t.strftime("%H:%M:%S")
        self.log.config(state="normal")
        tag = level if level in ("warn", "catch") else ""
        self.log.insert("end", f"[{ts}] {msg}\n", tag)
        # 限制行数
        if int(self.log.index("end-1c").split(".")[0]) > 300:
            self.log.delete("1.0", "100.0")
        self.log.see("end")
        self.log.config(state="disabled")

    # ================= 关闭 =================
    def _on_close(self) -> None:
        try:
            self.controller.stop()
            self.controller.wait(timeout=1.5)  # 让工作线程写完本次会话记录
            self.hotkeys.stop()
            self._collect_config()
            self._save()
            self._gui_cap.close()
        finally:
            self.root.destroy()
