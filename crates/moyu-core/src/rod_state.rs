//! 竿状态分类：两模板最近邻。
//!
//! 校准时存下"收回"和"甩出"两张模板，每帧看当前画面离哪张更近；
//! 两张都不像就是"未知"（开了菜单、切了物品、鱼竿坏了）。阈值由校准数据算出，没有灵敏度滑块。

use serde::{Deserialize, Serialize};

use crate::image::{mad, Grid};

/// 最近的模板必须比另一个近这么多（d_near < d_far × RATIO）才采信。
pub const DECISION_RATIO: f32 = 0.6;
/// 运行中模板自适应速率（仅高置信帧）。
pub const ADAPT_ALPHA: f32 = 0.05;
/// 两张模板在某像素上平均每通道差不到 亮度×FRAC + FLOOR，就算两个状态共有的背景。
const SHARED_FRAC: f32 = 0.15;
const SHARED_FLOOR: f32 = 8.0;
/// 前景增益范围两端各再放宽这么多。
const GAIN_SLACK: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RodState {
    /// 线收回来了（手里是空竿）。
    In,
    /// 线甩出去了。
    Out,
    /// 认不出。
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    pub state: RodState,
    pub d_in: f32,
    pub d_out: f32,
}

/// 校准得到的竿状态模型。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RodModel {
    pub w: usize,
    pub h: usize,
    /// 是否先做亮度归一化（校准时两种特征都评估，取区分度高的）。
    pub normalize: bool,
    pub tpl_in: Vec<[f32; 3]>,
    pub tpl_out: Vec<[f32; 3]>,
    pub tau_in: f32,
    pub tau_out: f32,
    /// 校准时两模板间的距离，仅用于诊断/显示。
    pub between: f32,
    /// 两个状态共有的背景像素（校准时定下，之后不变：竿在画面里的位置是固定的）。
    /// 旧版校准文件没有这一项，第一次用时由模板算出。
    #[serde(default)]
    shared: Vec<bool>,
    /// 这张模板的前景（只有这个状态才有的像素，如手里的竿）上次更新以来，背景的明暗变了多少倍。
    #[serde(skip, default = "unity")]
    gain_in: f32,
    #[serde(skip, default = "unity")]
    gain_out: f32,
}

fn unity() -> f32 {
    1.0
}

