//! 自动校准：程序自己甩两轮竿，在候选区域里滑窗挑出"收回/甩出"区分度最高的小框。
//!
//! 流程（约 8 秒）：收回采样 → 甩竿 → 甩出采样 → 收竿 → 收回采样 → 再来一轮。
//! `Calibrator` 是 sans-IO 状态机：调用方每帧喂候选区（并集）网格，按它吐出的 `Click` 去点。

use serde::{Deserialize, Serialize};

use crate::image::{analysis_scale, mad, CellRect, Grid, RelRect};
use crate::rod_state::{RodModel, RodState};

/// 候选区域来源。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// 第一人称右下角的手持鱼竿。
    Hand,
    /// 底部快捷栏选中格。
    Hotbar,
}

impl Source {
    pub fn area(self) -> RelRect {
        match self {
            Source::Hand => RelRect::new(0.5, 0.45, 0.5, 0.55),
            Source::Hotbar => RelRect::new(0.3, 0.85, 0.4, 0.15),
        }
    }
}

/// 某个客户区尺寸下的分析几何。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    pub client_w: u32,
    pub client_h: u32,
    pub k: u32,
    pub grid_w: u32,
    pub grid_h: u32,
    /// 所有候选区的外包矩形（校准时只截这一块）。
    pub union: CellRect,
    pub candidates: Vec<(Source, CellRect)>,
}

