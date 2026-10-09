//! 列出一段录像里声音检测器的每次触发，带音调性，用来看误触是什么声音、定过滤门槛。
//!
//! 甩竿/收竿按右键交替推（没有人手右键就用注入的右键，即 AutoMoyu 全自动自己点的）。
//! 每次触发标一个类别：
//! - `hit`：落在某次收竿前 [-1200, +150] ms（人手标注 = 真咬钩；注入 = 程序因它收了竿）
//! - `wait`：在甩竿 1.5 s 后、收竿前 1.2 s 之前（等鱼期间的误触）
//! - `other`：甩竿声、收竿后的声音等，引擎本来就会忽略

use std::path::Path;

use anyhow::{Context, Result};
use moyu_core::bite_audio::{BiteAudioDetector, Sensitivity};

use crate::fixture::{read_jsonl, InputRec};

pub fn run(dir: &Path, wav: &str, tonal_max: Option<f32>) -> Result<()> {
    let ev: Vec<InputRec> = read_jsonl(&dir.join("events.jsonl"))?;
    let human: Vec<u64> = ev.iter().filter(|e| e.kind == "rdown" && !e.injected).map(|e| e.t_ms).collect();
    let injected: Vec<u64> = ev.iter().filter(|e| e.kind == "rdown" && e.injected).map(|e| e.t_ms).collect();
    let (clicks, src) = if human.is_empty() { (injected, "注入") } else { (human, "人手") };
    let casts: Vec<u64> = clicks.iter().step_by(2).copied().collect();
    let reels: Vec<u64> = clicks.iter().skip(1).step_by(2).copied().collect();

    let mut r = hound::WavReader::open(dir.join(wav)).with_context(|| wav.to_string())?;
    let sr = r.spec().sample_rate;
    let samples: Vec<f32> =
        r.samples::<i16>().map(|s| s.map(|v| v as f32 / i16::MAX as f32)).collect::<Result<_, _>>()?;

    let mut d = BiteAudioDetector::new(sr, Sensitivity::Normal);
    d.set_tonal_max(tonal_max);
    let mut next_cast = casts.iter().peekable();
    let mut recent = std::collections::VecDeque::new();
    println!("{src}右键 {} 次（甩 {} / 收 {}），音调性门槛 {tonal_max:?}", clicks.len(), casts.len(), reels.len());
    println!("{:>8} {:>8} {:>7} {:>7} {:>6} {:>6}  类别", "开始ms", "触发ms", "电平", "门槛", "音调", "最大");
    let (mut n_hit, mut n_wait) = (0, 0);
    for chunk in samples.chunks(sr as usize / 100) {
        if next_cast.peek().is_some_and(|&&c| d.now_ms() >= c) {
            next_cast.next();
            d.note_cast();
        }
        for l in d.push(chunk) {
            recent.push_back(l);
            if recent.len() > 8 {
                recent.pop_front();
            }
            let Some(start) = l.onset_start_ms else { continue };
            let tonals: Vec<f32> = recent.iter().map(|x| x.tonal).collect();
            let mean = tonals.iter().sum::<f32>() / tonals.len() as f32;
            let max = tonals.iter().copied().fold(0.0, f32::max);
            let kind = if let Some(i) = reels.iter().position(|&b| start + 1_200 >= b && start <= b + 150) {
                n_hit += 1;
                format!("hit（第 {} 次收竿）", i + 1)
            } else if casts.iter().zip(&reels).any(|(&c, &b)| start >= c + 1_500 && start + 1_200 < b) {
                n_wait += 1;
                "wait".into()
            } else {
                "other".into()
            };
            println!("{start:>8} {:>8} {:>7.1} {:>7.1} {mean:>6.3} {max:>6.3}  {kind}", l.t_ms, l.level_db, l.gate_db);
        }
    }
    println!("合计：hit {n_hit} / {}，wait {n_wait}", reels.len());
    Ok(())
}
