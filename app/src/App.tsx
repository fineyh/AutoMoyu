import { useEffect, useRef } from "react";

import { TitleBar } from "./components/TitleBar";
import { Toast } from "./components/ui";
import { api } from "./lib/api";
import { useStore, type Tab } from "./lib/store";
import { Calibrate } from "./routes/Calibrate";
import { Home } from "./routes/Home";
import { Settings } from "./routes/Settings";
import { Stats } from "./routes/Stats";

const TABS: { id: Tab; label: string }[] = [
  { id: "home", label: "首页" },
  { id: "stats", label: "统计" },
  { id: "settings", label: "设置" },
];

/** 钓到时的一声"叮"（WebAudio 合成，不带音频文件）。 */
function ding() {
  try {
    const ctx = new AudioContext();
    const o = ctx.createOscillator();
    const g = ctx.createGain();
    o.type = "square";
    o.frequency.setValueAtTime(880, ctx.currentTime);
    o.frequency.setValueAtTime(1320, ctx.currentTime + 0.07);
    g.gain.setValueAtTime(0.06, ctx.currentTime);
    g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.25);
    o.connect(g).connect(ctx.destination);
    o.start();
    o.stop(ctx.currentTime + 0.26);
    o.onended = () => void ctx.close();
  } catch {
    /* 没有音频设备就算了 */
  }
}

export function App() {
  const tab = useStore((s) => s.tab);
  const setTab = useStore((s) => s.setTab);
  const calib = useStore((s) => s.status?.calib);
  const pulse = useStore((s) => s.catchPulse);
  const sound = useStore((s) => s.settings?.ui.catchSound);
  const onboarded = useStore((s) => s.settings?.onboarded);
  const hasCal = useStore((s) => !!s.status?.calibration);
  const asked = useRef(false);

  useEffect(() => {
    if (pulse > 0 && sound) ding();
  }, [pulse, sound]);

  // 首次运行：直接进校准向导
  useEffect(() => {
    if (onboarded === false && !hasCal && !asked.current) {
      asked.current = true;
      void api.calOpen();
    }
  }, [onboarded, hasCal]);

  return (
    <div className="app">
      <TitleBar />
      {calib ? (
        <main className="body">
          <Calibrate />
        </main>
      ) : (
        <>
          <nav className="tabs" role="tablist">
            {TABS.map((t) => (
              <button key={t.id} role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>
                {t.label}
              </button>
            ))}
          </nav>
          <main className="body" role="tabpanel">
            {tab === "home" && <Home />}
            {tab === "stats" && <Stats />}
            {tab === "settings" && <Settings />}
          </main>
        </>
      )}
      <Toast />
    </div>
  );
}
