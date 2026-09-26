//! 声音咬钩检测：1–6 kHz 频带能量相对滚动底噪的突增。
//!
//! 咬钩时游戏会播放明显的水花声，和画面、光照、分辨率都无关。
//! 底噪 = 最近 5 秒频带能量的中位数；能量 > 底噪 × K 且在上升沿 → 咬钩。

use std::collections::VecDeque;
use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};

pub const FRAME: usize = 1024;
pub const HOP: usize = 512;
const BAND_HZ: (f32, f32) = (1_000.0, 6_000.0);
const FLOOR_WINDOW_MS: u64 = 5_000;
const REFRACTORY_MS: u64 = 600;
/// 绝对下限，避免完全静音时底噪≈0 导致任何小声音都触发。
const ABS_FLOOR: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Sensitivity {
    Low,
    #[default]
    Normal,
    High,
}

impl Sensitivity {
    pub fn k(self) -> f32 {
        match self {
            Sensitivity::Low => 12.0,
            Sensitivity::Normal => 7.0,
            Sensitivity::High => 4.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevel {
    pub t_ms: u64,
    /// 当前频带能量 / 底噪。
    pub ratio: f32,
    pub threshold: f32,
    pub onset: bool,
}

pub struct BiteAudioDetector {
    sample_rate: u32,
    k: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    buf: Vec<f32>,
    scratch_in: Vec<f32>,
    spectrum: Vec<realfft::num_complex::Complex<f32>>,
    band: (usize, usize),
    history: VecDeque<(u64, f32)>,
    prev_energy: f32,
    last_onset: Option<u64>,
    samples_seen: u64,
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
            k: sensitivity.k(),
            spectrum: fft.make_output_vec(),
            scratch_in: fft.make_input_vec(),
            fft,
            window,
            buf: Vec::with_capacity(FRAME * 2),
            band: (bin(BAND_HZ.0).max(1), bin(BAND_HZ.1)),
            history: VecDeque::new(),
            prev_energy: 0.0,
            last_onset: None,
            samples_seen: 0,
        }
    }

    pub fn set_sensitivity(&mut self, s: Sensitivity) {
        self.k = s.k();
    }

    /// 清空底噪历史（比如刚甩竿、换了场景）。
    pub fn reset(&mut self) {
        self.history.clear();
        self.prev_energy = 0.0;
        self.last_onset = None;
    }

    fn floor(&self) -> f32 {
        if self.history.is_empty() {
            return ABS_FLOOR;
        }
        let mut v: Vec<f32> = self.history.iter().map(|h| h.1).collect();
        let mid = v.len() / 2;
        v.select_nth_unstable_by(mid, f32::total_cmp);
        v[mid].max(ABS_FLOOR)
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
            self.buf.drain(..HOP);
            // 时间戳取本帧最后一个样本的时刻
            let t_ms = (self.samples_seen + FRAME as u64) * 1000 / self.sample_rate as u64;
            self.samples_seen += HOP as u64;
            if self.fft.process(&mut self.scratch_in, &mut self.spectrum).is_err() {
                continue;
            }
            let (b0, b1) = self.band;
            let energy: f32 =
                self.spectrum[b0..b1].iter().map(|c| c.norm_sqr()).sum::<f32>() / (b1 - b0).max(1) as f32;
            let floor = self.floor();
            let ratio = energy / floor;
            let warmed = self.history.len() >= 20;
            let rising = energy > self.prev_energy * 1.5;
            let refractory = self.last_onset.is_some_and(|t| t_ms.saturating_sub(t) < REFRACTORY_MS);
            let onset = warmed && ratio > self.k && rising && !refractory;
            if onset {
                self.last_onset = Some(t_ms);
            }
            // 所有帧都进底噪：水花只有一两百毫秒，动不了 5 秒窗口的中位数；
            // 而开始下雨这类持续变大的声音会被底噪慢慢吸收，不会一直误触发。
            self.history.push_back((t_ms, energy));
            while self.history.front().is_some_and(|h| t_ms.saturating_sub(h.0) > FLOOR_WINDOW_MS) {
                self.history.pop_front();
            }
            self.prev_energy = energy;
            out.push(AudioLevel { t_ms, ratio, threshold: self.k, onset });
        }
        out
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

    /// 背景：低频"音乐"+ 轻微白噪；在 splash_at 秒处加 150ms 高频水花。
    fn clip(sr: u32, secs: f32, splashes: &[f32]) -> Vec<f32> {
        let mut r = Rng(12345);
        (0..(sr as f32 * secs) as usize)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let mut v = 0.3 * (2.0 * std::f32::consts::PI * 220.0 * t).sin() + 0.01 * r.next();
                for &s in splashes {
                    if t >= s && t < s + 0.15 {
                        let env = 1.0 - (t - s) / 0.15;
                        v += 0.4 * env * r.next();
                    }
                }
                v
            })
            .collect()
    }

    #[test]
    fn detects_each_splash_once() {
        let sr = 48_000;
        let splashes = [3.0, 7.5, 12.0];
        let mut d = BiteAudioDetector::new(sr, Sensitivity::Normal);
        let onsets: Vec<u64> =
            clip(sr, 15.0, &splashes).chunks(480).flat_map(|c| d.push(c)).filter(|l| l.onset).map(|l| l.t_ms).collect();
        assert_eq!(onsets.len(), 3, "{onsets:?}");
        for (o, s) in onsets.iter().zip(splashes) {
            let dt = *o as f32 / 1000.0 - s;
            assert!((0.0..0.1).contains(&dt), "onset {o} vs splash {s}");
        }
    }

    #[test]
    fn music_alone_never_triggers() {
        let sr = 44_100;
        let mut d = BiteAudioDetector::new(sr, Sensitivity::High);
        let n = clip(sr, 20.0, &[]).chunks(441).flat_map(|c| d.push(c)).filter(|l| l.onset).count();
        assert_eq!(n, 0);
    }

    #[test]
    fn downmix_stereo() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }
}
