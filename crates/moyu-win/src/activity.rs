//! 玩家接管检测：低级鼠标 + 键盘钩子，只记"真人"输入。
//!
//! - 带 `INJECTED` 标记的事件（我们自己的 SendInput，或别的连点器）一律忽略。
//! - 只算游戏在前台时的输入：在本程序界面上点按钮、在别的窗口打字都不算。
//! - 按键 / 鼠标按键 / 滚轮：立刻算。
//! - 鼠标移动：连续移动超过 `MOVE_MIN_MS` 才算，碰一下桌子不算。游戏锁定光标时
//!   钩子里的坐标不可靠，所以只看"动了多久"，不看"动了多远"。
//! - 热键（F6/F7）不算，否则按 F6 继续会立刻被当成接管。

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::channel;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

use crate::input::MOYU_TAG;

/// 连续移动这么久才算在用鼠标。
const MOVE_MIN_MS: u64 = 200;
/// 两次移动间隔超过这个就算新的一段。
const MOVE_GAP_MS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Key,
    Button,
    Wheel,
    Move,
}

impl InputKind {
    fn from_u8(v: u8) -> Option<InputKind> {
        Some(match v {
            1 => InputKind::Key,
            2 => InputKind::Button,
            3 => InputKind::Wheel,
            4 => InputKind::Move,
            _ => return None,
        })
    }
    fn to_u8(self) -> u8 {
        match self {
            InputKind::Key => 1,
            InputKind::Button => 2,
            InputKind::Wheel => 3,
            InputKind::Move => 4,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            InputKind::Key => "按键",
            InputKind::Button => "鼠标按键",
            InputKind::Wheel => "滚轮",
            InputKind::Move => "移动鼠标",
        }
    }
}

// 钩子回调是裸函数，状态只能放静态变量；同一时刻只支持一个监视器。
static EPOCH: OnceLock<Instant> = OnceLock::new();
static TARGET: AtomicIsize = AtomicIsize::new(0);
/// 最近一次真人输入（EPOCH 起的毫秒 + 1；0 = 没有）。
static LAST: AtomicU64 = AtomicU64::new(0);
static LAST_KIND: AtomicU8 = AtomicU8::new(0);
static MOVE_START: AtomicU64 = AtomicU64::new(0);
static MOVE_LAST: AtomicU64 = AtomicU64::new(0);
static IGNORE_VK: [AtomicBool; 256] = [const { AtomicBool::new(false) }; 256];

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn game_foreground() -> bool {
    let t = TARGET.load(Ordering::Relaxed);
    t != 0 && unsafe { GetForegroundWindow().0 as isize } == t
}

fn mark(kind: InputKind, t: u64) {
    LAST.store(t + 1, Ordering::Relaxed);
    LAST_KIND.store(kind.to_u8(), Ordering::Relaxed);
}

unsafe extern "system" fn mouse_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(l.0 as *const MSLLHOOKSTRUCT);
        let human = info.flags & LLMHF_INJECTED == 0 && info.dwExtraInfo != MOYU_TAG;
        if human && game_foreground() {
            let t = now_ms();
            match w.0 as u32 {
                WM_MOUSEMOVE => {
                    if t.saturating_sub(MOVE_LAST.load(Ordering::Relaxed)) > MOVE_GAP_MS {
                        MOVE_START.store(t, Ordering::Relaxed);
                    }
                    MOVE_LAST.store(t, Ordering::Relaxed);
                    if t.saturating_sub(MOVE_START.load(Ordering::Relaxed)) >= MOVE_MIN_MS {
                        mark(InputKind::Move, t);
                    }
                }
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => mark(InputKind::Button, t),
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => mark(InputKind::Wheel, t),
                _ => {}
            }
        }
    }
    CallNextHookEx(None, code, w, l)
}

unsafe extern "system" fn key_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 && matches!(w.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        let info = &*(l.0 as *const KBDLLHOOKSTRUCT);
        let human = (info.flags & LLKHF_INJECTED).0 == 0 && info.dwExtraInfo != MOYU_TAG;
        let ignored = IGNORE_VK.get(info.vkCode as usize).is_some_and(|b| b.load(Ordering::Relaxed));
        if human && !ignored && game_foreground() {
            mark(InputKind::Key, now_ms());
        }
    }
    CallNextHookEx(None, code, w, l)
}

