//! 模拟黄昏：用白天录像校准，再把白天画面逐渐过渡到夜晚画面，看竿状态还跟不跟得住。
//!
//! 真正的昼夜交替没录下来，这里拿两段同一地点的录像按权重混合来近似：
//! 每帧随机抽一张同状态的白天帧和夜晚帧，按 w(t) 线性混合（w 平滑地从 0 走到 1）。
//! `--darken` 再把夜晚的背景（水、墙）额外压暗，而手里的竿不压——
//! 水下或没火把的地方，背景比手持物品暗得多，这是录像里没有、实际会遇到的情况。
//! `--rod-darken` 让竿也一起变暗（没有火把、只靠天光的地方）。
//! 节奏模仿自动甩竿：甩出 5~15 秒，收回 1 秒。

use std::path::Path;

use anyhow::{ensure, Result};
use moyu_core::calibrate::{search, FRAMES_PER_BATCH};
use moyu_core::rod_state::RodModel;
use moyu_core::{Grid, RodState};

use crate::analyze::{load, Loaded};

const FPS: u32 = 15;

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n
    }
}

fn crops(l: &Loaded, roi: moyu_core::CellRect, state: RodState) -> Vec<Grid> {
    l.labeled
        .iter()
        .filter(|x| x.state == state)
        .map(|x| l.grids[x.idx].as_ref().unwrap().crop(roi))
        .collect()
}

/// 只留下参考模型（不自适应）也认同标注的帧，去掉钓鱼机模式下的标注噪声。
fn consistent(m: &RodModel, gs: Vec<Grid>, state: RodState) -> Vec<Grid> {
    gs.into_iter().filter(|g| m.classify(g).state == state).collect()
}

pub fn run(day: &Path, night: &Path, seconds: u32, darkens: &[f32], rod_darken: f32, seeds: u32) -> Result<()> {
    let d = load(day)?;
    let n = load(night)?;
    ensure!(d.geom.union == n.geom.union, "两段录像的窗口尺寸不同");

    // 和 analyze 一样：用白天前两轮校准
    let li: Vec<_> = d.labeled.iter().filter(|l| l.state == RodState::In).collect();
    let lo: Vec<_> = d.labeled.iter().filter(|l| l.state == RodState::Out).collect();
    let n_cal = FRAMES_PER_BATCH.max(li.len().min(lo.len()).min(2 * 6));
    let cal_in: Vec<Grid> = li.iter().take(n_cal).map(|l| d.grids[l.idx].clone().unwrap()).collect();
    let cal_out: Vec<Grid> = lo.iter().take(n_cal).map(|l| d.grids[l.idx].clone().unwrap()).collect();
    let cal = search(&d.geom, &cal_in, &cal_out)?;
    let local = cal.roi.relative_to(d.geom.union);
    println!("白天校准：{:?} 区，框 {:?}，归一化 {}，区分度 {:.1}x", cal.source, cal.roi, cal.model.normalize, cal.ratio);

    let day_in = consistent(&cal.model, crops(&d, local, RodState::In), RodState::In);
    let day_out = consistent(&cal.model, crops(&d, local, RodState::Out), RodState::Out);
    let (ni, no) = (crops(&n, local, RodState::In), crops(&n, local, RodState::Out));
    let night_ref = RodModel::fit(&ni, &no, cal.model.normalize);
    let night_in = consistent(&night_ref, ni, RodState::In);
    let night_out = consistent(&night_ref, no, RodState::Out);
    ensure!(
        [&day_in, &day_out, &night_in, &night_out].iter().all(|v| v.len() >= 5),
        "可用样本太少（白天 收回 {} 甩出 {}，夜晚 收回 {} 甩出 {}）",
        day_in.len(),
        day_out.len(),
        night_in.len(),
        night_out.len()
    );
    println!(
        "样本：白天 收回 {} 甩出 {}，夜晚 收回 {} 甩出 {}",
        day_in.len(),
        day_out.len(),
        night_in.len(),
        night_out.len()
    );

    // 竿（前景）= 校准时两张模板差得多的像素，且只在"收回"帧里是竿；其余都当背景。
    let fg: Vec<bool> = {
        let mi = moyu_core::image::mean_of(&day_in);
        let mo = moyu_core::image::mean_of(&day_out);
        mi.px.iter().zip(&mo.px).map(|(a, b)| (0..3).map(|c| (a[c] - b[c]).abs()).sum::<f32>() / 3.0 > 40.0).collect()
    };

    let day_s = 30u32;
    let night_s = 60u32;
    let total = (day_s + seconds + night_s) * FPS;
    for &darken in darkens {
        let (mut wrong, mut longest, mut pauses) = (0u32, 0u32, 0u32);
        for seed in 0..seeds {
            let mut model = cal.model.clone();
            let mut rng = Rng(0x9e37_79b9 ^ (seed + 1).wrapping_mul(2_654_435_761));
            let mut state = RodState::Out;
            let mut left = 0u32;
            let mut run = 0u32;
            for f in 0..total {
                if left == 0 {
                    state = if state == RodState::Out { RodState::In } else { RodState::Out };
                    left = if state == RodState::In { FPS } else { FPS * (5 + rng.below(11) as u32) };
                }
                left -= 1;
                let t = f as f32 / FPS as f32;
                let x = ((t - day_s as f32) / seconds.max(1) as f32).clamp(0.0, 1.0);
                let w = x * x * (3.0 - 2.0 * x);
                let (dp, np) = if state == RodState::In { (&day_in, &night_in) } else { (&day_out, &night_out) };
                let a = &dp[rng.below(dp.len())];
                let b = &np[rng.below(np.len())];
                let mut g = a.clone();
                for (i, (o, (p, q))) in g.px.iter_mut().zip(a.px.iter().zip(&b.px)).enumerate() {
                    let k = if state == RodState::In && fg[i] { rod_darken } else { darken };
                    for c in 0..3 {
                        o[c] = (1.0 - w) * p[c] + w * k * q[c];
                    }
                }
                let c = model.classify_and_adapt(&g);
                if seed == 0 && std::env::var_os("MOYU_VERBOSE").is_some() && (c.state != state || f % FPS == 0) {
                    println!("  t={t:5.1} w={w:.2} 真 {state:?} 算 {:?} d_in {:.0} d_out {:.0}", c.state, c.d_in, c.d_out);
                }
                if c.state == state {
                    run = 0;
                } else {
                    wrong += 1;
                    run += 1;
                    longest = longest.max(run);
                    if run == FPS * 10 {
                        pauses += 1;
                    }
                }
            }
        }
        println!(
            "背景额外压暗 ×{darken:.2}，竿 ×{rod_darken:.2}：认错/认不出 {:.2}%，最长连续 {:.1} 秒，会触发暂停 {pauses} 次（{seeds} 轮）",
            wrong as f32 * 100.0 / (total * seeds) as f32,
            longest as f32 / FPS as f32
        );
    }
    Ok(())
}
