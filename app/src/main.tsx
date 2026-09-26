import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { useStore, wireBackend } from "./lib/store";
import { startUpdateLoop } from "./lib/updater";
import { Overlay } from "./overlay/Overlay";
import "./styles.css";

const isOverlay = new URLSearchParams(location.search).get("view") === "overlay";
const root = document.documentElement;
if (isOverlay) root.classList.add("overlay");

// 主题：跟随系统 / 强制深浅
useStore.subscribe((s, prev) => {
  const t = s.settings?.ui.theme;
  if (t === prev.settings?.ui.theme) return;
  if (t === "light" || t === "dark") root.dataset.theme = t;
  else delete root.dataset.theme;
});

// 禁掉 WebView 的右键菜单和 Ctrl+R 刷新，像个原生应用
window.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
});
window.addEventListener("keydown", (e) => {
  if ((e.ctrlKey && (e.key === "r" || e.key === "p")) || e.key === "F5") e.preventDefault();
});

void wireBackend();
if (!isOverlay) startUpdateLoop();

createRoot(document.getElementById("root")!).render(<StrictMode>{isOverlay ? <Overlay /> : <App />}</StrictMode>);
