//! 找游戏窗口：按"进程名 + 标题"双重匹配。
//!
//! - UWP 版：顶层窗口属于 ApplicationFrameHost.exe，真正的游戏进程在子窗口 CoreWindow 上。
//! - GDK 版：顶层窗口直接属于 Minecraft.Windows.exe。
//! 两种都要能认出来，并拿到真实的游戏 PID（用于进程声音环回、判断游戏是否退出）。

use serde::Serialize;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT, STILL_ACTIVE};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetClientRect, GetForegroundWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    SetForegroundWindow, ShowWindow, GWL_EXSTYLE, SW_RESTORE, WS_EX_TOOLWINDOW,
};

/// 已知的 Minecraft 基岩版进程名（小写）。
const MC_PROCESSES: &[&str] = &["minecraft.windows.exe", "minecraft.exe", "minecraftpe.exe"];
const FRAME_HOST: &str = "applicationframehost.exe";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameWindow {
    /// 顶层窗口句柄（数值，便于跨线程传递）。
    pub hwnd: isize,
    /// 真实游戏进程（UWP 时是 CoreWindow 的进程）。
    pub pid: u32,
    pub process: String,
    pub title: String,
    /// 客户区在屏幕上的物理像素矩形。
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub minimized: bool,
}

impl GameWindow {
    pub fn is_minecraft(&self) -> bool {
        MC_PROCESSES.contains(&self.process.to_lowercase().as_str())
    }
}

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

fn window_text(h: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn pid_of(h: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

/// 进程可执行文件名（不含路径）。
pub fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let hp = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;
        let r = QueryFullProcessImageNameW(hp, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size);
        let _ = CloseHandle(hp);
        r.ok()?;
        let full = String::from_utf16_lossy(&buf[..size as usize]);
        Some(full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string())
    }
}

pub fn process_alive(pid: u32) -> bool {
    unsafe {
        let Ok(hp) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0u32;
        let ok = GetExitCodeProcess(hp, &mut code).is_ok();
        let _ = CloseHandle(hp);
        ok && code == STILL_ACTIVE.0 as u32
    }
}

/// UWP 宿主窗口里找 CoreWindow 子窗口的进程。
fn uwp_child_pid(top: HWND, host_pid: u32) -> Option<u32> {
    struct Ctx {
        host: u32,
        found: Option<u32>,
    }
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        let ctx = &mut *(l.0 as *mut Ctx);
        let pid = pid_of(h);
        if pid != ctx.host && class_name(h) == "Windows.UI.Core.CoreWindow" {
            ctx.found = Some(pid);
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut ctx = Ctx { host: host_pid, found: None };
    unsafe {
        let _ = EnumChildWindows(Some(top), Some(cb), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    ctx.found
}

fn client_rect(h: HWND) -> Option<(i32, i32, u32, u32)> {
    unsafe {
        let mut rc = RECT::default();
        GetClientRect(h, &mut rc).ok()?;
        let mut pt = POINT { x: 0, y: 0 };
        if !ClientToScreen(h, &mut pt).as_bool() {
            return None;
        }
        Some((pt.x, pt.y, (rc.right - rc.left).max(0) as u32, (rc.bottom - rc.top).max(0) as u32))
    }
}

fn describe(h: HWND) -> Option<GameWindow> {
    unsafe {
        if !IsWindowVisible(h).as_bool() {
            return None;
        }
        if GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
    }
    let title = window_text(h);
    if title.is_empty() {
        return None;
    }
    let mut pid = pid_of(h);
    let mut process = process_name(pid).unwrap_or_default();
    if process.eq_ignore_ascii_case(FRAME_HOST) {
        if let Some(child) = uwp_child_pid(h, pid) {
            pid = child;
            process = process_name(child).unwrap_or(process);
        }
    }
    let minimized = unsafe { IsIconic(h).as_bool() };
    let (x, y, w, hh) = client_rect(h).unwrap_or((0, 0, 0, 0));
    Some(GameWindow { hwnd: h.0 as isize, pid, process, title, x, y, w, h: hh, minimized })
}

/// 所有可见的普通顶层窗口（"选择窗口"列表用）。
pub fn list_windows() -> Vec<GameWindow> {
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        let v = &mut *(l.0 as *mut Vec<GameWindow>);
        if let Some(w) = describe(h) {
            v.push(w);
        }
        BOOL(1)
    }
    let mut v: Vec<GameWindow> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut v as *mut _ as isize));
    }
    let own = std::process::id();
    v.retain(|w| w.pid != own);
    v
}

/// 自动找 Minecraft：进程名命中优先，其次标题正好是 "Minecraft"；同分取面积大的。
pub fn find_minecraft() -> Option<GameWindow> {
    let score = |w: &GameWindow| -> u32 {
        let t = w.title.trim().to_lowercase();
        let mut s = 0;
        if w.is_minecraft() {
            s += 4;
        }
        if t == "minecraft" || t.starts_with("minecraft ") {
            s += 2;
        } else if t.contains("minecraft") && w.is_minecraft() {
            s += 1;
        }
        s
    };
    list_windows()
        .into_iter()
        .filter(|w| score(w) >= 2)
        .max_by_key(|w| (score(w), w.w as u64 * w.h as u64))
}

/// 按句柄刷新（窗口挪动、改尺寸）。窗口没了返回 `None`。
pub fn refresh(h: isize) -> Option<GameWindow> {
    unsafe {
        if !IsWindow(Some(hwnd(h))).as_bool() {
            return None;
        }
    }
    describe(hwnd(h))
}

pub fn is_foreground(h: isize) -> bool {
    unsafe { GetForegroundWindow().0 as isize == h }
}

/// 把游戏切回前台（"切回游戏"按钮）。Windows 限制后台进程抢前台，先补一个 Alt 解锁。
pub fn focus(h: isize) -> bool {
    unsafe {
        let w = hwnd(h);
        if IsIconic(w).as_bool() {
            let _ = ShowWindow(w, SW_RESTORE);
        }
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, dwFlags: flags, ..Default::default() } },
        };
        SendInput(&[key(Default::default()), key(KEYEVENTF_KEYUP)], std::mem::size_of::<INPUT>() as i32);
        SetForegroundWindow(w).as_bool()
    }
}

/// 游戏内浮层：不抢焦点、不进 Alt+Tab。
pub fn make_overlay(h: isize) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongW, WS_EX_NOACTIVATE};
    unsafe {
        let w = hwnd(h);
        let ex = GetWindowLongW(w, GWL_EXSTYLE) as u32;
        SetWindowLongW(w, GWL_EXSTYLE, (ex | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as i32);
    }
}

/// 不激活地摆放/显示/隐藏一个置顶窗口（物理像素）。
pub fn place_noactivate(h: isize, rect: Option<(i32, i32, u32, u32)>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };
    unsafe {
        let _ = match rect {
            Some((x, y, w, hh)) => SetWindowPos(
                hwnd(h),
                Some(HWND_TOPMOST),
                x,
                y,
                w as i32,
                hh as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            ),
            None => SetWindowPos(hwnd(h), None, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_HIDEWINDOW),
        };
    }
}
