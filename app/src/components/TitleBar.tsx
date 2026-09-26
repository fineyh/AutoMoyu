// 自定义标题栏：拖动区 + 置顶 / 最小化 / 关闭（关闭默认收进托盘）。

import { Minus, Pin, X } from "lucide-react";
import { useEffect, useState } from "react";

import { api, currentWindow } from "../lib/api";
import { useStore } from "../lib/store";
import { Bobber } from "./ui";

export function TitleBar() {
  const settings = useStore((s) => s.settings);
  const patch = useStore((s) => s.patchSettings);
  const pinned = settings?.ui.alwaysOnTop ?? false;
  const [beta, setBeta] = useState(false);
  useEffect(() => {
    api.appInfo().then((i) => setBeta(i.version.includes("-")));
  }, []);
  return (
    <header className="titlebar" data-tauri-drag-region>
      <Bobber className="h-[15px] w-[12px] pointer-events-none" />
      <span className="name pointer-events-none">AutoMoyu</span>
      {beta && <span className="chip pointer-events-none">beta</span>}
      <span className="flex-1" data-tauri-drag-region />
      <button
        className={`tb-btn ${pinned ? "on" : ""}`}
        title={pinned ? "取消置顶" : "窗口置顶"}
        aria-pressed={pinned}
        onClick={() => patch({ ui: { alwaysOnTop: !pinned } })}
      >
        <Pin size={14} />
      </button>
      <button className="tb-btn" title="最小化" onClick={async () => (await currentWindow())?.minimize()}>
        <Minus size={14} />
      </button>
      <button className="tb-btn close" title="关闭" onClick={async () => (await currentWindow())?.close()}>
        <X size={14} />
      </button>
    </header>
  );
}
