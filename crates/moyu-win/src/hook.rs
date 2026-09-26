//! 低级鼠标钩子：记录右键按下/松开，并区分是人点的还是程序注入的。
//! 用于 Phase 0 录制（把玩家自己的右键当作甩竿/收竿标注）。运行时的接管检测在 `activity`。

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::input::MOYU_TAG;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    RightDown,
    RightUp,
}

#[derive(Clone, Copy, Debug)]
pub struct MouseEvent {
    pub at: Instant,
    pub button: Button,
    /// 被任何程序注入（SendInput）。
    pub injected: bool,
    /// 是我们自己注入的。
    pub ours: bool,
}

static SINK: Mutex<Option<Sender<MouseEvent>>> = Mutex::new(None);

unsafe extern "system" fn proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let button = match w.0 as u32 {
            WM_RBUTTONDOWN => Some(Button::RightDown),
            WM_RBUTTONUP => Some(Button::RightUp),
            _ => None,
        };
        if let Some(button) = button {
            let info = &*(l.0 as *const MSLLHOOKSTRUCT);
            let ev = MouseEvent {
                at: Instant::now(),
                button,
                injected: info.flags & LLMHF_INJECTED != 0,
                ours: info.dwExtraInfo == MOYU_TAG,
            };
            if let Ok(g) = SINK.lock() {
                if let Some(tx) = g.as_ref() {
                    let _ = tx.send(ev);
                }
            }
        }
    }
    CallNextHookEx(None, code, w, l)
}

/// 装好的钩子；drop 时卸载并结束消息循环线程。同一时刻只支持一个。
pub struct MouseHook {
    thread_id: u32,
    join: Option<std::thread::JoinHandle<()>>,
}

impl MouseHook {
    pub fn install() -> anyhow::Result<(MouseHook, Receiver<MouseEvent>)> {
        let (tx, rx) = channel();
        *SINK.lock().unwrap() = Some(tx);
        let (ready_tx, ready_rx) = channel();
        let join = std::thread::Builder::new().name("mouse-hook".into()).spawn(move || unsafe {
            let hmod = GetModuleHandleW(None).ok();
            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(proc), hmod.map(|h| h.into()), 0);
            let _ = ready_tx.send((GetCurrentThreadId(), hook.is_ok()));
            let Ok(hook) = hook else { return };
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(hook);
        })?;
        let (thread_id, ok) = ready_rx.recv()?;
        if !ok {
            anyhow::bail!("SetWindowsHookExW 失败");
        }
        Ok((MouseHook { thread_id, join: Some(join) }, rx))
    }
}

impl Drop for MouseHook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        *SINK.lock().unwrap() = None;
    }
}