impl Geometry {
    pub fn new(client_w: u32, client_h: u32) -> Self {
        let k = analysis_scale(client_w);
        let (grid_w, grid_h) = (client_w / k, client_h / k);
        let candidates: Vec<(Source, CellRect)> =
            [Source::Hand, Source::Hotbar].iter().map(|&s| (s, s.area().to_cells(grid_w, grid_h))).collect();
        let x0 = candidates.iter().map(|c| c.1.x).min().unwrap();
        let y0 = candidates.iter().map(|c| c.1.y).min().unwrap();
        let x1 = candidates.iter().map(|c| c.1.x + c.1.w).max().unwrap();
        let y1 = candidates.iter().map(|c| c.1.y + c.1.h).max().unwrap();
        Self { client_w, client_h, k, grid_w, grid_h, union: CellRect::new(x0, y0, x1 - x0, y1 - y0), candidates }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    Poor,
    Good,
    Excellent,
}

impl Quality {
    pub fn from_ratio(r: f32) -> Self {
        if r >= 8.0 {
            Quality::Excellent
        } else if r >= 4.0 {
            Quality::Good
        } else {
            Quality::Poor
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationResult {
    pub client_w: u32,
    pub client_h: u32,
    pub k: u32,
    pub source: Source,
    /// 客户区网格坐标下的识别框。
    pub roi: CellRect,
    pub rel: RelRect,
    pub model: RodModel,
    /// 区分度 = 模板间距 / (组内抖动之和)。
    pub ratio: f32,
    pub quality: Quality,
    /// 用校准样本自测的分类准确率。
    pub accuracy: f32,
}

impl CalibrationResult {
    /// 识别框在客户区里的像素矩形。
    pub fn roi_px(&self) -> (u32, u32, u32, u32) {
        self.roi.to_px(self.k)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CalError {
    /// 样本太少（窗口丢了/没聚焦）。
    NotEnoughSamples,
    /// 甩竿前后画面几乎没变化。
    NoSignal,
    /// 找到了变化，但同一状态前后不一致（中途被机器收竿、视角动了）。
    Inconsistent,
}

impl std::fmt::Display for CalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CalError::NotEnoughSamples => "没采到足够的画面：请保持游戏在前台",
            CalError::NoSignal => "甩竿前后画面几乎没变化：手上可能不是鱼竿，或面前没有水",
            CalError::Inconsistent => "两轮试甩结果不一致：请别动鼠标，站在原地重试",
        })
    }
}

impl std::error::Error for CalError {}

// ---------------------------------------------------------------------------
// 采样流程
// ---------------------------------------------------------------------------

pub const FRAMES_PER_BATCH: usize = 10;
const SETTLE_MS: u64 = 500;
const AFTER_CAST_MS: u64 = 1300;
const AFTER_REEL_MS: u64 = 900;
const MIN_BETWEEN: f32 = 5.0;
const MIN_RATIO: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Wait(u64),
    Sample(RodState),
    Click,
}

/// 给界面显示的进度。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CalProgress {
    /// 第几轮（1 或 2）。
    pub round: u32,
    /// 当前在采哪种画面；`None` 表示在等待/甩竿。
    pub sampling: Option<RodState>,
    pub frames_in: usize,
    pub frames_out: usize,
    /// 0..1
    pub progress: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CalOutput {
    Click,
    Progress(CalProgress),
}

pub struct Calibrator {
    pub geom: Geometry,
    stages: Vec<Stage>,
    idx: usize,
    stage_start: Option<u64>,
    batch_count: usize,
    samples_in: Vec<Grid>,
    samples_out: Vec<Grid>,
}

impl Calibrator {
    pub fn new(client_w: u32, client_h: u32) -> Self {
        use Stage::*;
        let round = [Sample(RodState::In), Click, Wait(AFTER_CAST_MS), Sample(RodState::Out), Click, Wait(AFTER_REEL_MS)];
        let mut stages = vec![Wait(SETTLE_MS)];
        stages.extend_from_slice(&round);
        stages.extend_from_slice(&round);
        stages.push(Sample(RodState::In));
        Self {
            geom: Geometry::new(client_w, client_h),
            stages,
            idx: 0,
            stage_start: None,
            batch_count: 0,
            samples_in: Vec::new(),
            samples_out: Vec::new(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.idx >= self.stages.len()
    }

    pub fn progress(&self) -> CalProgress {
        let total = (self.stages.len()) as f32;
        let round = if self.idx <= 6 { 1 } else { 2 };
        let sampling = match self.stages.get(self.idx) {
            Some(Stage::Sample(s)) => Some(*s),
            _ => None,
        };
        let sub = if sampling.is_some() { self.batch_count as f32 / FRAMES_PER_BATCH as f32 } else { 0.0 };
        CalProgress {
            round,
            sampling,
            frames_in: self.samples_in.len(),
            frames_out: self.samples_out.len(),
            progress: ((self.idx as f32 + sub) / total).min(1.0),
        }
    }

    /// 每帧调用一次。`frame` 是 `geom.union` 这块的网格；没截到就传 `None`。
    pub fn tick(&mut self, now_ms: u64, frame: Option<Grid>) -> Vec<CalOutput> {
        let mut out = Vec::new();
        while let Some(&stage) = self.stages.get(self.idx) {
            let start = *self.stage_start.get_or_insert(now_ms);
            match stage {
                Stage::Wait(ms) => {
                    if now_ms.saturating_sub(start) < ms {
                        break;
                    }
                }
                Stage::Click => out.push(CalOutput::Click),
                Stage::Sample(state) => {
                    if let Some(f) = frame.as_ref() {
                        if f.w == self.geom.union.w as usize && f.h == self.geom.union.h as usize {
                            match state {
                                RodState::Out => self.samples_out.push(f.clone()),
                                _ => self.samples_in.push(f.clone()),
                            }
                            self.batch_count += 1;
                        }
                    }
                    if self.batch_count < FRAMES_PER_BATCH {
                        break;
                    }
                    self.batch_count = 0;
                }
            }
            self.idx += 1;
            self.stage_start = None;
            if matches!(stage, Stage::Click) {
                break; // 点完这一帧就停，下一帧再继续计时
            }
        }
        out.push(CalOutput::Progress(self.progress()));
        out
    }

    pub fn finish(&self) -> Result<CalibrationResult, CalError> {
        search(&self.geom, &self.samples_in, &self.samples_out)
    }
}

// ---------------------------------------------------------------------------
// 选区
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Scored {
    source: Source,
    rect: CellRect, // 客户区网格坐标
    normalize: bool,
    between: f32,
    ratio: f32,
}

/// 滑窗候选框边长（格子数；1920 宽时 1 格 = 4 像素）。
const BOX_SIZES: [u32; 6] = [5, 6, 8, 10, 12, 16];

fn features_into(src: &Grid, r: CellRect, normalize: bool, out: &mut Vec<[f32; 3]>) {
    out.clear();
    let (x0, y0, w, h) = (r.x as usize, r.y as usize, r.w as usize, r.h as usize);
    for y in y0..y0 + h {
        out.extend_from_slice(&src.px[y * src.w + x0..y * src.w + x0 + w]);
    }
    if normalize {
        let n = out.len() as f32;
        let luma: f32 = out.iter().map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]).sum::<f32>() / n;
        let gain = 128.0 / luma.max(16.0);
        out.iter_mut().for_each(|c| *c = c.map(|v| v * gain));
    }
}

struct Scratch {
    fin: Vec<Vec<[f32; 3]>>,
    fout: Vec<Vec<[f32; 3]>>,
    mean_in: Vec<[f32; 3]>,
    mean_out: Vec<[f32; 3]>,
}

fn mean_into(fs: &[Vec<[f32; 3]>], out: &mut Vec<[f32; 3]>) {
    out.clear();
    out.resize(fs[0].len(), [0.0; 3]);
    for f in fs {
        for (o, p) in out.iter_mut().zip(f) {
            o[0] += p[0];
            o[1] += p[1];
            o[2] += p[2];
        }
    }
    let inv = 1.0 / fs.len() as f32;
    out.iter_mut().for_each(|o| *o = o.map(|v| v * inv));
}

/// 评估一个窗口：返回 (模板间距, 区分度)。`r` 是相对并集网格的坐标。
fn eval(si: &[Grid], so: &[Grid], r: CellRect, normalize: bool, s: &mut Scratch) -> (f32, f32) {
    for (g, buf) in si.iter().zip(s.fin.iter_mut()) {
        features_into(g, r, normalize, buf);
    }
    for (g, buf) in so.iter().zip(s.fout.iter_mut()) {
        features_into(g, r, normalize, buf);
    }
    mean_into(&s.fin, &mut s.mean_in);
    mean_into(&s.fout, &mut s.mean_out);
    let between = mad(&s.mean_in, &s.mean_out);
    let j_in = s.fin.iter().map(|f| mad(f, &s.mean_in)).sum::<f32>() / s.fin.len() as f32;
    let j_out = s.fout.iter().map(|f| mad(f, &s.mean_out)).sum::<f32>() / s.fout.len() as f32;
    (between, between / (j_in + j_out + 0.5))
}

/// 在所有候选区里滑窗，找区分度最高的小框并建模。
/// `samples_*` 是 `geom.union` 这块的网格。
pub fn search(geom: &Geometry, samples_in: &[Grid], samples_out: &[Grid]) -> Result<CalibrationResult, CalError> {
    if samples_in.len() < FRAMES_PER_BATCH || samples_out.len() < FRAMES_PER_BATCH {
        return Err(CalError::NotEnoughSamples);
    }
    let mut s = Scratch {
        fin: vec![Vec::new(); samples_in.len()],
        fout: vec![Vec::new(); samples_out.len()],
        mean_in: Vec::new(),
        mean_out: Vec::new(),
    };
    let mut all: Vec<Scored> = Vec::new();
    for &(source, area) in &geom.candidates {
        let local = area.relative_to(geom.union);
        for &size in BOX_SIZES.iter().filter(|&&b| b <= area.w && b <= area.h) {
            let stride = (size / 3).max(1);
            for normalize in [false, true] {
                let mut y = 0;
                while y + size <= local.h {
                    let mut x = 0;
                    while x + size <= local.w {
                        let r = CellRect::new(local.x + x, local.y + y, size, size);
                        let (between, ratio) = eval(samples_in, samples_out, r, normalize, &mut s);
                        if between >= MIN_BETWEEN {
                            let rect = CellRect::new(r.x + geom.union.x, r.y + geom.union.y, size, size);
                            all.push(Scored { source, rect, normalize, between, ratio });
                        }
                        x += stride;
                    }
                    y += stride;
                }
            }
        }
    }
    let best_ratio = all.iter().map(|c| c.ratio).fold(0.0f32, f32::max);
    // 区分度 < 1.5 基本就是噪声碰巧超了门限
    if best_ratio < MIN_RATIO {
        return Err(CalError::NoSignal);
    }
    // 区分度差不多（≥90%）时选更大的框：对视角轻微摇晃更稳。
    // 亮度归一化的特征对突然的明暗变化（闪电、火把）更稳，同等条件下优先。
    let eff = |c: &Scored| c.ratio * if c.normalize { 1.0 } else { 0.85 };
    let best_eff = all.iter().map(eff).fold(0.0f32, f32::max);
    let pick = *all
        .iter()
        .filter(|c| eff(c) >= 0.9 * best_eff)
        .max_by(|a, b| (a.rect.w * a.rect.h).cmp(&(b.rect.w * b.rect.h)).then(eff(a).total_cmp(&eff(b))))
        .unwrap();

    let local = pick.rect.relative_to(geom.union);
    let crop = |gs: &[Grid]| gs.iter().map(|g| g.crop(local)).collect::<Vec<_>>();
    let (ci, co) = (crop(samples_in), crop(samples_out));
    let model = RodModel::fit(&ci, &co, pick.normalize);
    let correct = ci.iter().filter(|g| model.classify(g).state == RodState::In).count()
        + co.iter().filter(|g| model.classify(g).state == RodState::Out).count();
    let accuracy = correct as f32 / (ci.len() + co.len()) as f32;
    if accuracy < 0.9 {
        return Err(CalError::Inconsistent);
    }
    let mut quality = Quality::from_ratio(pick.ratio);
    if accuracy < 1.0 {
        quality = quality.min(Quality::Good);
    }
    let _ = pick.between;
    Ok(CalibrationResult {
        client_w: geom.client_w,
        client_h: geom.client_h,
        k: geom.k,
        source: pick.source,
        roi: pick.rect,
        rel: RelRect::from_cells(pick.rect, geom.grid_w, geom.grid_h),
        model,
        ratio: pick.ratio,
        quality,
        accuracy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成一整块并集区域：水面噪声 + 一处"竿头"在甩出时变化。
    fn scene(geom: &Geometry, out: bool, seed: u32, tip: (u32, u32)) -> Grid {
        let (w, h) = (geom.union.w as usize, geom.union.h as usize);
        let mut g = Grid::new(w, h);
        let mut s = seed.wrapping_mul(747796405).wrapping_add(2891336453);
        for (i, p) in g.px.iter_mut().enumerate() {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let n = (s % 31) as f32 - 15.0; // 明显的水面波动
            let base = 60.0 + ((i % w) as f32 * 0.2);
            *p = [base + n, base + 40.0 + n, 180.0 + n];
        }
        let (tx, ty) = (tip.0 as usize - geom.union.x as usize, tip.1 as usize - geom.union.y as usize);
        for y in ty..ty + 8 {
            for x in tx..tx + 8 {
                let on = if out { (x + y) % 3 == 0 } else { x >= tx + 3 && x <= tx + 4 };
                if on {
                    g.set(x, y, [40.0, 25.0, 10.0]);
                }
            }
        }
        g
    }

    fn run(geom: &Geometry, tip: (u32, u32)) -> Result<CalibrationResult, CalError> {
        let si: Vec<_> = (0..20).map(|i| scene(geom, false, i, tip)).collect();
        let so: Vec<_> = (0..20).map(|i| scene(geom, true, 100 + i, tip)).collect();
        search(geom, &si, &so)
    }

    #[test]
    fn geometry_1080p() {
        let g = Geometry::new(1920, 1080);
        assert_eq!((g.k, g.grid_w, g.grid_h), (4, 480, 270));
        assert_eq!(g.union.x, 144);
        assert!(g.union.x + g.union.w <= 480 && g.union.y + g.union.h <= 270);
    }

    #[test]
    fn finds_the_changing_spot() {
        let geom = Geometry::new(1920, 1080);
        let tip = (360, 160);
        let r = run(&geom, tip).expect("calibration");
        assert_eq!(r.source, Source::Hand);
        // 选中框必须和竿头方块有交集
        let hit = r.roi.x < tip.0 + 8 && tip.0 < r.roi.x + r.roi.w && r.roi.y < tip.1 + 8 && tip.1 < r.roi.y + r.roi.h;
        assert!(hit, "roi {:?}", r.roi);
        assert!(r.quality >= Quality::Good, "ratio {}", r.ratio);
        assert_eq!(r.accuracy, 1.0);
    }

    #[test]
    fn nothing_changes_means_no_signal() {
        let geom = Geometry::new(1280, 720);
        let si: Vec<_> = (0..10).map(|i| scene(&geom, false, i, (300, 200))).collect();
        let so: Vec<_> = (0..10).map(|i| scene(&geom, false, 50 + i, (300, 200))).collect();
        assert_eq!(search(&geom, &si, &so).unwrap_err(), CalError::NoSignal);
    }

    #[test]
    fn calibrator_schedule_clicks_four_times() {
        let mut c = Calibrator::new(1920, 1080);
        let tip = (360, 160);
        let mut out_state = false;
        let mut clicks = 0;
        let mut t = 0;
        while !c.is_done() && t < 60_000 {
            let f = scene(&c.geom, out_state, t as u32, tip);
            for o in c.tick(t, Some(f)) {
                if o == CalOutput::Click {
                    clicks += 1;
                    out_state = !out_state;
                }
            }
            t += 66;
        }
        assert!(c.is_done());
        assert_eq!(clicks, 4);
        assert!(!out_state, "校准结束时竿应收回");
        assert!(t < 12_000, "校准应在 12 秒内完成，用了 {t}ms");
        let r = c.finish().unwrap();
        assert!(r.quality >= Quality::Good);
    }
}
