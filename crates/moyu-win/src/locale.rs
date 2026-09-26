//! 系统界面语言。

use windows::Win32::Globalization::GetUserDefaultUILanguage;

/// Windows 显示语言是不是中文（简体/繁体都算）。
pub fn ui_is_chinese() -> bool {
    const LANG_CHINESE: u16 = 0x04;
    // SAFETY: 无参数、只读系统设置
    let id = unsafe { GetUserDefaultUILanguage() };
    id & 0x3ff == LANG_CHINESE
}

#[cfg(test)]
mod tests {
    #[test]
    fn does_not_panic() {
        let _ = super::ui_is_chinese();
    }
}
