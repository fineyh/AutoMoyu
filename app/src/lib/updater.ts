// 自动更新：启动 10 秒后检查，之后每 24 小时一次；发现新版本就后台下载。
// 钓鱼进行中不打扰——横幅只在没在钓鱼时出现（见 UpdateBanner）。

import { api, inTauri, on } from "./api";
import { useStore } from "./store";

const FIRST_CHECK_MS = 10_000;
const EVERY_MS = 24 * 3600_000;

export async function checkForUpdate(manual = false) {
  const { setUpdate, settings, update } = useStore.getState();
  if (update.state === "downloading" || update.state === "ready") return update;
  try {
    const info = await api.updateCheck();
    if (!info || (!manual && settings?.update.skipVersion === info.version)) {
      setUpdate({ state: "none" });
      return useStore.getState().update;
    }
    setUpdate({ state: "available", info });
    if (settings?.update.auto) void download();
  } catch (e) {
    // 检查失败静默（国内网络常见），手动检查时才提示
    if (manual) setUpdate({ state: "error", message: String(e) });
  }
  return useStore.getState().update;
}

export async function download() {
  const u = useStore.getState().update;
  if (u.state !== "available") return;
  const info = u.info;
  useStore.getState().setUpdate({ state: "downloading", info, progress: null });
  try {
    await api.updateDownload();
    useStore.getState().setUpdate({ state: "ready", info });
  } catch (e) {
    useStore.getState().setUpdate({ state: "available", info });
    useStore.getState().showToast(`下载更新失败：${e}`);
  }
}

export function startUpdateLoop() {
  if (!inTauri) return;
  void on<{ state: string; downloaded: number; total: number | null }>("update-progress", (p) => {
    const u = useStore.getState().update;
    if (u.state === "downloading" && p.total) {
      useStore.getState().setUpdate({ ...u, progress: p.downloaded / p.total });
    }
  });
  const tick = () => {
    if (useStore.getState().settings?.update.auto) void checkForUpdate();
  };
  setTimeout(tick, FIRST_CHECK_MS);
  setInterval(tick, EVERY_MS);
}
