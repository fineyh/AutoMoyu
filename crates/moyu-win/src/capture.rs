//! 屏幕区域截图（GDI BitBlt，和 v0.1 用的 mss 同一条路径，基岩版窗口化/无边框下已验证可用）。
//!
//! 不带 CAPTUREBLT，所以分层窗口（我们的游戏内浮层）不会被截进去；
//! 浮层另外还设了 WDA_EXCLUDEFROMCAPTURE，双保险。

use anyhow::{bail, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE};

use moyu_core::Grid;

/// 一块 BGRA 截图。
pub struct Bgra {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Bgra {
    pub fn to_grid(&self, k: usize) -> Grid {
        Grid::from_bgra(&self.data, self.w, self.h, self.w * 4, k)
    }
}

struct Surface {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    w: i32,
    h: i32,
}

/// 可复用的截图器：尺寸不变时复用同一块 DIB，每帧只做一次 BitBlt + 拷贝。
/// 不是 `Send`：谁用谁在自己的线程里创建。
pub struct ScreenCapture {
    surf: Option<Surface>,
}

impl Default for ScreenCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenCapture {
    pub fn new() -> Self {
        Self { surf: None }
    }

    fn ensure(&mut self, w: i32, h: i32) -> Result<&Surface> {
        if self.surf.as_ref().is_some_and(|s| s.w == w && s.h == h) {
            return Ok(self.surf.as_ref().unwrap());
        }
        self.release();
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // 自上而下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(screen), &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
            ReleaseDC(None, screen);
            let bmp = match bmp {
                Ok(b) => b,
                Err(e) => {
                    let _ = DeleteDC(dc);
                    bail!("CreateDIBSection 失败: {e}");
                }
            };
            let old = SelectObject(dc, bmp.into());
            self.surf = Some(Surface { dc, bmp, old, bits: bits as *mut u8, w, h });
        }
        Ok(self.surf.as_ref().unwrap())
    }

    fn release(&mut self) {
        if let Some(s) = self.surf.take() {
            unsafe {
                SelectObject(s.dc, s.old);
                let _ = DeleteObject(s.bmp.into());
                let _ = DeleteDC(s.dc);
            }
        }
    }

    /// 截屏幕上 (x, y, w, h) 这块（物理像素）。
    pub fn grab(&mut self, x: i32, y: i32, w: u32, h: u32) -> Result<Bgra> {
        if w == 0 || h == 0 {
            bail!("空区域");
        }
        let (w, h) = (w as i32, h as i32);
        let s = self.ensure(w, h)?;
        let (dc, bits) = (s.dc, s.bits);
        unsafe {
            let screen = GetDC(None);
            let r = BitBlt(dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
            ReleaseDC(None, screen);
            r?;
            let len = (w * h * 4) as usize;
            let data = std::slice::from_raw_parts(bits, len).to_vec();
            Ok(Bgra { w: w as usize, h: h as usize, data })
        }
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.release();
    }
}

/// 让某个窗口不被任何截图方式捕获（游戏内浮层用）。
pub fn exclude_from_capture(hwnd: isize, exclude: bool) -> bool {
    unsafe {
        SetWindowDisplayAffinity(HWND(hwnd as *mut _), if exclude { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE })
            .is_ok()
    }
}
