//! 离线评估一段录像。
//!
//! 标注来自你自己的右键：
//! - 有钓鱼机：每次右键都是甩竿 → 右键前 0.1~0.7 秒是"收回"，右键后 1.3~2.3 秒是"甩出"。
//! - 没钓鱼机：右键交替为甩竿/收竿 → 收竿右键前 0.1~0.7 秒也是"甩出"，且咬钩≈收竿右键前一刻。
//!
//! 只用**前两轮**的标注样本做校准（和真实校准一样），其余全部用来测，
//! 这样夜晚/天气变化是否跟得住也能看出来。

use std::path::Path;

use anyhow::{ensure, Context, Result};
use moyu_core::bite_audio::{BiteAudioDetector, Sensitivity};
use moyu_core::calibrate::{search, FRAMES_PER_BATCH};
use moyu_core::{Geometry, Grid, RodState};
use serde::Serialize;

use crate::fixture::{load_png, read_jsonl, FrameRec, InputRec, Meta};
use crate::Scenario;

const DEBOUNCE: usize = 3;
const MIN_OUT_MS: u64 = 1_000;

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct Report {
    frames: usize,
    clicks: usize,
    roi_source: String,
    roi_px: (u32, u32, u32, u32),
    normalize: bool,
    calib_ratio: f32,
    calib_quality: String,
    labeled_eval_frames: usize,
    labeled_accuracy: f32,
    unknown_fraction: f32,
    worst_margin: f32,
    median_margin: f32,
    expected_catches: usize,
    counted_catches: usize,
    audio: Vec<AudioReport>,
    verdict_rod: bool,
    verdict_audio: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioReport {
    sensitivity: String,
    bites: usize,
    hits: usize,
    recall: f32,
    false_alarms: usize,
    false_per_10min_out: f32,
}

struct Labeled {
    idx: usize,
    state: RodState,
}

pub fn run(dir: &Path, json: Option<&Path>) -> Result<()> {
    let meta: Meta =
        serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json")).context("meta.json")?)?;
    let frames: Vec<FrameRec> = read_jsonl(&dir.join("frames.jsonl"))?;
    let clicks: Vec<u64> = read_jsonl::<InputRec>(&dir.join("events.jsonl"))?
        .into_iter()
        .filter(|e| e.kind == "rdown" && !e.injected)
        .map(|e| e.t_ms)
        .collect();
    let geom = Geometry::new(meta.client_w, meta.client_h);
    ensure!(geom.union == meta.union, "录像的候选区和当前算法不一致（录像版本太旧）");

    // 读帧（只有游戏在前台的帧才有图）
    let mut grids: Vec<Option<Grid>> = Vec::with_capacity(frames.len());
    for f in &frames {
        let p = dir.join(format!("union/{:06}.png", f.i));
        grids.push(if f.foreground && p.exists() { Some(load_png(&p)?) } else { None });
    }
    let have = grids.iter().filter(|g| g.is_some()).count();
    println!("{} 帧（有画面 {have}），右键 {} 次，场景 {:?}", frames.len(), clicks.len(), meta.scenario);

    // ---- 标注 ----
    let (casts, reels): (Vec<u64>, Vec<u64>) = match meta.scenario {
        Scenario::Machine => (clicks.clone(), Vec::new()),
        Scenario::Manual => {
            let c = clicks.iter().step_by(2).copied().collect();
            let r = clicks.iter().skip(1).step_by(2).copied().collect();
            (c, r)
        }
    };
    let in_window = |t: u64, lo: i64, hi: i64, anchor: u64| {
        let d = t as i64 - anchor as i64;
        d >= lo && d <= hi
    };
    let next_click_after = |t: u64| clicks.iter().copied().find(|&c| c > t);
    let mut labeled: Vec<Labeled> = Vec::new();
    for (idx, f) in frames.iter().enumerate() {
        if grids[idx].is_none() {
            continue;
        }
        let t = f.t_ms;
        let st = if casts.iter().any(|&c| in_window(t, -700, -100, c)) {
            Some(RodState::In)
        } else if casts.iter().any(|&c| {
            in_window(t, 1_300, 2_300, c) && next_click_after(c).is_none_or(|n| n > c + 2_500)
        }) || reels.iter().any(|&r| in_window(t, -700, -100, r))
        {
            Some(RodState::Out)
        } else {
            None
        };
        if let Some(state) = st {
            labeled.push(Labeled { idx, state });
        }
    }
    let li: Vec<&Labeled> = labeled.iter().filter(|l| l.state == RodState::In).collect();
    let lo: Vec<&Labeled> = labeled.iter().filter(|l| l.state == RodState::Out).collect();
    ensure!(
        li.len() >= FRAMES_PER_BATCH && lo.len() >= FRAMES_PER_BATCH,
        "标注样本不够（收回 {}，甩出 {}）：至少要完整钓 2 条",
        li.len(),
        lo.len()
    );

    // ---- 用前两轮校准 ----
    let n_cal = FRAMES_PER_BATCH.max(li.len().min(lo.len()).min(2 * 6));
    let cal_in: Vec<Grid> = li.iter().take(n_cal).map(|l| grids[l.idx].clone().unwrap()).collect();
    let cal_out: Vec<Grid> = lo.iter().take(n_cal).map(|l| grids[l.idx].clone().unwrap()).collect();
    let cal_set: std::collections::HashSet<usize> =
        li.iter().take(n_cal).chain(lo.iter().take(n_cal)).map(|l| l.idx).collect();
    let cal = match search(&geom, &cal_in, &cal_out) {
        Ok(c) => c,
        Err(e) => {
            println!("校准失败：{e}");
            return Ok(());
        }
    };
    let local = cal.roi.relative_to(geom.union);
    println!(
        "校准：{:?} 区，框 {:?}（像素 {:?}），归一化 {}，区分度 {:.1}x（{:?}）",
        cal.source,
        cal.roi,
        cal.roi_px(),
        cal.model.normalize,
        cal.ratio,
        cal.quality
    );

    // ---- 按时间顺序分类（带自适应），统计 ----
    let mut model = cal.model.clone();
    let mut states: Vec<Option<RodState>> = vec![None; frames.len()];
    let mut margins: Vec<(usize, f32)> = Vec::new(); // (idx, d_own/d_other)
    let mut unknown = 0usize;
    for (idx, g) in grids.iter().enumerate() {
        let Some(g) = g else { continue };
        let c = model.classify_and_adapt(&g.crop(local));
        states[idx] = Some(c.state);
        unknown += (c.state == RodState::Unknown) as usize;
        margins.push((idx, c.d_in.min(c.d_out) / c.d_in.max(c.d_out).max(1e-3)));
    }
    let eval: Vec<&Labeled> = labeled.iter().filter(|l| !cal_set.contains(&l.idx)).collect();
    let correct = eval.iter().filter(|l| states[l.idx] == Some(l.state)).count();
    let accuracy = if eval.is_empty() { 1.0 } else { correct as f32 / eval.len() as f32 };
    let mut labeled_margins: Vec<f32> = eval
        .iter()
        .map(|l| {
            let g = grids[l.idx].as_ref().unwrap().crop(local);
            let c = cal.model.classify(&g);
            let (own, other) = if l.state == RodState::In { (c.d_in, c.d_out) } else { (c.d_out, c.d_in) };
            own / other.max(1e-3)
        })
        .collect();
    labeled_margins.sort_by(f32::total_cmp);
    let worst = labeled_margins.last().copied().unwrap_or(0.0);
    let median = labeled_margins.get(labeled_margins.len() / 2).copied().unwrap_or(0.0);

    // ---- 防抖后的状态跳变 → 数鱼 ----
    let mut stable: Option<RodState> = None;
    let (mut cand, mut cnt) = (None, 0usize);
    let mut out_since: Option<u64> = None;
    let mut catches = 0usize;
    let mut reel_times: Vec<u64> = Vec::new();
    let mut out_ms = 0u64;
    let mut out_periods: Vec<(u64, u64)> = Vec::new();
    for (idx, s) in states.iter().enumerate() {
        let Some(s) = *s else { continue };
        if cand == Some(s) {
            cnt += 1;
        } else {
            cand = Some(s);
            cnt = 1;
        }
        if cnt >= DEBOUNCE && stable != Some(s) && s != RodState::Unknown {
            let t = frames[idx].t_ms;
            if s == RodState::In && stable == Some(RodState::Out) {
                if let Some(o) = out_since.take() {
                    if t - o >= MIN_OUT_MS {
                        catches += 1;
                        reel_times.push(t);
                    }
                    out_ms += t - o;
                    out_periods.push((o, t));
                }
            }
            if s == RodState::Out {
                out_since = Some(t);
            }
            stable = Some(s);
        }
    }
    let expected = match meta.scenario {
        Scenario::Machine => casts.len().saturating_sub(1).max(casts.len().min(1)),
        Scenario::Manual => reels.len(),
    };
    println!(
        "竿状态：测试帧 {} 帧准确率 {:.1}%，未知 {:.1}%，最差间距比 {:.2}（<{:.2} 才稳），中位 {:.2}",
        eval.len(),
        accuracy * 100.0,
        unknown as f32 * 100.0 / have.max(1) as f32,
        worst,
        moyu_core::rod_state::DECISION_RATIO,
        median
    );
    println!("数鱼：算法数到 {catches} 条，标注约 {expected} 条");

    // ---- 声音 ----
    let mut audio = Vec::new();
    let wav_path = dir.join("audio.wav");
    if wav_path.exists() {
        let mut r = hound::WavReader::open(&wav_path)?;
        let sr = r.spec().sample_rate;
        let samples: Vec<f32> = r.samples::<i16>().map(|s| s.map(|v| v as f32 / i16::MAX as f32)).collect::<Result<_, _>>()?;
        // 咬钩真值：手动 = 收竿右键；有机器 = 画面里 甩出→收回 的时刻
        let (bites, lead): (Vec<u64>, u64) = match meta.scenario {
            Scenario::Manual => (reels.clone(), 1_500),
            Scenario::Machine => (reel_times.clone(), 1_000),
        };
        let out_total_ms: u64 = match meta.scenario {
            Scenario::Machine => out_ms,
            Scenario::Manual => casts.iter().zip(&reels).map(|(c, r)| r.saturating_sub(*c)).sum(),
        };
        let periods: Vec<(u64, u64)> = match meta.scenario {
            Scenario::Machine => out_periods.clone(),
            Scenario::Manual => casts.iter().copied().zip(reels.iter().copied()).collect(),
        };
        for sens in [Sensitivity::Low, Sensitivity::Normal, Sensitivity::High] {
            let mut d = BiteAudioDetector::new(sr, sens);
            let onsets: Vec<u64> =
                samples.chunks(sr as usize / 100).flat_map(|c| d.push(c)).filter(|l| l.onset).map(|l| l.t_ms).collect();
            let hits = bites.iter().filter(|&&b| onsets.iter().any(|&o| o + lead >= b && o <= b + 200)).count();
            // 误报：在甩出期间（落水静默期之后）、且不在任何咬钩前窗口里的触发
            let fa = onsets
                .iter()
                .filter(|&&o| periods.iter().any(|&(s, e)| o > s + 1_200 && o < e))
                .filter(|&&o| !bites.iter().any(|&b| o + lead >= b && o <= b + 200))
                .count();
            let rep = AudioReport {
                sensitivity: format!("{sens:?}"),
                bites: bites.len(),
                hits,
                recall: if bites.is_empty() { 0.0 } else { hits as f32 / bites.len() as f32 },
                false_alarms: fa,
                false_per_10min_out: fa as f32 * 600_000.0 / out_total_ms.max(1) as f32,
            };
            println!(
                "声音（{:>6}）：咬钩 {} 次，命中 {}（召回 {:.0}%），误报 {} 次（每 10 分钟甩出 {:.1} 次）",
                rep.sensitivity,
                rep.bites,
                rep.hits,
                rep.recall * 100.0,
                rep.false_alarms,
                rep.false_per_10min_out
            );
            audio.push(rep);
        }
    } else {
        println!("没有 audio.wav，跳过声音评估。");
    }

    let verdict_rod = cal.ratio >= 5.0 && accuracy >= 0.98;
    let verdict_audio = audio
        .iter()
        .find(|a| a.sensitivity == "Normal")
        .map(|a| a.recall >= 0.95 && a.false_per_10min_out <= 1.0);
    println!(
        "结论：竿状态 {}；声音咬钩 {}",
        if verdict_rod { "达标（区分度 ≥5x 且准确率 ≥98%）" } else { "未达标" },
        match verdict_audio {
            Some(true) => "达标（召回 ≥95%，误报 ≤1 次/10 分钟）",
            Some(false) => "未达标",
            None => "无数据",
        }
    );
    let report = Report {
        frames: frames.len(),
        clicks: clicks.len(),
        roi_source: format!("{:?}", cal.source),
        roi_px: cal.roi_px(),
        normalize: cal.model.normalize,
        calib_ratio: cal.ratio,
        calib_quality: format!("{:?}", cal.quality),
        labeled_eval_frames: eval.len(),
        labeled_accuracy: accuracy,
        unknown_fraction: unknown as f32 / have.max(1) as f32,
        worst_margin: worst,
        median_margin: median,
        expected_catches: expected,
        counted_catches: catches,
        audio,
        verdict_rod,
        verdict_audio,
    };
    let _ = &margins;
    if let Some(p) = json {
        std::fs::write(p, serde_json::to_string_pretty(&report)?)?;
    }
    Ok(())
}
