// 统计：累计、每日条数、上钩用时分布、场次列表；导出 CSV。

import { useEffect, useState } from "react";

import { BarChart } from "../components/BarChart";
import { api, saveDialog } from "../lib/api";
import { useStore } from "../lib/store";
import { humanDuration } from "../lib/text";
import type { StatsView } from "../lib/types";

function md(date: string) {
  const [, m, d] = date.split("-").map(Number);
  return { short: `${m}/${d}`, long: `${m} 月 ${d} 日` };
}

/** 1:12 这种 小时:分钟 */
function hm(ms: number) {
  const m = Math.round(ms / 60000);
  return `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}`;
}

function dur(ms: number) {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} 秒`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} 分 ${String(s % 60).padStart(2, "0")} 秒`;
  return `${Math.floor(m / 60)} 小时 ${m % 60} 分`;
}

export function Stats() {
  const [v, setV] = useState<StatsView | null>(null);
  const [table, setTable] = useState(false);
  const toast = useStore((s) => s.showToast);
  const running = useStore((s) => !!s.status?.session);
  const load = () => api.stats(7).then(setV).catch((e) => toast(String(e)));
  useEffect(() => {
    void load();
  }, [running]);

  const exportCsv = async () => {
    const p = await saveDialog("AutoMoyu-场次.csv", "CSV", ["csv"]);
    if (!p) return;
    try {
      const n = await api.exportCsv(p);
      toast(`已导出 ${n} 场`);
    } catch (e) {
      toast(`导出失败：${e}`);
    }
  };

  if (!v) return <div className="empty">正在读取…</div>;
  const s = v.summary;
  const days = v.days;
  const range = days.length ? `${md(days[0].date).long} – ${md(days[days.length - 1].date).long}` : "";
  const hasBites = v.biteBuckets.some((b) => b.count > 0);

  return (
    <>
      <div className="kpis">
        <div>
          <b>{s.catches}</b>
          <span>累计（条）</span>
        </div>
        <div>
          <b>{hm(s.activeMs)}</b>
          <span>累计时长</span>
        </div>
        <div>
          <b>{s.sessions}</b>
          <span>场</span>
        </div>
      </div>
      {s.sessions > 0 ? (
        <p className="fun">
          已替你摸鱼 <b>{humanDuration(s.activeMs)}</b>
          {s.since && `，从 ${s.since} 开始`}。
        </p>
      ) : (
        <p className="fun">还没有记录。用过 v0.1？可以把旧数据导进来。</p>
      )}

      {days.length > 0 && (
        <div className="card">
          <div className="card-h">
            <b>每日条数</b>
            <span>
              {range} ·{" "}
              <button className="link" onClick={() => setTable(!table)}>
                {table ? "图表" : "表格"}
              </button>
            </span>
          </div>
          <BarChart
            unit="条"
            showTable={table}
            ariaLabel={`每日条数：${days.map((d) => `${md(d.date).long} ${d.catches} 条`).join("，")}`}
            bars={days.map((d) => ({ key: d.date, label: md(d.date).short, title: md(d.date).long, value: d.catches }))}
          />
        </div>
      )}

      {hasBites && (
        <div className="card">
          <div className="card-h">
            <b>上钩用时</b>
            <span>{s.avgBiteMs ? `平均 ${(s.avgBiteMs / 1000).toFixed(1)} 秒` : ""}</span>
          </div>
          <BarChart
            unit="条"
            showTable={table}
            ariaLabel={`上钩用时分布：${v.biteBuckets.map((b) => `${b.label} ${b.count} 条`).join("，")}`}
            bars={v.biteBuckets.map((b) => ({ key: b.label, label: b.label, title: `上钩用时 ${b.label}${b.label.includes("秒") ? "" : " 秒"}`, value: b.count }))}
          />
        </div>
      )}

      <div className="card">
        <div className="card-h">
          <b>最近场次</b>
          {v.recent.length > 0 && (
            <button className="link" onClick={exportCsv}>
              导出 CSV
            </button>
          )}
        </div>
        {v.recent.length === 0 && <div className="empty">钓完一场就会出现在这里。</div>}
        <ul className="sess">
          {v.recent.slice(0, 20).map((r) => {
            const d = new Date(r.startedAt);
            return (
              <li key={r.id}>
                <span>
                  {d.getMonth() + 1} 月 {d.getDate()} 日 {String(d.getHours()).padStart(2, "0")}:{String(d.getMinutes()).padStart(2, "0")}
                  <br />
                  <small>
                    {r.mode === "full" ? "全自动" : "自动甩竿"} · {dur(r.activeMs)}
                    {r.source === "legacy" ? " · v0.1" : ""}
                  </small>
                </span>
                <b>{r.catches} 条</b>
              </li>
            );
          })}
        </ul>
      </div>
    </>
  );
}
