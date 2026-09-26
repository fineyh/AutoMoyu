//! 分析网格：把截图按 k×k 盒式滤波缩小成 RGB f32 网格。
//!
//! 校准和运行时用**同一个整数缩放因子 k、同样对齐到 k 的整数倍的矩形**，
//! 所以校准时从大图里裁出的小框，和运行时单独截这个小框得到的数值完全一致。

use serde::{Deserialize, Serialize};

/// 分析网格目标宽度：客户区宽度 / k ≈ 480。
pub const ANALYSIS_WIDTH: u32 = 480;

/// 由客户区宽度决定整数缩放因子 k（1920 → 4，2560 → 5，1280 → 3）。
pub fn analysis_scale(client_w: u32) -> u32 {
    ((client_w as f32 / ANALYSIS_WIDTH as f32).round() as u32).max(1)
}

/// 以网格格子为单位的矩形。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl CellRect {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// 换算成客户区像素矩形 (x, y, w, h)。
    pub fn to_px(self, k: u32) -> (u32, u32, u32, u32) {
        (self.x * k, self.y * k, self.w * k, self.h * k)
    }

    /// 相对于 `outer` 左上角的偏移。
    pub fn relative_to(self, outer: CellRect) -> CellRect {
        CellRect::new(self.x - outer.x, self.y - outer.y, self.w, self.h)
    }
}

/// 相对客户区的比例矩形（0..1），给界面画框用。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RelRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// 转成网格格子矩形（向内取整，保证不越界）。
    pub fn to_cells(self, grid_w: u32, grid_h: u32) -> CellRect {
        let x0 = (self.x * grid_w as f32).ceil() as u32;
        let y0 = (self.y * grid_h as f32).ceil() as u32;
        let x1 = (((self.x + self.w) * grid_w as f32).floor() as u32).min(grid_w);
        let y1 = (((self.y + self.h) * grid_h as f32).floor() as u32).min(grid_h);
        CellRect::new(x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
    }

    pub fn from_cells(r: CellRect, grid_w: u32, grid_h: u32) -> Self {
        Self::new(
            r.x as f32 / grid_w as f32,
            r.y as f32 / grid_h as f32,
            r.w as f32 / grid_w as f32,
            r.h as f32 / grid_h as f32,
        )
    }
}

/// RGB 网格，数值范围 0..255。
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 3]>,
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[0.0; 3]; w * h] }
    }

    pub fn filled(w: usize, h: usize, c: [f32; 3]) -> Self {
        Self { w, h, px: vec![c; w * h] }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> [f32; 3] {
        self.px[y * self.w + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: [f32; 3]) {
        self.px[y * self.w + x] = c;
    }

    /// 从 BGRA8 缓冲区（stride 为每行字节数）做 k×k 盒式滤波。
    /// 输出尺寸 = floor(width/k) × floor(height/k)。
    pub fn from_bgra(buf: &[u8], width: usize, height: usize, stride: usize, k: usize) -> Self {
        let k = k.max(1);
        let gw = width / k;
        let gh = height / k;
        let mut g = Grid::new(gw, gh);
        let inv = 1.0 / (k * k) as f32;
        let mut acc = vec![[0u32; 3]; gw];
        for gy in 0..gh {
            acc.iter_mut().for_each(|a| *a = [0; 3]);
            for dy in 0..k {
                let row = &buf[(gy * k + dy) * stride..];
                for (gx, a) in acc.iter_mut().enumerate() {
                    let base = gx * k * 4;
                    for dx in 0..k {
                        let p = base + dx * 4;
                        a[0] += row[p + 2] as u32; // R
                        a[1] += row[p + 1] as u32; // G
                        a[2] += row[p] as u32; // B
                    }
                }
            }
            for (gx, a) in acc.iter().enumerate() {
                g.set(gx, gy, [a[0] as f32 * inv, a[1] as f32 * inv, a[2] as f32 * inv]);
            }
        }
        g
    }

    /// 从 RGB8 紧凑缓冲区构造（不缩放），录像回放/测试用。
    pub fn from_rgb8(buf: &[u8], w: usize, h: usize) -> Self {
        let px = buf
            .chunks_exact(3)
            .take(w * h)
            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
            .collect();
        Self { w, h, px }
    }

    /// 转回 RGB8（诊断截图用）。
    pub fn to_rgb8(&self) -> Vec<u8> {
        self.px
            .iter()
            .flat_map(|c| c.map(|v| v.round().clamp(0.0, 255.0) as u8))
            .collect()
    }

    /// 裁出一块（格子坐标，调用方保证不越界）。
    pub fn crop(&self, r: CellRect) -> Grid {
        let (x0, y0, w, h) = (r.x as usize, r.y as usize, r.w as usize, r.h as usize);
        let mut px = Vec::with_capacity(w * h);
        for y in y0..y0 + h {
            px.extend_from_slice(&self.px[y * self.w + x0..y * self.w + x0 + w]);
        }
        Grid { w, h, px }
    }

    pub fn mean_luma(&self) -> f32 {
        if self.px.is_empty() {
            return 0.0;
        }
        let s: f32 = self.px.iter().map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]).sum();
        s / self.px.len() as f32
    }

    /// 亮度归一化：整体乘一个增益让平均亮度≈128（最多放大 8 倍），
    /// 抵消昼夜/天气带来的整体明暗变化，只保留"图案"。
    pub fn normalized(&self) -> Grid {
        let gain = 128.0 / self.mean_luma().max(16.0);
        Grid { w: self.w, h: self.h, px: self.px.iter().map(|c| c.map(|v| v * gain)).collect() }
    }
}

