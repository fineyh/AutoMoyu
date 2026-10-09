// 浏览器预览用的假后端（pnpm web）。?mock=idle|running|paused|takeover|calib|done|nowin|update|spatial 切换场景，&lang=en|zh 指定语言。
// 只用于看界面，打包进 Tauri 后不会走到这里。

import type { FeedItem, Settings, StatsView, Status } from "./types";

const params = new URLSearchParams(location.search);
const scene = params.get("mock") ?? "running";
const lang = params.get("lang");
const listeners = new Map<string, Set<(p: unknown) => void>>();
const emit = (e: string, p: unknown) => listeners.get(e)?.forEach((cb) => cb(p));

const settings: Settings = {
  version: 1,
  mode: scene === "spatial" ? "full" : "rodOnly",
  biteSource: "audio",
  hotkeys: { toggle: "F6", overlay: "F7" },
  autoStop: { kind: "none", value: 0 },
  ui: { theme: "system", language: lang === "en" || lang === "zh" ? lang : "auto", alwaysOnTop: false, closeToTray: true, overlay: true, catchSound: false },
  notify: { onStop: true, onError: true },
  update: { auto: true, channel: "beta", skipVersion: null },
  advanced: { clickHoldMs: 90, maxWaitS: 60, focusGuard: true, takeover: true, takeoverIdleS: 5, window: null, biteSensitivity: "normal", debugOverlay: false },
  onboarded: true,
};

const cal = {
  w: 1920, h: 1080, quality: "excellent" as const, ratio: 43.9, source: "hand" as const,
  rel: { x: 0.7375, y: 0.6, w: 0.05, h: 0.089 }, accuracy: 1, createdAt: "2026-09-26T12:00:00+08:00",
};

let t = 0;
let catches = 23;
const status = (): Status => {
  const base: Status = {
    phase: "idle", pauseReason: null, lastStop: null, mode: settings.mode, session: null,
    window: { title: "Minecraft", process: "Minecraft.Windows.exe", w: 1920, h: 1080, foreground: true, pinned: false },
    calibration: cal, calib: null, audio: null, signal: null, overlayOn: true, spatialSound: scene === "spatial",
    lang: settings.ui.language !== "auto" ? settings.ui.language : navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en",
  };
  if (scene === "nowin") return { ...base, window: null, calibration: null };
  if (scene === "idle" || scene === "update" || scene === "spatial") return base;
  if (scene === "calib" || scene === "done") {
    return {
      ...base,
      calib: {
        step: scene === "done" ? "done" : "sampling", countdown: null, progress: 0.62, round: 2, sampling: "out",
        framesIn: 20, framesOut: 17, error: null, result: scene === "done" ? cal : null,
        unionRel: { x: 0.3, y: 0.45, w: 0.7, h: 0.55 },
        candidates: [["hand", { x: 0.5, y: 0.45, w: 0.5, h: 0.55 }], ["hotbar", { x: 0.3, y: 0.85, w: 0.4, h: 0.15 }]],
        preview: null,
      },
    };
  }
  const cyc = t % 120; // 8 秒一条
  const pausedBy = scene === "paused" ? "notForeground" : scene === "takeover" ? "userActive" : null;
  const phase = pausedBy ? "paused" : cyc < 6 ? "casting" : cyc < 100 ? "waiting" : cyc < 106 ? "reeling" : "caught";
  return {
    ...base,
    phase,
    pauseReason: pausedBy,
    session: {
      catches, activeMs: 724_000 + t * 66, outForMs: phase === "waiting" ? (cyc - 6) * 66 : null, avgBiteMs: 8100,
      resumeInMs: pausedBy === "userActive" ? 5000 - ((t * 66) % 5000) : null,
    },
    signal: { dIn: 3.1, dOut: 18.4, rod: "out", audioDb: -48.2, audioGateDb: -31.5 },
  };
};

