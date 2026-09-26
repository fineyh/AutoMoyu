//! 运行时阻止电脑睡眠/关屏（关屏后截图可能拿到旧画面）。

use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

/// 持有期间保持唤醒。执行状态绑定在调用线程上：在哪个线程创建就在哪个线程销毁。
pub struct KeepAwake(());

impl KeepAwake {
    pub fn new() -> Self {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED) };
        KeepAwake(())
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
    }
}
