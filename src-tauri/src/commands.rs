//! 前端 → Rust 的命令。耗时操作都在服务线程里做，这里只转发或读快照。

use std::path::PathBuf;

use moyu_win::GameWindow;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::service::{Cmd, Status};
use crate::settings::{Settings, WindowMatch};
use crate::stats::StatsView;
use crate::{hotkeys, AppState};

type R<T> = Result<T, String>;

fn svc(state: &State<'_, AppState>, c: Cmd) {
    if let Some(s) = state.service.get() {
        s.send(c);
    }
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Option<Status> {
    state.service.get().map(|s| s.snapshot())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<'_, AppState>, patch: Value) -> R<Settings> {
    let (old, new) = {
        let mut cur = state.settings.lock().unwrap();
        let old = cur.clone();
        let new = cur.merged(&patch).map_err(|e| e.to_string())?;
        new.save().map_err(|e| e.to_string())?;
        *cur = new.clone();
        (old, new)
    };
    if old.hotkeys != new.hotkeys {
        if let Err(e) = hotkeys::register(&app, &new.hotkeys) {
            // 热键被占用：退回旧的
            let _ = hotkeys::register(&app, &old.hotkeys);
            let mut cur = state.settings.lock().unwrap();
            cur.hotkeys = old.hotkeys.clone();
            let _ = cur.save();
            return Err(if crate::i18n::is_en() {
                format!("Couldn't register the hotkey (another app may be using it): {e}")
            } else {
                format!("热键注册失败（可能被其他程序占用）：{e}")
            });
        }
    }
    if old.ui.language != new.ui.language {
        crate::i18n::apply(new.ui.language);
        crate::tray::relabel();
    }
    if old.ui.always_on_top != new.ui.always_on_top {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_always_on_top(new.ui.always_on_top);
        }
    }
    svc(&state, Cmd::Settings(Box::new(new.clone())));
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

#[tauri::command]
pub fn hotkey(state: State<'_, AppState>) {
    svc(&state, Cmd::Hotkey);
}

#[tauri::command]
pub fn start(state: State<'_, AppState>) {
    svc(&state, Cmd::Start);
}

#[tauri::command]
pub fn toggle_pause(state: State<'_, AppState>) {
    svc(&state, Cmd::TogglePause);
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) {
    svc(&state, Cmd::Stop);
}

#[tauri::command]
pub fn calibration_open(state: State<'_, AppState>) {
    svc(&state, Cmd::CalOpen);
}

#[tauri::command]
pub fn calibration_start(state: State<'_, AppState>) {
    svc(&state, Cmd::CalStart);
}

#[tauri::command]
pub fn calibration_cancel(state: State<'_, AppState>) {
    svc(&state, Cmd::CalCancel);
}

#[tauri::command]
pub fn calibration_reset(state: State<'_, AppState>) {
    svc(&state, Cmd::CalReset);
}

#[tauri::command]
pub fn focus_game(state: State<'_, AppState>) {
    svc(&state, Cmd::FocusGame);
}

/// 打开 Windows 声音设置（关空间音效用）。
#[tauri::command]
pub fn open_sound_settings(app: AppHandle) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url("ms-settings:sound", None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_game_windows() -> Vec<GameWindow> {
    let mut v = moyu_win::window::list_windows();
    v.retain(|w| w.w >= 320 && w.h >= 200);
    v.sort_by_key(|w| std::cmp::Reverse((w.is_minecraft(), w.w as u64 * w.h as u64)));
    v
}

#[tauri::command]
pub fn pick_window(app: AppHandle, state: State<'_, AppState>, pick: Option<WindowMatch>) -> R<Settings> {
    let patch = serde_json::json!({ "advanced": { "window": pick } });
    let s = set_settings(app, state.clone(), patch)?;
    svc(&state, Cmd::PickWindow(s.advanced.window.clone()));
    Ok(s)
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>, days: Option<u32>) -> R<StatsView> {
    state.stats.lock().unwrap().view(days.unwrap_or(7)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_stats_csv(state: State<'_, AppState>, path: PathBuf) -> R<usize> {
    state.stats.lock().unwrap().export_csv(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_diagnostics(state: State<'_, AppState>, path: PathBuf) -> R<()> {
    let status = state.service.get().map(|s| s.snapshot());
    let json = serde_json::to_string_pretty(&status).unwrap_or_default();
    crate::diagnostics::export(&path, &json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_info() -> Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "dataDir": crate::settings::data_dir(),
    })
}
