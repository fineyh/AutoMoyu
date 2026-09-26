// 校准向导：找到游戏 → 准备 → 自动试甩 → 完成。
// 在游戏里按 F6 开始；程序自己甩两轮竿，挑出最好认的小框，阈值由数据算出。

import { useState } from "react";

import { WindowPicker } from "../components/ui";
import { api } from "../lib/api";
import { useStore } from "../lib/store";
import { qualityText } from "../lib/text";
import type { CalRunView, RelRect, Source } from "../lib/types";

const sourceName: Record<Source, string> = { hand: "手持竿", hotbar: "快捷栏" };

/** 把客户区比例坐标换算成预览图（候选区外包矩形）内的百分比。 */
function within(r: RelRect, u: RelRect) {
  return {
    left: `${((r.x - u.x) / u.w) * 100}%`,
    top: `${((r.y - u.y) / u.h) * 100}%`,
    width: `${(r.w / u.w) * 100}%`,
    height: `${(r.h / u.h) * 100}%`,
  };
}

function Preview({ c, chosen, aspect }: { c: CalRunView; chosen?: RelRect | null; aspect: number }) {
  const u = c.unionRel;
  // 贴着预览上边的框，标签放到框里面，免得被裁掉
  const inside = (r: RelRect) => (u && (r.y - u.y) / u.h < 0.12 ? "in" : "");
  return (
    <div className="prev">
      {c.preview ? (
        <img src={c.preview} alt="游戏画面右下部分" />
      ) : (
        <div className="ph" style={{ aspectRatio: aspect }}>
          切回游戏后这里会显示游戏画面
        </div>
      )}
      {u &&
        !chosen &&
        c.candidates.map(([s, r]) => (
          <div key={s} className={`roi ${s === "hotbar" ? "low" : inside(r)}`} style={within(r, u)}>
            <span>候选：{sourceName[s]}</span>
          </div>
        ))}
      {u && chosen && (
        <div className={`roi chosen ${inside(chosen)}`} style={within(chosen, u)}>
          <span>识别区域</span>
        </div>
      )}
    </div>
  );
}

