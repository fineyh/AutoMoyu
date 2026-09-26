// 小组件：浮漂图标、开关、下拉、提示条、窗口选择。

import * as RS from "@radix-ui/react-select";
import { AlertTriangle, ChevronDown, Info } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";

import { api } from "../lib/api";
import { t } from "../lib/i18n";
import { useStore } from "../lib/store";
import type { GameWindow } from "../lib/types";

/** 8×10 像素浮漂（和应用图标同一张）。 */
export function Bobber({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 8 10" shapeRendering="crispEdges" className={className} aria-hidden="true">
      <rect x="3" y="0" width="2" height="1" fill="#3A2A1A" />
      <rect x="2" y="1" width="4" height="3" fill="#D42A2A" />
      <rect x="3" y="1" width="1" height="1" fill="#FF7A6E" />
      <rect x="2" y="4" width="4" height="2" fill="#F4F4F4" />
      <rect x="5" y="4" width="1" height="2" fill="#D5DCE6" />
      <rect x="2" y="6" width="4" height="1" fill="#C8D6F2" />
      <rect x="3" y="7" width="2" height="1" fill="#A9C0EC" />
    </svg>
  );
}

export function Switch({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      className="sw"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    />
  );
}

export function Select<T extends string>({
  value,
  options,
  onChange,
  label,
  disabled,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <RS.Root value={value} onValueChange={(v) => onChange(v as T)} disabled={disabled}>
      <RS.Trigger className="sel-trigger" aria-label={label}>
        <RS.Value />
        <RS.Icon>
          <ChevronDown size={12} className="muted" />
        </RS.Icon>
      </RS.Trigger>
      <RS.Portal>
        <RS.Content className="sel-content" position="popper" sideOffset={4} align="end">
          <RS.Viewport>
            {options.map((o) => (
              <RS.Item key={o.value} value={o.value} className="sel-item">
                <RS.ItemText>{o.label}</RS.ItemText>
              </RS.Item>
            ))}
          </RS.Viewport>
        </RS.Content>
      </RS.Portal>
    </RS.Root>
  );
}

export function Note({ title, hint, children, tone = "warn" }: { title: string; hint?: string; children?: ReactNode; tone?: "warn" | "info" }) {
  return (
    <div className={`note ${tone === "info" ? "info" : ""}`} role="status">
      {tone === "info" ? <Info size={18} /> : <AlertTriangle size={18} />}
      <div>
        <b>{title}</b>
        {hint && <span>{hint}</span>}
      </div>
      {children}
    </div>
  );
}

export function Toast() {
  const toast = useStore((s) => s.toast);
  const [shown, setShown] = useState<number | null>(null);
  useEffect(() => {
    if (!toast) return;
    setShown(toast.id);
    const timer = setTimeout(() => setShown(null), 3500);
    return () => clearTimeout(timer);
  }, [toast]);
  if (!toast || shown !== toast.id) return null;
  return (
    <div className="toast" role="alert" onClick={() => setShown(null)}>
      {toast.text}
    </div>
  );
}

/** 手动选择游戏窗口（自动识别失败时用）。 */
export function WindowPicker({ onClose }: { onClose: () => void }) {
  const [list, setList] = useState<GameWindow[] | null>(null);
  const settings = useStore((s) => s.settings);
  useEffect(() => {
    api.listWindows().then(setList);
  }, []);
  const pick = async (w: GameWindow | null) => {
    const s = await api.pickWindow(w ? { process: w.process, title: w.title } : null);
    useStore.setState({ settings: s });
    onClose();
  };
  return (
    <div className="sheet-bg" onClick={onClose}>
      <div className="sheet" onClick={(e) => e.stopPropagation()} role="dialog" aria-label={t("选择游戏窗口", "Pick the game window")}>
        <div className="wz">
          <b>{t("选择游戏窗口", "Pick the game window")}</b>
          <button className="link" onClick={onClose}>{t("关闭", "Close")}</button>
        </div>
        <button className="win-item" onClick={() => pick(null)}>
          <span>{t("自动识别", "Automatic")}{settings?.advanced.window ? "" : t("（当前）", " (current)")}</span>
          <small>{t("按进程名和标题自动找 Minecraft", "Finds Minecraft by process name and title")}</small>
        </button>
        {list === null && <div className="empty">{t("正在列出窗口…", "Listing windows…")}</div>}
        {list?.length === 0 && <div className="empty">{t("没有可选的窗口。请先打开游戏，并改成窗口化或无边框。", "No windows to pick. Open the game first and switch it to windowed or borderless.")}</div>}
        {list?.map((w) => (
          <button className="win-item" key={w.hwnd} onClick={() => pick(w)}>
            <span>{w.title}</span>
            <small>
              {w.process} · {w.w}×{w.h}
              {settings?.advanced.window?.title === w.title && settings.advanced.window.process === w.process ? t(" · 当前", " · current") : ""}
            </small>
          </button>
        ))}
      </div>
    </div>
  );
}
