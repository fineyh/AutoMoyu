//! 钓鱼服务线程：15 Hz 循环里做 找窗口 → 截图 → 分类 → 状态机 → 点击，
//! 也负责校准流程、写统计库、向界面推状态。
//!
//! 所有 Windows 资源（截图 DC、防睡眠、声音采集）都只在这个线程里创建和销毁。

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use moyu_core::bite_audio::{AudioLevel, BiteAudioDetector};
use moyu_core::calibrate::{CalOutput, Calibrator, Geometry, Quality, Source};
use moyu_core::engine::{Engine, Event, Mode, Observation, Output, PauseReason, Phase, StopReason, WindowObs};
use moyu_core::{Grid, RelRect, RodModel, RodState};
use moyu_win::audio::{AudioCapture, AudioSource};
use moyu_win::power::KeepAwake;
use moyu_win::{window, GameWindow, ScreenCapture};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::calib_store::{self, Stored};
use crate::settings::{BiteSource, Settings, WindowMatch};
use crate::stats::Stats;
use crate::{notify, overlay, tray};

const TICK: Duration = Duration::from_millis(66);
const SCAN_EVERY: Duration = Duration::from_secs(2);
const COUNTDOWN_MS: u64 = 3_000;

pub enum Cmd {
    /// F6：含义随状态变化（校准准备中 → 开始校准；运行中 → 暂停/继续；空闲 → 开始）。
    Hotkey,
    Start,
    TogglePause,
    Stop,
    CalOpen,
    CalStart,
    CalCancel,
    CalReset,
    ToggleOverlay,
    Settings(Box<Settings>),
    PickWindow(Option<WindowMatch>),
    FocusGame,
    Quit(Sender<()>),
}

