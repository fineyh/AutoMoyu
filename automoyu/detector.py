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
    hand_frac_x: float = 0.66,
    hand_frac_y: float = 0.5,
    red_weight: float = 2.0,
    anchor_x: float = 0.5,
    anchor_y: float = 0.2,
    debug: Optional[dict] = None,
) -> Optional[dict]:
    """比较甩竿前/后两帧，在"新出现且最集中"的地方框出浮漂，自动给出判定小框。

    - before：甩竿前的画面（水里还没有浮漂）。
    - after ：甩竿落水稳定后的画面（浮漂已经在水面上）。
    两帧相减，新出现的浮漂会亮起来；水面动画是高频、分散的噪声。再用"红色度"给浮漂
    的红白顶端加权。对这张打分图用与浮漂框同大小的滑动窗口求和，取和最大的窗口——
    也就是"新东西最扎堆"的一小块，正是浮漂。

    若最强窗口并不比整体平均明显（比值 < min_ratio），说明没有明显的新目标（可能没
    甩出去 / 浮漂被挡），返回 None，让上层重甩。

    第一人称手持鱼竿永远钉在画面右下角。甩竿那一下鱼竿会大幅甩动，两帧相减在右下角
    产生巨大的运动差(diff)，竿身的木色/附魔色段还会贡献"新出现的红"——两路信号都被
    它霸占，导致框选死死锁在右下角的竿上而不是浮漂（正是本函数曾经的主要误定位）。
    因此在打分前把右下角这块手持竿区域(x≥hand_frac_x·W 且 y≥hand_frac_y·H)清零、排除
    在搜索之外；均值也只按剩余可搜索区域算，避免抠掉一大块后 ratio 被虚高。

    origin: after 帧左上角在屏幕上的绝对坐标 (left, top)，用于把结果换算成屏幕绝对框。
    hand_frac_x/hand_frac_y: 右下角手持竿排除区的起点（占宽/高的比例）。设 >=1 可关闭。
            上层已把搜索区收成中央窄带、天然排除了手持竿，故默认传 1.0 关闭本排除。
    red_weight: "新出现的红顶"在打分里的权重。浮漂红白顶是最可靠的特征，加大它能让
            有红时的定位更笃定(与噪声拉开差距)；红被水色冲淡/看不见时该项≈0、不影响，
            此时仍靠画面差分在窄带里定位。不做"必须有红"的硬门槛——实测浮漂偏小或逆光
            时红顶极弱(几乎为 0)，硬要求红会把这些本可定位的竿误判成没浮漂而空甩。
    anchor_x/anchor_y: 浮漂在判定小框里的落点(占框宽/高的比例)。滑窗命中的位置≈浮漂
            所在，默认把它摆在框的「中上方」(x=0.5 水平居中, y=0.2 靠上)，这样框内浮漂
            下方留出足够空间去捕捉咬钩时浮漂下沉/溅水的向下位移。(0,0)=左上角。
    debug:  传入一个 dict 时，会被就地填上打分图/选中窗口/比值等信息，供上层把
            「定位依据」渲染成图保存下来排查（见 render_bobber_debug）。即使这一竿
            没定位到(返回 None) 也会填，方便看清它到底盯上了哪块。
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
    # 只认"新出现的红"：浮漂红顶落在水面上，是 after 比 before 多出来的红色度。
    # 若用 after 的绝对红色度(rb - max(gb,bb))，静止的暖色地形(沙/红沙/泥/下界岩)、
    # 夕阳天空、手臂等会到处恒定加分，把窗口拉到浮漂以外的红色区域——这正是
    # "定位到的不是浮漂"的主因。改成 (after 红 - before 红)：静止暖色两帧相同 -> 0。
    red_a = np.maximum(0, ra - np.maximum(ga, ba))
    red_b = np.maximum(0, rb - np.maximum(gb, bb))
    red_new = np.maximum(0, red_b - red_a).astype(np.float64)  # 新出现的红顶
    score = diff + red_weight * red_new

    # 抠掉右下角手持鱼竿区：甩竿的运动差+竿身红色否则会把窗口牢牢吸到这儿。
    hx = int(W * hand_frac_x) if 0.0 < hand_frac_x < 1.0 else W
    hy = int(H * hand_frac_y) if 0.0 < hand_frac_y < 1.0 else H
    if hx < W and hy < H:
        score[hy:, hx:] = 0.0

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
    # 均值只按未抠掉的可搜索像素算：否则抠掉一大片 0 会把整体均值压低、ratio 虚高，
    # 让"其实没浮漂"的水面噪声也轻松越过 min_ratio 造成误定位。
    searchable = int(np.count_nonzero(s > 0.0))
    mean = (float(s.sum()) / searchable) if searchable > 0 else 0.0
    ratio = (best_avg / mean) if mean > 1e-6 else 0.0

    # 滑窗命中的窗口左上角≈浮漂位置。按 anchor 把浮漂摆到框内指定落点(默认中上方)：
    # 令 hit 点落在框的 (anchor_x, anchor_y) 处，即把框整体平移。
    hit_x = wx * ds
    hit_y = wy * ds
    left_rel = int(min(max(0, hit_x - box * anchor_x), max(0, W - box)))
    top_rel = int(min(max(0, hit_y - box * anchor_y), max(0, H - box)))
    if debug is not None:
        debug.update({"score": s, "ds": int(ds), "box": int(box),
                      "left_rel": left_rel, "top_rel": top_rel,
                      "ratio": float(ratio), "best_avg": float(best_avg),
                      "mean": float(mean), "min_ratio": float(min_ratio)})

    if mean <= 1e-6 or best_avg < mean * min_ratio:
        return None
    ox, oy = origin
    return {"left": int(ox + left_rel), "top": int(oy + top_rel),
            "width": int(box), "height": int(box)}


def render_bobber_debug(after_bgra: np.ndarray, debug: dict) -> np.ndarray:
    """把 auto_locate_bobber 的打分依据画到 after 帧上，返回一张可保存的 BGRA 图。

    - 红色越亮 = 该处"打分(变化+新红顶)"越高，也就是定位器认为越像浮漂的地方。
    - 亮绿色方框 = 定位器最终选中(或即便被 min_ratio 否决也最想选)的判定小框。
    这样一眼就能看出它到底盯上了哪块：是浮漂，还是鱼竿/水花/岸边/暖色地形。
    """
    base = np.ascontiguousarray(after_bgra[:, :, :3], dtype=np.float64)
    H, W = base.shape[:2]
    out = base.copy()

    s = debug.get("score")
    if s is not None and getattr(s, "size", 0):
        ds = max(1, int(debug.get("ds", 1)))
        heat = np.repeat(np.repeat(s, ds, axis=0), ds, axis=1)
        # 对齐到 after 尺寸（下采样后可能略小/略大）。
        hm = np.zeros((H, W), dtype=np.float64)
        hh, hw = min(H, heat.shape[0]), min(W, heat.shape[1])
        hm[:hh, :hw] = heat[:hh, :hw]
        m = float(hm.max())
        if m > 1e-6:
            hm /= m
        a = (0.65 * hm)[..., None]           # 越热越红
        red_bgr = np.array([40.0, 40.0, 255.0])  # BGR
        out = out * (1.0 - a) + red_bgr * a

    out = np.clip(out, 0, 255).astype(np.uint8)

    # 画选中框（亮绿，2px 边）。
    box = int(debug.get("box", 0))
    lr = int(debug.get("left_rel", 0))
    tr = int(debug.get("top_rel", 0))
    if box > 0:
        x0, y0 = max(0, lr), max(0, tr)
        x1, y1 = min(W, lr + box), min(H, tr + box)
        green = np.array([0, 255, 0], dtype=np.uint8)  # BGR
        t = 2
        if x1 > x0 and y1 > y0:
            out[y0:min(H, y0 + t), x0:x1] = green
            out[max(0, y1 - t):y1, x0:x1] = green
            out[y0:y1, x0:min(W, x0 + t)] = green
            out[y0:y1, max(0, x1 - t):x1] = green
    return out
