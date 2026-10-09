//! 声音咬钩检测：2–12 kHz 频带里"又响又持续"的宽频声。
//!
//! Phase 0 录像（fixtures/*-manual）得出的结论：
//! - 咬钩水花是 0.3~0.8 秒的宽频声，从 1 kHz 一直铺到 16 kHz；
//!   等待期间的杂音（鱼游近的咕噜声、雨声起伏、环境音）都是一两帧的尖峰，或者更小声。
//!   所以判据是"持续变响"而不是"突然变响"：最近 ~85 ms 里至少 6/8 的分析帧同时满足
//!   ① 比底噪高 [`REL_DB`]，② 超过绝对门槛。
//! - 只看相对底噪不够：白天安静时底噪接近数字静音，任何小声音都"高出很多"。
//!   绝对门槛跟着**甩竿声**走（[`BiteAudioDetector::note_cast`]）：甩竿声的音量随游戏音量
//!   一起变，水花比它低 ~20 dB 以内，等待期杂音更低（按本文件的量法）。
//! - 底噪用 5 秒窗口的 25% 分位数：饵钓 III 下甩竿间隔只有 2~3 秒，5 秒里一大半是收竿/
//!   甩竿/落水声，中位数会被抬高。
//! - 生物叫声（猫、狗、村民……）也够响够长，但它们是一串谐波，能量集中在几条窄线上；
//!   水花是噪声，能量铺满频带。所以每帧再算一个音调性，太"有音高"的帧不算响（见 [`DEFAULT_TONAL_MAX`]）。
//! - 游戏声音比画面/输入晚约 250 ms，水花经常在你看到浮漂下沉并右键之后才出现在音频里。

use std::collections::VecDeque;
use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};

pub const FRAME: usize = 1024;
pub const HOP: usize = 512;
const BAND_HZ: (f32, f32) = (2_000.0, 12_000.0);
const FLOOR_WINDOW_MS: u64 = 5_000;
const FLOOR_QUANTILE: f32 = 0.25;
/// 比底噪至少高这么多（dB）。
const REL_DB: f32 = 12.0;
/// 持续判定窗口（帧数，~85 ms @48k）与其中至少要"响"的帧数。
const SUSTAIN_WIN: usize = 8;
const SUSTAIN_MIN: usize = 6;
/// 触发后，持续判定要先断开这么久才重新布防（同一段水花只报一次）。
const REARM_MS: u64 = 150;
/// 甩竿声在 note_cast 之后的这段音频里找（含游戏声音延迟）。
const THROW_WINDOW_MS: (u64, u64) = (100, 700);
/// 还没量到甩竿声时假定的甩竿声音量（dB，Phase 0 手钓录像约 -12）。
const DEFAULT_THROW_DB: f32 = -12.0;
const THROW_HISTORY: usize = 5;
/// 默认音调性门槛（见 [`BiteAudioDetector::set_tonal_max`]）。
///
/// 实测（帧级音调性）：咬钩水花多在 0.1 以下；服务器上的猫叫、其他生物叫声 0.3~0.99。
/// 0.3 时 Phase 0 手钓录像召回 23/24、11/11，服务器录像里 22 次收竿中由叫声触发的 11 次全部滤掉。
pub const DEFAULT_TONAL_MAX: f32 = 0.3;
/// 数字静音下的 dB。
const SILENCE_DB: f32 = -120.0;
/// 音调性：某个频点比周围 ±[`TONAL_HALF_BINS`] 个频点的中位数高出这么多（dB）就算"谐波峰"。
const TONAL_PEAK_DB: f32 = 10.0;
const TONAL_HALF_BINS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Sensitivity {
    Low,
    #[default]
    Normal,
    High,
}