/// 平均绝对差（逐像素逐通道）。两图尺寸必须一致。
pub fn mad(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    if a.is_empty() {
        return 0.0;
    }
    let mut s = 0.0f32;
    for (p, q) in a.iter().zip(b) {
        s += (p[0] - q[0]).abs() + (p[1] - q[1]).abs() + (p[2] - q[2]).abs();
    }
    s / (a.len() * 3) as f32
}

/// 多张同尺寸网格的逐像素平均。
pub fn mean_of(grids: &[Grid]) -> Grid {
    let first = &grids[0];
    let mut out = Grid::new(first.w, first.h);
    for g in grids {
        for (o, p) in out.px.iter_mut().zip(&g.px) {
            o[0] += p[0];
            o[1] += p[1];
            o[2] += p[2];
        }
    }
    let inv = 1.0 / grids.len() as f32;
    out.px.iter_mut().for_each(|o| *o = o.map(|v| v * inv));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_matches_common_resolutions() {
        assert_eq!(analysis_scale(1920), 4);
        assert_eq!(analysis_scale(2560), 5);
        assert_eq!(analysis_scale(1280), 3);
        assert_eq!(analysis_scale(800), 2);
        assert_eq!(analysis_scale(100), 1);
    }

    #[test]
    fn crop_of_big_capture_equals_small_capture() {
        // 同一块屏幕：整块截图后裁出 vs 只截那一小块，结果必须一模一样。
        let (w, h, k) = (40usize, 24usize, 4usize);
        let mut buf = vec![0u8; w * h * 4];
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((i * 37) % 251) as u8;
        }
        let big = Grid::from_bgra(&buf, w, h, w * 4, k);
        let r = CellRect::new(3, 2, 5, 3);
        let (px, py, pw, ph) = r.to_px(k as u32);
        let mut small = Vec::new();
        for y in py..py + ph {
            let s = (y as usize * w + px as usize) * 4;
            small.extend_from_slice(&buf[s..s + pw as usize * 4]);
        }
        let g2 = Grid::from_bgra(&small, pw as usize, ph as usize, pw as usize * 4, k);
        assert_eq!(big.crop(r), g2);
    }

    #[test]
    fn normalization_cancels_global_gain() {
        let mut a = Grid::new(4, 4);
        for (i, p) in a.px.iter_mut().enumerate() {
            *p = [(i * 10) as f32 + 40.0, 80.0, 60.0];
        }
        let dark = Grid { w: 4, h: 4, px: a.px.iter().map(|c| c.map(|v| v * 0.5)).collect() };
        assert!(mad(&a.normalized().px, &dark.normalized().px) < 1e-3);
    }

    #[test]
    fn rel_to_cells_stays_inside() {
        let r = RelRect::new(0.5, 0.45, 0.5, 0.55).to_cells(480, 270);
        assert!(r.x + r.w <= 480 && r.y + r.h <= 270);
        assert_eq!(r.x, 240);
    }
}
