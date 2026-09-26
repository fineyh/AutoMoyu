// 界面文案：短词、人话、不出现技术名词（不写 MAD、阈值、ROI）。

import type { EngineEvent, Phase, PauseReason, Quality, StopReason } from "./types";

export const phaseText: Record<Phase, string> = {
  idle: "待机",
  starting: "准备中",
  casting: "甩竿",
  waiting: "等待上钩",
  reeling: "收竿",
  caught: "钓到了",
  paused: "已暂停",
};

export interface Reason {
  title: string;
  hint: string;
  action?: { label: string; kind: "focus" | "recalibrate" | "pickWindow" };
}

export const pauseText: Record<PauseReason, Reason> = {
  user: { title: "已暂停", hint: "按 F6 或点「继续」接着钓" },
  notForeground: {
    title: "Minecraft 不在前台",
    hint: "切回游戏后会自动继续",
    action: { label: "切回游戏", kind: "focus" },
  },
  windowMissing: {
    title: "找不到 Minecraft",
    hint: "正在每 2 秒重找一次",
    action: { label: "选择窗口", kind: "pickWindow" },
  },
  sizeChanged: {
    title: "窗口大小变了",
    hint: "需要按新尺寸重新校准，约 10 秒",
    action: { label: "重新校准", kind: "recalibrate" },
  },
  rodUnknown: {
    title: "认不出鱼竿",
    hint: "可能打开了菜单、换了物品，或校准的位置不对；画面恢复后自动继续，一直这样就重新校准",
    action: { label: "重新校准", kind: "recalibrate" },
  },
  castFailed: {
    title: "没甩出去",
    hint: "手上可能不是鱼竿，或面前没有水",
    action: { label: "重新校准", kind: "recalibrate" },
  },
  userActive: { title: "你在操作游戏，已让出控制", hint: "停手几秒后自动继续" },
};

export const stopText: Record<StopReason, string> = {
  user: "本次钓鱼结束",
  gameClosed: "Minecraft 已关闭，本次钓鱼结束",
  duration: "到达设定时长，已停止",
  catches: "钓满设定条数，已停止",
  deadline: "到点了，已停止",
};

export const qualityText: Record<Quality, string> = { excellent: "优", good: "良", poor: "差" };

export function secs(ms: number, digits = 1) {
  return `${(ms / 1000).toFixed(digits)} 秒`;
}

/** 事件流里的一行；返回 null 表示不显示。 */
export function feedLine(e: EngineEvent): { text: string; sub?: string; tone?: "catch" | "warn" } | null {
  switch (e.kind) {
    case "cast":
      return { text: "甩竿" };
    case "catch":
      return { text: "钓到 1 条", sub: `上钩用时 ${secs(e.biteWaitMs)}`, tone: "catch" };
    case "empty":
      return { text: "等太久没上钩，收竿重甩", sub: secs(e.waitedMs, 0) };
    case "bounced":
      return { text: "刚甩出就被收回，等一下再甩", sub: secs(e.outMs) };
    case "castFailed":
      return { text: "没甩出去，重试", sub: `第 ${e.consecutive} 次`, tone: "warn" };
    case "reel":
      return e.cause === "bite" ? { text: "听到咬钩，收竿" } : null;
    case "paused":
      return { text: pauseText[e.reason].title, tone: "warn" };
    case "resumed":
      return { text: "继续" };
    case "stopped":
      return { text: stopText[e.reason], sub: `${e.catches} 条` };
    default:
      return null;
  }
}

export function clock(ms: number) {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const p = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${p(m)}:${p(ss)}` : `${p(m)}:${p(ss)}`;
}

export function humanDuration(ms: number) {
  const m = Math.round(ms / 60000);
  if (m < 60) return `${m} 分`;
  return `${Math.floor(m / 60)} 小时 ${m % 60} 分`;
}

export function timeOf(at: number) {
  const d = new Date(at);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
