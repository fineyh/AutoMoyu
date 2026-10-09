// 统计：累计、每日条数、上钩用时分布、场次列表；导出 CSV。

import { useEffect, useState } from "react";

import { BarChart } from "../components/BarChart";
import { api, saveDialog } from "../lib/api";
import { isEn, plural, t } from "../lib/i18n";
import { useStore } from "../lib/store";
import { fish, humanDuration, modeName } from "../lib/text";
import type { StatsView } from "../lib/types";

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function monthDay(m: number, d: number) {
  return isEn() ? `${MONTHS[m - 1]} ${d}` : `${m} 月 ${d} 日`;
}

function md(date: string) {
  const [, m, d] = date.split("-").map(Number);
  return { short: `${m}/${d}`, long: monthDay(m, d) };
}

/** 1:12 这种 小时:分钟 */
function hm(ms: number) {
  const m = Math.round(ms / 60000);
  return `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}`;
}

function dur(ms: number) {
  const s = Math.round(ms / 1000);
  if (s < 60) return t(`${s} 秒`, `${s} s`);
  const m = Math.floor(s / 60);
  const ss = String(s % 60).padStart(2, "0");
  if (m < 60) return t(`${m} 分 ${ss} 秒`, `${m} min ${ss} s`);
  return t(`${Math.floor(m / 60)} 小时 ${m % 60} 分`, `${Math.floor(m / 60)} h ${m % 60} min`);
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
    const p = await saveDialog(t("AutoMoyu-场次.csv", "AutoMoyu-sessions.csv"), "CSV", ["csv"]);
    if (!p) return;
    try {
      const n = await api.exportCsv(p);
      toast(t(`已导出 ${n} 场`, `Exported ${plural(n, "session", "sessions")}`));
    } catch (e) {
      toast(t(`导出失败：${e}`, `Export failed: ${e}`));
    }
  };

  if (!v) return <div className="empty">{t("正在读取…", "Loading…")}</div>;
  const s = v.summary;
  const days = v.days;
  const range = days.length ? `${md(days[0].date).long} – ${md(days[days.length - 1].date).long}` : "";
  const hasBites = v.biteBuckets.some((b) => b.count > 0);
  const sep = t("，", ", ");

  return (
    <>
      <div className="kpis">
        <div>
          <b>{s.catches}</b>
          <span>{t("累计（条）", "Fish caught")}</span>
        </div>
        <div>
          <b>{hm(s.activeMs)}</b>
          <span>{t("累计时长", "Time fishing")}</span>
        </div>
        <div>
          <b>{s.sessions}</b>
          <span>{t("场", "Sessions")}</span>
        </div>
      </div>
      {s.sessions > 0 ? (
        <p className="fun">
          {t("已替你摸鱼 ", "Fished for you for ")}
          <b>{humanDuration(s.activeMs)}</b>
          {s.since && t(`，从 ${s.since} 开始`, ` since ${s.since}`)}
          {t("。", ".")}
        </p>
      ) : (
        <p className="fun">{t("还没有记录，钓完一场就有了。", "Nothing yet. Finish a session and it shows up here.")}</p>
      )}

      {days.length > 0 && (
        <div className="card">
          <div className="card-h">
            <b>{t("每日条数", "Fish per day")}</b>
            <span>
              {range} ·{" "}
              <button className="link" onClick={() => setTable(!table)}>
                {table ? t("图表", "Chart") : t("表格", "Table")}
              </button>
            </span>
          </div>
          <BarChart
            unit={t("条", "fish")}
            showTable={table}
            ariaLabel={`${t("每日条数：", "Fish per day: ")}${days.map((d) => `${md(d.date).long} ${fish(d.catches)}`).join(sep)}`}
            bars={days.map((d) => ({ key: d.date, label: md(d.date).short, title: md(d.date).long, value: d.catches }))}
          />
        </div>
      )}

      {hasBites && (
        <div className="card">
          <div className="card-h">
            <b>{t("上钩用时", "Time to bite")}</b>
            <span>{s.avgBiteMs ? t(`平均 ${(s.avgBiteMs / 1000).toFixed(1)} 秒`, `avg ${(s.avgBiteMs / 1000).toFixed(1)} s`) : ""}</span>
          </div>
          <BarChart
            unit={t("条", "fish")}
            showTable={table}
            ariaLabel={`${t("上钩用时分布：", "Time to bite: ")}${v.biteBuckets.map((b) => `${b.label} ${fish(b.count)}`).join(sep)}`}
            bars={v.biteBuckets.map((b) => ({
              key: b.label,
              label: b.label,
              title: t(`上钩用时 ${b.label} 秒`, `Bite after ${b.label} s`),
              value: b.count,
            }))}
          />
        </div>
      )}

      <div className="card">
        <div className="card-h">
          <b>{t("最近场次", "Recent sessions")}</b>
          {v.recent.length > 0 && (
            <button className="link" onClick={exportCsv}>
              {t("导出 CSV", "Export CSV")}
            </button>
          )}
        </div>
        {v.recent.length === 0 && <div className="empty">{t("钓完一场就会出现在这里。", "Finished sessions show up here.")}</div>}
        <ul className="sess">
          {v.recent.slice(0, 20).map((r) => {
            const d = new Date(r.startedAt);
            return (
              <li key={r.id}>
                <span>
                  {monthDay(d.getMonth() + 1, d.getDate())} {String(d.getHours()).padStart(2, "0")}:{String(d.getMinutes()).padStart(2, "0")}
                  <br />
                  <small>
                    {modeName(r.mode)} · {dur(r.activeMs)}
                    {r.source === "legacy" ? " · v0.1" : ""}
                  </small>
                </span>
                <b>{fish(r.catches)}</b>
              </li>
            );
          })}
        </ul>
      </div>
    </>
  );
}
