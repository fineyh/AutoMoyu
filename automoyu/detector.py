"""检测器：不依赖任何保存的参考图，每次甩竿后当场取基准，再看"变化"。

- XpBarDetector：数经验条里"绿色像素"的数量，钓到鱼 -> 经验增加 -> 绿色数量变化。
- GenericDetector：把区域缩小成灰度，算与基准的平均绝对差（MAD）。适合鱼钩/浮漂等。

灵敏度 1..10：数字越大越灵敏（更小的变化就触发）。
measure() 返回 (metric, threshold, triggered)，GUI 用它做实时调参。
"""
from __future__ import annotations

from typing import Optional

import numpy as np


def _to_rgb(frame_bgra: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    b = frame_bgra[..., 0].astype(np.int16)
    g = frame_bgra[..., 1].astype(np.int16)
    r = frame_bgra[..., 2].astype(np.int16)
    return r, g, b


def green_mask(frame_bgra: np.ndarray) -> np.ndarray:
    """Minecraft 经验条是亮黄绿色（lime）。挑出"绿色明显占优"的像素。"""
    r, g, b = _to_rgb(frame_bgra)
    return (g > 90) & (g > r + 25) & (g > b + 25)


def _downscale_gray(frame_bgra: np.ndarray, target: int = 64) -> np.ndarray:
    """BGR->灰度并缩小到最长边 ~target 像素，省算力也更稳。"""
    r, g, b = _to_rgb(frame_bgra)
    gray = (0.299 * r + 0.587 * g + 0.114 * b)
    h, w = gray.shape
    step = max(1, int(round(max(h, w) / target)))
    return gray[::step, ::step]


def frame_mad(a_bgra: np.ndarray, b_bgra: np.ndarray, target: int = 64) -> float:
    """两帧画面的平均绝对差（缩小成灰度后比较）。用来判断"画面在不在动"。"""
    ga = _downscale_gray(a_bgra, target)
    gb = _downscale_gray(b_bgra, target)
    if ga.shape != gb.shape:
        return 999.0
    return float(np.abs(ga - gb).mean())


class BaseDetector:
    name = "base"

    def __init__(self, sensitivity: int = 5) -> None:
        self.sensitivity = int(sensitivity)
        self._baseline = None

    def set_sensitivity(self, s: int) -> None:
        self.sensitivity = int(max(1, min(10, s)))

    def set_baseline(self, frame_bgra: np.ndarray) -> None:
        raise NotImplementedError

    def measure(self, frame_bgra: np.ndarray) -> tuple[float, float, bool]:
        raise NotImplementedError

    @property
    def has_baseline(self) -> bool:
        return self._baseline is not None


class XpBarDetector(BaseDetector):
    name = "xp"

    def set_baseline(self, frame_bgra: np.ndarray) -> None:
        self._baseline = int(green_mask(frame_bgra).sum())
        self._area = frame_bgra.shape[0] * frame_bgra.shape[1]

    def _threshold(self) -> float:
        # 灵敏度 1..10 -> 触发所需变化占面积比例 0.6% .. 0.04%
        # 钓上一条鱼经验条只涨一小截，阈值太高会导致 meter 跑不满、检测不到、迟迟不重甩。
        frac = np.interp(self.sensitivity, [1, 10], [0.006, 0.0004])
        return max(2.0, frac * self._area)

    def measure(self, frame_bgra: np.ndarray) -> tuple[float, float, bool]:
        if self._baseline is None:
            self._area = frame_bgra.shape[0] * frame_bgra.shape[1]
            return 0.0, self._threshold(), False
        cur = int(green_mask(frame_bgra).sum())
        delta = float(abs(cur - self._baseline))
        thr = self._threshold()
        return delta, thr, delta >= thr


class GenericDetector(BaseDetector):
    """区域与基准的平均绝对差（MAD）。适合鱼钩/浮漂。

    自适应基准(adaptive)：每一帧没触发时，让基准以 adapt_rate 缓慢向当前帧靠拢。
    这样水面持续的轻微晃动会被基准"吸收"、不会累积成误判；只有浮漂被咬突然下沉
    这种"瞬间大变化"才会一下子拉开与基准的差、越过阈值触发。触发的那几帧不更新
    基准，避免把下沉动作本身也吃掉。关掉 adaptive 时退回原来的"固定基准"行为。
    """

    name = "generic"

    _TARGET = 64  # 把区域缩小到最长边 ~64 像素再比较，省算力也更稳

    def __init__(self, sensitivity: int = 5, adaptive: bool = True,
                 adapt_rate: float = 0.12) -> None:
        super().__init__(sensitivity)
        self.adaptive = bool(adaptive)
        self.adapt_rate = float(max(0.0, min(1.0, adapt_rate)))

    def _prep(self, frame_bgra: np.ndarray) -> np.ndarray:
        return _downscale_gray(frame_bgra, self._TARGET)

    def set_baseline(self, frame_bgra: np.ndarray) -> None:
        self._baseline = self._prep(frame_bgra)

    def _threshold(self) -> float:
        # 灵敏度 1..10 -> MAD 阈值 22 .. 3（灰度 0..255）
        return float(np.interp(self.sensitivity, [1, 10], [22.0, 3.0]))

    def measure(self, frame_bgra: np.ndarray) -> tuple[float, float, bool]:
        cur = self._prep(frame_bgra)
        thr = self._threshold()
        if self._baseline is None or self._baseline.shape != cur.shape:
            self._baseline = cur
            return 0.0, thr, False
        mad = float(np.abs(cur - self._baseline).mean())
        trig = mad >= thr
        if self.adaptive and not trig and self.adapt_rate > 0.0:
            a = self.adapt_rate
            self._baseline = (1.0 - a) * self._baseline + a * cur
        return mad, thr, trig


class HookStateDetector(BaseDetector):
    """状态匹配：存一张"有钩"参考照，判断当前画面是否≈参考（钩在手上、未甩出）。

    与 GenericDetector 相反：GenericDetector 是"离开基准就触发"，这里
    triggered=True 表示"当前画面和参考照足够像"，即"钩在"。用于手持鱼竿的钩：
      钩在  -> 线收回来了/没在钓
      钩不在 -> 线甩出去了/正在钓
    """

    name = "hookstate"
    _TARGET = 64

    def _prep(self, frame_bgra: np.ndarray) -> np.ndarray:
        return _downscale_gray(frame_bgra, self._TARGET)

    def set_baseline(self, frame_bgra: np.ndarray) -> None:
        self._baseline = self._prep(frame_bgra)

    def _threshold(self) -> float:
        # 灵敏度 1..10 -> 匹配容忍度 MAD 4..16（灰度 0..255）。
        # 越大越"宽松"，越容易判定"钩在"（更快认定钓上/收线回来）。
        return float(np.interp(self.sensitivity, [1, 10], [4.0, 16.0]))

    def measure(self, frame_bgra: np.ndarray) -> tuple[float, float, bool]:
        cur = self._prep(frame_bgra)
        thr = self._threshold()
        if self._baseline is None or self._baseline.shape != cur.shape:
            return 999.0, thr, False
        mad = float(np.abs(cur - self._baseline).mean())
        return mad, thr, mad <= thr  # 注意：<= 触发（越像越算"钩在"）


def make_detector(cfg: dict) -> BaseDetector:
    target = cfg.get("target", "xp")
    sensitivity = int(cfg.get("sensitivity", 5))
    # 自动定位浮漂：判定的是"浮漂小框"里的突变(下沉)，用通用差分检测器。
    if cfg.get("auto_bobber"):
        return GenericDetector(
            sensitivity,
            adaptive=bool(cfg.get("hook_adaptive", True)),
            adapt_rate=float(cfg.get("hook_adapt_rate", 0.12)),
        )
    if target == "xp":
        return XpBarDetector(sensitivity)
    if target == "hookstate":
        return HookStateDetector(sensitivity)
    return GenericDetector(
        sensitivity,
        adaptive=bool(cfg.get("hook_adaptive", True)),
        adapt_rate=float(cfg.get("hook_adapt_rate", 0.12)),
    )


def auto_locate_xp_bar(screen_bgra: np.ndarray) -> Optional[dict]:
    """在整屏下部自动寻找经验条（一条水平的亮绿色线）。

    需要当前经验条里有一点绿色（非满级 0 经验的空条）。找不到返回 None。
    返回 {left, top, width, height}（含少量留白）。
    """
    H, W = screen_bgra.shape[:2]
    y0 = int(H * 0.70)  # 只看下部 30%
    strip = screen_bgra[y0:H]
    mask = green_mask(strip)

    row_counts = mask.sum(axis=1)
    if row_counts.max() < W * 0.04:
        return None  # 没有足够长的绿线，判定失败

    best_row = int(np.argmax(row_counts))
    thresh = max(3, int(row_counts[best_row] * 0.3))
    # 向上下扩展，把整条经验条的高度都包住
    top = best_row
    while top > 0 and row_counts[top - 1] >= thresh:
        top -= 1
    bot = best_row
    while bot < strip.shape[0] - 1 and row_counts[bot + 1] >= thresh:
        bot += 1

    band = mask[top:bot + 1]
    col_any = band.any(axis=0)
    xs = np.where(col_any)[0]
    if xs.size == 0:
        return None
    x_left, x_right = int(xs.min()), int(xs.max())

    pad_x = max(2, int((x_right - x_left) * 0.03))
    pad_y = 3
    left = max(0, x_left - pad_x)
    right = min(W - 1, x_right + pad_x)
    abs_top = max(0, y0 + top - pad_y)
    abs_bot = min(H - 1, y0 + bot + pad_y)

    width = right - left + 1
    height = abs_bot - abs_top + 1
    if width < 20 or height < 3:
        return None
    return {"left": left, "top": abs_top, "width": width, "height": height}


def auto_locate_bobber(
    before_bgra: np.ndarray,
    after_bgra: np.ndarray,
    origin: tuple[int, int] = (0, 0),
    box: int = 64,
    min_ratio: float = 1.8,
) -> Optional[dict]:
    """比较甩竿前/后两帧，在"新出现且最集中"的地方框出浮漂，自动给出判定小框。

    - before：甩竿前的画面（水里还没有浮漂）。
    - after ：甩竿落水稳定后的画面（浮漂已经在水面上）。
    两帧相减，新出现的浮漂会亮起来；水面动画是高频、分散的噪声。再用"红色度"给浮漂
    的红白顶端加权。对这张打分图用与浮漂框同大小的滑动窗口求和，取和最大的窗口——
    也就是"新东西最扎堆"的一小块，正是浮漂。

    若最强窗口并不比整体平均明显（比值 < min_ratio），说明没有明显的新目标（可能没
    甩出去 / 浮漂被挡），返回 None，让上层重甩。

    origin: after 帧左上角在屏幕上的绝对坐标 (left, top)，用于把结果换算成屏幕绝对框。
    返回 {left, top, width, height}（屏幕绝对坐标，正方形边长≈box）或 None。
    """
    if (before_bgra.ndim != 3 or after_bgra.ndim != 3
            or before_bgra.shape != after_bgra.shape):
        return None
    H, W = after_bgra.shape[:2]
    box = int(max(8, min(box, H, W)))

    ra, ga, ba = _to_rgb(before_bgra)
    rb, gb, bb = _to_rgb(after_bgra)
    gray_a = 0.299 * ra + 0.587 * ga + 0.114 * ba
    gray_b = 0.299 * rb + 0.587 * gb + 0.114 * bb
    diff = np.abs(gray_b - gray_a)
    red = np.maximum(0, rb - np.maximum(gb, bb)).astype(np.float64)  # 浮漂红顶
    score = diff + 0.5 * red

    # 下采样加速；滑窗用积分图 O(N) 求所有窗口和。
    ds = max(1, box // 24)
    s = np.ascontiguousarray(score[::ds, ::ds], dtype=np.float64)
    bw = max(2, box // ds)
    sh, sw = s.shape
    if sh < bw or sw < bw:
        return None
    ii = np.zeros((sh + 1, sw + 1), dtype=np.float64)
    ii[1:, 1:] = np.cumsum(np.cumsum(s, axis=0), axis=1)
    win = ii[bw:, bw:] - ii[:-bw, bw:] - ii[bw:, :-bw] + ii[:-bw, :-bw]
    flat = int(np.argmax(win))
    wy, wx = divmod(flat, win.shape[1])
    best_avg = float(win[wy, wx]) / float(bw * bw)
    mean = float(s.mean())
    if mean <= 1e-6 or best_avg < mean * min_ratio:
        return None

    left_rel = int(min(wx * ds, max(0, W - box)))
    top_rel = int(min(wy * ds, max(0, H - box)))
    ox, oy = origin
    return {"left": int(ox + left_rel), "top": int(oy + top_rel),
            "width": int(box), "height": int(box)}
