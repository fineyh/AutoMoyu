//! 全局热键（底层同样是 RegisterHotKey，游戏聚焦时也能收到）。

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::service::Cmd;
use crate::settings::Hotkeys;
use crate::AppState;

static CURRENT: Mutex<Option<(Shortcut, Shortcut)>> = Mutex::new(None);

pub fn register(app: &AppHandle, keys: &Hotkeys) -> anyhow::Result<()> {
    let toggle: Shortcut = keys.toggle.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
    let overlay: Shortcut = keys.overlay.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(toggle)?;
    gs.register(overlay)?;
    *CURRENT.lock().unwrap() = Some((toggle, overlay));
    Ok(())
}

pub fn handle(app: &AppHandle, sc: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    let Some((toggle, overlay)) = *CURRENT.lock().unwrap() else { return };
    let Some(svc) = app.state::<AppState>().service.get().cloned() else { return };
    if *sc == toggle {
        svc.send(Cmd::Hotkey);
    } else if *sc == overlay {
        svc.send(Cmd::ToggleOverlay);
    }
}
