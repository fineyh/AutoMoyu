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
}

fn luma(px: &[[f32; 3]]) -> f32 {
    px.iter().map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]).sum::<f32>() / px.len().max(1) as f32
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
        Self {
            w: tpl_in.w,
            h: tpl_in.h,
            normalize,
            tau_in: tau(max_j(&fin, &tpl_in)),
            tau_out: tau(max_j(&fout, &tpl_out)),
            tpl_in: tpl_in.px,
            tpl_out: tpl_out.px,
            between,
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
        let d_in = mad(&f.px, &self.tpl_in);
        let d_out = mad(&f.px, &self.tpl_out);
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
        let f = self.features(g);
        let c = self.classify_features(&f);
        let (own, other, d_own, d_other) = match c.state {
            RodState::In => (&mut self.tpl_in, &mut self.tpl_out, c.d_in, c.d_out),
            RodState::Out => (&mut self.tpl_out, &mut self.tpl_in, c.d_out, c.d_in),
            RodState::Unknown => return c,
        };
        if d_own < 0.45 * d_other {
            // 整体明暗变化对两个状态是一样的：看到的这张模板跟着当前帧走，
            // 另一张按同样的亮度比例缩放，这样甩出等了很久再收回时也认得出。
            let before = luma(own);
            for (t, p) in own.iter_mut().zip(&f.px) {
                for ch in 0..3 {
                    t[ch] += ADAPT_ALPHA * (p[ch] - t[ch]);
                }
            }
            let gain = luma(own) / before.max(1e-3);
            other.iter_mut().for_each(|t| *t = t.map(|v| v * gain));
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
                g.set(x, y, [60.0 * light, 40.0 * light, 20.0 * light]);
            }
        }
        if state == RodState::Out {
            // 线：一条细亮线
            for y in 0..tip {
                g.set(w / 2, y, [220.0 * light, 220.0 * light, 220.0 * light]);
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

    #[test]
    fn normalized_model_survives_sudden_night() {
        let si: Vec<_> = (0..10).map(|i| synth(RodState::In, 12, 12, 1.0, i)).collect();
        let so: Vec<_> = (0..10).map(|i| synth(RodState::Out, 12, 12, 1.0, 100 + i)).collect();
        let m = RodModel::fit(&si, &so, true);
        assert_eq!(m.classify(&synth(RodState::In, 12, 12, 0.4, 9)).state, RodState::In);
        assert_eq!(m.classify(&synth(RodState::Out, 12, 12, 0.4, 9)).state, RodState::Out);
    }
}
