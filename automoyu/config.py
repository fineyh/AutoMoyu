"""配置的读写（JSON）。"""
from __future__ import annotations

import json

from . import paths

DEFAULTS = {
    "mode": "semi",            # "semi" 半自动(只甩竿) | "full" 全自动(甩竿+收竿)
    "target": "xp",            # "xp" 经验条 | "hook" 鱼钩/浮漂(通用差分) | "hookstate" 手持竿钩(状态匹配)
    "region": None,            # {left, top, width, height}
    "sensitivity": 5,          # 1..10
    "duration_min": 0,         # 本次时长(分钟)，0=不限
    "toggle_key": "F6",        # 开始/停止
    "stop_key": "F8",          # 紧急停止
    "always_on_top": True,
    "focus_guard": True,       # 只在目标窗口聚焦时点击
    "target_window": "Minecraft",
    # 时序（毫秒）
    "settle_ms": 1500,         # 甩竿后至少等这么久（跳过甩竿/入水动画）再开始判稳
    "settle_stabilize": True,  # 甩竿后自适应等画面稳定(浮漂落水静止)再取基准，避免刚甩出就误判
    "settle_max_ms": 5000,     # 等稳定的最长时间，超时用最后一帧兜底
    "settle_quiet_mad": 3.0,   # 相邻帧差 ≤ 此值算"画面静止"(灰度 0..255)；水面偏动可调大
    "recast_delay_ms": 900,    # 检测到钓上后，重新甩竿前的等待
    "max_wait_s": 45,          # 长时间没检测到 -> 保险重甩
    "cast_confirm_s": 3,       # 手持竿钩：甩竿后等钩离开手上的最长确认时间，超时判定没甩出去
    "poll_hz": 15,             # 检测频率
    "click_hold_ms": 90,       # 右键按住时长(ms)；<50 可能落在一个游戏刻内被丢弃 -> 甩不出去
    # 全自动专用
    "bite_reel_delay_ms": 60,  # 检测到咬钩 -> 收竿的延迟
    "post_reel_delay_ms": 1200,  # 收竿后再甩竿前的等待
    "confirm_frames": 2,       # 连续多少帧超阈值才确认(去抖)
}


def load() -> dict:
    paths.ensure_data_dir()
    cfg = dict(DEFAULTS)
    try:
        with open(paths.CONFIG_PATH, "r", encoding="utf-8") as f:
            saved = json.load(f)
        if isinstance(saved, dict):
            cfg.update({k: saved[k] for k in saved if k in DEFAULTS})
    except FileNotFoundError:
        pass
    except Exception:
        pass
    return cfg


def save(cfg: dict) -> None:
    paths.ensure_data_dir()
    data = {k: cfg.get(k, DEFAULTS[k]) for k in DEFAULTS}
    tmp = paths.CONFIG_PATH + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
    import os
    os.replace(tmp, paths.CONFIG_PATH)
