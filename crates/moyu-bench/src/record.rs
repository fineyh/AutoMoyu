//! Phase 0 录制：你正常钓鱼，程序只看不点。
//!
//! 记下候选区/浮漂区的分析网格、游戏声音、你自己的右键（人点的，不含注入的），
//! 之后用 `analyze` 离线评估，所有算法改动都拿这些真实片段回归。

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use moyu_core::{Geometry, Grid};
use moyu_win::hook::{Button, MouseHook};
use moyu_win::{window, ScreenCapture};

use crate::fixture::{save_png, FrameRec, InputRec, Meta, CENTER_AREA};
use crate::Scenario;

pub fn run(dir: &Path, scenario: Scenario, fps: u32, minutes: u32) -> Result<()> {
    let Some(mut w) = window::find_minecraft() else { bail!("没找到 Minecraft 窗口，先打开游戏") };
    if w.w == 0 || w.h == 0 || w.minimized {
        bail!("游戏窗口最小化了");
    }
    for sub in ["union", "center", "thumbs"] {
        fs::create_dir_all(dir.join(sub))?;
    }
    let geom = Geometry::new(w.w, w.h);
    let k = geom.k;
    let center = CENTER_AREA.to_cells(geom.grid_w, geom.grid_h);

    let stop = Arc::new(AtomicBool::new(false));
    {
        let s = stop.clone();
        ctrlc::set_handler(move || s.store(true, Ordering::SeqCst)).context("Ctrl+C 处理")?;
    }

    // 声音
    let (atx, arx) = mpsc::channel::<Vec<f32>>();
    let audio = match moyu_win::audio::AudioCapture::start(Some(w.pid), atx) {
        Ok(a) => Some(a),
        Err(e) => {
            println!("声音采集不可用（{e}），只录画面。");
            None
        }
    };
    let meta = Meta {
        version: 1,
        scenario,
        started_at: chrono::Local::now().to_rfc3339(),
        fps,
        client_w: w.w,
        client_h: w.h,
        k,
        union: geom.union,
        center,
        window_title: w.title.clone(),
        window_process: w.process.clone(),
        audio_source: audio.as_ref().map(|a| format!("{:?}", a.source)),
        audio_rate: audio.as_ref().map(|a| a.sample_rate),
    };
    fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;
    let wav_spec = hound::WavSpec {
        channels: 1,
        sample_rate: moyu_win::audio::SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = match audio {
        Some(_) => Some(hound::WavWriter::create(dir.join("audio.wav"), wav_spec)?),
        None => None,
    };

    // 鼠标
    let (_hook, mrx) = MouseHook::install()?;

    // PNG 编码放到后台线程
    let (jtx, jrx) = mpsc::sync_channel::<(PathBuf, Grid)>(256);
    let writer = std::thread::spawn(move || {
        for (p, g) in jrx {
            if let Err(e) = save_png(&p, &g) {
                eprintln!("写 {} 失败: {e}", p.display());
            }
        }
    });

    let mut frames = BufWriter::new(File::create(dir.join("frames.jsonl"))?);
    let mut events = BufWriter::new(File::create(dir.join("events.jsonl"))?);
    let t0 = Instant::now();
    let period = Duration::from_millis(1000 / fps.max(1) as u64);
    let end = t0 + Duration::from_secs(minutes as u64 * 60);
    let mut cap = ScreenCapture::new();
    let mut i = 0u32;
    let mut last_thumb: Option<Instant> = None;
    let mut last_refresh = Instant::now();
    let (mut clicks, mut audio_samples) = (0u32, 0usize);

    println!("开始录制 -> {}（{}x{}，k={}，{fps} fps）。正常钓鱼即可，按 Ctrl+C 结束。", dir.display(), w.w, w.h, k);
    println!("场景：{scenario:?}。最好白天、夜晚各钓一会儿；中途别改窗口大小。");
    while !stop.load(Ordering::SeqCst) && Instant::now() < end {
        let tick = Instant::now();
        if last_refresh.elapsed() > Duration::from_secs(1) {
            last_refresh = Instant::now();
            match window::refresh(w.hwnd) {
                Some(nw) if nw.w == meta.client_w && nw.h == meta.client_h => w = nw,
                Some(nw) if nw.minimized => w = nw,
                Some(_) => {
                    println!("窗口尺寸变了，停止录制（同一段录像必须同一尺寸）。");
                    break;
                }
                None => {
                    println!("游戏窗口没了，停止录制。");
                    break;
                }
            }
        }
        let fg = window::is_foreground(w.hwnd) && !w.minimized;
        if fg {
            let (ux, uy, uw, uh) = geom.union.to_px(k);
            if let Ok(img) = cap.grab(w.x + ux as i32, w.y + uy as i32, uw, uh) {
                let _ = jtx.send((dir.join(format!("union/{i:06}.png")), img.to_grid(k as usize)));
            }
            let (cx, cy, cw, ch) = center.to_px(k);
            if let Ok(img) = cap.grab(w.x + cx as i32, w.y + cy as i32, cw, ch) {
                let _ = jtx.send((dir.join(format!("center/{i:06}.png")), img.to_grid(k as usize)));
            }
            if last_thumb.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                last_thumb = Some(Instant::now());
                if let Ok(img) = cap.grab(w.x, w.y, w.w, w.h) {
                    let _ = jtx.send((dir.join(format!("thumbs/{i:06}.png")), img.to_grid(k as usize * 2)));
                }
            }
        }
        let t_ms = tick.duration_since(t0).as_millis() as u64;
        writeln!(frames, "{}", serde_json::to_string(&FrameRec { i, t_ms, foreground: fg })?)?;
        i += 1;

        while let Ok(m) = mrx.try_recv() {
            if m.injected {
                continue;
            }
            let down = m.button == Button::RightDown;
            clicks += down as u32;
            let rec = InputRec {
                t_ms: m.at.saturating_duration_since(t0).as_millis() as u64,
                kind: if down { "rdown" } else { "rup" }.into(),
                injected: m.injected,
            };
            writeln!(events, "{}", serde_json::to_string(&rec)?)?;
        }
        if let Some(wv) = wav.as_mut() {
            while let Ok(chunk) = arx.try_recv() {
                audio_samples += chunk.len();
                for s in chunk {
                    wv.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
                }
            }
            // 进程环回在游戏静音时不送数据：补零，让 wav 的时间轴和画面对齐
            let sr = moyu_win::audio::SAMPLE_RATE as f64;
            let expected = (t0.elapsed().as_secs_f64() * sr) as usize;
            if expected > audio_samples + (0.25 * sr) as usize {
                let pad = expected - audio_samples - (0.05 * sr) as usize;
                for _ in 0..pad {
                    wv.write_sample(0i16)?;
                }
                audio_samples += pad;
            }
        }
        if i % (fps * 10).max(1) == 0 {
            println!(
                "  {:>4}s · {i} 帧 · 右键 {clicks} 次 · 声音 {:.1}s{}",
                t0.elapsed().as_secs(),
                audio_samples as f32 / moyu_win::audio::SAMPLE_RATE as f32,
                if fg { "" } else { " · 游戏不在前台（不录画面）" }
            );
        }
        if let Some(rest) = period.checked_sub(tick.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    drop(jtx);
    writer.join().ok();
    frames.flush()?;
    events.flush()?;
    if let Some(wv) = wav {
        wv.finalize()?;
    }
    println!(
        "录完：{i} 帧，右键 {clicks} 次，用时 {} 秒。下一步：moyu-bench analyze {}",
        t0.elapsed().as_secs(),
        dir.display()
    );
    Ok(())
}
