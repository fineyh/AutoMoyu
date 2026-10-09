// 首页：模式 → 主视觉 → 连接状态 → 暂停原因 → 主按钮 → 三个数字 → 动态。

import { Square } from "lucide-react";
import { useEffect, useState } from "react";

import { Hero } from "../components/Hero";
import { UpdateBanner } from "../components/UpdateBanner";
import { Note, WindowPicker } from "../components/ui";
import { api } from "../lib/api";
import { t } from "../lib/i18n";
import { useStore } from "../lib/store";
import { clock, feedLine, modeHint, modeName, pauseText, qualityText, secs, stopText, timeOf } from "../lib/text";
import type { Mode, StatsView } from "../lib/types";

export function Home() {
  const status = useStore((s) => s.status);
  const settings = useStore((s) => s.settings);
  const feed = useStore((s) => s.feed);
  const patch = useStore((s) => s.patchSettings);
  const [stats, setStats] = useState<StatsView | null>(null);
  const [picker, setPicker] = useState(false);

  const session = status?.session ?? null;
  const running = !!session;
  const paused = status?.phase === "paused";
  const key = settings?.hotkeys.toggle ?? "F6";

  useEffect(() => {
    if (!running) api.stats().then(setStats).catch(() => {});
  }, [running]);

  const setMode = (mode: Mode) => patch({ mode });
  const win = status?.window;
  const cal = status?.calibration;
  let reason = paused && status?.pauseReason ? pauseText(status.pauseReason) : null;
  if (reason && status?.pauseReason === "userActive" && session?.resumeInMs != null) {
    const n = Math.ceil(session.resumeInMs / 1000);
    reason = { ...reason, hint: t(`停手 ${n} 秒后自动继续`, `Resuming in ${n} s if you stay idle`) };
  }

  const act = (kind: "focus" | "recalibrate" | "pickWindow") => {
    if (kind === "focus") void api.focusGame();
    if (kind === "recalibrate") void api.calOpen();
    if (kind === "pickWindow") setPicker(true);
  };

  let primary: { label: string; onClick: () => void; cls: string };
  if (!running) {
    primary = cal
      ? { label: t("开始钓鱼", "Start fishing"), onClick: () => void api.start(), cls: "" }
      : { label: t("校准并开始", "Calibrate & start"), onClick: () => void api.calOpen(), cls: "" };
  } else if (paused) {
    primary = { label: t("继续", "Resume"), onClick: () => void api.togglePause(), cls: "" };
  } else {
    primary = { label: t("暂停", "Pause"), onClick: () => void api.togglePause(), cls: "running" };
  }

  const rate = session && session.activeMs > 60_000 ? Math.round((session.catches * 3_600_000) / session.activeMs) : null;
  const lastStop = !running && status?.lastStop && status.lastStop !== "user" ? stopText(status.lastStop) : null;
  const avgBite = (ms: number | null | undefined) => (ms ? (ms / 1000).toFixed(1) : "—");

  return (
    <>
      <UpdateBanner />
      <div className="seg" role="group" aria-label={t("模式", "Mode")}>
        <button aria-pressed={status?.mode === "rodOnly"} disabled={running} onClick={() => setMode("rodOnly")}>
          {modeName("rodOnly")}
          <small>{modeHint("rodOnly")}</small>
        </button>
        <button aria-pressed={status?.mode === "full"} disabled={running} onClick={() => setMode("full")}>
          {modeName("full")}
          <small>{modeHint("full")}</small>
        </button>
      </div>

      <Hero />

      <div className="conn">
        {win ? (
          <>
            <i className={win.foreground ? "" : "warn"} />
            <span>
              Minecraft · {win.w}×{win.h} · {cal ? t("已校准", "calibrated") : t("未校准", "not calibrated")}
              {status?.mode === "full" && running
                ? t(
                    ` · 咬钩识别：${status.audio === "system" ? "声音（整机）" : status.audio ? "声音" : "仅超时"}`,
                    ` · bites: ${status.audio === "system" ? "by sound (system-wide)" : status.audio ? "by sound" : "timeout only"}`,
                  )
                : ""}
            </span>
            {cal ? (
              <span className={`q ${cal.quality}`}>
                {t("信号", "Signal")} {qualityText(cal.quality)}
              </span>
            ) : (
              <button className="q" onClick={() => void api.calOpen()}>
                {t("去校准", "Calibrate")}
              </button>
            )}
          </>
        ) : (
          <>
            <i className="off" />
            <span>{t("没找到 Minecraft（窗口化或无边框）", "Minecraft not found (windowed or borderless)")}</span>
            <button className="q" onClick={() => setPicker(true)}>
              {t("选择窗口", "Pick window")}
            </button>
          </>
        )}
      </div>

      {status?.spatialSound && status.mode === "full" && (
        <Note
          title={t("系统已开启空间音效", "Spatial sound is on")}
          hint={t(
            "会影响咬钩识别，其他软件的声音也可能导致误收竿，建议关闭。",
            "It interferes with bite detection, and other apps' sounds may cause false reels. We recommend turning it off.",
          )}
        >
          <button className="ghost" onClick={() => void api.openSoundSettings()}>
            {t("声音设置", "Sound settings")}
          </button>
        </Note>
      )}
      {reason && (
        <Note title={reason.title} hint={reason.hint}>
          {reason.action && (
            <button className="ghost" onClick={() => act(reason.action!.kind)}>
              {reason.action.label}
            </button>
          )}
        </Note>
      )}
      {lastStop && <Note tone="info" title={lastStop} />}
      {!cal && win && !running && (
        <Note
          tone="info"
          title={t("第一次用，先校准一次", "First time? Calibrate once")}
          hint={t("程序自己甩两次竿，找出最好认的画面，约 10 秒", "AutoMoyu casts twice by itself to find the clearest spot, about 10 s")}
        />
      )}

      <div className="btn-row">
        <button className={`primary ${primary.cls}`} onClick={primary.onClick} disabled={!win && !running}>
          {primary.label} <kbd>{key}</kbd>
        </button>
        {running && (
          <button className="square" title={t("结束本次", "End session")} aria-label={t("结束本次", "End session")} onClick={() => void api.stop()}>
            <Square size={16} />
          </button>
        )}
      </div>

      <div className="kpis">
        {session ? (
          <>
            <div>
              <b>{session.catches}</b>
              <span>{t("本次（条）", "This session")}</span>
            </div>
            <div>
              <b>{clock(session.activeMs)}</b>
              <span>{t("用时", "Time")}</span>
            </div>
            {paused || rate === null ? (
              <div>
                <b>
                  {avgBite(session.avgBiteMs)}
                  <small>{t(" 秒", " s")}</small>
                </b>
                <span>{t("平均上钩", "Avg bite")}</span>
              </div>
            ) : (
              <div>
                <b>{rate}</b>
                <span>{t("条 / 小时", "fish / hour")}</span>
              </div>
            )}
          </>
        ) : (
          <>
            <div>
              <b>{stats?.summary.todayCatches ?? "—"}</b>
              <span>{t("今日（条）", "Today")}</span>
            </div>
            <div>
              <b>{stats?.summary.catches ?? "—"}</b>
              <span>{t("累计（条）", "All time")}</span>
            </div>
            <div>
              <b>
                {avgBite(stats?.summary.avgBiteMs)}
                <small>{t(" 秒", " s")}</small>
              </b>
              <span>{t("平均上钩", "Avg bite")}</span>
            </div>
          </>
        )}
      </div>

      <div className="label">{t("动态", "Activity")}</div>
      <ul className="feed" aria-live="polite">
        {feed
          .map((f) => ({ f, line: feedLine(f.event) }))
          .filter((x) => x.line)
          .slice(0, 12)
          .map(({ f, line }) => (
            <li key={`${f.at}-${f.event.kind}`}>
              <time>{timeOf(f.at)}</time>
              <span className={line!.tone === "catch" ? "c" : line!.tone === "warn" ? "w" : ""}>{line!.text}</span>
              {line!.sub && <span className="sub">{line!.sub}</span>}
            </li>
          ))}
        {feed.length === 0 && (
          <li className="empty">
            {running
              ? t("等第一条上钩…", "Waiting for the first bite…")
              : t(
                  `切回游戏按 ${key} 开始；${secs((settings?.advanced.maxWaitS ?? 60) * 1000, 0)}没上钩会自动重甩。`,
                  `Switch to the game and press ${key} to start. No bite within ${secs((settings?.advanced.maxWaitS ?? 60) * 1000, 0)} means an automatic recast.`,
                )}
          </li>
        )}
      </ul>
      {picker && <WindowPicker onClose={() => setPicker(false)} />}
    </>
  );
}