// ---------------------------------------------------------------------------
// 给界面的状态快照
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WindowView {
    pub title: String,
    pub process: String,
    pub w: u32,
    pub h: u32,
    pub foreground: bool,
    pub pinned: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CalView {
    pub w: u32,
    pub h: u32,
    pub quality: Quality,
    pub ratio: f32,
    pub source: Source,
    pub rel: RelRect,
    pub accuracy: f32,
    pub created_at: String,
}

impl From<&Stored> for CalView {
    fn from(s: &Stored) -> Self {
        let r = &s.result;
        CalView {
            w: r.client_w,
            h: r.client_h,
            quality: r.quality,
            ratio: r.ratio,
            source: r.source,
            rel: r.rel,
            accuracy: r.accuracy,
            created_at: s.created_at.clone(),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CalRunView {
    /// prepare | countdown | sampling | analyzing | done | failed
    pub step: &'static str,
    pub countdown: Option<u32>,
    pub progress: f32,
    pub round: u32,
    pub sampling: Option<RodState>,
    pub frames_in: usize,
    pub frames_out: usize,
    pub error: Option<String>,
    pub result: Option<CalView>,
    /// 候选区外包矩形在客户区里的比例坐标，预览图就是这块。
    pub union_rel: Option<RelRect>,
    pub candidates: Vec<(Source, RelRect)>,
    pub preview: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub catches: u32,
    pub active_ms: u64,
    /// 这一竿已经甩出多久。
    pub out_for_ms: Option<u64>,
    pub avg_bite_ms: Option<u64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SignalView {
    pub d_in: f32,
    pub d_out: f32,
    pub rod: Option<RodState>,
    pub audio_db: Option<f32>,
    pub audio_gate_db: Option<f32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub pause_reason: Option<PauseReason>,
    pub last_stop: Option<StopReason>,
    pub mode: Mode,
    pub session: Option<SessionView>,
    pub window: Option<WindowView>,
    pub calibration: Option<CalView>,
    pub calib: Option<CalRunView>,
    pub audio: Option<AudioSource>,
    pub signal: Option<SignalView>,
    pub overlay_on: bool,
}

impl Status {
    fn initial(s: &Settings) -> Self {
        Status {
            phase: Phase::Idle,
            pause_reason: None,
            last_stop: None,
            mode: s.mode,
            session: None,
            window: None,
            calibration: None,
            calib: None,
            audio: None,
            signal: None,
            overlay_on: true,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    pub at: i64,
    pub event: Event,
}

// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ServiceHandle {
    tx: Sender<Cmd>,
    pub status: Arc<Mutex<Status>>,
}

impl ServiceHandle {
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
    pub fn snapshot(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
    /// 退出前收尾（结束场次、恢复睡眠）。
    pub fn shutdown(&self) {
        let (tx, rx) = mpsc::channel();
        self.send(Cmd::Quit(tx));
        let _ = rx.recv_timeout(Duration::from_secs(2));
    }
}

pub fn spawn(app: AppHandle, settings: Settings, stats: Arc<Mutex<Stats>>) -> ServiceHandle {
    let (tx, rx) = mpsc::channel();
    let status = Arc::new(Mutex::new(Status::initial(&settings)));
    let st = status.clone();
    std::thread::Builder::new()
        .name("moyu-service".into())
        .spawn(move || {
            let mut svc = Svc::new(app, settings, stats, st);
            svc.run(rx);
        })
        .expect("spawn service");
    ServiceHandle { tx, status }
}

struct AudioRig {
    _cap: AudioCapture,
    rx: Receiver<Vec<f32>>,
    det: BiteAudioDetector,
    level: Option<AudioLevel>,
}

struct Session {
    id: i64,
    _awake: KeepAwake,
    audio: Option<AudioRig>,
    model: RodModel,
    cal: Stored,
    bite_waits: Vec<u64>,
    last_save: Instant,
}

enum CalStep {
    Prepare,
    Countdown { until: u64 },
    Sampling(Box<Calibrator>),
    Done(CalView),
    Failed(String),
}

struct CalRun {
    step: CalStep,
    progress: moyu_core::calibrate::CalProgress,
    preview: Option<String>,
    last_preview: Option<Instant>,
    last_in_frame: Option<Grid>,
}

#[derive(Default)]
struct Tracker {
    win: Option<GameWindow>,
    last_scan: Option<Instant>,
}

impl Tracker {
    /// 返回 true 表示之前跟踪的游戏进程退出了。
    fn update(&mut self, pin: Option<&WindowMatch>) -> bool {
        if let Some(w) = &self.win {
            match window::refresh(w.hwnd) {
                Some(nw) if pin.is_none_or(|p| p.process.eq_ignore_ascii_case(&nw.process)) => {
                    self.win = Some(nw);
                    return false;
                }
                _ => {
                    let closed = !window::process_alive(w.pid);
                    tracing::info!("游戏窗口丢失（进程{}）", if closed { "已退出" } else { "仍在" });
                    self.win = None;
                    self.last_scan = None;
                    return closed;
                }
            }
        }
        if self.last_scan.is_none_or(|t| t.elapsed() >= SCAN_EVERY) {
            self.last_scan = Some(Instant::now());
            self.win = match pin {
                Some(p) => window::list_windows()
                    .into_iter()
                    .find(|w| w.process.eq_ignore_ascii_case(&p.process) && w.title == p.title),
                None => window::find_minecraft(),
            };
            if let Some(w) = &self.win {
                tracing::info!("找到游戏窗口：{} · {} · {}x{}", w.title, w.process, w.w, w.h);
            }
        }
        false
    }
}

struct Svc {
    app: AppHandle,
    settings: Settings,
    stats: Arc<Mutex<Stats>>,
    status: Arc<Mutex<Status>>,
    t0: Instant,
    tracker: Tracker,
    cap: ScreenCapture,
    engine: Engine,
    session: Option<Session>,
    cal: Option<CalRun>,
    calibration: Option<Stored>,
    closed: bool,
    overlay_on: bool,
    signal: Option<SignalView>,
    last_emit: Instant,
    dirty: bool,
}

impl Svc {
    fn new(app: AppHandle, settings: Settings, stats: Arc<Mutex<Stats>>, status: Arc<Mutex<Status>>) -> Self {
        let engine = Engine::new(settings.engine_config(0));
        Svc {
            app,
            settings,
            stats,
            status,
            t0: Instant::now(),
            tracker: Tracker::default(),
            cap: ScreenCapture::new(),
            engine,
            session: None,
            cal: None,
            calibration: None,
            closed: false,
            overlay_on: true,
            signal: None,
            last_emit: Instant::now(),
            dirty: true,
        }
    }

    fn now(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    fn run(&mut self, rx: Receiver<Cmd>) {
        let mut next = Instant::now();
        loop {
            next += TICK;
            if next < Instant::now() {
                next = Instant::now() + TICK; // 落后太多（比如点击阻塞）就别追帧
            }
            while let Ok(cmd) = rx.recv_timeout(next.saturating_duration_since(Instant::now())) {
                if let Cmd::Quit(done) = cmd {
                    let now = self.now();
                    let out = self.engine.stop(now, StopReason::User);
                    self.handle(out);
                    overlay::hide(&self.app);
                    let _ = done.send(());
                    return;
                }
                self.command(cmd);
            }
            self.tick();
        }
    }

    // ---- 命令 ----

    fn command(&mut self, cmd: Cmd) {
        let now = self.now();
        tracing::debug!("命令：{}", cmd_name(&cmd));
        self.dirty = true;
        match cmd {
            Cmd::Hotkey => match self.cal.as_ref().map(|c| &c.step) {
                Some(CalStep::Prepare | CalStep::Failed(_)) => self.cal_countdown(now),
                Some(CalStep::Countdown { .. } | CalStep::Sampling(_)) => self.cal_cancel("已取消校准"),
                Some(CalStep::Done(_)) => {
                    self.cal = None;
                    if !self.engine.is_running() {
                        self.start(now);
                    } else if self.engine.pause_reason().is_some() {
                        // 为重新校准而暂停的场次：直接接着钓
                        let out = self.engine.toggle_pause(now);
                        self.handle(out);
                    }
                }
                None => {
                    if self.engine.is_running() {
                        let out = self.engine.toggle_pause(now);
                        self.handle(out);
                    } else {
                        self.start(now);
                    }
                }
            },
            Cmd::Start => {
                if !self.engine.is_running() {
                    self.start(now);
                }
            }
            Cmd::TogglePause => {
                let out = self.engine.toggle_pause(now);
                self.handle(out);
            }
            Cmd::Stop => {
                let out = self.engine.stop(now, StopReason::User);
                self.handle(out);
            }
            Cmd::CalOpen => self.cal_open(),
            Cmd::CalStart => {
                if let Some(w) = &self.tracker.win {
                    window::focus(w.hwnd);
                }
                self.cal_countdown(now);
            }
            Cmd::CalCancel => {
                if self.cal.is_some() {
                    self.cal_cancel("");
                    self.cal = None;
                }
            }
            Cmd::CalReset => {
                let _ = calib_store::clear();
                self.calibration = None;
            }
            Cmd::ToggleOverlay => self.overlay_on = !self.overlay_on,
            Cmd::Settings(s) => {
                let mode_changed = s.mode != self.settings.mode;
                self.settings = *s;
                if mode_changed && self.engine.is_running() {
                    let out = self.engine.stop(now, StopReason::User);
                    self.handle(out);
                }
                if !self.engine.is_running() {
                    self.engine.set_config(self.settings.engine_config(now));
                }
            }
            Cmd::PickWindow(m) => {
                self.settings.advanced.window = m;
                self.tracker = Tracker::default();
            }
            Cmd::FocusGame => {
                if let Some(w) = &self.tracker.win {
                    window::focus(w.hwnd);
                }
            }
            Cmd::Quit(_) => unreachable!(),
        }
    }

    fn start(&mut self, now: u64) {
        let Some(w) = self.tracker.win.clone() else {
            let _ = self.app.emit("toast", "找不到 Minecraft：请先打开游戏（窗口化或无边框）");
            return;
        };
        let Some(cal) = self.calibration.clone().filter(|c| c.result.client_w == w.w && c.result.client_h == w.h)
        else {
            self.cal_open();
            return;
        };
        let mode = self.settings.mode;
        let id = self.stats.lock().unwrap().begin(mode_str(mode)).unwrap_or(-1);
        let audio = if mode == Mode::Full && self.settings.bite_source == BiteSource::Audio {
            let (tx, rx) = mpsc::channel();
            match AudioCapture::start(Some(w.pid), tx) {
                Ok(cap) => {
                    let det = BiteAudioDetector::new(cap.sample_rate, self.settings.advanced.bite_sensitivity);
                    Some(AudioRig { _cap: cap, rx, det, level: None })
                }
                Err(e) => {
                    tracing::warn!("声音采集失败：{e}");
                    let _ = self.app.emit("toast", "录不到游戏声音：本次只靠最长等待收竿");
                    None
                }
            }
        } else {
            None
        };
        tracing::info!("开始钓鱼：模式 {mode:?}，识别框 {:?}", cal.result.roi_px());
        self.session = Some(Session {
            id,
            _awake: KeepAwake::new(),
            audio,
            model: cal.result.model.clone(),
            cal,
            bite_waits: Vec::new(),
            last_save: Instant::now(),
        });
        self.engine = Engine::new(self.settings.engine_config(now));
        self.status.lock().unwrap().last_stop = None;
        let out = self.engine.start(now);
        self.handle(out);
    }

    // ---- 每帧 ----

    fn tick(&mut self) {
        let now = self.now();
        let before = self.tracker.win.as_ref().map(|w| (w.hwnd, w.w, w.h, w.x, w.y));
        self.closed |= self.tracker.update(self.settings.advanced.window.as_ref());
        let after = self.tracker.win.as_ref().map(|w| (w.hwnd, w.w, w.h, w.x, w.y));
        if before != after {
            self.dirty = true;
        }
        // 尺寸对应的校准
        if let Some(w) = &self.tracker.win {
            let matches = self.calibration.as_ref().is_some_and(|c| c.result.client_w == w.w && c.result.client_h == w.h);
            if !matches && w.w > 0 && w.h > 0 {
                if let Some(c) = calib_store::load(w.w, w.h) {
                    self.calibration = Some(c);
                    self.dirty = true;
                }
            }
        }
        if self.cal.is_some() {
            self.tick_cal(now);
        }
        if self.engine.is_running() {
            self.tick_session(now);
        } else {
            self.closed = false;
        }
        self.sync_overlay();
        let running = self.engine.is_running();
        let interval = if running || self.cal.is_some() { 250 } else { 1000 };
        if self.dirty || self.last_emit.elapsed() >= Duration::from_millis(interval) {
            self.publish();
        }
    }

    fn window_obs(&self, cal: &Stored) -> WindowObs {
        match &self.tracker.win {
            None if self.closed => WindowObs::Closed,
            None => WindowObs::Missing,
            Some(w) if w.minimized => WindowObs::NotForeground,
            Some(w) if (w.w, w.h) != (cal.result.client_w, cal.result.client_h) => WindowObs::SizeChanged,
            Some(w) if !window::is_foreground(w.hwnd) => WindowObs::NotForeground,
            Some(_) => WindowObs::Ok,
        }
    }

    fn tick_session(&mut self, now: u64) {
        // 窗口尺寸变了但那个尺寸校准过：直接换模型
        if let (Some(w), Some(s)) = (&self.tracker.win, &mut self.session) {
            if (w.w, w.h) != (s.cal.result.client_w, s.cal.result.client_h) {
                if let Some(c) = self.calibration.as_ref().filter(|c| (c.result.client_w, c.result.client_h) == (w.w, w.h)) {
                    s.model = c.result.model.clone();
                    s.cal = c.clone();
                }
            }
        }
        let Some(cal) = self.session.as_ref().map(|s| s.cal.clone()) else { return };
        let obs_window = self.window_obs(&cal);
        let mut rod = None;
        let mut sig = SignalView::default();
        if matches!(obs_window, WindowObs::Ok) || (!self.settings.advanced.focus_guard && obs_window == WindowObs::NotForeground) {
            let w = self.tracker.win.as_ref().unwrap();
            let (rx, ry, rw, rh) = cal.result.roi_px();
            if let Ok(img) = self.cap.grab(w.x + rx as i32, w.y + ry as i32, rw, rh) {
                let g = img.to_grid(cal.result.k as usize);
                let s = self.session.as_mut().unwrap();
                let c = s.model.classify_and_adapt(&g);
                rod = Some(c.state);
                sig.d_in = c.d_in;
                sig.d_out = c.d_out;
                sig.rod = Some(c.state);
            }
        }
        let mut bite = false;
        if let Some(a) = self.session.as_mut().and_then(|s| s.audio.as_mut()) {
            while let Ok(chunk) = a.rx.try_recv() {
                for l in a.det.push(&chunk) {
                    bite |= l.onset;
                    a.level = Some(l);
                }
            }
            if let Some(l) = a.level {
                sig.audio_db = Some(l.level_db);
                sig.audio_gate_db = Some(l.gate_db).filter(|g| g.is_finite());
            }
        }
        self.signal = Some(sig);
        let out = self.engine.tick(Observation { now_ms: now, window: obs_window, rod, bite });
        self.handle(out);
        if let Some(s) = self.session.as_mut() {
            if s.last_save.elapsed() > Duration::from_secs(10) {
                s.last_save = Instant::now();
                let _ = self.stats.lock().unwrap().progress(s.id, self.engine.catches(), self.engine.active_ms());
            }
        }
    }

    fn click(&self) {
        let fg = self.tracker.win.as_ref().is_some_and(|w| window::is_foreground(w.hwnd));
        if fg || !self.settings.advanced.focus_guard {
            moyu_win::input::right_click(self.settings.advanced.click_hold_ms);
        } else {
            tracing::warn!("游戏不在前台，跳过点击");
        }
    }

    fn handle(&mut self, outs: Vec<Output>) {
        for o in outs {
            match o {
                Output::Click => self.click(),
                Output::Event(ev) => self.on_event(ev),
            }
        }
    }

    fn on_event(&mut self, ev: Event) {
        self.dirty = true;
        let sid = self.session.as_ref().map(|s| s.id);
        let record = |kind: &str, data: serde_json::Value, stats: &Arc<Mutex<Stats>>| {
            if let Some(id) = sid {
                if let Err(e) = stats.lock().unwrap().event(id, kind, &data) {
                    tracing::warn!("写统计失败：{e}");
                }
            }
        };
        match &ev {
            Event::Started | Event::Phase { .. } => {}
            Event::Cast => {
                // 用这次甩竿声的音量给咬钩判定定绝对门槛（跟着游戏音量走）
                if let Some(a) = self.session.as_mut().and_then(|s| s.audio.as_mut()) {
                    a.det.note_cast();
                }
                record("cast", serde_json::json!({}), &self.stats)
            }
            Event::Reel { cause } => record("reel", serde_json::json!({ "cause": cause }), &self.stats),
            Event::CastFailed { consecutive } => {
                record("castFailed", serde_json::json!({ "consecutive": consecutive }), &self.stats)
            }
            Event::Empty { waited_ms } => record("empty", serde_json::json!({ "waitedMs": waited_ms }), &self.stats),
            Event::Bounced { out_ms } => record("bounced", serde_json::json!({ "outMs": out_ms }), &self.stats),
            Event::Catch { bite_wait_ms, total } => {
                record("catch", serde_json::json!({ "biteWaitMs": bite_wait_ms }), &self.stats);
                if let Some(s) = self.session.as_mut() {
                    s.bite_waits.push(*bite_wait_ms);
                    let _ = self.stats.lock().unwrap().progress(s.id, *total, self.engine.active_ms());
                }
            }
            Event::Paused { reason } => {
                record("pause", serde_json::json!({ "reason": reason }), &self.stats);
                if matches!(reason, PauseReason::CastFailed | PauseReason::RodUnknown) && self.settings.notify.on_error {
                    notify::pause(&self.app, *reason);
                }
            }
            Event::Resumed => record("resume", serde_json::json!({}), &self.stats),
            Event::Stopped { reason, catches, active_ms } => {
                record("stop", serde_json::json!({ "reason": reason }), &self.stats);
                if let Some(s) = self.session.take() {
                    let _ = self.stats.lock().unwrap().end(s.id, *catches, *active_ms, &format!("{reason:?}"));
                    let avg = avg(&s.bite_waits);
                    if *reason != StopReason::User && self.settings.notify.on_stop {
                        notify::stopped(&self.app, *reason, *catches, *active_ms, avg);
                    }
                }
                tracing::info!("结束钓鱼：{reason:?}，{catches} 条，{} 秒", active_ms / 1000);
                self.status.lock().unwrap().last_stop = Some(*reason);
                self.signal = None;
            }
        }
        if !matches!(ev, Event::Phase { .. } | Event::Started) {
            let _ = self.app.emit("engine-event", FeedItem { at: chrono::Local::now().timestamp_millis(), event: ev });
        }
    }

    // ---- 校准 ----

    fn cal_open(&mut self) {
        if !matches!(self.cal.as_ref().map(|c| &c.step), Some(CalStep::Countdown { .. } | CalStep::Sampling(_))) {
            self.cal = Some(CalRun {
                step: CalStep::Prepare,
                progress: Default::default(),
                preview: None,
                last_preview: None,
                last_in_frame: None,
            });
        }
        let _ = self.app.emit("navigate", "calibrate");
        self.dirty = true;
    }

    fn cal_countdown(&mut self, now: u64) {
        if self.cal.is_none() {
            self.cal_open();
        }
        if self.engine.is_running() && self.engine.pause_reason().is_none() {
            let out = self.engine.toggle_pause(now);
            self.handle(out);
        }
        if let Some(c) = self.cal.as_mut() {
            c.step = CalStep::Countdown { until: now + COUNTDOWN_MS };
            c.progress = Default::default();
        }
    }

    fn cal_cancel(&mut self, why: &str) {
        if let Some(c) = self.cal.as_mut() {
            c.step = if why.is_empty() { CalStep::Prepare } else { CalStep::Failed(why.into()) };
        }
    }

    fn union_grid(&mut self, geom: &Geometry) -> Option<Grid> {
        let w = self.tracker.win.as_ref()?;
        let (ux, uy, uw, uh) = geom.union.to_px(geom.k);
        let img = self.cap.grab(w.x + ux as i32, w.y + uy as i32, uw, uh).ok()?;
        Some(img.to_grid(geom.k as usize))
    }

    fn tick_cal(&mut self, now: u64) {
        let fg = self.tracker.win.as_ref().is_some_and(|w| !w.minimized && window::is_foreground(w.hwnd));
        let size = self.tracker.win.as_ref().map(|w| (w.w, w.h));
        let Some(mut run) = self.cal.take() else { return };
        let mut frame_for_preview = None;
        let mut analyze = false;
        match &mut run.step {
            CalStep::Prepare | CalStep::Done(_) | CalStep::Failed(_) => {
                if let (true, Some((w, h))) = (fg, size) {
                    if run.last_preview.is_none_or(|t| t.elapsed() > Duration::from_millis(700)) {
                        frame_for_preview = self.union_grid(&Geometry::new(w, h));
                    }
                }
            }
            CalStep::Countdown { until } => {
                if now >= *until {
                    run.step = match (fg, size) {
                        (true, Some((w, h))) if w >= 320 && h >= 200 => {
                            tracing::info!("开始校准：{w}x{h}");
                            CalStep::Sampling(Box::new(Calibrator::new(w, h)))
                        }
                        (_, None) => CalStep::Failed("找不到 Minecraft".into()),
                        (false, _) => {
                            tracing::info!("倒计时结束时游戏不在前台，校准取消");
                            CalStep::Failed("请先切回游戏，再按 F6".into())
                        }
                        _ => CalStep::Failed("请先切回游戏，再按 F6".into()),
                    };
                }
            }
            CalStep::Sampling(c) => {
                if !fg || size != Some((c.geom.client_w, c.geom.client_h)) {
                    run.step = CalStep::Failed("校准时切出了游戏或改了窗口大小，已中止".into());
                } else {
                    let geom = c.geom.clone();
                    let frame = self.union_grid(&geom);
                    if let Some(f) = &frame {
                        if c.progress().sampling == Some(RodState::In) {
                            run.last_in_frame = Some(f.clone());
                        }
                        if run.last_preview.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
                            frame_for_preview = Some(f.clone());
                        }
                    }
                    let outs = c.tick(now, frame);
                    for o in outs {
                        match o {
                            CalOutput::Click => {
                                moyu_win::input::right_click(self.settings.advanced.click_hold_ms);
                            }
                            CalOutput::Progress(p) => run.progress = p,
                        }
                    }
                    analyze = c.is_done();
                }
            }
        }
        if analyze {
            // 分析要几百毫秒：先把"分析中"推给界面
            self.cal = Some(clone_run_meta(&run));
            self.publish_with_step("analyzing");
            self.cal = None;
            let res = match &run.step {
                CalStep::Sampling(c) => c.finish(),
                _ => unreachable!(),
            };
            run.step = match res {
                Ok(res) => match calib_store::save(&res, run.last_in_frame.as_ref()) {
                    Ok(stored) => {
                        tracing::info!("校准完成：{:?} {:?} 区分度 {:.1}", res.source, res.roi_px(), res.ratio);
                        let view = CalView::from(&stored);
                        self.calibration = Some(stored);
                        if !self.settings.onboarded {
                            self.settings.onboarded = true;
                            // 设置的"真身"在 AppState 里（命令也会改它），从那里改再保存
                            let state = self.app.state::<crate::AppState>();
                            let mut cur = state.settings.lock().unwrap();
                            cur.onboarded = true;
                            let _ = cur.save();
                            let _ = self.app.emit("settings-changed", &*cur);
                        }
                        CalStep::Done(view)
                    }
                    Err(e) => CalStep::Failed(format!("保存校准失败：{e}")),
                },
                Err(e) => {
                    tracing::info!("校准失败：{e:?}");
                    CalStep::Failed(e.to_string())
                }
            };
        }
        if let Some(f) = frame_for_preview {
            run.preview = calib_store::data_url(&f);
            run.last_preview = Some(Instant::now());
        }
        self.cal = Some(run);
    }

    // ---- 浮层 / 状态推送 ----

    fn sync_overlay(&mut self) {
        let active = self.engine.is_running()
            || matches!(self.cal.as_ref().map(|c| &c.step), Some(CalStep::Countdown { .. } | CalStep::Sampling(_)));
        let target = self.tracker.win.as_ref().filter(|w| !w.minimized && window::is_foreground(w.hwnd));
        let show = self.settings.ui.overlay && self.overlay_on && active && target.is_some();
        let debug = self.settings.advanced.debug_overlay;
        overlay::sync(&self.app, show.then(|| target.unwrap()), debug);
    }

    fn build_status(&self) -> Status {
        let now = self.now();
        let win = self.tracker.win.as_ref();
        let calibration = self
            .calibration
            .as_ref()
            .filter(|c| win.is_none_or(|w| (c.result.client_w, c.result.client_h) == (w.w, w.h)))
            .map(CalView::from);
        let calib = self.cal.as_ref().map(|c| {
            let geom = win.map(|w| Geometry::new(w.w, w.h));
            let (step, countdown, error, result) = match &c.step {
                CalStep::Prepare => ("prepare", None, None, None),
                CalStep::Countdown { until } => {
                    ("countdown", Some(((until.saturating_sub(now)) as f32 / 1000.0).ceil() as u32), None, None)
                }
                CalStep::Sampling(_) => ("sampling", None, None, None),
                CalStep::Done(v) => ("done", None, None, Some(v.clone())),
                CalStep::Failed(e) => ("failed", None, Some(e.clone()), None),
            };
            CalRunView {
                step,
                countdown,
                progress: c.progress.progress,
                round: c.progress.round.max(1),
                sampling: c.progress.sampling,
                frames_in: c.progress.frames_in,
                frames_out: c.progress.frames_out,
                error,
                result,
                union_rel: geom.as_ref().map(|g| RelRect::from_cells(g.union, g.grid_w, g.grid_h)),
                candidates: geom
                    .as_ref()
                    .map(|g| g.candidates.iter().map(|(s, r)| (*s, RelRect::from_cells(*r, g.grid_w, g.grid_h))).collect())
                    .unwrap_or_default(),
                preview: c.preview.clone(),
            }
        });
        let session = self.session.as_ref().map(|s| SessionView {
            catches: self.engine.catches(),
            active_ms: self.engine.active_ms(),
            out_for_ms: self.engine.out_since().map(|t| now.saturating_sub(t)),
            avg_bite_ms: avg(&s.bite_waits),
        });
        let last_stop = self.status.lock().unwrap().last_stop;
        Status {
            phase: self.engine.phase(),
            pause_reason: self.engine.pause_reason(),
            last_stop,
            mode: self.settings.mode,
            session,
            window: win.map(|w| WindowView {
                title: w.title.clone(),
                process: w.process.clone(),
                w: w.w,
                h: w.h,
                foreground: window::is_foreground(w.hwnd),
                pinned: self.settings.advanced.window.is_some(),
            }),
            calibration,
            calib,
            audio: self.session.as_ref().and_then(|s| s.audio.as_ref().map(|a| a._cap.source)),
            signal: self.signal.clone(),
            overlay_on: self.overlay_on,
        }
    }

    fn publish(&mut self) {
        let st = self.build_status();
        tray::update(&self.app, st.phase, self.engine.is_running());
        *self.status.lock().unwrap() = st.clone();
        let _ = self.app.emit("status", st);
        self.last_emit = Instant::now();
        self.dirty = false;
    }

    fn publish_with_step(&mut self, step: &'static str) {
        let mut st = self.build_status();
        if let Some(c) = st.calib.as_mut() {
            c.step = step;
            c.progress = 1.0;
        }
        let _ = self.app.emit("status", st);
    }
}

fn clone_run_meta(r: &CalRun) -> CalRun {
    CalRun {
        step: CalStep::Prepare,
        progress: r.progress.clone(),
        preview: r.preview.clone(),
        last_preview: r.last_preview,
        last_in_frame: None,
    }
}

fn cmd_name(c: &Cmd) -> &'static str {
    match c {
        Cmd::Hotkey => "hotkey",
        Cmd::Start => "start",
        Cmd::TogglePause => "togglePause",
        Cmd::Stop => "stop",
        Cmd::CalOpen => "calOpen",
        Cmd::CalStart => "calStart",
        Cmd::CalCancel => "calCancel",
        Cmd::CalReset => "calReset",
        Cmd::ToggleOverlay => "toggleOverlay",
        Cmd::Settings(_) => "settings",
        Cmd::PickWindow(_) => "pickWindow",
        Cmd::FocusGame => "focusGame",
        Cmd::Quit(_) => "quit",
    }
}

fn avg(v: &[u64]) -> Option<u64> {
    (!v.is_empty()).then(|| v.iter().sum::<u64>() / v.len() as u64)
}

pub fn mode_str(m: Mode) -> &'static str {
    match m {
        Mode::RodOnly => "rodOnly",
        Mode::Full => "full",
    }
}
