//! 系统通知：只在需要人的时候发。

use moyu_core::engine::{PauseReason, StopReason};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::i18n::{is_en, t};

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
        StopReason::GameClosed => t("Minecraft 已关闭，本次钓鱼结束", "Minecraft closed, session ended").to_string(),
        StopReason::Catches if is_en() => format!("Stopped: caught {catches} fish"),
        StopReason::Catches => format!("已停止：钓满 {catches} 条"),
        StopReason::Duration => t("已停止：到达设定时长", "Stopped: time limit reached").to_string(),
        StopReason::Deadline => t("已停止：到点了", "Stopped: scheduled stop time").to_string(),
        StopReason::User => return,
    };
    let dur = fmt_duration(active_ms);
    let mut body = if is_en() { format!("{catches} fish · {dur}") } else { format!("本次 {catches} 条 · 用时 {dur}") };
    if let Some(a) = avg_bite {
        let s = a as f64 / 1000.0;
        body.push_str(&if is_en() { format!(" · avg bite {s:.1} s") } else { format!(" · 平均上钩 {s:.1} 秒") });
    }
    send(app, &title, &body);
}

pub fn pause(app: &AppHandle, reason: PauseReason) {
    let (t, b) = match reason {
        PauseReason::CastFailed => (
            t("已暂停：没甩出去", "Paused: the cast didn't go out"),
            t(
                "手上可能不是鱼竿，或面前没有水。检查后按 F6 继续。",
                "You may not be holding a fishing rod, or there's no water ahead. Fix it, then press F6 to resume.",
            ),
        ),
        PauseReason::RodUnknown => (
            t("已暂停：认不出鱼竿", "Paused: can't recognise the rod"),
            t(
                "可能打开了菜单，或鱼竿坏了。画面恢复后会自动继续。",
                "A menu may be open, or the rod broke. It resumes automatically once the view is back.",
            ),
        ),
        _ => return,
    };
    send(app, t, b);
}
