// 首页：模式 → 主视觉 → 连接状态 → 暂停原因 → 主按钮 → 三个数字 → 动态。

import { Square } from "lucide-react";
import { useEffect, useState } from "react";

import { Hero } from "../components/Hero";
import { UpdateBanner } from "../components/UpdateBanner";
import { Note, WindowPicker } from "../components/ui";
import { api } from "../lib/api";
import { useStore } from "../lib/store";
import { clock, feedLine, pauseText, qualityText, secs, stopText, timeOf } from "../lib/text";
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
  let reason = paused && status?.pauseReason ? pauseText[status.pauseReason] : null;
  if (reason && status?.pauseReason === "userActive" && session?.resumeInMs != null) {
    reason = { ...reason, hint: `停手 ${Math.ceil(session.resumeInMs / 1000)} 秒后自动继续` };
  }

  const act = (kind: "focus" | "recalibrate" | "pickWindow") => {
    if (kind === "focus") void api.focusGame();
    if (kind === "recalibrate") void api.calOpen();
    if (kind === "pickWindow") setPicker(true);
  };

  let primary: { label: string; onClick: () => void; cls: string };
  if (!running) {
    primary = cal
      ? { label: "开始钓鱼", onClick: () => void api.start(), cls: "" }
      : { label: "校准并开始", onClick: () => void api.calOpen(), cls: "" };
  } else if (paused) {
    primary = { label: "继续", onClick: () => void api.togglePause(), cls: "" };
  } else {
    primary = { label: "暂停", onClick: () => void api.togglePause(), cls: "running" };
  }

  const rate = session && session.activeMs > 60_000 ? Math.round((session.catches * 3_600_000) / session.activeMs) : null;
  const lastStop = !running && status?.lastStop && status.lastStop !== "user" ? stopText[status.lastStop] : null;

  return (
    <>
      <UpdateBanner />
      <div className="seg" role="group" aria-label="模式">
        <button aria-pressed={status?.mode === "rodOnly"} disabled={running} onClick={() => setMode("rodOnly")}>
          自动甩竿<small>有钓鱼机</small>
        </button>
        <button aria-pressed={status?.mode === "full"} disabled={running} onClick={() => setMode("full")}>
          全自动<small>无钓鱼机 · 测试版</small>
        </button>
      </div>

      <Hero />

      <div className="conn">
        {win ? (
          <>
            <i className={win.foreground ? "" : "warn"} />
            <span>
              Minecraft · {win.w}×{win.h} · {cal ? "已校准" : "未校准"}
              {status?.mode === "full" && running ? ` · 咬钩识别：${status.audio ? "声音" : "仅超时"}` : ""}
            </span>
            {cal ? (
              <span className={`q ${cal.quality}`}>信号 {qualityText[cal.quality]}</span>
            ) : (
              <button className="q" onClick={() => void api.calOpen()}>
                去校准
              </button>
            )}
          </>
        ) : (
          <>
            <i className="off" />
            <span>没找到 Minecraft（窗口化或无边框）</span>
            <button className="q" onClick={() => setPicker(true)}>
              选择窗口
            </button>
          </>
        )}
      </div>

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
        <Note tone="info" title="第一次用，先校准一次" hint="程序自己甩两次竿，找出最好认的画面，约 10 秒" />
      )}

      <div className="btn-row">
        <button className={`primary ${primary.cls}`} onClick={primary.onClick} disabled={!win && !running}>
          {primary.label} <kbd>{key}</kbd>
        </button>
        {running && (
          <button className="square" title="结束本次" aria-label="结束本次" onClick={() => void api.stop()}>
            <Square size={16} />
          </button>
        )}
      </div>

      <div className="kpis">
        {session ? (
          <>
            <div>
              <b>{session.catches}</b>
              <span>本次（条）</span>
            </div>
            <div>
              <b>{clock(session.activeMs)}</b>
              <span>用时</span>
            </div>
            {paused || rate === null ? (
              <div>
                <b>
                  {session.avgBiteMs ? (session.avgBiteMs / 1000).toFixed(1) : "—"}
                  <small> 秒</small>
                </b>
                <span>平均上钩</span>
              </div>
            ) : (
              <div>
                <b>{rate}</b>
                <span>条 / 小时</span>
              </div>
            )}
          </>
        ) : (
          <>
            <div>
              <b>{stats?.summary.todayCatches ?? "—"}</b>
              <span>今日（条）</span>
            </div>
            <div>
              <b>{stats?.summary.catches ?? "—"}</b>
              <span>累计（条）</span>
            </div>
            <div>
              <b>
                {stats?.summary.avgBiteMs ? (stats.summary.avgBiteMs / 1000).toFixed(1) : "—"}
                <small> 秒</small>
              </b>
              <span>平均上钩</span>
            </div>
          </>
        )}
      </div>

      <div className="label">动态</div>
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
            {running ? "等第一条上钩…" : `切回游戏按 ${key} 开始；${secs((settings?.advanced.maxWaitS ?? 60) * 1000, 0)}没上钩会自动重甩。`}
          </li>
        )}
      </ul>
      {picker && <WindowPicker onClose={() => setPicker(false)} />}
    </>
  );
}
