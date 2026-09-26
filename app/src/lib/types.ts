// 与 Rust 端 serde(camelCase) 结构一一对应。

export type Mode = "rodOnly" | "full";
export type Phase = "idle" | "starting" | "casting" | "waiting" | "reeling" | "caught" | "paused";
export type PauseReason = "user" | "notForeground" | "windowMissing" | "sizeChanged" | "rodUnknown" | "castFailed";
export type StopReason = "user" | "gameClosed" | "duration" | "catches" | "deadline";
export type RodState = "in" | "out" | "unknown";
export type Quality = "poor" | "good" | "excellent";
export type Source = "hand" | "hotbar";

export interface RelRect { x: number; y: number; w: number; h: number }

export interface WindowView {
  title: string;
  process: string;
  w: number;
  h: number;
  foreground: boolean;
  pinned: boolean;
}

export interface CalView {
  w: number;
  h: number;
  quality: Quality;
  ratio: number;
  source: Source;
  rel: RelRect;
  accuracy: number;
  createdAt: string;
}

export type CalStep = "prepare" | "countdown" | "sampling" | "analyzing" | "done" | "failed";

export interface CalRunView {
  step: CalStep;
  countdown: number | null;
  progress: number;
  round: number;
  sampling: RodState | null;
  framesIn: number;
  framesOut: number;
  error: string | null;
  result: CalView | null;
  unionRel: RelRect | null;
  candidates: [Source, RelRect][];
  preview: string | null;
}

export interface SessionView {
  catches: number;
  activeMs: number;
  outForMs: number | null;
  avgBiteMs: number | null;
}

export interface SignalView {
  dIn: number;
  dOut: number;
  rod: RodState | null;
  audioDb: number | null;
  audioGateDb: number | null;
}

export interface Status {
  phase: Phase;
  pauseReason: PauseReason | null;
  lastStop: StopReason | null;
  mode: Mode;
  session: SessionView | null;
  window: WindowView | null;
  calibration: CalView | null;
  calib: CalRunView | null;
  audio: "process" | "system" | null;
  signal: SignalView | null;
  overlayOn: boolean;
}

export type EngineEvent =
  | { kind: "started" }
  | { kind: "cast" }
  | { kind: "castFailed"; consecutive: number }
  | { kind: "reel"; cause: "bite" | "timeout" }
  | { kind: "catch"; biteWaitMs: number; total: number }
  | { kind: "empty"; waitedMs: number }
  | { kind: "paused"; reason: PauseReason }
  | { kind: "resumed" }
  | { kind: "stopped"; reason: StopReason; catches: number; activeMs: number }
  | { kind: "phase"; phase: Phase; reason: PauseReason | null };

export interface FeedItem { at: number; event: EngineEvent }

export type Theme = "system" | "light" | "dark";
export type AutoStopKind = "none" | "minutes" | "catches" | "at";
export type Sensitivity = "low" | "normal" | "high";

export interface WindowMatch { process: string; title: string }

export interface Settings {
  version: number;
  mode: Mode;
  biteSource: "audio" | "visual";
  hotkeys: { toggle: string; overlay: string };
  autoStop: { kind: AutoStopKind; value: number };
  ui: { theme: Theme; alwaysOnTop: boolean; closeToTray: boolean; overlay: boolean; catchSound: boolean };
  notify: { onStop: boolean; onError: boolean };
  update: { auto: boolean; channel: "stable" | "beta"; skipVersion: string | null };
  advanced: {
    clickHoldMs: number;
    maxWaitS: number;
    focusGuard: boolean;
    window: WindowMatch | null;
    biteSensitivity: Sensitivity;
    debugOverlay: boolean;
  };
  onboarded: boolean;
}

export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object | null ? DeepPartial<NonNullable<T[K]>> | T[K] : T[K] };

export interface GameWindow {
  hwnd: number;
  pid: number;
  process: string;
  title: string;
  w: number;
  h: number;
  minimized: boolean;
}

export interface StatsView {
  summary: {
    catches: number;
    activeMs: number;
    sessions: number;
    since: string | null;
    todayCatches: number;
    avgBiteMs: number | null;
  };
  days: { date: string; catches: number }[];
  biteBuckets: { label: string; count: number }[];
  recent: {
    id: number;
    startedAt: string;
    activeMs: number;
    catches: number;
    mode: Mode;
    source: "app" | "legacy";
    stopReason: string | null;
  }[];
}

export interface UpdateInfo { version: string; current: string; notes: string | null; date: string | null }

export interface AppInfo { version: string; dataDir: string }
