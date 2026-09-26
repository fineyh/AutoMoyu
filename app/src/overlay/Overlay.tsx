// 游戏内浮层（独立的透明窗口，鼠标穿透，不被截图捕获）。
// 平时是左上角一个状态胶囊；开了调试浮层时窗口铺满客户区，额外画出识别框。

import { t } from "../lib/i18n";
import { useStore } from "../lib/store";
import { phaseText } from "../lib/text";

export function Overlay() {
  const st = useStore((s) => s.status);
  const debug = useStore((s) => s.settings?.advanced.debugOverlay);
  if (!st) return null;
  const c = st.calib;
  const paused = st.phase === "paused";
  let text: string = phaseText(st.phase);
  let extra: string | null = null;
  if (c?.step === "countdown") {
    text = t("校准即将开始", "Calibration starting");
  } else if (c?.step === "sampling") {
    text = t("校准中 · 别动鼠标", "Calibrating · keep the mouse still");
  } else if (st.pauseReason === "userActive") {
    text = t("你在操作", "You're playing");
    if (st.session?.resumeInMs != null) {
      const n = Math.ceil(st.session.resumeInMs / 1000);
      extra = t(`${n}s 后继续`, `resuming in ${n}s`);
    }
  } else if (st.phase === "waiting" && st.session?.outForMs != null) {
    extra = `${(st.session.outForMs / 1000).toFixed(1)}s`;
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
            {t("本次 ", "")}
            <b>{st.session.catches}</b>
            {t(" 条", " fish")}
          </>
        )}
      </div>
      {debug && roi && (
        <div
          className="roi chosen"
          style={{ left: `${roi.x * 100}%`, top: `${roi.y * 100}%`, width: `${roi.w * 100}%`, height: `${roi.h * 100}%` }}
        >
          <span>
            {st.signal?.rod === "in" ? t("收回", "reeled in") : st.signal?.rod === "out" ? t("甩出", "cast out") : t("识别区域", "detection area")}
            {st.signal ? ` · ${st.signal.dIn.toFixed(0)}/${st.signal.dOut.toFixed(0)}` : ""}
          </span>
        </div>
      )}
    </div>
  );
}
