// 游戏内浮层（独立的透明窗口，鼠标穿透，不被截图捕获）。
// 平时是左上角一个状态胶囊；开了调试浮层时窗口铺满客户区，额外画出识别框。

import { useStore } from "../lib/store";
import { phaseText, secs } from "../lib/text";

export function Overlay() {
  const st = useStore((s) => s.status);
  const debug = useStore((s) => s.settings?.advanced.debugOverlay);
  if (!st) return null;
  const c = st.calib;
  const paused = st.phase === "paused";
  let text: string = phaseText[st.phase];
  let extra: string | null = null;
  if (c?.step === "countdown") {
    text = "校准即将开始";
  } else if (c?.step === "sampling") {
    text = "校准中 · 别动鼠标";
  } else if (st.phase === "waiting" && st.session?.outForMs != null) {
    extra = secs(st.session.outForMs).replace(" 秒", "s");
  }
  const roi = st.calibration?.rel;
  return (
    <div className="fixed inset-0">
      <div className="hud" style={{ left: debug ? 16 : 0, top: debug ? 16 : 0 }}>
        <i className={paused ? "p" : ""} />
        {c?.step === "countdown" && c.countdown != null ? <span className="cd">{c.countdown}</span> : null}
        <span>{text}</span>
        {extra && <span className="mono">{extra}</span>}
        {st.session && (
          <>
            <span className="sep" />
            本次 <b>{st.session.catches}</b> 条
          </>
        )}
      </div>
      {debug && roi && (
        <div
          className="roi chosen"
          style={{ left: `${roi.x * 100}%`, top: `${roi.y * 100}%`, width: `${roi.w * 100}%`, height: `${roi.h * 100}%` }}
        >
          <span>
            {st.signal?.rod === "in" ? "收回" : st.signal?.rod === "out" ? "甩出" : "识别区域"}
            {st.signal ? ` · ${st.signal.dIn.toFixed(0)}/${st.signal.dOut.toFixed(0)}` : ""}
          </span>
        </div>
      )}
    </div>
  );
}
