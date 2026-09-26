//! 界面语言：中文 / English。后端文案（托盘、通知、提示、校准失败原因）写成 `t("中文", "English")`。
//! 前端按 `Status.lang` 显示同一种语言。

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::settings::Language;

static EN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Lang {
    Zh,
    En,
}

/// 按设置定下当前语言；"跟随系统"看 Windows 显示语言，非中文一律英文。
pub fn apply(lang: Language) {
    let en = match lang {
        Language::Zh => false,
        Language::En => true,
        Language::Auto => !moyu_win::locale::ui_is_chinese(),
    };
    EN.store(en, Ordering::Relaxed);
}

pub fn is_en() -> bool {
    EN.load(Ordering::Relaxed)
}

pub fn current() -> Lang {
    if is_en() {
        Lang::En
    } else {
        Lang::Zh
    }
}

pub fn t(zh: &'static str, en: &'static str) -> &'static str {
    if is_en() {
        en
    } else {
        zh
    }
}
