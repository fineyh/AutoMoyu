//! SendInput 注入右键（和 AutoHotkey 同一条路径，能打进 UWP/DirectX）。
//!
//! 注入的事件在 dwExtraInfo 里带 `MOYU_TAG`，低级鼠标钩子据此区分"我们点的"和"人点的"。

use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
    MOUSE_EVENT_FLAGS,
};

/// "MOYU" 的 ASCII。
pub const MOYU_TAG: usize = 0x4D4F_5955;

fn mouse(flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dwFlags: flags, dwExtraInfo: MOYU_TAG, ..Default::default() } },
    }
}

/// 按住 `hold_ms` 再松开右键（第一人称下与光标位置无关）。阻塞调用线程。
pub fn right_click(hold_ms: u64) -> bool {
    let size = std::mem::size_of::<INPUT>() as i32;
    let down = unsafe { SendInput(&[mouse(MOUSEEVENTF_RIGHTDOWN)], size) };
    std::thread::sleep(Duration::from_millis(hold_ms));
    let up = unsafe { SendInput(&[mouse(MOUSEEVENTF_RIGHTUP)], size) };
    down == 1 && up == 1
}
