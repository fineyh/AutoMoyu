//! 录像目录格式（Phase 0 fixtures）。
//!
//! ```text
//! meta.json         Meta
//! frames.jsonl      每帧一行 FrameRec
//! union/000123.png  候选区（手持竿 + 快捷栏外包矩形）分析网格，RGB8
//! center/000123.png 画面中上部（浮漂区）分析网格，RGB8
//! thumbs/000123.png 整个客户区缩略图（每 2 秒一张，给人看）
//! audio.wav         游戏声音，单声道 16 bit
//! events.jsonl      鼠标右键：InputRec
//! ```

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;

use anyhow::{Context, Result};
use moyu_core::{CellRect, Grid};
use serde::{Deserialize, Serialize};

use crate::Scenario;

pub const CENTER_AREA: moyu_core::RelRect = moyu_core::RelRect::new(0.3, 0.05, 0.4, 0.6);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub version: u32,
    pub scenario: Scenario,
    pub started_at: String,
    pub fps: u32,
    pub client_w: u32,
    pub client_h: u32,
    pub k: u32,
    pub union: CellRect,
    pub center: CellRect,
    pub window_title: String,
    pub window_process: String,
    pub audio_source: Option<String>,
    pub audio_rate: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct FrameRec {
    pub i: u32,
    pub t_ms: u64,
    pub foreground: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InputRec {
    pub t_ms: u64,
    /// "rdown" | "rup"
    pub kind: String,
    pub injected: bool,
}

pub fn save_png(path: &Path, g: &Grid) -> Result<()> {
    let f = BufWriter::new(File::create(path)?);
    let mut enc = png::Encoder::new(f, g.w as u32, g.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    enc.write_header()?.write_image_data(&g.to_rgb8())?;
    Ok(())
}

pub fn load_png(path: &Path) -> Result<Grid> {
    let dec = png::Decoder::new(BufReader::new(File::open(path).with_context(|| format!("{}", path.display()))?));
    let mut r = dec.read_info()?;
    let mut buf = vec![0; r.output_buffer_size().unwrap_or(0)];
    let info = r.next_frame(&mut buf)?;
    anyhow::ensure!(info.color_type == png::ColorType::Rgb, "需要 RGB PNG");
    Ok(Grid::from_rgb8(&buf[..info.buffer_size()], info.width as usize, info.height as usize))
}

pub fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    BufReader::new(File::open(path)?)
        .lines()
        .filter(|l| l.as_ref().map_or(true, |s| !s.trim().is_empty()))
        .map(|l| Ok(serde_json::from_str(&l?)?))
        .collect()
}
