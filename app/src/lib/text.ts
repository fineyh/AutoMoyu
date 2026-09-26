// 界面文案：短词、人话、不出现技术名词（不写 MAD、阈值、ROI）。
// 都是函数：语言切换后重新渲染就拿到新语言。

import { plural, t } from "./i18n";
import type { EngineEvent, Phase, PauseReason, Quality, StopReason } from "./types";

export function phaseText(p: Phase): string {
  switch (p) {
    case "idle":
      return t("待机", "Idle");
    case "starting":
      return t("准备中", "Starting");
    case "casting":
      return t("甩竿", "Casting");
    case "waiting":
      return t("等待上钩", "Waiting for a bite");
    case "reeling":
      return t("收竿", "Reeling in");
    case "caught":
      return t("钓到了", "Caught!");
    case "paused":
      return t("已暂停", "Paused");
  }
}

export interface Reason {
  title: string;
  hint: string;
  action?: { label: string; kind: "focus" | "recalibrate" | "pickWindow" };
}

export function pauseText(r: PauseReason): Reason {
  const recalibrate = { label: t("重新校准", "Recalibrate"), kind: "recalibrate" as const };
  switch (r) {
    case "user":
      return { title: t("已暂停", "Paused"), hint: t("按 F6 或点「继续」接着钓", "Press F6 or click Resume to carry on") };
    case "notForeground":
      return {
        title: t("Minecraft 不在前台", "Minecraft isn't in the foreground"),
        hint: t("切回游戏后会自动继续", "Resumes automatically when you switch back"),
        action: { label: t("切回游戏", "Switch to game"), kind: "focus" },
      };
    case "windowMissing":
      return {
        title: t("找不到 Minecraft", "Minecraft not found"),
        hint: t("正在每 2 秒重找一次", "Looking again every 2 seconds"),
        action: { label: t("选择窗口", "Pick window"), kind: "pickWindow" },
      };
    case "sizeChanged":
      return {
        title: t("窗口大小变了", "Window size changed"),
        hint: t("需要按新尺寸重新校准，约 10 秒", "Recalibrate for the new size (about 10 s)"),
        action: recalibrate,
      };
    case "rodUnknown":
      return {
        title: t("认不出鱼竿", "Can't recognise the rod"),
        hint: t(
          "可能打开了菜单、换了物品，或校准的位置不对；画面恢复后自动继续，一直这样就重新校准",
          "A menu may be open, you switched items, or the calibration is off. It resumes once the view is back; if it keeps happening, recalibrate",
        ),
        action: recalibrate,
      };
    case "castFailed":
      return {
        title: t("没甩出去", "The cast didn't go out"),
        hint: t("手上可能不是鱼竿，或面前没有水", "You may not be holding a fishing rod, or there's no water ahead"),
        action: recalibrate,
      };
    case "userActive":
      return {
        title: t("你在操作游戏，已让出控制", "You're playing, so AutoMoyu stepped aside"),
        hint: t("停手几秒后自动继续", "Resumes a few seconds after you stop"),
      };
  }
}

export function stopText(r: StopReason): string {
  switch (r) {
    case "user":
      return t("本次钓鱼结束", "Session ended");
    case "gameClosed":
      return t("Minecraft 已关闭，本次钓鱼结束", "Minecraft closed, session ended");
    case "duration":
      return t("到达设定时长，已停止", "Stopped: time limit reached");
    case "catches":
      return t("钓满设定条数，已停止", "Stopped: catch limit reached");
    case "deadline":
      return t("到点了，已停止", "Stopped: scheduled stop time");
  }
}

export function qualityText(q: Quality): string {
  return q === "excellent" ? t("优", "Excellent") : q === "good" ? t("良", "Good") : t("差", "Poor");
}

export function secs(ms: number, digits = 1) {
  const v = (ms / 1000).toFixed(digits);
  return t(`${v} 秒`, `${v} s`);
}

/** "12 条" / "12 fish" */
export function fish(n: number) {
  return t(`${n} 条`, `${n} fish`);
}

/** 事件流里的一行；返回 null 表示不显示。 */
export function feedLine(e: EngineEvent): { text: string; sub?: string; tone?: "catch" | "warn" } | null {
  switch (e.kind) {
    case "cast":
      return { text: t("甩竿", "Cast") };
    case "catch":
      return { text: t("钓到 1 条", "Caught 1 fish"), sub: t(`上钩用时 ${secs(e.biteWaitMs)}`, `bite after ${secs(e.biteWaitMs)}`), tone: "catch" };
    case "empty":
      return { text: t("等太久没上钩，收竿重甩", "No bite for too long, recasting"), sub: secs(e.waitedMs, 0) };
    case "bounced":
      return { text: t("刚甩出就被收回，等一下再甩", "Reeled in right after casting, waiting before recasting"), sub: secs(e.outMs) };
    case "lagged":
      // 高延迟服务器每竿都会有，只提示一次
      return e.count === 1
        ? { text: t("服务器延迟高，甩竿画面会回跳，已自动适应", "High server lag makes the cast jump back; adjusted automatically") }
        : null;
    case "castFailed":
      return { text: t("没甩出去，重试", "Cast didn't go out, retrying"), sub: t(`第 ${e.consecutive} 次`, `attempt ${e.consecutive}`), tone: "warn" };
    case "reel":
      return e.cause === "bite" ? { text: t("听到咬钩，收竿", "Heard a bite, reeling in") } : null;
    case "paused":
      return { text: pauseText(e.reason).title, tone: "warn" };
    case "resumed":
      return { text: t("继续", "Resumed") };
    case "stopped":
      return { text: stopText(e.reason), sub: fish(e.catches) };
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
  if (m < 60) return t(`${m} 分`, plural(m, "minute", "minutes"));
  const h = Math.floor(m / 60);
  return t(`${h} 小时 ${m % 60} 分`, `${plural(h, "hour", "hours")} ${plural(m % 60, "minute", "minutes")}`);
}

export function timeOf(at: number) {
  const d = new Date(at);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
