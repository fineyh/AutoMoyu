//! 冒烟检查：列窗口 → 找 Minecraft → 截客户区 → 听 2 秒游戏声音。

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use moyu_win::{window, ScreenCapture};

pub fn run(out: &Path) -> Result<()> {
    let all = window::list_windows();
    println!("可见窗口 {} 个。", all.len());
    let Some(w) = window::find_minecraft() else {
        println!("没找到 Minecraft。候选：");
        for w in all.iter().take(15) {
            println!("  [{}] {} · {} · {}x{}", w.pid, w.process, w.title, w.w, w.h);
        }
        return Ok(());
    };
    println!(
        "找到：{} · 进程 {} (pid {}) · 客户区 {}x{} @ ({}, {}) · 前台 {}",
        w.title,
        w.process,
        w.pid,
        w.w,
        w.h,
        w.x,
        w.y,
        window::is_foreground(w.hwnd)
    );
    if w.minimized {
        println!("窗口最小化了，截不到画面。");
        return Ok(());
    }
    let mut cap = ScreenCapture::new();
    let t = Instant::now();
    let img = cap.grab(w.x, w.y, w.w, w.h)?;
    let dt = t.elapsed();
    let g = img.to_grid(2);
    crate::fixture::save_png(out, &g)?;
    let luma = g.mean_luma();
    println!("截图 {:.1} ms，平均亮度 {:.0}，已存 {}", dt.as_secs_f64() * 1e3, luma, out.display());
    if luma < 3.0 {
        println!("注意：画面几乎全黑，游戏可能是独占全屏，请改成窗口化或无边框。");
    }

    let (tx, rx) = std::sync::mpsc::channel();
    match moyu_win::audio::AudioCapture::start(Some(w.pid), tx) {
        Ok(a) => {
            let end = Instant::now() + Duration::from_secs(2);
            let (mut n, mut e) = (0usize, 0f64);
            while Instant::now() < end {
                if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
                    n += chunk.len();
                    e += chunk.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
                }
            }
            let rms = if n > 0 { (e / n as f64).sqrt() } else { 0.0 };
            println!("声音：{:?} · {} Hz · 2 秒收到 {} 个样本 · RMS {:.4}", a.source, a.sample_rate, n, rms);
        }
        Err(e) => println!("声音采集失败：{e}"),
    }
    Ok(())
}