const feed: FeedItem[] = [];
setInterval(() => {
  t++;
  if (scene === "running" && t % 120 === 106) {
    catches++;
    const at = Date.now();
    emit("engine-event", { at, event: { kind: "catch", biteWaitMs: 6200 + ((t * 37) % 5000), total: catches } });
    setTimeout(() => emit("engine-event", { at: Date.now(), event: { kind: "cast" } }), 700);
  }
  emit("status", status());
}, 66);

const stats: StatsView = {
  summary: { catches: 305, activeMs: 4_373_820, sessions: 108, since: "2026-07-08", todayCatches: 23, avgBiteMs: 8100 },
  days: [
    { date: "2026-07-08", catches: 85 }, { date: "2026-07-09", catches: 141 }, { date: "2026-07-10", catches: 33 },
    { date: "2026-07-11", catches: 12 }, { date: "2026-07-12", catches: 34 },
  ],
  biteBuckets: [
    { label: "<5", count: 18 }, { label: "5–10", count: 61 }, { label: "10–15", count: 29 },
    { label: "15–20", count: 9 }, { label: "20–30", count: 4 }, { label: "30+", count: 1 },
  ],
  recent: [
    { id: 3, startedAt: "2026-07-12T20:37:00+08:00", activeMs: 242_000, catches: 34, mode: "rodOnly", source: "legacy", stopReason: "user" },
    { id: 2, startedAt: "2026-07-11T04:23:00+08:00", activeMs: 152_000, catches: 11, mode: "rodOnly", source: "legacy", stopReason: "user" },
    { id: 1, startedAt: "2026-07-10T05:24:00+08:00", activeMs: 57_000, catches: 5, mode: "full", source: "legacy", stopReason: "user" },
  ],
};

export async function mockInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "get_status": return status();
    case "get_settings": return structuredClone(settings);
    case "set_settings": {
      const patch = (args?.patch ?? {}) as Record<string, unknown>;
      for (const [k, v] of Object.entries(patch)) {
        const cur = (settings as unknown as Record<string, unknown>)[k];
        (settings as unknown as Record<string, unknown>)[k] =
          v && typeof v === "object" && !Array.isArray(v) && cur && typeof cur === "object" ? { ...cur, ...v } : v;
      }
      emit("settings-changed", structuredClone(settings));
      emit("status", status());
      return structuredClone(settings);
    }
    case "get_stats": return stats;
    case "list_game_windows":
      return [{ hwnd: 1, pid: 42, process: "Minecraft.Windows.exe", title: "Minecraft", w: 1920, h: 1080, minimized: false }];
    case "app_info": return { version: "1.0.0-beta.1", dataDir: "C:\\Users\\me\\AppData\\Roaming\\AutoMoyu" };
    case "update_check":
      return scene === "update"
        ? {
            version: "1.0.0-beta.2", current: "1.0.0-beta.1", date: null,
            notes:
              "\n- 修好了一个问题\n- 统计页加了一张图\n- 托盘菜单可以直接切换模式\n- 浮层位置会记住\n- 校准更快了\n- 自动停止条件多了一项\n\n**English**\n\n- Fixed a problem\n- Added a chart to the stats page\n- Switch modes right from the tray menu\n- The overlay remembers where you put it\n- Calibration is faster\n- One more auto-stop condition\n\n---\n\n安装：…\n\nInstall: …",
          }
        : null;
    case "update_download": return new Promise((r) => setTimeout(r, 1500));
    default: return null;
  }
}

export function mockListen(event: string, cb: (p: unknown) => void) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  if (event === "engine-event" && feed.length === 0 && scene === "running") {
    const now = Date.now();
    [
      { at: now - 1000, event: { kind: "cast" } },
      { at: now - 2000, event: { kind: "catch", biteWaitMs: 7900, total: 23 } },
      { at: now - 11000, event: { kind: "cast" } },
      { at: now - 12000, event: { kind: "catch", biteWaitMs: 11200, total: 22 } },
    ].reverse().forEach((f) => setTimeout(() => cb(f), 10));
  }
  return () => listeners.get(event)!.delete(cb);
}
