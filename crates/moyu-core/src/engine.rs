//! 主循环状态机：电平触发、自带纠错。
//!
//! 每帧只问"竿现在是什么状态"，然后做对应的事：
//! - 收回 → 甩竿；2 秒内没变"甩出"算一次失败，连续 3 次暂停。
//! - 甩出 → （全自动）听到咬钩就收竿；超过最长等待也收竿（保险）。
//! - 甩出 → 收回（且甩出 ≥1 秒）= 钓到一条。
//! - 玩家在操作（接管）→ 让出控制；停手后竿状态从头认，接管期间的变化不算。
//!
//! sans-IO：调用方喂 `Observation`，按返回的 `Output::Click` 去点，把 `Output::Event` 转给界面/统计。

use serde::{Deserialize, Serialize};

use crate::rod_state::RodState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// 自动甩竿（有钓鱼机，上钩后机器自动收竿）。
    #[default]
    RodOnly,
    /// 全自动（程序自己判断咬钩并收竿）。
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutoStop {
    pub max_active_ms: Option<u64>,
    pub max_catches: Option<u32>,
    /// 引擎时钟下的截止时刻（调用方把"到某个时刻"换算好）。
    pub deadline_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    pub mode: Mode,
    pub focus_guard: bool,
    pub max_wait_ms: u64,
    pub cast_confirm_ms: u64,
    pub max_cast_failures: u32,
    pub debounce_frames: u32,
    pub min_out_for_catch_ms: u64,
    pub post_catch_delay_ms: u64,
    /// 甩出去不到 `min_out_for_catch_ms` 就被收回（弹回）后，下一竿多等这么久。
    pub bounce_backoff_ms: u64,
    pub reel_confirm_ms: u64,
    pub splash_silence_ms: u64,
    pub unknown_pause_ms: u64,
    pub auto_stop: AutoStop,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            mode: Mode::RodOnly,
            focus_guard: true,
            max_wait_ms: 60_000,
            cast_confirm_ms: 2_000,
            max_cast_failures: 3,
            debounce_frames: 3,
            min_out_for_catch_ms: 1_000,
            post_catch_delay_ms: 400,
            bounce_backoff_ms: 1_500,
            reel_confirm_ms: 2_000,
            splash_silence_ms: 1_200,
            unknown_pause_ms: 10_000,
            auto_stop: AutoStop::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowObs {
    /// 找到了、在前台、尺寸和校准一致。
    Ok,
    NotForeground,
    Missing,
    /// 之前跟踪的游戏进程退出了。
    Closed,
    SizeChanged,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub now_ms: u64,
    pub window: WindowObs,
    /// 本帧竿状态分类；没截到画面为 `None`。
    pub rod: Option<RodState>,
    /// 本帧是否检测到咬钩（仅全自动有意义）。
    pub bite: bool,
    /// 玩家最近在操作游戏（接管检测）；为真时让出控制，停手后自动继续。
    pub user_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Starting,
    Casting,
    Waiting,
    Reeling,
    Caught,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PauseReason {
    User,
    NotForeground,
    WindowMissing,
    SizeChanged,
    RodUnknown,
    CastFailed,
    /// 玩家在自己操作游戏。
    UserActive,
}

impl PauseReason {
    /// 条件恢复后是否自动继续。
    pub fn auto_resumes(self) -> bool {
        !matches!(self, PauseReason::User | PauseReason::CastFailed)
    }
    fn from_window(self) -> bool {
        matches!(self, PauseReason::NotForeground | PauseReason::WindowMissing | PauseReason::SizeChanged)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    User,
    GameClosed,
    Duration,
    Catches,
    Deadline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReelCause {
    Bite,
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Started,
    Phase { phase: Phase, reason: Option<PauseReason> },
    Cast,
    CastFailed { consecutive: u32 },
    Reel { cause: ReelCause },
    Catch { bite_wait_ms: u64, total: u32 },
    /// 甩出超时被收回，没算钓到。
    Empty { waited_ms: u64 },
    /// 甩出去很快又被收回（多半是钓鱼机还没复位就把新的一竿收了），没算钓到。
    Bounced { out_ms: u64 },
    Paused { reason: PauseReason },
    Resumed,
    Stopped { reason: StopReason, catches: u32, active_ms: u64 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    Click,
    Event(Event),
}

pub struct Engine {
    cfg: EngineConfig,
    running: bool,
    last_tick: u64,
    active_ms: u64,
    catches: u32,
    pause: Option<PauseReason>,
    stable: Option<RodState>,
    last_known: Option<RodState>,
    cand: Option<RodState>,
    cand_count: u32,
    unknown_since: Option<u64>,
    cast_at: Option<u64>,
    cast_failures: u32,
    out_since: Option<u64>,
    reel_at: Option<u64>,
    reel_cause: Option<ReelCause>,
    cooldown_until: u64,
    caught_until: u64,
    phase: Phase,
    emitted: (Phase, Option<PauseReason>),
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            cfg,
            running: false,
            last_tick: 0,
            active_ms: 0,
            catches: 0,
            pause: None,
            stable: None,
            last_known: None,
            cand: None,
            cand_count: 0,
            unknown_since: None,
            cast_at: None,
            cast_failures: 0,
            out_since: None,
            reel_at: None,
            reel_cause: None,
            cooldown_until: 0,
            caught_until: 0,
            phase: Phase::Idle,
            emitted: (Phase::Idle, None),
        }
    }

    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }
    pub fn set_config(&mut self, cfg: EngineConfig) {
        self.cfg = cfg;
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn pause_reason(&self) -> Option<PauseReason> {
        self.pause
    }
    pub fn catches(&self) -> u32 {
        self.catches
    }
    pub fn active_ms(&self) -> u64 {
        self.active_ms
    }
    /// 当前这一竿已经甩出多久（界面"等待上钩 · 6.2 秒"）。
    pub fn out_since(&self) -> Option<u64> {
        self.out_since
    }

    pub fn start(&mut self, now: u64) -> Vec<Output> {
        let cfg = self.cfg.clone();
        *self = Engine::new(cfg);
        self.running = true;
        self.last_tick = now;
        self.unknown_since = Some(now);
        self.phase = Phase::Starting;
        let mut out = vec![Output::Event(Event::Started)];
        self.emit_phase(&mut out);
        out
    }

    pub fn stop(&mut self, now: u64, reason: StopReason) -> Vec<Output> {
        if !self.running {
            return Vec::new();
        }
        self.account(now);
        self.running = false;
        self.pause = None;
        self.phase = Phase::Idle;
        let mut out = vec![Output::Event(Event::Stopped { reason, catches: self.catches, active_ms: self.active_ms })];
        self.emit_phase(&mut out);
        out
    }

    /// F6：运行中 → 暂停；暂停中 → 继续（条件仍不满足会立刻以对应原因再暂停）。
    pub fn toggle_pause(&mut self, now: u64) -> Vec<Output> {
        let mut out = Vec::new();
        if !self.running {
            return out;
        }
        self.account(now);
        if self.pause.is_some() {
            self.resume(&mut out);
            self.cast_failures = 0;
        } else {
            self.set_pause(PauseReason::User, &mut out);
        }
        self.emit_phase(&mut out);
        out
    }

    fn account(&mut self, now: u64) {
        let dt = now.saturating_sub(self.last_tick);
        if self.pause.is_none() {
            self.active_ms += dt;
        }
        self.last_tick = now;
    }

    fn set_pause(&mut self, reason: PauseReason, out: &mut Vec<Output>) {
        if self.pause == Some(reason) {
            return;
        }
        self.pause = Some(reason);
        self.cast_at = None;
        self.reel_at = None;
        self.phase = Phase::Paused;
        out.push(Output::Event(Event::Paused { reason }));
    }

    fn resume(&mut self, out: &mut Vec<Output>) {
        if self.pause.take().is_some() {
            self.cand = None;
            self.cand_count = 0;
            out.push(Output::Event(Event::Resumed));
        }
    }

    fn forget_rod(&mut self, now: u64) {
        self.stable = None;
        self.last_known = None;
        self.unknown_since = Some(now);
        self.out_since = None;
        self.reel_at = None;
        self.reel_cause = None;
        self.cast_at = None;
        self.caught_until = 0;
        self.cooldown_until = 0;
    }

    fn emit_phase(&mut self, out: &mut Vec<Output>) {
        let cur = (self.phase, if self.phase == Phase::Paused { self.pause } else { None });
        if cur != self.emitted {
            self.emitted = cur;
            out.push(Output::Event(Event::Phase { phase: cur.0, reason: cur.1 }));
        }
    }

    pub fn tick(&mut self, obs: Observation) -> Vec<Output> {
        let mut out = Vec::new();
        if !self.running {
            return out;
        }
        let now = obs.now_ms;
        self.account(now);

        // 1. 窗口
        let blocking = match obs.window {
            WindowObs::Closed => return self.stop(now, StopReason::GameClosed),
            WindowObs::Missing => Some(PauseReason::WindowMissing),
            WindowObs::SizeChanged => Some(PauseReason::SizeChanged),
            WindowObs::NotForeground if self.cfg.focus_guard => Some(PauseReason::NotForeground),
            _ => None,
        };
        let blocking = blocking.or(obs.user_active.then_some(PauseReason::UserActive));
        match (blocking, self.pause) {
            (Some(r), None) => self.set_pause(r, &mut out),
            (Some(r), Some(p)) if p.auto_resumes() => self.set_pause(r, &mut out),
            (None, Some(p)) if p.from_window() => self.resume(&mut out),
            (None, Some(PauseReason::UserActive)) => {
                // 玩家可能自己甩过/收过竿：竿状态从头认，这期间的变化不算钓到
                self.forget_rod(now);
                self.resume(&mut out);
            }
            _ => {}
        }
        if blocking.is_some() {
            self.emit_phase(&mut out);
            return out;
        }

        // 2. 自动停止
        let a = self.cfg.auto_stop;
        if a.max_active_ms.is_some_and(|m| self.active_ms >= m) {
            return self.stop(now, StopReason::Duration);
        }
        if a.deadline_ms.is_some_and(|d| now >= d) {
            return self.stop(now, StopReason::Deadline);
        }

        // 3. 防抖：连续 N 帧一致才采信
        if let Some(raw) = obs.rod {
            if self.cand == Some(raw) {
                self.cand_count += 1;
            } else {
                self.cand = Some(raw);
                self.cand_count = 1;
            }
            if self.cand_count >= self.cfg.debounce_frames && self.stable != Some(raw) {
                self.transition(raw, now, &mut out);
                if !self.running {
                    return out;
                }
            }
        }

        // 4. 长时间认不出 → 暂停；认出来了 → 继续
        let known = matches!(self.stable, Some(RodState::In | RodState::Out));
        match (known, self.pause) {
            (false, None) => {
                if self.unknown_since.is_some_and(|t| now.saturating_sub(t) >= self.cfg.unknown_pause_ms) {
                    self.set_pause(PauseReason::RodUnknown, &mut out);
                }
            }
            (true, Some(PauseReason::RodUnknown)) => self.resume(&mut out),
            _ => {}
        }

        // 5. 按状态行动
        if self.pause.is_some() {
            self.phase = Phase::Paused;
        } else {
            self.act(&obs, &mut out);
        }
        self.emit_phase(&mut out);
        out
    }

    fn transition(&mut self, new: RodState, now: u64, out: &mut Vec<Output>) {
        self.stable = Some(new);
        match new {
            RodState::Unknown => {
                self.unknown_since = Some(now);
                return;
            }
            RodState::Out => {
                if self.cast_at.take().is_some() {
                    self.cast_failures = 0;
                }
                if self.last_known != Some(RodState::Out) || self.out_since.is_none() {
                    self.out_since = Some(now);
                }
            }
            RodState::In => {
                if self.last_known == Some(RodState::Out) {
                    self.cooldown_until = now + self.cfg.post_catch_delay_ms;
                    if let Some(since) = self.out_since.filter(|_| self.pause.is_none()) {
                        let waited = now.saturating_sub(since);
                        if waited < self.cfg.min_out_for_catch_ms {
                            // 刚甩出就被收回：马上再甩多半还会被收，等钓鱼机复位
                            out.push(Output::Event(Event::Bounced { out_ms: waited }));
                            self.cooldown_until = now + self.cfg.bounce_backoff_ms;
                        } else if self.reel_cause == Some(ReelCause::Timeout) {
                            out.push(Output::Event(Event::Empty { waited_ms: waited }));
                        } else {
                            self.catches += 1;
                            let bite_wait_ms = self.reel_at.map_or(waited, |r| r.saturating_sub(since));
                            out.push(Output::Event(Event::Catch { bite_wait_ms, total: self.catches }));
                            self.caught_until = now + 900;
                        }
                    }
                }
                self.out_since = None;
                self.reel_at = None;
                self.reel_cause = None;
            }
        }
        self.unknown_since = None;
        self.last_known = Some(new);
        if let Some(m) = self.cfg.auto_stop.max_catches {
            if self.catches >= m {
                out.extend(self.stop(now, StopReason::Catches));
            }
        }
    }

    fn act(&mut self, obs: &Observation, out: &mut Vec<Output>) {
        let now = obs.now_ms;
        match self.stable {
            Some(RodState::In) => {
                if let Some(t) = self.cast_at {
                    if now.saturating_sub(t) >= self.cfg.cast_confirm_ms {
                        self.cast_at = None;
                        self.cast_failures += 1;
                        out.push(Output::Event(Event::CastFailed { consecutive: self.cast_failures }));
                        if self.cast_failures >= self.cfg.max_cast_failures {
                            self.set_pause(PauseReason::CastFailed, out);
                            return;
                        }
                    }
                }
                if self.cast_at.is_none() && now >= self.cooldown_until {
                    out.push(Output::Click);
                    out.push(Output::Event(Event::Cast));
                    self.cast_at = Some(now);
                }
                self.phase = if self.cast_at.is_none() && now < self.caught_until { Phase::Caught } else { Phase::Casting };
            }
            Some(RodState::Out) => {
                if self.reel_at.is_some_and(|t| now.saturating_sub(t) >= self.cfg.reel_confirm_ms) {
                    self.reel_at = None; // 点了没收回来，允许再点
                }
                if self.reel_at.is_none() {
                    let since = *self.out_since.get_or_insert(now);
                    let out_for = now.saturating_sub(since);
                    let cause = if self.cfg.mode == Mode::Full && obs.bite && out_for >= self.cfg.splash_silence_ms {
                        Some(ReelCause::Bite)
                    } else if out_for >= self.cfg.max_wait_ms {
                        Some(ReelCause::Timeout)
                    } else {
                        None
                    };
                    if let Some(cause) = cause {
                        out.push(Output::Click);
                        out.push(Output::Event(Event::Reel { cause }));
                        self.reel_at = Some(now);
                        self.reel_cause = Some(cause);
                    }
                }
                self.phase = if self.reel_at.is_some() { Phase::Reeling } else { Phase::Waiting };
            }
            Some(RodState::Unknown) | None => {
                if self.phase == Phase::Paused || self.stable.is_none() {
                    self.phase = Phase::Starting;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RodState::*;

    /// 模拟游戏：点一下切换竿状态；可选"钓鱼机"在甩出 N 毫秒后自动收竿。
    struct Sim {
        e: Engine,
        t: u64,
        rod: RodState,
        machine_after: Option<u64>,
        out_at: u64,
        cast_works: bool,
        /// 接下来这么多竿甩出 300ms 就被钓鱼机收回。
        bounces: u32,
        window: WindowObs,
        user_active: bool,
        bite_at: Option<u64>,
        events: Vec<Event>,
        clicks: u32,
    }

    impl Sim {
        fn new(cfg: EngineConfig) -> Self {
            let mut s = Sim {
                e: Engine::new(cfg),
                t: 0,
                rod: In,
                machine_after: Some(5_000),
                out_at: 0,
                cast_works: true,
                bounces: 0,
                window: WindowObs::Ok,
                user_active: false,
                bite_at: None,
                events: vec![],
                clicks: 0,
            };
            let o = s.e.start(0);
            s.absorb(o);
            s
        }
        fn absorb(&mut self, o: Vec<Output>) {
            for x in o {
                match x {
                    Output::Click => {
                        self.clicks += 1;
                        match self.rod {
                            In if self.cast_works => {
                                self.rod = Out;
                                self.out_at = self.t;
                            }
                            Out => self.rod = In,
                            _ => {}
                        }
                    }
                    Output::Event(ev) => self.events.push(ev),
                }
            }
        }
        fn run(&mut self, ms: u64) {
            let end = self.t + ms;
            while self.t < end {
                self.t += 66;
                if self.rod == Out && self.bounces > 0 && self.t - self.out_at >= 300 {
                    self.rod = In;
                    self.bounces -= 1;
                }
                if self.rod == Out {
                    if let Some(m) = self.machine_after {
                        if self.t - self.out_at >= m {
                            self.rod = In;
                        }
                    }
                }
                let bite = self.bite_at.is_some_and(|b| self.rod == Out && self.t - self.out_at >= b);
                let o = self.e.tick(Observation {
                    now_ms: self.t,
                    window: self.window,
                    rod: Some(self.rod),
                    bite,
                    user_active: self.user_active,
                });
                self.absorb(o);
            }
        }
        fn count<F: Fn(&Event) -> bool>(&self, f: F) -> usize {
            self.events.iter().filter(|e| f(e)).count()
        }
    }

    #[test]
    fn rod_only_loop_counts_catches() {
        let mut s = Sim::new(EngineConfig::default());
        s.run(60_000);
        let catches = s.count(|e| matches!(e, Event::Catch { .. }));
        // 每条约 5s 甩出 + 0.2s 防抖 + 0.4s 冷却 + 0.2s 防抖 ≈ 5.8s
        assert!((9..=11).contains(&catches), "catches={catches}");
        assert_eq!(s.e.catches() as usize, catches);
        assert_eq!(s.count(|e| matches!(e, Event::CastFailed { .. })), 0);
        if let Some(Event::Catch { bite_wait_ms, .. }) = s.events.iter().find(|e| matches!(e, Event::Catch { .. })) {
            assert!((4_900..=5_400).contains(bite_wait_ms), "{bite_wait_ms}");
        }
    }

    #[test]
    fn bounced_cast_waits_longer_before_recasting() {
        let mut s = Sim::new(EngineConfig::default());
        s.bounces = 1;
        s.run(1_500);
        assert_eq!(s.count(|e| matches!(e, Event::Bounced { .. })), 1);
        assert_eq!(s.clicks, 1, "弹回后应先等钓鱼机复位");
        s.run(1_000);
        assert_eq!(s.clicks, 2);
        s.run(6_000);
        assert_eq!(s.e.catches(), 1);
        assert_eq!(s.count(|e| matches!(e, Event::CastFailed { .. })), 0);
    }

    #[test]
    fn failed_casts_retry_then_pause() {
        let mut s = Sim::new(EngineConfig::default());
        s.cast_works = false;
        s.run(10_000);
        assert_eq!(s.count(|e| matches!(e, Event::CastFailed { .. })), 3);
        assert_eq!(s.e.pause_reason(), Some(PauseReason::CastFailed));
        assert_eq!(s.clicks, 3);
        // 用户修好后按 F6 继续
        s.cast_works = true;
        let o = s.e.toggle_pause(s.t);
        s.absorb(o);
        s.run(3_000);
        assert_eq!(s.e.phase(), Phase::Waiting);
    }

    #[test]
    fn one_failed_cast_self_heals() {
        let mut s = Sim::new(EngineConfig::default());
        s.cast_works = false;
        s.run(2_100); // 第一次甩竿（约 0.2s）没成功，2s 后判失败并重甩
        s.cast_works = true;
        s.run(8_000);
        assert_eq!(s.count(|e| matches!(e, Event::CastFailed { .. })), 1);
        assert!(s.e.catches() >= 1);
        assert_eq!(s.e.pause_reason(), None);
    }

    #[test]
    fn focus_loss_pauses_and_resumes() {
        let mut s = Sim::new(EngineConfig::default());
        s.run(1_000);
        s.window = WindowObs::NotForeground;
        let clicks = s.clicks;
        s.run(3_000);
        assert_eq!(s.e.pause_reason(), Some(PauseReason::NotForeground));
        s.machine_after = None;
        s.run(2_000);
        assert_eq!(s.clicks, clicks, "不在前台时不能点击");
        s.window = WindowObs::Ok;
        s.machine_after = Some(5_000);
        s.run(500);
        assert_eq!(s.e.pause_reason(), None);
        assert!(s.count(|e| matches!(e, Event::Resumed)) >= 1);
    }

    #[test]
    fn user_takeover_yields_and_does_not_count_manual_catch() {
        let mut s = Sim::new(EngineConfig::default());
        s.machine_after = None;
        s.run(1_000); // 已甩出
        s.user_active = true;
        s.run(300);
        assert_eq!(s.e.pause_reason(), Some(PauseReason::UserActive));
        let clicks = s.clicks;
        // 玩家自己收竿，停手
        s.rod = In;
        s.run(2_000);
        assert_eq!(s.clicks, clicks, "接管期间不能点击");
        s.user_active = false;
        s.machine_after = Some(5_000);
        s.run(1_000);
        assert_eq!(s.e.pause_reason(), None);
        assert_eq!(s.e.catches(), 0, "玩家手动收竿不算钓到");
        assert_eq!(s.clicks, clicks + 1, "停手后接着甩竿");
        s.run(6_000);
        assert_eq!(s.e.catches(), 1);
    }

    #[test]
    fn user_pause_is_not_lifted_by_idle() {
        let mut s = Sim::new(EngineConfig::default());
        s.run(1_000);
        let o = s.e.toggle_pause(s.t);
        s.absorb(o);
        s.user_active = true;
        s.run(500);
        s.user_active = false;
        s.run(500);
        assert_eq!(s.e.pause_reason(), Some(PauseReason::User));
    }

    #[test]
    fn game_closed_stops() {
        let mut s = Sim::new(EngineConfig::default());
        s.run(1_000);
        s.window = WindowObs::Closed;
        s.run(200);
        assert!(!s.e.is_running());
        assert!(s.events.iter().any(|e| matches!(e, Event::Stopped { reason: StopReason::GameClosed, .. })));
    }

    #[test]
    fn max_wait_reels_without_counting() {
        let cfg = EngineConfig { max_wait_ms: 3_000, ..Default::default() };
        let mut s = Sim::new(cfg);
        s.machine_after = None;
        s.run(4_500);
        assert_eq!(s.count(|e| matches!(e, Event::Reel { cause: ReelCause::Timeout })), 1);
        assert_eq!(s.count(|e| matches!(e, Event::Empty { .. })), 1);
        assert_eq!(s.e.catches(), 0);
    }

    #[test]
    fn full_auto_reels_on_bite_after_silence() {
        let cfg = EngineConfig { mode: Mode::Full, ..Default::default() };
        let mut s = Sim::new(cfg);
        s.machine_after = None;
        s.bite_at = Some(4_000);
        s.run(30_000);
        assert!(s.e.catches() >= 4, "catches={}", s.e.catches());
        assert_eq!(s.count(|e| matches!(e, Event::Reel { cause: ReelCause::Timeout })), 0);
    }

    #[test]
    fn splash_on_landing_is_ignored() {
        let cfg = EngineConfig { mode: Mode::Full, max_wait_ms: 8_000, ..Default::default() };
        let mut s = Sim::new(cfg);
        s.machine_after = None;
        s.bite_at = Some(0); // 一直有"水花"，但落水静默期内必须忽略
        s.run(1_500);
        assert_eq!(s.count(|e| matches!(e, Event::Reel { .. })), 0);
    }

    #[test]
    fn unknown_rod_pauses_then_resumes() {
        let mut s = Sim::new(EngineConfig::default());
        s.run(1_000);
        s.rod = Unknown;
        s.machine_after = None;
        s.run(11_000);
        assert_eq!(s.e.pause_reason(), Some(PauseReason::RodUnknown));
        s.rod = In;
        s.machine_after = Some(5_000);
        s.run(1_000);
        assert_eq!(s.e.pause_reason(), None);
    }

    #[test]
    fn auto_stop_by_catches() {
        let cfg = EngineConfig {
            auto_stop: AutoStop { max_catches: Some(2), ..Default::default() },
            ..Default::default()
        };
        let mut s = Sim::new(cfg);
        s.run(60_000);
        assert_eq!(s.e.catches(), 2);
        assert!(!s.e.is_running());
        assert!(s.events.iter().any(|e| matches!(e, Event::Stopped { reason: StopReason::Catches, .. })));
    }

    #[test]
    fn user_pause_excludes_time_and_clicks() {
        let mut s = Sim::new(EngineConfig::default());
        s.run(2_000);
        let o = s.e.toggle_pause(s.t);
        s.absorb(o);
        let (active, clicks) = (s.e.active_ms(), s.clicks);
        s.machine_after = None;
        s.run(5_000);
        assert_eq!(s.clicks, clicks);
        assert!(s.e.active_ms() - active < 100);
        assert_eq!(s.e.phase(), Phase::Paused);
    }

    #[test]
    fn debounce_ignores_single_glitch_frames() {
        let mut s = Sim::new(EngineConfig::default());
        s.machine_after = None;
        s.run(2_000); // 已甩出
        let casts = s.count(|e| matches!(e, Event::Cast));
        // 两帧误判为收回，不应触发甩竿
        for _ in 0..2 {
            s.t += 66;
            let o = s.e.tick(Observation { now_ms: s.t, window: WindowObs::Ok, rod: Some(In), bite: false, user_active: false });
            s.absorb(o);
        }
        s.run(500);
        assert_eq!(s.count(|e| matches!(e, Event::Cast)), casts);
        assert_eq!(s.e.catches(), 0);
    }

    #[test]
    fn events_serialize_for_frontend() {
        let j = serde_json::to_string(&Event::Catch { bite_wait_ms: 7900, total: 3 }).unwrap();
        assert_eq!(j, r#"{"kind":"catch","biteWaitMs":7900,"total":3}"#);
    }
}
