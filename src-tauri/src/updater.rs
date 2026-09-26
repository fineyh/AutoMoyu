//! 检查更新：GitHub 在前、国内镜像在后，失败自动换下一个；更新包 minisign 签名校验。
//! 后台下载，钓鱼进行中不打扰（由前端决定何时提示"重启以完成更新"）。

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::settings::Channel;
use crate::AppState;

pub struct Pending {
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

pub type PendingUpdate = Mutex<Pending>;

pub fn pending() -> PendingUpdate {
    Mutex::new(Pending { update: None, bytes: None })
}

fn endpoints(ch: Channel) -> Vec<Url> {
    let list: &[&str] = match ch {
        Channel::Stable => &[
            "https://github.com/fineyh/AutoMoyu/releases/latest/download/latest.json",
            "https://gitee.com/fineyh/AutoMoyu/releases/download/latest/latest.json",
        ],
        // 测试通道：每次发版（含 beta 和正式版）都会刷新这里
        Channel::Beta => &[
            "https://github.com/fineyh/AutoMoyu/releases/download/updater-beta/latest.json",
            "https://gitee.com/fineyh/AutoMoyu/releases/download/updater-beta/latest.json",
        ],
    };
    list.iter().filter_map(|u| Url::parse(u).ok()).collect()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    state: &'static str,
    downloaded: usize,
    total: Option<u64>,
}

#[tauri::command]
pub async fn update_check(app: AppHandle, state: State<'_, AppState>, pending: State<'_, PendingUpdate>) -> Result<Option<UpdateInfo>, String> {
    let ch = state.settings.lock().unwrap().update.channel;
    let updater = app
        .updater_builder()
        .endpoints(endpoints(ch))
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
    });
    let mut p = pending.lock().unwrap();
    if p.update.as_ref().map(|u| &u.version) != found.as_ref().map(|u| &u.version) {
        p.bytes = None;
    }
    p.update = found;
    Ok(info)
}

#[tauri::command]
pub async fn update_download(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), String> {
    let update = pending.lock().unwrap().update.clone().ok_or("没有可用的更新")?;
    let mut got = 0usize;
    let a2 = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                got += chunk;
                let _ = a2.emit("update-progress", Progress { state: "downloading", downloaded: got, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    pending.lock().unwrap().bytes = Some(bytes);
    let _ = app.emit("update-progress", Progress { state: "ready", downloaded: got, total: None });
    Ok(())
}

#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, AppState>, pending: State<'_, PendingUpdate>) -> Result<(), String> {
    let (update, bytes) = {
        let mut p = pending.lock().unwrap();
        (p.update.clone().ok_or("没有可用的更新")?, p.bytes.take().ok_or("更新还没下载完")?)
    };
    if let Some(s) = state.service.get() {
        s.shutdown();
    }
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
