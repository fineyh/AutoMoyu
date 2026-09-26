//! 设置：`%APPDATA%\AutoMoyu\settings.json`，带 version 字段便于以后迁移。
//! 前端传部分字段（JSON merge patch），合并后整体校验再保存。

use std::path::PathBuf;

use anyhow::Result;
use moyu_core::bite_audio::Sensitivity;
use moyu_core::engine::{AutoStop, EngineConfig, Mode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("AutoMoyu")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum BiteSource {
    #[default]
    Audio,
    Visual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AutoStopKind {
    #[default]
    None,
    /// value = 分钟
    Minutes,
    /// value = 条数
    Catches,
    /// value = 一天中的第几分钟（到这个时刻停）
    At,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoStopSetting {
    pub kind: AutoStopKind,
    pub value: u32,
}

impl Default for AutoStopSetting {
    fn default() -> Self {
        Self { kind: AutoStopKind::None, value: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hotkeys {
    pub toggle: String,
    pub overlay: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self { toggle: "F6".into(), overlay: "F7".into() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ui {
    pub theme: Theme,
    pub always_on_top: bool,
    pub close_to_tray: bool,
    pub overlay: bool,
    pub catch_sound: bool,
}

impl Default for Ui {
    fn default() -> Self {
        Self { theme: Theme::System, always_on_top: false, close_to_tray: true, overlay: true, catch_sound: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Notify {
    pub on_stop: bool,
    pub on_error: bool,
}

impl Default for Notify {
    fn default() -> Self {
        Self { on_stop: true, on_error: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Update {
    pub auto: bool,
    pub channel: Channel,
    pub skip_version: Option<String>,
}

impl Default for Update {
    fn default() -> Self {
        // beta 版本默认走测试通道，才能收到 beta.N 的更新
        let beta = env!("CARGO_PKG_VERSION").contains('-');
        Self { auto: true, channel: if beta { Channel::Beta } else { Channel::Stable }, skip_version: None }
    }
}

/// 手动指定的游戏窗口（按进程名 + 标题匹配，重启后仍有效）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowMatch {
    pub process: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Advanced {
    pub click_hold_ms: u64,
    pub max_wait_s: u64,
    pub focus_guard: bool,
    /// 玩家自己操作游戏时让出控制。
    pub takeover: bool,
    /// 停手这么久后自动继续。
    pub takeover_idle_s: u64,
    pub window: Option<WindowMatch>,
    pub bite_sensitivity: Sensitivity,
    pub debug_overlay: bool,
}

impl Default for Advanced {
    fn default() -> Self {
        Self {
            click_hold_ms: 90,
            max_wait_s: 60,
            focus_guard: true,
            takeover: true,
            takeover_idle_s: 5,
            window: None,
            bite_sensitivity: Sensitivity::Normal,
            debug_overlay: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub mode: Mode,
    pub bite_source: BiteSource,
    pub hotkeys: Hotkeys,
    pub auto_stop: AutoStopSetting,
    pub ui: Ui,
    pub notify: Notify,
    pub update: Update,
    pub advanced: Advanced,
    /// 首次运行向导是否已走过。
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            mode: Mode::RodOnly,
            bite_source: BiteSource::Audio,
            hotkeys: Hotkeys::default(),
            auto_stop: AutoStopSetting::default(),
            ui: Ui::default(),
            notify: Notify::default(),
            update: Update::default(),
            advanced: Advanced::default(),
            onboarded: false,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        data_dir().join("settings.json")
    }

    pub fn load() -> Settings {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .map(|s| s.sanitized())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(data_dir())?;
        let tmp = Self::path().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, Self::path())?;
        Ok(())
    }

    /// 合并前端传来的部分字段。
    pub fn merged(&self, patch: &Value) -> Result<Settings> {
        let mut base = serde_json::to_value(self)?;
        merge(&mut base, patch);
        Ok(serde_json::from_value::<Settings>(base)?.sanitized())
    }

    fn sanitized(mut self) -> Settings {
        self.version = 1;
        self.advanced.click_hold_ms = self.advanced.click_hold_ms.clamp(30, 500);
        self.advanced.max_wait_s = self.advanced.max_wait_s.clamp(10, 600);
        self.advanced.takeover_idle_s = self.advanced.takeover_idle_s.clamp(1, 60);
        if self.auto_stop.kind == AutoStopKind::At {
            self.auto_stop.value = self.auto_stop.value.min(24 * 60 - 1);
        }
        self
    }

    /// 由设置生成引擎配置。`now_ms` 用来把"到某个时刻"换算成引擎时钟。
    pub fn engine_config(&self, now_ms: u64) -> EngineConfig {
        let a = &self.auto_stop;
        let auto_stop = match a.kind {
            AutoStopKind::None => AutoStop::default(),
            AutoStopKind::Minutes if a.value > 0 => {
                AutoStop { max_active_ms: Some(a.value as u64 * 60_000), ..Default::default() }
            }
            AutoStopKind::Catches if a.value > 0 => AutoStop { max_catches: Some(a.value), ..Default::default() },
            AutoStopKind::At => {
                use chrono::Timelike;
                let t = chrono::Local::now();
                let now_min = t.hour() * 60 + t.minute();
                let mut delta_min = a.value as i64 - now_min as i64;
                if delta_min <= 0 {
                    delta_min += 24 * 60;
                }
                let ms = (delta_min * 60 - t.second() as i64).max(0) as u64 * 1000;
                AutoStop { deadline_ms: Some(now_ms + ms), ..Default::default() }
            }
            _ => AutoStop::default(),
        };
        EngineConfig {
            mode: self.mode,
            focus_guard: self.advanced.focus_guard,
            max_wait_ms: self.advanced.max_wait_s * 1000,
            auto_stop,
            ..EngineConfig::default()
        }
    }
}

fn merge(a: &mut Value, b: &Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (a, b) => *a = b.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_merges_nested() {
        let s = Settings::default();
        let m = s.merged(&serde_json::json!({"ui": {"theme": "dark"}, "advanced": {"maxWaitS": 5}})).unwrap();
        assert_eq!(m.ui.theme, Theme::Dark);
        assert!(m.ui.close_to_tray, "未提到的字段保持不变");
        assert_eq!(m.advanced.max_wait_s, 10, "越界值被夹住");
    }

    #[test]
    fn unknown_or_missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"mode":"full","future":1}"#).unwrap();
        assert_eq!(s.mode, Mode::Full);
        assert_eq!(s.hotkeys.toggle, "F6");
    }

    #[test]
    fn auto_stop_minutes() {
        let mut s = Settings::default();
        s.auto_stop = AutoStopSetting { kind: AutoStopKind::Minutes, value: 30 };
        assert_eq!(s.engine_config(0).auto_stop.max_active_ms, Some(1_800_000));
    }
}
