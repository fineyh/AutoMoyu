// 对 Rust 命令的薄封装。浏览器里直接打开（pnpm web）时走 mock，方便看界面。

import type {
  AppInfo,
  DeepPartial,
  GameWindow,
  Settings,
  StatsView,
  Status,
  UpdateInfo,
  WindowMatch,
} from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Unlisten = () => void;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) {
    const { mockInvoke } = await import("./mock");
    return mockInvoke(cmd, args) as T;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function on<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
  if (!inTauri) {
    const { mockListen } = await import("./mock");
    return mockListen(event, cb as (p: unknown) => void);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

export const api = {
  status: () => call<Status | null>("get_status"),
  settings: () => call<Settings>("get_settings"),
  setSettings: (patch: DeepPartial<Settings>) => call<Settings>("set_settings", { patch }),
  hotkey: () => call<void>("hotkey"),
  start: () => call<void>("start"),
  togglePause: () => call<void>("toggle_pause"),
  stop: () => call<void>("stop"),
  calOpen: () => call<void>("calibration_open"),
  calStart: () => call<void>("calibration_start"),
  calCancel: () => call<void>("calibration_cancel"),
  calReset: () => call<void>("calibration_reset"),
  focusGame: () => call<void>("focus_game"),
  listWindows: () => call<GameWindow[]>("list_game_windows"),
  pickWindow: (pick: WindowMatch | null) => call<Settings>("pick_window", { pick }),
  stats: (days = 7) => call<StatsView>("get_stats", { days }),
  exportCsv: (path: string) => call<number>("export_stats_csv", { path }),
  importLegacy: (path: string) => call<number>("import_legacy", { path }),
  exportDiagnostics: (path: string) => call<void>("export_diagnostics", { path }),
  appInfo: () => call<AppInfo>("app_info"),
  updateCheck: () => call<UpdateInfo | null>("update_check"),
  updateDownload: () => call<void>("update_download"),
  updateInstall: () => call<void>("update_install"),
};

// ---- 窗口 / 对话框 / 外链 ----

export async function currentWindow() {
  if (!inTauri) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

export async function saveDialog(defaultPath: string, name: string, ext: string[]): Promise<string | null> {
  if (!inTauri) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath, filters: [{ name, extensions: ext }] });
}

export async function openDialog(name: string, ext: string[]): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: false, directory: false, filters: [{ name, extensions: ext }] });
  return typeof r === "string" ? r : null;
}

export async function openUrl(url: string) {
  if (!inTauri) {
    window.open(url, "_blank");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

export async function revealPath(path: string) {
  if (!inTauri) return;
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}
