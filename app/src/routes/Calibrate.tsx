// 校准向导：找到游戏 → 准备 → 自动试甩 → 完成。
// 在游戏里按 F6 开始；程序自己甩两轮竿，挑出最好认的小框，阈值由数据算出。

import { useState } from "react";

import { WindowPicker } from "../components/ui";
import { api } from "../lib/api";
import { t } from "../lib/i18n";
import { useStore } from "../lib/store";
import { qualityText } from "../lib/text";
import type { CalRunView, RelRect, Source } from "../lib/types";

const sourceName = (s: Source) => (s === "hand" ? t("手持竿", "the rod in hand") : t("快捷栏", "the hotbar"));

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
        <img src={c.preview} alt={t("游戏画面右下部分", "Lower right part of the game view")} />
      ) : (
        <div className="ph" style={{ aspectRatio: aspect }}>
          {t("切回游戏后这里会显示游戏画面", "The game view shows up here once you switch back to it")}
        </div>
      )}
      {u &&
        !chosen &&
        c.candidates.map(([s, r]) => (
          <div key={s} className={`roi ${s === "hotbar" ? "low" : inside(r)}`} style={within(r, u)}>
            <span>
              {t("候选：", "Candidate: ")}
              {sourceName(s)}
            </span>
          </div>
        ))}
      {u && chosen && (
        <div className={`roi chosen ${inside(chosen)}`} style={within(chosen, u)}>
          <span>{t("识别区域", "Detection area")}</span>
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
  const labels = [t("找到游戏", "Game"), t("准备", "Ready"), t("自动试甩", "Test casts"), t("完成", "Done")];
  const cancel = () => void api.calCancel();
  const u = c.unionRel;
  const aspect = u && win ? (u.w * win.w) / (u.h * win.h) : 16 / 9;
  const cancelLabel = t("取消", "Cancel");

  return (
    <>
      <div className="wz">
        <b>{t("校准", "Calibration")}</b>
        <span>
          {c.step === "sampling" || c.step === "countdown"
            ? t("约 10 秒 · 请别动鼠标", "About 10 s · keep the mouse still")
            : t("约 10 秒", "About 10 s")}
        </span>
      </div>
      <ol className="steps" aria-label={t("校准步骤", "Calibration steps")}>
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
            <b>{t("没找到 Minecraft", "Minecraft not found")}</b>
            <ul className="tips">
              <li>{t("先打开 Minecraft 基岩版，进入世界", "Open Minecraft Bedrock Edition and load a world")}</li>
              <li>
                {t(
                  "设置里把游戏改成窗口化或无边框（独占全屏截不到画面）",
                  "Switch the game to windowed or borderless (exclusive fullscreen can't be captured)",
                )}
              </li>
              <li>{t("找到后这里会自动进入下一步", "This moves on by itself once the game is found")}</li>
            </ul>
          </div>
          <button className="secondary" onClick={() => setPicker(true)}>
            {t("手动选择窗口", "Pick window manually")}
          </button>
        </>
      )}

      {win && (c.step === "prepare" || c.step === "failed") && (
        <>
          <Preview c={c} aspect={aspect} />
          {c.step === "failed" && c.error && (
            <div className="note" role="alert">
              <div>
                <b>{t("校准没成功", "Calibration didn't work")}</b>
                <span>{c.error}</span>
              </div>
            </div>
          )}
          <div className="card">
            <b>{t("准备好这三件事", "Three things first")}</b>
            <ul className="tips">
              <li>{t("手里拿着鱼竿，线是收回来的", "Hold a fishing rod with the line reeled in")}</li>
              <li>
                {status?.mode === "full"
                  ? t("准星对着水面", "Aim the crosshair at water")
                  : t("站在钓鱼机的位置，准星对着水", "Stand at your fish farm spot, aiming at the water")}
              </li>
              <li>
                {t("切回游戏按 ", "Switch to the game and press ")}
                <span className="kbd">{key}</span>
                {t("，程序会自己甩两次竿", "; AutoMoyu casts twice by itself")}
              </li>
            </ul>
            {status?.mode === "full" && (
              <span className="fine">
                {t(
                  "野钓模式要靠咬钩的水花声收竿：游戏「音效」音量别调到 0，音乐可以关掉。",
                  "In open water mode AutoMoyu reels in on the bite splash: keep the game's sound effects volume above 0 (music can be off).",
                )}
              </span>
            )}
          </div>
          <button className="primary" onClick={() => void api.calStart()}>
            {t("切到游戏并开始", "Switch to game & start")} <kbd>{key}</kbd>
          </button>
          <button className="secondary" onClick={cancel}>
            {cancelLabel}
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
            {t("马上开始，请别动鼠标", "Starting now, keep the mouse still")}
          </p>
          <button className="secondary" onClick={cancel}>
            {cancelLabel} <span className="kbd">{key}</span>
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
                  ? t("正在挑选最好认的区域…", "Picking the clearest area…")
                  : `${t(`第 ${c.round} 轮`, `Round ${c.round}`)} · ${
                      c.sampling === "out"
                        ? t("采集甩出画面", "capturing cast-out frames")
                        : c.sampling === "in"
                          ? t("采集收回画面", "capturing reeled-in frames")
                          : t("甩竿中", "casting")
                    }`}
              </span>
              <span className="mono">{Math.round(c.progress * 100)}%</span>
            </div>
          </div>
          <ul className="checks">
            <li className={c.framesIn >= 30 ? "ok" : c.sampling === "in" ? "run" : "todo"}>
              {t(`收回时的画面 · ${c.framesIn} 帧`, `Reeled-in frames · ${c.framesIn}`)}
            </li>
            <li className={c.framesOut >= 20 ? "ok" : c.sampling === "out" ? "run" : c.framesOut > 0 ? "run" : "todo"}>
              {t(`甩出时的画面 · ${c.framesOut} 帧`, `Cast-out frames · ${c.framesOut}`)}
            </li>
            <li className={c.step === "analyzing" ? "run" : "todo"}>{t("挑选识别区域", "Pick detection area")}</li>
          </ul>
          <button className="secondary" onClick={cancel}>
            {cancelLabel}
          </button>
        </>
      )}

      {c.step === "done" && c.result && (
        <>
          <Preview c={c} chosen={c.result.rel} aspect={aspect} />
          <div className={`qual ${c.result.quality}`}>
            <span>{t("区分度", "Contrast")}</span>
            <span className="meter">
              <i style={{ width: `${Math.min(100, (c.result.ratio / 30) * 100)}%` }} />
            </span>
            <b>
              {qualityText(c.result.quality)} · {c.result.ratio.toFixed(1)}×
            </b>
          </div>
          <p className="fine">
            {t("已校准", "Calibrated")} · {c.result.w}×{c.result.h} · {t("识别", "watching ")}
            {sourceName(c.result.source)}
            {c.result.quality === "poor" &&
              t(
                "。信号太弱，开钓后很可能认不出鱼竿：看看上面的框是不是落在手里的鱼竿上；关掉「视角摇晃」、别站在会动的方块旁、别动鼠标，再校准一次。",
                ". The signal is weak and the rod may not be recognised while fishing. Check that the box above sits on the rod in your hand, turn off View Bobbing, stay away from moving blocks, keep the mouse still, and calibrate again.",
              )}
          </p>
          <button className="primary" onClick={() => void api.hotkey()}>
            {status?.session ? t("继续钓鱼", "Resume fishing") : t("开始钓鱼", "Start fishing")} <kbd>{key}</kbd>
          </button>
          <div className="btn-row">
            <button className="secondary" onClick={() => void api.calStart()}>
              {t("再校准一次", "Calibrate again")}
            </button>
            <button className="secondary" onClick={cancel}>
              {t("完成", "Done")}
            </button>
          </div>
        </>
      )}
      {picker && <WindowPicker onClose={() => setPicker(false)} />}
    </>
  );
}