export function Calibrate() {
  const status = useStore((s) => s.status);
  const settings = useStore((s) => s.settings);
  const [picker, setPicker] = useState(false);
  const c = status?.calib;
  if (!c) return null;
  const key = settings?.hotkeys.toggle ?? "F6";
  const win = status?.window;
  const stepIdx = !win ? 0 : c.step === "prepare" || c.step === "failed" ? 1 : c.step === "done" ? 4 : 2;
  const labels = ["找到游戏", "准备", "自动试甩", "完成"];
  const cancel = () => void api.calCancel();
  const u = c.unionRel;
  const aspect = u && win ? (u.w * win.w) / (u.h * win.h) : 16 / 9;

  return (
    <>
      <div className="wz">
        <b>校准</b>
        <span>{c.step === "sampling" || c.step === "countdown" ? "约 10 秒 · 请别动鼠标" : "约 10 秒"}</span>
      </div>
      <ol className="steps" aria-label="校准步骤">
        {labels.map((l, i) => (
          <li
            key={l}
            className={i < stepIdx ? "done" : i === stepIdx ? "cur" : ""}
            style={i === 2 && c.step === "sampling" ? ({ "--p": `${Math.round(c.progress * 100)}%` } as React.CSSProperties) : undefined}
          >
            <span>{l}</span>
          </li>
        ))}
      </ol>

      {!win && (
        <>
          <div className="card">
            <b>没找到 Minecraft</b>
            <ul className="tips">
              <li>先打开 Minecraft 基岩版，进入世界</li>
              <li>设置里把游戏改成窗口化或无边框（独占全屏截不到画面）</li>
              <li>找到后这里会自动进入下一步</li>
            </ul>
          </div>
          <button className="secondary" onClick={() => setPicker(true)}>
            手动选择窗口
          </button>
        </>
      )}

      {win && (c.step === "prepare" || c.step === "failed") && (
        <>
          <Preview c={c} aspect={aspect} />
          {c.step === "failed" && c.error && (
            <div className="note" role="alert">
              <div>
                <b>校准没成功</b>
                <span>{c.error}</span>
              </div>
            </div>
          )}
          <div className="card">
            <b>准备好这三件事</b>
            <ul className="tips">
              <li>手里拿着鱼竿，线是收回来的</li>
              <li>{status?.mode === "full" ? "准星对着水面" : "站在钓鱼机的位置，准星对着水"}</li>
              <li>
                切回游戏按 <span className="kbd">{key}</span>，程序会自己甩两次竿
              </li>
            </ul>
            {status?.mode === "full" && <span className="fine">全自动还会听咬钩的水花声：游戏「音效」音量别调到 0，音乐可以关掉。</span>}
          </div>
          <button className="primary" onClick={() => void api.calStart()}>
            切到游戏并开始 <kbd>{key}</kbd>
          </button>
          <button className="secondary" onClick={cancel}>
            取消
          </button>
        </>
      )}

      {c.step === "countdown" && (
        <>
          <Preview c={c} aspect={aspect} />
          <div className="big-count num" aria-live="assertive">
            {c.countdown ?? ""}
          </div>
          <p className="fine" style={{ textAlign: "center" }}>
            马上开始，请别动鼠标
          </p>
          <button className="secondary" onClick={cancel}>
            取消 <span className="kbd">{key}</span>
          </button>
        </>
      )}

      {(c.step === "sampling" || c.step === "analyzing") && (
        <>
          <Preview c={c} aspect={aspect} />
          <div className="prog">
            <div className="bar">
              <i style={{ width: `${Math.round(c.progress * 100)}%` }} />
            </div>
            <div className="row">
              <span>
                {c.step === "analyzing"
                  ? "正在挑选最好认的区域…"
                  : `第 ${c.round} 轮 · ${c.sampling === "out" ? "采集甩出画面" : c.sampling === "in" ? "采集收回画面" : "甩竿中"}`}
              </span>
              <span className="mono">{Math.round(c.progress * 100)}%</span>
            </div>
          </div>
          <ul className="checks">
            <li className={c.framesIn >= 30 ? "ok" : c.sampling === "in" ? "run" : "todo"}>收回时的画面 · {c.framesIn} 帧</li>
            <li className={c.framesOut >= 20 ? "ok" : c.sampling === "out" ? "run" : c.framesOut > 0 ? "run" : "todo"}>
              甩出时的画面 · {c.framesOut} 帧
            </li>
            <li className={c.step === "analyzing" ? "run" : "todo"}>挑选识别区域</li>
          </ul>
          <button className="secondary" onClick={cancel}>
            取消
          </button>
        </>
      )}

      {c.step === "done" && c.result && (
        <>
          <Preview c={c} chosen={c.result.rel} aspect={aspect} />
          <div className={`qual ${c.result.quality}`}>
            <span>区分度</span>
            <span className="meter">
              <i style={{ width: `${Math.min(100, (c.result.ratio / 15) * 100)}%` }} />
            </span>
            <b>
              {qualityText[c.result.quality]} · {c.result.ratio.toFixed(1)}×
            </b>
          </div>
          <p className="fine">
            已校准 · {c.result.w}×{c.result.h} · 识别{sourceName[c.result.source]}
            {c.result.quality === "poor" && "。信号偏弱：关掉「视角摇晃」、别站在会动的方块旁、关掉光影后重试会更稳。"}
          </p>
          <button
            className="primary"
            onClick={() => void api.hotkey()}
          >
            {status?.session ? "继续钓鱼" : "开始钓鱼"} <kbd>{key}</kbd>
          </button>
          <div className="btn-row">
            <button className="secondary" onClick={() => void api.calStart()}>
              再校准一次
            </button>
            <button className="secondary" onClick={cancel}>
              完成
            </button>
          </div>
        </>
      )}
      {picker && <WindowPicker onClose={() => setPicker(false)} />}
    </>
  );
}