impl Sensitivity {
    /// 绝对门槛 = 甩竿声音量 − 这个值（dB）。越大越灵敏。
    pub fn below_throw_db(self) -> f32 {
        match self {
            Sensitivity::Low => 17.5,
            Sensitivity::Normal => 19.0,
            Sensitivity::High => 20.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevel {
    pub t_ms: u64,
    /// 当前频带电平（dB）。
    pub level_db: f32,
    /// 当前生效的门槛：max(底噪 + REL_DB, 甩竿声 − 灵敏度偏移)。
    pub gate_db: f32,
    pub onset: bool,
    /// 频带能量里落在谐波峰上的比例（0..1）。水花是噪声，接近 0；猫狗叫、村民、甩竿"嗖"声是一串谐波，偏高。
    #[serde(default)]
    pub tonal: f32,
    /// onset 时：这段持续声音开始的时刻（离线评估用来对齐标注，不含判定延迟）。
    pub onset_start_ms: Option<u64>,
}

pub struct BiteAudioDetector {
    sample_rate: u32,
    below_throw: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    buf: Vec<f32>,
    scratch_in: Vec<f32>,
    spectrum: Vec<realfft::num_complex::Complex<f32>>,
    band: (usize, usize),
    /// 底噪窗口（所有帧都进：水花只有零点几秒，动不了 25% 分位数）。
    history: VecDeque<(u64, f32)>,
    /// 最近 SUSTAIN_WIN 帧：(时刻, 电平, 是否"响")。
    recent: VecDeque<(u64, f32, bool)>,
    armed: bool,
    quiet_since: Option<u64>,
    throws: VecDeque<f32>,
    /// 正在量甩竿声：(开始, 结束, 目前最大持续电平)。
    measuring: Option<(u64, u64, f32)>,
    samples_seen: u64,
    /// 音调性超过这个值的帧不算"响"（滤掉动物叫声等有音高的声音）；`None` = 不滤。
    tonal_max: Option<f32>,
    log_buf: Vec<f32>,
    med_buf: Vec<f32>,
}

impl BiteAudioDetector {
    pub fn new(sample_rate: u32, sensitivity: Sensitivity) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FRAME);
        let window = (0..FRAME)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FRAME - 1) as f32).cos())
            .collect();
        let bin = |hz: f32| ((hz / sample_rate as f32 * FRAME as f32).round() as usize).min(FRAME / 2);
        Self {
            sample_rate,
            below_throw: sensitivity.below_throw_db(),
            spectrum: fft.make_output_vec(),
            scratch_in: fft.make_input_vec(),
            fft,
            window,
            buf: Vec::with_capacity(FRAME * 2),
            band: (bin(BAND_HZ.0).max(1), bin(BAND_HZ.1)),
            history: VecDeque::new(),
            recent: VecDeque::with_capacity(SUSTAIN_WIN + 1),
            armed: true,
            quiet_since: None,
            throws: VecDeque::new(),
            measuring: None,
            samples_seen: 0,
            tonal_max: Some(DEFAULT_TONAL_MAX),
            log_buf: Vec::new(),
            med_buf: Vec::new(),
        }
    }

    /// 离线评估用：改音调性门槛，`None` 关掉过滤。
    pub fn set_tonal_max(&mut self, v: Option<f32>) {
        self.tonal_max = v;
    }

    pub fn set_sensitivity(&mut self, s: Sensitivity) {
        self.below_throw = s.below_throw_db();
    }

    /// 刚点了甩竿：接下来的一小段音频里量甩竿声有多响，用来定绝对门槛。
    pub fn note_cast(&mut self) {
        let now = self.now_ms();
        self.measuring = Some((now + THROW_WINDOW_MS.0, now + THROW_WINDOW_MS.1, SILENCE_DB));
    }

    /// 最近几次甩竿声的中位数（dB）；还没量到时返回默认值。
    pub fn throw_db(&self) -> f32 {
        if self.throws.is_empty() {
            return DEFAULT_THROW_DB;
        }
        let mut v: Vec<f32> = self.throws.iter().copied().collect();
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    }

    /// 已喂入音频的时长（ms）。
    pub fn now_ms(&self) -> u64 {
        (self.samples_seen + self.buf.len() as u64) * 1000 / self.sample_rate as u64
    }

    fn floor(&self) -> Option<f32> {
        if self.history.len() < 40 {
            return None;
        }
        let mut v: Vec<f32> = self.history.iter().map(|h| h.1).collect();
        let k = ((v.len() - 1) as f32 * FLOOR_QUANTILE) as usize;
        v.select_nth_unstable_by(k, f32::total_cmp);
        Some(v[k])
    }

    /// 喂单声道样本（-1..1）。返回每个分析帧的电平。
    /// 时间戳按已喂样本数推算，所以调用方要按顺序、不丢包地喂。
    pub fn push(&mut self, samples: &[f32]) -> Vec<AudioLevel> {
        self.buf.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.buf.len() >= FRAME {
            for (d, (s, w)) in self.scratch_in.iter_mut().zip(self.buf.iter().zip(&self.window)) {
                *d = s * w;
            }
            // 时间戳取本帧最后一个样本的时刻
            let t_ms = (self.samples_seen + FRAME as u64) * 1000 / self.sample_rate as u64;
            self.buf.drain(..HOP);
            self.samples_seen += HOP as u64;
            if self.fft.process(&mut self.scratch_in, &mut self.spectrum).is_err() {
                continue;
            }
            let (b0, b1) = self.band;
            let power: f32 =
                self.spectrum[b0..b1].iter().map(|c| c.norm_sqr()).sum::<f32>() / (b1 - b0).max(1) as f32;
            let level_db = (10.0 * power.max(1e-12).log10()).max(SILENCE_DB);
            let tonal = self.tonality();
            out.push(self.step(t_ms, level_db, tonal));
        }
        out
    }

    /// 当前频谱在频带里的音调性：高出局部中位数 [`TONAL_PEAK_DB`] 的频点占了多少能量。
    /// 白噪声单帧频点按指数分布，高出中位数 10 dB 的概率约千分之一，所以水花接近 0。
    fn tonality(&mut self) -> f32 {
        let (b0, b1) = self.band;
        self.log_buf.clear();
        self.log_buf.extend(self.spectrum[b0..b1].iter().map(|c| 10.0 * c.norm_sqr().max(1e-20).log10()));
        let n = self.log_buf.len();
        let (mut peak, mut total) = (0.0f32, 0.0f32);
        for k in 0..n {
            let lo = k.saturating_sub(TONAL_HALF_BINS);
            let hi = (k + TONAL_HALF_BINS + 1).min(n);
            self.med_buf.clear();
            self.med_buf.extend_from_slice(&self.log_buf[lo..hi]);
            let m = self.med_buf.len() / 2;
            let med = *self.med_buf.select_nth_unstable_by(m, f32::total_cmp).1;
            let p = self.spectrum[b0 + k].norm_sqr();
            total += p;
            if self.log_buf[k] - med > TONAL_PEAK_DB {
                peak += p;
            }
        }
        if total > 0.0 { peak / total } else { 0.0 }
    }

    fn step(&mut self, t_ms: u64, level_db: f32, tonal: f32) -> AudioLevel {
        let floor = self.floor();
        let gate_db = floor.map_or(f32::INFINITY, |f| (f + REL_DB).max(self.throw_db() - self.below_throw));
        let hot = level_db > gate_db && self.tonal_max.is_none_or(|m| tonal <= m);
        self.recent.push_back((t_ms, level_db, hot));
        if self.recent.len() > SUSTAIN_WIN {
            self.recent.pop_front();
        }

        // "持续电平"：最近 SUSTAIN_WIN 帧里第 SUSTAIN_MIN 响的那一帧。
        let sustained_db = if self.recent.len() == SUSTAIN_WIN {
            let mut v: Vec<f32> = self.recent.iter().map(|r| r.1).collect();
            v.sort_by(f32::total_cmp);
            Some(v[SUSTAIN_WIN - SUSTAIN_MIN])
        } else {
            None
        };

        if let Some((from, to, peak)) = self.measuring.as_mut() {
            if t_ms >= *from {
                if let Some(s) = sustained_db {
                    *peak = peak.max(s);
                }
            }
            if t_ms >= *to {
                if *peak > SILENCE_DB {
                    self.throws.push_back(*peak);
                    if self.throws.len() > THROW_HISTORY {
                        self.throws.pop_front();
                    }
                }
                self.measuring = None;
            }
        }

        let sustained = sustained_db.is_some() && self.recent.iter().filter(|r| r.2).count() >= SUSTAIN_MIN;
        let mut onset = false;
        let mut onset_start_ms = None;
        if sustained {
            self.quiet_since = None;
            if self.armed {
                self.armed = false;
                onset = true;
                onset_start_ms = self.recent.iter().find(|r| r.2).map(|r| r.0);
            }
        } else {
            let since = *self.quiet_since.get_or_insert(t_ms);
            if t_ms.saturating_sub(since) >= REARM_MS {
                self.armed = true;
            }
        }

        self.history.push_back((t_ms, level_db));
        while self.history.front().is_some_and(|h| t_ms.saturating_sub(h.0) > FLOOR_WINDOW_MS) {
            self.history.pop_front();
        }
        AudioLevel { t_ms, level_db, gate_db, onset, tonal, onset_start_ms }
    }
}

