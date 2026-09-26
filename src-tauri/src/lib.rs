//! AutoMoyu 桌面应用：Tauri 壳，把 moyu-core（算法）和 moyu-win（平台）接起来。

mod calib_store;
mod commands;
mod diagnostics;
mod hotkeys;
mod notify;
mod overlay;
mod service;
mod settings;
mod stats;
mod tray;
mod updater;

use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Manager, RunEvent, WindowEvent};

use crate::service::ServiceHandle;
use crate::settings::{data_dir, Settings};
use crate::stats::Stats;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub stats: Arc<Mutex<Stats>>,
    pub service: OnceLock<ServiceHandle>,
}

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::prelude::*;
    let dir = data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("automoyu")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
        .ok()?;
    let (nb, guard) = tracing_appender::non_blocking(appender);
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,wry=warn,tao=warn".into());
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(nb).with_ansi(false))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
    Some(guard)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = std::fs::create_dir_all(data_dir());
    let _log = init_logging();
    tracing::info!("AutoMoyu {} 启动，数据目录 {}", env!("CARGO_PKG_VERSION"), data_dir().display());
    let settings = Settings::load();
    let stats = Arc::new(Mutex::new(Stats::open(&data_dir().join("stats.db")).expect("打开统计库失败")));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| tray::show_main(app)))
        .plugin(tauri_plugin_window_state::Builder::default().with_denylist(&["overlay"]).build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, sc, e| hotkeys::handle(app, sc, e.state()))
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(updater::pending())
        .setup(move |app| {
            let handle = app.handle().clone();
            let main = app.get_webview_window("main").expect("main window");
            let _ = main.set_always_on_top(settings.ui.always_on_top);
            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                stats: stats.clone(),
                service: OnceLock::new(),
            });
            tray::create(&handle)?;
            if let Err(e) = overlay::create(&handle) {
                tracing::warn!("创建浮层失败：{e}");
            }
            let svc = service::spawn(handle.clone(), settings.clone(), stats.clone());
            let _ = app.state::<AppState>().service.set(svc);
            if let Err(e) = hotkeys::register(&handle, &settings.hotkeys) {
                tracing::warn!("热键注册失败：{e}");
            }
            let _ = main.show();
            Ok(())
        })
        .on_window_event(|w, e| {
            if w.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = e {
                let to_tray = w.state::<AppState>().settings.lock().unwrap().ui.close_to_tray;
                if to_tray {
                    api.prevent_close();
                    let _ = w.hide();
                } else {
                    w.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::set_settings,
            commands::hotkey,
            commands::start,
            commands::toggle_pause,
            commands::stop,
            commands::calibration_open,
            commands::calibration_start,
            commands::calibration_cancel,
            commands::calibration_reset,
            commands::focus_game,
            commands::list_game_windows,
            commands::pick_window,
            commands::get_stats,
            commands::export_stats_csv,
            commands::import_legacy,
            commands::export_diagnostics,
            commands::app_info,
            updater::update_check,
            updater::update_download,
            updater::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("启动 Tauri 失败");

    app.run(|app, e| {
        if let RunEvent::Exit = e {
            if let Some(s) = app.state::<AppState>().service.get() {
                s.shutdown();
            }
        }
    });
}
