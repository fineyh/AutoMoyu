// 全局状态：后端推来的状态快照、事件流、设置、更新。

import { create } from "zustand";

import { api, on } from "./api";
import { getLang, setLang } from "./i18n";
import type { DeepPartial, FeedItem, Lang, Settings, Status, UpdateInfo } from "./types";

export type Tab = "home" | "stats" | "settings";

export type UpdateState =
  | { state: "none" }
  | { state: "available"; info: UpdateInfo }
  | { state: "downloading"; info: UpdateInfo; progress: number | null }
  | { state: "ready"; info: UpdateInfo }
  | { state: "error"; message: string };

interface Store {
  status: Status | null;
  /** 界面语言；变了整棵树重新渲染（见 main.tsx）。 */
  lang: Lang;
  settings: Settings | null;
  feed: FeedItem[];
  tab: Tab;
  toast: { id: number; text: string } | null;
  update: UpdateState;
  /** 每钓到一条 +1，用来触发主视觉的 "+1" 动画。 */
  catchPulse: number;
  setTab: (t: Tab) => void;
  showToast: (text: string) => void;
  patchSettings: (p: DeepPartial<Settings>) => Promise<void>;
  setUpdate: (u: UpdateState) => void;
}

export const useStore = create<Store>((set) => ({
  status: null,
  lang: getLang(),
  settings: null,
  feed: [],
  tab: "home",
  toast: null,
  update: { state: "none" },
  catchPulse: 0,
  setTab: (tab) => set({ tab }),
  showToast: (text) => set({ toast: { id: Date.now(), text } }),
  patchSettings: async (p) => {
    try {
      const s = await api.setSettings(p);
      set({ settings: s });
    } catch (e) {
      set({ toast: { id: Date.now(), text: String(e) } });
    }
  },
  setUpdate: (update) => set({ update }),
}));

let wired = false;

/** 接上后端事件。只调一次。 */
export async function wireBackend() {
  if (wired) return;
  wired = true;
  const set = useStore.setState;
  const withLang = (status: Status | null) => {
    if (!status) return { status };
    if (status.lang !== getLang()) setLang(status.lang);
    return { status, lang: status.lang };
  };
  const [status, settings] = await Promise.all([api.status(), api.settings()]);
  set({ ...withLang(status), settings });
  await on<Status>("status", (s) => set(withLang(s)));
  await on<Settings>("settings-changed", (s) => set({ settings: s }));
  await on<string>("toast", (text) => set({ toast: { id: Date.now(), text } }));
  await on<string>("navigate", () => set({ tab: "home" }));
  await on<FeedItem>("engine-event", (item) => {
    set((st) => ({
      feed: [item, ...st.feed].slice(0, 40),
      catchPulse: item.event.kind === "catch" ? st.catchPulse + 1 : st.catchPulse,
    }));
  });
}
