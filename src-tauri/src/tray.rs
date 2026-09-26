//! 托盘：图标颜色表示状态；菜单：开始/暂停、显示主窗口、退出。

use std::sync::{Mutex, OnceLock};

use moyu_core::engine::Phase;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::service::Cmd;
use crate::AppState;

const ID: &str = "main";
static TOGGLE: OnceLock<MenuItem<Wry>> = OnceLock::new();
static LAST: Mutex<Option<(u8, bool)>> = Mutex::new(None);

fn icon(kind: u8) -> Image<'static> {
    let bytes: &[u8] = match kind {
        1 => include_bytes!("../icons/tray/running.png"),
        2 => include_bytes!("../icons/tray/paused.png"),
        _ => include_bytes!("../icons/tray/idle.png"),
    };
    Image::from_bytes(bytes).expect("tray icon")
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "开始钓鱼\tF6", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &show, &PredefinedMenuItem::separator(app)?, &quit])?;
    let _ = TOGGLE.set(toggle);
    TrayIconBuilder::with_id(ID)
        .icon(icon(0))
        .tooltip("AutoMoyu")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "toggle" => {
                if let Some(s) = app.state::<AppState>().service.get() {
                    s.send(Cmd::Hotkey);
                }
            }
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 状态变了才更新（服务线程每帧都会调）。
pub fn update(app: &AppHandle, phase: Phase, running: bool) {
    let kind = match (running, phase) {
        (false, _) => 0u8,
        (true, Phase::Paused) => 2,
        (true, _) => 1,
    };
    {
        let mut last = LAST.lock().unwrap();
        if *last == Some((kind, running)) {
            return;
        }
        *last = Some((kind, running));
    }
    if let Some(t) = app.tray_by_id(ID) {
        let _ = t.set_icon(Some(icon(kind)));
        let tip = match kind {
            1 => "AutoMoyu · 钓鱼中",
            2 => "AutoMoyu · 已暂停",
            _ => "AutoMoyu",
        };
        let _ = t.set_tooltip(Some(tip));
    }
    if let Some(m) = TOGGLE.get() {
        let _ = m.set_text(match kind {
            1 => "暂停钓鱼\tF6",
            2 => "继续钓鱼\tF6",
            _ => "开始钓鱼\tF6",
        });
    }
}
