//! 进程 DPI 感知：所有坐标用物理像素。Tauri 应用由清单声明，命令行工具需要手动调用。

use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};

pub fn set_per_monitor_v2() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
