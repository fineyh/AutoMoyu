//! 校准结果：`calibration/<宽>x<高>.json` + 模板/预览 PNG（诊断包里也会带上）。

use std::io::Cursor;
use std::path::PathBuf;

use anyhow::Result;
use base64::Engine as _;
use moyu_core::{CalibrationResult, Grid, RodState};
use serde::{Deserialize, Serialize};

use crate::settings::data_dir;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub created_at: String,
    pub result: CalibrationResult,
}

pub fn dir() -> PathBuf {
    data_dir().join("calibration")
}

fn path(w: u32, h: u32) -> PathBuf {
    dir().join(format!("{w}x{h}.json"))
}

pub fn load(w: u32, h: u32) -> Option<Stored> {
    let s = std::fs::read_to_string(path(w, h)).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn save(result: &CalibrationResult, preview: Option<&Grid>) -> Result<Stored> {
    std::fs::create_dir_all(dir())?;
    let stored = Stored { created_at: chrono::Local::now().to_rfc3339(), result: result.clone() };
    let (w, h) = (result.client_w, result.client_h);
    std::fs::write(path(w, h), serde_json::to_string(&stored)?)?;
    let m = &result.model;
    for (state, name) in [(RodState::In, "in"), (RodState::Out, "out")] {
        let rgb = m.template_rgb8(state);
        std::fs::write(dir().join(format!("{w}x{h}-{name}.png")), encode_png(&rgb, m.w, m.h)?)?;
    }
    if let Some(p) = preview {
        std::fs::write(dir().join(format!("{w}x{h}-preview.png")), encode_png(&p.to_rgb8(), p.w, p.h)?)?;
    }
    Ok(stored)
}

pub fn clear() -> Result<()> {
    if dir().exists() {
        std::fs::remove_dir_all(dir())?;
    }
    Ok(())
}

pub fn encode_png(rgb: &[u8], w: usize, h: usize) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(Cursor::new(&mut buf), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(rgb)?;
    }
    Ok(buf)
}

pub fn data_url(g: &Grid) -> Option<String> {
    let png = encode_png(&g.to_rgb8(), g.w, g.h).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}
