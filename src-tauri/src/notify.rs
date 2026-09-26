//! 系统通知：只在需要人的时候发。

use moyu_core::engine::{PauseReason, StopReason};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

fn send(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("发通知失败：{e}");
    }
}

pub fn fmt_duration(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn stopped(app: &AppHandle, reason: StopReason, catches: u32, active_ms: u64, avg_bite: Option<u64>) {
    let title = match reason {
        StopReason::GameClosed => "Minecraft 已关闭，本次钓鱼结束".to_string(),
        StopReason::Catches => format!("已停止：钓满 {catches} 条"),
        StopReason::Duration => "已停止：到达设定时长".to_string(),
        StopReason::Deadline => "已停止：到点了".to_string(),
        StopReason::User => return,
    };
    let mut body = format!("本次 {catches} 条 · 用时 {}", fmt_duration(active_ms));
    if let Some(a) = avg_bite {
        body.push_str(&format!(" · 平均上钩 {:.1} 秒", a as f64 / 1000.0));
    }
    send(app, &title, &body);
}

pub fn pause(app: &AppHandle, reason: PauseReason) {
    let (t, b) = match reason {
        PauseReason::CastFailed => ("已暂停：没甩出去", "手上可能不是鱼竿，或面前没有水。检查后按 F6 继续。"),
        PauseReason::RodUnknown => ("已暂停：认不出鱼竿", "可能打开了菜单，或鱼竿坏了。画面恢复后会自动继续。"),
        _ => return,
    };
    send(app, t, b);
}
