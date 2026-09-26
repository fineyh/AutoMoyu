// 单序列柱状图：细柱、4px 圆角数据端贴基线、柱间留缝、网格退后；
// 只给最高和最后一根标数，其余靠悬停/键盘焦点提示和表格视图。

import { useState } from "react";

export interface Bar {
  key: string;
  label: string;
  value: number;
  /** 提示里的完整说明，比如 "7 月 9 日" */
  title?: string;
}

function niceMax(v: number) {
  if (v <= 0) return 4;
  const p = 10 ** Math.floor(Math.log10(v));
  const n = [1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10].find((m) => m * p >= v)!;
  return n * p;
}

/** 顶部两角圆、底部平的柱子。 */
function barPath(x: number, y: number, w: number, h: number, r: number) {
  const rr = Math.min(r, w / 2, h);
  return `M${x},${y + h}V${y + rr}Q${x},${y} ${x + rr},${y}H${x + w - rr}Q${x + w},${y} ${x + w},${y + rr}V${y + h}Z`;
}

export function BarChart({ bars, unit, ariaLabel, showTable }: { bars: Bar[]; unit: string; ariaLabel: string; showTable: boolean }) {
  const [hover, setHover] = useState<number | null>(null);
  const W = 320;
  const H = 150;
  const left = 30;
  const right = 8;
  const top = 14;
  const bottom = 22;
  const plotW = W - left - right;
  const plotH = H - top - bottom;
  const max = niceMax(Math.max(0, ...bars.map((b) => b.value)));
  const ticks = [0, max / 2, max];
  const slot = plotW / Math.max(1, bars.length);
  const bw = Math.min(28, Math.max(4, slot - 6));
  const maxIdx = bars.reduce((m, b, i) => (b.value > bars[m].value ? i : m), 0);
  const y = (v: number) => top + plotH - (v / max) * plotH;
  const labelEvery = Math.ceil(bars.length / 8);

  if (showTable) {
    return (
      <table className="w-full text-[12.5px]">
        <tbody>
          {bars.map((b) => (
            <tr key={b.key} className="border-t border-[var(--line)] first:border-t-0">
              <td className="py-1 text-[var(--muted)]">{b.title ?? b.label}</td>
              <td className="py-1 text-right num">
                {b.value} {unit}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    );
  }

  const h = hover !== null ? bars[hover] : null;
  return (
    <div className="relative">
      <svg className="chart" viewBox={`0 0 ${W} ${H}`} role="img" aria-label={ariaLabel}>
        {ticks.map((t, i) => (
          <g key={t}>
            <line className="grid" x1={left} x2={W - right} y1={y(t)} y2={y(t)} strokeDasharray={i === 0 ? undefined : "2 3"} />
            <text x={left - 6} y={y(t) + 3} textAnchor="end">
              {Number.isInteger(t) ? t : t.toFixed(1)}
            </text>
          </g>
        ))}
        {bars.map((b, i) => {
          const cx = left + slot * i + slot / 2;
          const bh = Math.max(b.value > 0 ? 2 : 0, (b.value / max) * plotH);
          const showVal = b.value > 0 && (i === maxIdx || i === bars.length - 1);
          return (
            <g
              key={b.key}
              tabIndex={0}
              role="img"
              aria-label={`${b.title ?? b.label}: ${b.value} ${unit}`}
              onPointerEnter={() => setHover(i)}
              onPointerLeave={() => setHover(null)}
              onFocus={() => setHover(i)}
              onBlur={() => setHover(null)}
              style={{ outline: "none" }}
            >
              {/* 命中区比柱子大：整列 */}
              <rect x={left + slot * i} y={top} width={slot} height={plotH} fill="transparent" />
              {bh > 0 && (
                <path
                  className="bar"
                  d={barPath(cx - bw / 2, y(0) - bh, bw, bh, 4)}
                  opacity={hover === null || hover === i ? 1 : 0.55}
                />
              )}
              {showVal && (
                <text className="val" x={cx} y={y(0) - bh - 4} textAnchor="middle">
                  {b.value}
                </text>
              )}
              {i % labelEvery === 0 && (
                <text x={cx} y={H - 6} textAnchor="middle">
                  {b.label}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      {h && hover !== null && (
        <div
          className="pointer-events-none absolute -translate-x-1/2 rounded-md border border-[var(--line)] bg-[var(--surface)] px-2 py-1 text-[12px] shadow-[var(--shadow)] whitespace-nowrap"
          style={{ left: `${((left + slot * hover + slot / 2) / W) * 100}%`, top: 0 }}
        >
          <b className="num">
            {h.value} {unit}
          </b>
          <span className="ml-1.5 text-[var(--muted)]">{h.title ?? h.label}</span>
        </div>
      )}
    </div>
  );
}