/// 装好的钩子；drop 时卸载。
pub struct ActivityMonitor {
    thread_id: u32,
    join: Option<std::thread::JoinHandle<()>>,
}

impl ActivityMonitor {
    /// `ignore_vk`：不算接管的虚拟键码（热键）。
    pub fn install(ignore_vk: &[u32]) -> anyhow::Result<ActivityMonitor> {
        for b in &IGNORE_VK {
            b.store(false, Ordering::Relaxed);
        }
        for &vk in ignore_vk {
            if let Some(b) = IGNORE_VK.get(vk as usize) {
                b.store(true, Ordering::Relaxed);
            }
        }
        LAST.store(0, Ordering::Relaxed);
        LAST_KIND.store(0, Ordering::Relaxed);
        MOVE_START.store(0, Ordering::Relaxed);
        MOVE_LAST.store(0, Ordering::Relaxed);
        let (ready_tx, ready_rx) = channel();
        let join = std::thread::Builder::new().name("activity-hook".into()).spawn(move || unsafe {
            let hmod = GetModuleHandleW(None).ok().map(|h| h.into());
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0);
            let key = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), hmod, 0);
            let ok = mouse.is_ok() && key.is_ok();
            let _ = ready_tx.send((GetCurrentThreadId(), ok));
            if ok {
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            if let Ok(h) = mouse {
                let _ = UnhookWindowsHookEx(h);
            }
            if let Ok(h) = key {
                let _ = UnhookWindowsHookEx(h);
            }
        })?;
        let (thread_id, ok) = ready_rx.recv()?;
        if !ok {
            let _ = join.join();
            anyhow::bail!("SetWindowsHookExW 失败");
        }
        Ok(ActivityMonitor { thread_id, join: Some(join) })
    }

    /// 只有这个窗口在前台时的输入才算（顶层窗口句柄；`None` = 都不算）。
    pub fn set_target(&self, hwnd: Option<isize>) {
        TARGET.store(hwnd.unwrap_or(0), Ordering::Relaxed);
    }

    /// 最近一次真人输入是多久以前、是什么。
    pub fn last_input(&self) -> Option<(Duration, InputKind)> {
        let last = LAST.load(Ordering::Relaxed);
        let kind = InputKind::from_u8(LAST_KIND.load(Ordering::Relaxed))?;
        (last > 0).then(|| (Duration::from_millis(now_ms().saturating_sub(last - 1)), kind))
    }
}

impl Drop for ActivityMonitor {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        TARGET.store(0, Ordering::Relaxed);
    }
}

/// 热键名（"F6"、"Ctrl+Shift+K"）里的主键 → 虚拟键码；带修饰键时把修饰键也算上。
pub fn hotkey_vks(name: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let parts: Vec<String> = name.split('+').map(|p| p.trim().to_ascii_uppercase()).collect();
    for p in &parts {
        match p.as_str() {
            "CTRL" | "CONTROL" | "CMDORCTRL" | "COMMANDORCONTROL" => out.extend([0x11, 0xA2, 0xA3]),
            "SHIFT" => out.extend([0x10, 0xA0, 0xA1]),
            "ALT" | "OPTION" => out.extend([0x12, 0xA4, 0xA5]),
            "SUPER" | "WIN" | "META" | "CMD" | "COMMAND" => out.extend([0x5B, 0x5C]),
            k => {
                let k = k.strip_prefix("KEY").or_else(|| k.strip_prefix("DIGIT")).unwrap_or(k);
                if let Some(n) = k.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=24).contains(n)) {
                    out.push(0x70 + n - 1); // VK_F1..
                } else if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) {
                    out.push(k.as_bytes()[0] as u32);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hotkeys() {
        assert_eq!(hotkey_vks("F6"), vec![0x75]);
        assert_eq!(hotkey_vks("f7"), vec![0x76]);
        assert_eq!(hotkey_vks("Ctrl+K"), vec![0x11, 0xA2, 0xA3, b'K' as u32]);
        assert_eq!(hotkey_vks("Alt+Digit3"), vec![0x12, 0xA4, 0xA5, b'3' as u32]);
    }
}
