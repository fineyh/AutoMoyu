// 主视觉：一小片水面和浮漂。动画就是状态——等待时轻晃，收竿时下沉溅水，钓到时冒 +1。

import { useEffect, useState } from "react";

import { useStore } from "../lib/store";
import { clock, phaseText, secs } from "../lib/text";
import type { Phase } from "../lib/types";
import { Bobber } from "./ui";

const scene: Record<Phase, string> = {
  idle: "idle",
  starting: "idle",
  casting: "cast",
  waiting: "wait",
  reeling: "bite",
  caught: "catch",
  paused: "paused",
};

const WAVES: [number, number, number][] = [
  [10, 60, 14], [66, 70, 10], [28, 84, 18], [78, 46, 8], [6, 90, 9], [56, 92, 12],
];

export function Hero() {
  const status = useStore((s) => s.status);
  const pulse = useStore((s) => s.catchPulse);
  const [plusKey, setPlusKey] = useState(0);
  useEffect(() => {
    if (pulse > 0) setPlusKey(pulse);
  }, [pulse]);

  const phase = status?.phase ?? "idle";
  const s = status?.session;
  let sub = "";
  if (phase === "waiting" && s?.outForMs != null) sub = secs(s.outForMs);
  else if (phase === "paused" && s) sub = `${clock(s.activeMs)} · 本次 ${s.catches} 条`;
  else if (phase === "idle") sub = status?.calibration ? "按 F6 开始" : "先校准一次（约 10 秒）";

  return (
    <div className="hero" data-phase={scene[phase]} aria-live="polite">
      {WAVES.map(([l, t, w], i) => (
        <span key={i} className="w" style={{ left: `${l}%`, top: `${t}%`, width: `${w}%` }} />
      ))}
      <div className="st">
        <span className="ph">
          <i />
          {phaseText[phase]}
        </span>
        {sub && <span className="t num">{sub}</span>}
      </div>
      <span className="ringbase" />
      <span className="ripple" />
      <div className="bob">
        <Bobber />
      </div>
      <span key={plusKey} className={`plus ${plusKey ? "go" : ""}`}>
        +1
      </span>
    </div>
  );
}
