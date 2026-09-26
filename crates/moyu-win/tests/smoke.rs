//! 需要真实桌面会话的冒烟测试（CI 的 Windows runner 上也能跑）。

#[test]
fn grabs_a_screen_region() {
    moyu_win::dpi::set_per_monitor_v2();
    let mut cap = moyu_win::ScreenCapture::new();
    let a = cap.grab(0, 0, 64, 48).expect("grab");
    assert_eq!((a.w, a.h, a.data.len()), (64, 48, 64 * 48 * 4));
    // 同尺寸复用 DIB，换尺寸重建
    let b = cap.grab(10, 10, 32, 16).expect("grab 2");
    assert_eq!(b.data.len(), 32 * 16 * 4);
    let g = a.to_grid(4);
    assert_eq!((g.w, g.h), (16, 12));
}

#[test]
fn lists_windows_without_panicking() {
    let v = moyu_win::window::list_windows();
    assert!(v.iter().all(|w| w.pid != std::process::id()));
}

#[test]
fn mouse_hook_installs_and_uninstalls() {
    let (hook, _rx) = moyu_win::hook::MouseHook::install().expect("hook");
    drop(hook);
}

#[test]
fn system_loopback_starts() {
    let (tx, _rx) = std::sync::mpsc::channel();
    // 没有音频设备的机器上允许失败，但不能 panic/卡死
    if let Ok(a) = moyu_win::audio::AudioCapture::start(None, tx) {
        assert_eq!(a.source, moyu_win::audio::AudioSource::System);
    }
}

#[test]
fn activity_monitor_ignores_injected_input() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_F24,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let mon = moyu_win::activity::ActivityMonitor::install(&[]).expect("hook");
    mon.set_target(Some(unsafe { GetForegroundWindow().0 as isize }));
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_F24, dwFlags: flags, ..Default::default() } },
    };
    unsafe { SendInput(&[key(Default::default()), key(KEYEVENTF_KEYUP)], std::mem::size_of::<INPUT>() as i32) };
    std::thread::sleep(std::time::Duration::from_millis(200));
    // 注入的按键不算；测试期间真人碰了鼠标可能记成 Move，那不影响本断言
    assert_ne!(mon.last_input().map(|(_, k)| k), Some(moyu_win::activity::InputKind::Key));
    drop(mon);
}