/// 交错多声道 → 单声道。
pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved.chunks_exact(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u32);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    /// 背景：低频"音乐"+ 轻微白噪；`bursts` = (开始秒, 时长秒, 幅度) 的宽频噪声。
    fn clip(sr: u32, secs: f32, bursts: &[(f32, f32, f32)]) -> Vec<f32> {
        let mut r = Rng(12345);
        (0..(sr as f32 * secs) as usize)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let mut v = 0.3 * (2.0 * std::f32::consts::PI * 220.0 * t).sin() + 0.001 * r.next();
                for &(s, len, amp) in bursts {
                    if t >= s && t < s + len {
                        v += amp * r.next();
                    }
                }
                v
            })
            .collect()
    }

    fn onsets(sr: u32, sens: Sensitivity, audio: &[f32], casts: &[f32]) -> Vec<u64> {
        let mut d = BiteAudioDetector::new(sr, sens);
        let mut out = Vec::new();
        let mut casts = casts.iter().peekable();
        for c in audio.chunks(sr as usize / 100) {
            if casts.peek().is_some_and(|&&t| d.now_ms() as f32 >= t * 1000.0) {
                casts.next();
                d.note_cast();
            }
            out.extend(d.push(c).into_iter().filter(|l| l.onset).map(|l| l.t_ms));
        }
        out
    }

    #[test]
    fn sustained_splash_fires_once_each() {
        let sr = 48_000;
        // 甩竿声（1 s 处，响）+ 三次水花（比甩竿声低 ~8 dB，持续 0.4 s）
        let bursts = [(1.25, 0.3, 0.5), (4.0, 0.4, 0.2), (8.5, 0.4, 0.2), (13.0, 0.4, 0.2)];
        let o = onsets(sr, Sensitivity::Normal, &clip(sr, 15.0, &bursts), &[1.0]);
        // 甩竿声本身也会触发（引擎在落水静默期内会忽略），之后每段水花各一次
        assert_eq!(o.len(), 4, "{o:?}");
        for (t, s) in o[1..].iter().zip([4.0, 8.5, 13.0]) {
            let dt = *t as f32 / 1000.0 - s;
            assert!((0.0..0.15).contains(&dt), "onset {t} vs splash {s}");
        }
    }

    #[test]
    fn short_ticks_and_quiet_sounds_never_fire() {
        let sr = 44_100;
        let mut bursts = vec![(1.25, 0.3, 0.5)];
        // 鱼游近的"咕噜"：响但只有 40 ms
        bursts.extend((0..20).map(|i| (3.0 + i as f32 * 0.5, 0.04, 0.3)));
        // 持续但小声（比甩竿声低 ~30 dB）
        bursts.push((14.0, 1.0, 0.015));
        let o = onsets(sr, Sensitivity::High, &clip(sr, 16.0, &bursts), &[1.0]);
        assert!(o.iter().all(|&t| t < 2_000), "{o:?}");
    }

    #[test]
    fn gate_follows_game_volume() {
        let sr = 48_000;
        // 整体音量调低 20 dB：甩竿声和水花一起变小，照样能检出
        let bursts = [(1.25, 0.3, 0.05), (5.0, 0.4, 0.02)];
        let o = onsets(sr, Sensitivity::Normal, &clip(sr, 8.0, &bursts), &[1.0]);
        assert!(o.iter().any(|&t| (5_000..5_150).contains(&t)), "{o:?}");
    }

    #[test]
    fn animal_calls_are_ignored_but_splash_still_fires() {
        let sr = 48_000;
        let mut a = clip(sr, 12.0, &[(1.25, 0.3, 0.5), (8.0, 0.4, 0.2)]);
        // "喵"：基频 600→800→600 Hz 滑动、到 12 kHz 的谐波，0.6 s，和水花差不多响
        let (s0, len) = (4.0f32, 0.6f32);
        let mut phase = 0.0f32;
        for (i, v) in a.iter_mut().enumerate() {
            let t = i as f32 / sr as f32;
            if t >= s0 && t < s0 + len {
                let u = (t - s0) / len;
                let f0 = 600.0 + 200.0 * (std::f32::consts::PI * u).sin();
                phase += 2.0 * std::f32::consts::PI * f0 / sr as f32;
                *v += (1..=20).map(|h| 0.06 * (h as f32 * phase).sin()).sum::<f32>();
            }
        }
        let o = onsets(sr, Sensitivity::Normal, &a, &[1.0]);
        assert!(!o.iter().any(|&t| (4_000..4_700).contains(&t)), "叫声不该触发：{o:?}");
        assert!(o.iter().any(|&t| (8_000..8_150).contains(&t)), "水花应触发：{o:?}");

        let mut d = BiteAudioDetector::new(sr, Sensitivity::Normal);
        d.set_tonal_max(None);
        let mut raw = Vec::new();
        for c in a.chunks(480) {
            raw.extend(d.push(c).into_iter().filter(|l| l.onset).map(|l| l.t_ms));
        }
        assert!(raw.iter().any(|&t| (4_000..4_700).contains(&t)), "不滤的话叫声会触发（测试本身有效）：{raw:?}");
    }

    #[test]
    fn downmix_stereo() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }
}