fn luma(c: &[f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// 两张模板几乎一样的像素：两个状态共有的背景。
fn shared_mask(a: &[[f32; 3]], b: &[[f32; 3]]) -> Vec<bool> {
    a.iter()
        .zip(b)
        .map(|(p, q)| {
            let d = (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f32>() / 3.0;
            d < SHARED_FRAC * luma(p).max(luma(q)) + SHARED_FLOOR
        })
        .collect()
}

fn luma_where(px: &[[f32; 3]], mask: &[bool]) -> f32 {
    px.iter().zip(mask).filter(|(_, &m)| m).map(|(c, _)| luma(c)).sum()
}

/// 当前帧到模板的距离（每像素每通道平均绝对差）。
/// 背景像素直接比；前景像素允许整体乘一个增益：前景没看到的这段时间里，
/// 它的明暗可能完全没变（水下、火把旁天黑时水暗了、竿不变），也可能和背景一样变了（露天只靠天光），
/// 所以增益限制在 [1, 背景变化倍数] 之间（再留点余量）。背景本身不放宽，
/// 菜单那种整屏突然变暗仍然认不出。
fn distance(f: &[[f32; 3]], t: &[[f32; 3]], shared: &[bool], bg_gain: f32) -> f32 {
    if f.is_empty() {
        return 0.0;
    }
    let (mut num, mut den) = (0.0f32, 0.0f32);
    for ((p, q), _) in f.iter().zip(t).zip(shared).filter(|(_, &s)| !s) {
        for c in 0..3 {
            num += p[c] * q[c];
            den += q[c] * q[c];
        }
    }
    let lo = bg_gain.min(1.0) * (1.0 - GAIN_SLACK);
    let hi = bg_gain.max(1.0) * (1.0 + GAIN_SLACK);
    let g = if den > 0.0 { (num / den).clamp(lo, hi) } else { 1.0 };
    let mut sum = 0.0f32;
    for ((p, q), &s) in f.iter().zip(t).zip(shared) {
        let k = if s { 1.0 } else { g };
        sum += (p[0] - k * q[0]).abs() + (p[1] - k * q[1]).abs() + (p[2] - k * q[2]).abs();
    }
    sum / (f.len() * 3) as f32
}

impl RodModel {
    /// 由两组样本（已经是本 ROI 的原始网格）建模。
    pub fn fit(samples_in: &[Grid], samples_out: &[Grid], normalize: bool) -> Self {
        let feat = |g: &Grid| if normalize { g.normalized() } else { g.clone() };
        let fin: Vec<Grid> = samples_in.iter().map(feat).collect();
        let fout: Vec<Grid> = samples_out.iter().map(feat).collect();
        let tpl_in = crate::image::mean_of(&fin);
        let tpl_out = crate::image::mean_of(&fout);
        let between = mad(&tpl_in.px, &tpl_out.px);
        let max_j = |fs: &[Grid], t: &Grid| fs.iter().map(|f| mad(&f.px, &t.px)).fold(0.0f32, f32::max);
        // 未知判定只用来发现"完全不像"（菜单、换了物品）；收回/甩出之间靠最近邻比较决定。
        let tau = |j: f32| (3.0 * j).clamp(between, 1.5 * between);
        let shared = shared_mask(&tpl_in.px, &tpl_out.px);
        Self {
            w: tpl_in.w,
            h: tpl_in.h,
            normalize,
            tau_in: tau(max_j(&fin, &tpl_in)),
            tau_out: tau(max_j(&fout, &tpl_out)),
            tpl_in: tpl_in.px,
            tpl_out: tpl_out.px,
            between,
            shared,
            gain_in: 1.0,
            gain_out: 1.0,
        }
    }

    fn features(&self, g: &Grid) -> Grid {
        if self.normalize {
            g.normalized()
        } else {
            g.clone()
        }
    }

    pub fn classify(&self, g: &Grid) -> Classification {
        if g.w != self.w || g.h != self.h {
            return Classification { state: RodState::Unknown, d_in: f32::INFINITY, d_out: f32::INFINITY };
        }
        let f = self.features(g);
        self.classify_features(&f)
    }

    fn classify_features(&self, f: &Grid) -> Classification {
        let computed;
        let shared = if self.shared.len() == self.tpl_in.len() {
            &self.shared
        } else {
            computed = shared_mask(&self.tpl_in, &self.tpl_out);
            &computed
        };
        let d_in = distance(&f.px, &self.tpl_in, shared, self.gain_in);
        let d_out = distance(&f.px, &self.tpl_out, shared, self.gain_out);
        let state = if d_in < self.tau_in && d_in < DECISION_RATIO * d_out {
            RodState::In
        } else if d_out < self.tau_out && d_out < DECISION_RATIO * d_in {
            RodState::Out
        } else {
            RodState::Unknown
        };
        Classification { state, d_in, d_out }
    }

    /// 分类并在高置信时把当前帧以 α 融进对应模板，跟住昼夜/天气变化。
    pub fn classify_and_adapt(&mut self, g: &Grid) -> Classification {
        if g.w != self.w || g.h != self.h {
            return self.classify(g);
        }
        if self.shared.len() != self.tpl_in.len() {
            self.shared = shared_mask(&self.tpl_in, &self.tpl_out);
        }
        let f = self.features(g);
        let c = self.classify_features(&f);
        let shared = &self.shared;
        let (own, other, own_gain, other_gain, d_own, d_other) = match c.state {
            RodState::In => (&mut self.tpl_in, &mut self.tpl_out, &mut self.gain_in, &mut self.gain_out, c.d_in, c.d_out),
            RodState::Out => (&mut self.tpl_out, &mut self.tpl_in, &mut self.gain_out, &mut self.gain_in, c.d_out, c.d_in),
            RodState::Unknown => return c,
        };
        if d_own < 0.45 * d_other {
            // 看到的这张模板跟着当前帧走；共有的背景在另一张模板里也同步更新，
            // 另一张的前景（比如竿）这时看不到，不动它，只记下背景又变了多少倍，分类时据此放宽。
            let before = luma_where(other, shared);
            for ((t, o), (p, &s)) in own.iter_mut().zip(other.iter_mut()).zip(f.px.iter().zip(shared)) {
                for ch in 0..3 {
                    t[ch] += ADAPT_ALPHA * (p[ch] - t[ch]);
                    if s {
                        o[ch] += ADAPT_ALPHA * (p[ch] - o[ch]);
                    }
                }
            }
            if before > 1.0 {
                *other_gain *= luma_where(other, shared) / before;
            }
            *own_gain += ADAPT_ALPHA * (1.0 - *own_gain);
        }
        c
    }

    /// 模板转成 RGB8（诊断/界面预览）。
    pub fn template_rgb8(&self, state: RodState) -> Vec<u8> {
        let t = if state == RodState::Out { &self.tpl_out } else { &self.tpl_in };
        t.iter().flat_map(|c| c.map(|v| v.round().clamp(0.0, 255.0) as u8)).collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 合成一小块"手持鱼竿"：收回时竿头在 (x0,y0)，甩出时竿头上抬，另加噪声。
    pub fn synth(state: RodState, w: usize, h: usize, light: f32, seed: u32) -> Grid {
        synth_lit(state, w, h, light, light, seed)
    }

    /// 背景和竿（含线）分别给亮度：水下/火把旁天黑时两者变化不一样。
    pub fn synth_lit(state: RodState, w: usize, h: usize, light: f32, rod_light: f32, seed: u32) -> Grid {
        let mut g = Grid::filled(w, h, [90.0 * light, 140.0 * light, 200.0 * light]);
        let mut s = seed.wrapping_mul(2654435761).wrapping_add(1);
        for p in g.px.iter_mut() {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let n = ((s % 9) as f32 - 4.0) * light;
            *p = p.map(|v| (v + n).max(0.0));
        }
        let tip = if state == RodState::Out { h / 4 } else { h / 2 };
        for y in tip..h {
            for x in w / 2..w / 2 + 2 {
                g.set(x, y, [60.0 * rod_light, 40.0 * rod_light, 20.0 * rod_light]);
            }
        }
        if state == RodState::Out {
            // 线：一条细亮线
            for y in 0..tip {
                g.set(w / 2, y, [220.0 * rod_light, 220.0 * rod_light, 220.0 * rod_light]);
            }
        }
        g
    }

    fn model() -> RodModel {
        let si: Vec<_> = (0..10).map(|i| synth(RodState::In, 12, 12, 1.0, i)).collect();
        let so: Vec<_> = (0..10).map(|i| synth(RodState::Out, 12, 12, 1.0, 100 + i)).collect();
        RodModel::fit(&si, &so, false)
    }

    #[test]
    fn classifies_both_states() {
        let m = model();
        for i in 0..20 {
            assert_eq!(m.classify(&synth(RodState::In, 12, 12, 1.0, 500 + i)).state, RodState::In);
            assert_eq!(m.classify(&synth(RodState::Out, 12, 12, 1.0, 700 + i)).state, RodState::Out);
        }
    }

    #[test]
    fn menu_is_unknown() {
        let m = model();
        let menu = Grid::filled(12, 12, [30.0, 30.0, 30.0]);
        assert_eq!(m.classify(&menu).state, RodState::Unknown);
        let wrong_size = Grid::filled(8, 8, [0.0; 3]);
        assert_eq!(m.classify(&wrong_size).state, RodState::Unknown);
    }

    #[test]
    fn adapts_to_slow_dusk() {
        let si: Vec<_> = (0..10).map(|i| synth(RodState::In, 12, 12, 1.0, i)).collect();
        let so: Vec<_> = (0..10).map(|i| synth(RodState::Out, 12, 12, 1.0, 100 + i)).collect();
        let mut m = RodModel::fit(&si, &so, false);
        // 光照每帧降 0.1%，走到 55% 亮度：自适应应一直认得出（按真实节奏交替两种状态）。
        let mut light = 1.0f32;
        let mut i = 0u32;
        while light > 0.55 {
            let st = if (i / 40) % 2 == 0 { RodState::Out } else { RodState::In };
            let c = m.classify_and_adapt(&synth(st, 12, 12, light, 1000 + i));
            assert_eq!(c.state, st, "light={light}");
            light *= 0.999;
            i += 1;
        }
    }

    /// 按自动甩竿的节奏（甩出 8 秒、收回 1 秒，15 帧/秒）走一段黄昏，`lights(x)` 给出进度 x 时的（背景, 竿）亮度。
    fn run_dusk(lights: impl Fn(f32) -> (f32, f32)) {
        let mut m = model();
        let total = 15 * 9 * 20;
        for i in 0..total {
            let st = if i % 135 < 120 { RodState::Out } else { RodState::In };
            let (bg, rod) = lights(i as f32 / total as f32);
            let c = m.classify_and_adapt(&synth_lit(st, 12, 12, bg, rod, 2000 + i));
            assert_eq!(c.state, st, "frame {i} bg={bg:.2} rod={rod:.2} {c:?}");
        }
    }

    #[test]
    fn dusk_background_darkens_rod_does_not() {
        // 水下/火把旁：水暗到 25%，手里的竿亮度不变。
        run_dusk(|x| (1.0 - 0.75 * x, 1.0));
    }

    #[test]
    fn dusk_everything_darkens() {
        // 露天只靠天光：背景和竿一起暗到 30%。
        run_dusk(|x| (1.0 - 0.7 * x, 1.0 - 0.7 * x));
    }

    #[test]
    fn sudden_dim_is_not_in() {
        // 暂停菜单：整屏一下子变暗。不能当成"收回"去甩竿。
        let mut m = model();
        for i in 0..30 {
            m.classify_and_adapt(&synth(RodState::In, 12, 12, 1.0, 300 + i));
        }
        let c = m.classify(&synth(RodState::In, 12, 12, 0.5, 9));
        assert_eq!(c.state, RodState::Unknown, "{c:?}");
    }

    #[test]
    fn normalized_model_survives_sudden_night() {
        let si: Vec<_> = (0..10).map(|i| synth(RodState::In, 12, 12, 1.0, i)).collect();
        let so: Vec<_> = (0..10).map(|i| synth(RodState::Out, 12, 12, 1.0, 100 + i)).collect();
        let m = RodModel::fit(&si, &so, true);
        assert_eq!(m.classify(&synth(RodState::In, 12, 12, 0.4, 9)).state, RodState::In);
        assert_eq!(m.classify(&synth(RodState::Out, 12, 12, 0.4, 9)).state, RodState::Out);
    }
}
