//! 游戏内浮层：透明、鼠标穿透、不抢焦点、不被截图捕获，盖在游戏客户区左上角。
//! 调试模式下铺满客户区，画出识别框。

use std::sync::Mutex;

use moyu_win::GameWindow;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "overlay";
/// 胶囊浮层的 CSS 尺寸。
const PILL: (f64, f64) = (340.0, 52.0);

static LAST: Mutex<Option<Option<(i32, i32, u32, u32)>>> = Mutex::new(None);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=overlay".into()))
        .title("AutoMoyu Overlay")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .inner_size(PILL.0, PILL.1)
        .build()?;
    win.set_ignore_cursor_events(true)?;
    if let Ok(h) = win.hwnd() {
        let h = h.0 as isize;
        moyu_win::window::make_overlay(h);
        if !moyu_win::capture::exclude_from_capture(h, true) {
            tracing::warn!("浮层无法设置为不被截图（系统版本过旧？）");
        }
    }
    Ok(())
}

fn hwnd(app: &AppHandle) -> Option<(isize, f64)> {
    let w = app.get_webview_window(LABEL)?;
    let scale = w.scale_factor().unwrap_or(1.0);
    Some((w.hwnd().ok()?.0 as isize, scale))
}

/// 每帧调用：`target` 为 `None` 时隐藏。
pub fn sync(app: &AppHandle, target: Option<&GameWindow>, debug: bool) {
    let Some((h, scale)) = hwnd(app) else { return };
    let rect = target.map(|g| {
        if debug {
            (g.x, g.y, g.w, g.h)
        } else {
            let pad = (16.0 * scale) as i32;
            (g.x + pad, g.y + pad, (PILL.0 * scale) as u32, (PILL.1 * scale) as u32)
        }
    });
    let mut last = LAST.lock().unwrap();
    if *last != Some(rect) {
        moyu_win::window::place_noactivate(h, rect);
        *last = Some(rect);
    }
}

pub fn hide(app: &AppHandle) {
    sync(app, None, false);
}
