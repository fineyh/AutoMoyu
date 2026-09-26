//! 托盘：图标颜色表示状态；菜单：开始/暂停、显示主窗口、退出。

use std::sync::{Mutex, OnceLock};

use moyu_core::engine::Phase;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::i18n::t;
use crate::service::Cmd;
use crate::AppState;

const ID: &str = "main";
static TOGGLE: OnceLock<MenuItem<Wry>> = OnceLock::new();
static SHOW: OnceLock<MenuItem<Wry>> = OnceLock::new();
static QUIT: OnceLock<MenuItem<Wry>> = OnceLock::new();
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
    let toggle = MenuItem::with_id(app, "toggle", toggle_text(0), true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", show_text(), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_text(), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &show, &PredefinedMenuItem::separator(app)?, &quit])?;
    let _ = TOGGLE.set(toggle);
    let _ = SHOW.set(show);
    let _ = QUIT.set(quit);
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

fn toggle_text(kind: u8) -> &'static str {
    match kind {
        1 => t("暂停钓鱼\tF6", "Pause fishing\tF6"),
        2 => t("继续钓鱼\tF6", "Resume fishing\tF6"),
        _ => t("开始钓鱼\tF6", "Start fishing\tF6"),
    }
}
fn show_text() -> &'static str {
    t("显示主窗口", "Show window")
}
fn quit_text() -> &'static str {
    t("退出", "Quit")
}

/// 切换语言后重设菜单文字；开始/暂停项和提示在下一帧 `update` 里刷新。
pub fn relabel() {
    *LAST.lock().unwrap() = None;
    if let Some(m) = SHOW.get() {
        let _ = m.set_text(show_text());
    }
    if let Some(m) = QUIT.get() {
        let _ = m.set_text(quit_text());
    }
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
    if let Some(tray) = app.tray_by_id(ID) {
        let _ = tray.set_icon(Some(icon(kind)));
        let tip = match kind {
            1 => t("AutoMoyu · 钓鱼中", "AutoMoyu · Fishing"),
            2 => t("AutoMoyu · 已暂停", "AutoMoyu · Paused"),
            _ => "AutoMoyu",
        };
        let _ = tray.set_tooltip(Some(tip));
    }
    if let Some(m) = TOGGLE.get() {
        let _ = m.set_text(toggle_text(kind));
    }
}
