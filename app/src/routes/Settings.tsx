// 设置：Win11 设置页式的分组列表。没有毫秒/阈值输入框；少数真正需要的旋钮收在「高级」里。

import { ChevronDown, ChevronRight } from "lucide-react";
import { useEffect, useState } from "react";

import { Select, Switch, WindowPicker } from "../components/ui";
import { api, openUrl, revealPath, saveDialog } from "../lib/api";
import { t } from "../lib/i18n";
import { useStore } from "../lib/store";
import { checkForUpdate } from "../lib/updater";
import type { AppInfo, AutoStopKind, Settings as S } from "../lib/types";

const REPO = "https://github.com/fineyh/AutoMoyu";

/** 把 keydown 变成 Tauri 快捷键字符串，比如 "F6"、"Ctrl+Shift+K"。 */
function shortcutFrom(e: KeyboardEvent): string | null {
  const k = e.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(k)) return null;
  let key: string;
  if (/^F\d{1,2}$/.test(k)) key = k;
  else if (e.code.startsWith("Key")) key = e.code.slice(3);
  else if (e.code.startsWith("Digit")) key = e.code.slice(5);
  else if (["Home", "End", "Insert", "PageUp", "PageDown", "Pause", "ScrollLock"].includes(e.code)) key = e.code;
  else return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
  // 单个字母/数字不带修饰键会影响打字，不允许
  if (mods.length === 0 && key.length === 1) return null;
  return [...mods, key].join("+");
}

function HotkeyField({ value, onChange, label }: { value: string; onChange: (v: string) => void; label: string }) {
  const [rec, setRec] = useState(false);
  useEffect(() => {
    if (!rec) return;
    const h = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape") return setRec(false);
      const s = shortcutFrom(e);
      if (s) {
        onChange(s);
        setRec(false);
      }
    };
    window.addEventListener("keydown", h, true);
    return () => window.removeEventListener("keydown", h, true);
  }, [rec, onChange]);
  return (
    <button
      className={`kbd ${rec ? "rec" : ""}`}
      aria-label={t(`${label}：${value}，点击后按新按键`, `${label}: ${value}. Click, then press a new key`)}
      onClick={() => setRec(!rec)}
    >
      {rec ? t("按下新按键…", "Press a key…") : value}
    </button>
  );
}

function NumberField({ value, min, max, onCommit, suffix }: { value: number; min: number; max: number; onCommit: (v: number) => void; suffix: string }) {
  const [v, setV] = useState(String(value));
  useEffect(() => setV(String(value)), [value]);
  const commit = () => {
    const n = Math.round(Number(v));
    if (Number.isFinite(n)) onCommit(Math.min(max, Math.max(min, n)));
    else setV(String(value));
  };
  return (
    <span className="flex items-center gap-1.5">
      <input
        className="field"
        inputMode="numeric"
        value={v}
        onChange={(e) => setV(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
      <span className="muted">{suffix}</span>
    </span>
  );
}

const autoStopOptions = (): { value: AutoStopKind; label: string }[] => [
  { value: "none", label: t("不限", "Never") },
  { value: "minutes", label: t("运行一段时间", "After some time") },
  { value: "catches", label: t("钓到一定条数", "After some fish") },
  { value: "at", label: t("到某个时刻", "At a set time") },
];

export function Settings() {
  const s = useStore((st) => st.settings);
  const status = useStore((st) => st.status);
  const update = useStore((st) => st.update);
  const patch = useStore((st) => st.patchSettings);
  const toast = useStore((st) => st.showToast);
  const [adv, setAdv] = useState(false);
  const [picker, setPicker] = useState(false);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [checking, setChecking] = useState(false);
  useEffect(() => {
    api.appInfo().then(setInfo);
  }, []);
  if (!s) return null;
  const running = !!status?.session;
  const p = (x: Parameters<typeof patch>[0]) => void patch(x);
  const sec = t("秒", "s");

  const at = s.autoStop.value;
  const timeStr = `${String(Math.floor(at / 60)).padStart(2, "0")}:${String(at % 60).padStart(2, "0")}`;

  const exportDiag = async () => {
    const d = new Date();
    const date = `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}`;
    const path = await saveDialog(t(`AutoMoyu-诊断-${date}.zip`, `AutoMoyu-diagnostics-${date}.zip`), t("诊断包", "Diagnostics"), ["zip"]);
    if (!path) return;
    try {
      await api.exportDiagnostics(path);
      toast(t("诊断包已导出", "Diagnostics exported"));
      void revealPath(path);
    } catch (e) {
      toast(t(`导出失败：${e}`, `Export failed: ${e}`));
    }
  };

  const manualCheck = async () => {
    setChecking(true);
    const u = await checkForUpdate(true);
    setChecking(false);
    if (u.state === "none") toast(t("已是最新版本", "You're up to date"));
    if (u.state === "error") toast(t(`检查更新失败：${u.message}`, `Update check failed: ${u.message}`));
  };

  const updateLine =
    update.state === "ready"
      ? t(`新版本 ${update.info.version} 已下载，停止钓鱼后可重启更新`, `Version ${update.info.version} downloaded; restart to update once you stop fishing`)
      : update.state === "downloading"
        ? t(`正在下载 ${update.info.version}…`, `Downloading ${update.info.version}…`)
        : update.state === "available"
          ? t(`发现新版本 ${update.info.version}`, `Version ${update.info.version} available`)
          : t("点右边检查", "Click to check");

  return (
    <>
      <section className="grp" aria-label={t("常规", "General")}>
        <div className="grp-h">{t("常规", "General")}</div>
        <div className="row">
          <span className="lbl">{t("开始 / 暂停", "Start / pause")}</span>
          <HotkeyField label={t("开始 / 暂停", "Start / pause")} value={s.hotkeys.toggle} onChange={(v) => p({ hotkeys: { toggle: v } })} />
        </div>
        <div className="row">
          <span className="lbl">
            {t("显示游戏内浮层", "In-game overlay")}
            <small>{t("透明小条盖在游戏左上角，不会被识别截到", "A small see-through bar at the game's top left; never captured by detection")}</small>
          </span>
          <HotkeyField label={t("浮层热键", "Overlay hotkey")} value={s.hotkeys.overlay} onChange={(v) => p({ hotkeys: { overlay: v } })} />
          <Switch label={t("显示游戏内浮层", "In-game overlay")} checked={s.ui.overlay} onChange={(v) => p({ ui: { overlay: v } })} />
        </div>
        <div className="row">
          <span className="lbl">{t("关闭窗口时最小化到托盘", "Close to tray")}</span>
          <Switch label={t("关闭窗口时最小化到托盘", "Close to tray")} checked={s.ui.closeToTray} onChange={(v) => p({ ui: { closeToTray: v } })} />
        </div>
        <div className="row">
          <span className="lbl">{t("主题", "Theme")}</span>
          <Select
            label={t("主题", "Theme")}
            value={s.ui.theme}
            onChange={(v) => p({ ui: { theme: v } })}
            options={[
              { value: "system", label: t("跟随系统", "System") },
              { value: "dark", label: t("深色", "Dark") },
              { value: "light", label: t("浅色", "Light") },
            ]}
          />
        </div>
        <div className="row">
          <span className="lbl">{t("语言", "Language")}</span>
          <Select
            label="语言 / Language"
            value={s.ui.language}
            onChange={(v) => p({ ui: { language: v } })}
            options={[
              { value: "auto", label: t("跟随系统", "System") },
              { value: "zh", label: "中文" },
              { value: "en", label: "English" },
            ]}
          />
        </div>
      </section>

      <section className="grp" aria-label={t("钓鱼", "Fishing")}>
        <div className="grp-h">{t("钓鱼", "Fishing")}</div>
        <div className="row">
          <span className="lbl">{t("模式", "Mode")}</span>
          <Select<S["mode"]>
            label={t("模式", "Mode")}
            value={s.mode}
            disabled={running}
            onChange={(v) => p({ mode: v })}
            options={[
              { value: "rodOnly", label: t("自动甩竿（有钓鱼机）", "Auto-cast (with a fish farm)") },
              { value: "full", label: t("全自动（无钓鱼机）", "Full auto (no farm)") },
            ]}
          />
        </div>
        {s.mode === "full" && (
          <div className="row">
            <span className="lbl">
              {t("咬钩识别", "Bite detection")}
              <small>{t("只听 Minecraft 自己的声音；音乐、语音不影响", "Listens to Minecraft only; your music and voice chat don't matter")}</small>
            </span>
            <Select
              label={t("咬钩灵敏度", "Bite sensitivity")}
              value={s.advanced.biteSensitivity}
              onChange={(v) => p({ advanced: { biteSensitivity: v } })}
              options={[
                { value: "low", label: t("灵敏度 低", "Low sensitivity") },
                { value: "normal", label: t("灵敏度 标准", "Normal sensitivity") },
                { value: "high", label: t("灵敏度 高", "High sensitivity") },
              ]}
            />
          </div>
        )}
        <button className="row clickable" onClick={() => void api.calOpen()}>
          <span className="lbl">
            {t("重新校准", "Recalibrate")}
            <small>
              {status?.calibration
                ? t(`已校准 ${status.calibration.w}×${status.calibration.h}`, `Calibrated for ${status.calibration.w}×${status.calibration.h}`)
                : t("当前窗口尺寸还没校准", "This window size isn't calibrated yet")}
            </small>
          </span>
          <ChevronRight size={14} className="muted" />
        </button>
      </section>

      <section className="grp" aria-label={t("自动停止", "Auto stop")}>
        <div className="grp-h">{t("自动停止", "Auto stop")}</div>
        <div className="row">
          <span className="lbl">
            {t("停止条件", "Stop")}
            <small>{t("停下时发系统通知", "Sends a notification when it stops")}</small>
          </span>
          <Select
            label={t("停止条件", "Stop")}
            value={s.autoStop.kind}
            onChange={(v) => p({ autoStop: { kind: v, value: v === "minutes" ? 60 : v === "catches" ? 200 : v === "at" ? 7 * 60 : 0 } })}
            options={autoStopOptions()}
          />
        </div>
        {s.autoStop.kind === "minutes" && (
          <div className="row">
            <span className="lbl">{t("运行时长", "Run for")}</span>
            <NumberField value={s.autoStop.value} min={1} max={24 * 60} suffix={t("分钟", "min")} onCommit={(v) => p({ autoStop: { value: v } })} />
          </div>
        )}
        {s.autoStop.kind === "catches" && (
          <div className="row">
            <span className="lbl">{t("钓到", "Catch")}</span>
            <NumberField value={s.autoStop.value} min={1} max={100000} suffix={t("条", "fish")} onCommit={(v) => p({ autoStop: { value: v } })} />
          </div>
        )}
        {s.autoStop.kind === "at" && (
          <div className="row">
            <span className="lbl">{t("到这个时刻停", "Stop at")}</span>
            <input
              type="time"
              className="field"
              style={{ width: 96 }}
              value={timeStr}
              onChange={(e) => {
                const [h, m] = e.target.value.split(":").map(Number);
                if (Number.isFinite(h) && Number.isFinite(m)) p({ autoStop: { value: h * 60 + m } });
              }}
            />
          </div>
        )}
      </section>

      <section className="grp" aria-label={t("通知", "Notifications")}>
        <div className="grp-h">{t("通知", "Notifications")}</div>
        <div className="row">
          <span className="lbl">{t("自动停止、游戏关闭时通知", "When it auto-stops or the game closes")}</span>
          <Switch label={t("停止时通知", "Notify on stop")} checked={s.notify.onStop} onChange={(v) => p({ notify: { onStop: v } })} />
        </div>
        <div className="row">
          <span className="lbl">
            {t("出问题时通知", "When something goes wrong")}
            <small>{t("没甩出去、认不出鱼竿", "Cast failed, rod not recognised")}</small>
          </span>
          <Switch label={t("出问题时通知", "Notify on problems")} checked={s.notify.onError} onChange={(v) => p({ notify: { onError: v } })} />
        </div>
        <div className="row">
          <span className="lbl">{t("钓到时提示音", "Sound on catch")}</span>
          <Switch label={t("钓到时提示音", "Sound on catch")} checked={s.ui.catchSound} onChange={(v) => p({ ui: { catchSound: v } })} />
        </div>
      </section>

      <section className="grp" aria-label={t("更新", "Updates")}>
        <div className="grp-h">{t("更新", "Updates")}</div>
        <div className="row">
          <span className="lbl">{t("自动检查更新", "Check automatically")}</span>
          <Switch label={t("自动检查更新", "Check automatically")} checked={s.update.auto} onChange={(v) => p({ update: { auto: v } })} />
        </div>
        <div className="row">
          <span className="lbl">
            {t("更新通道", "Channel")}
            <small>{t("测试版更新更快，偶尔不稳定", "Beta gets updates sooner but may be less stable")}</small>
          </span>
          <Select
            label={t("更新通道", "Channel")}
            value={s.update.channel}
            onChange={(v) => p({ update: { channel: v } })}
            options={[
              { value: "stable", label: t("稳定版", "Stable") },
              { value: "beta", label: t("测试版", "Beta") },
            ]}
          />
        </div>
        <div className="row">
          <span className="lbl">
            {t("版本", "Version")} {info?.version ?? ""}
            <small>{updateLine}</small>
          </span>
          <button className="ghost" disabled={checking} onClick={manualCheck}>
            {checking ? t("检查中…", "Checking…") : t("检查更新", "Check now")}
          </button>
        </div>
        <button className="row clickable" onClick={() => void openUrl(`${REPO}/releases`)}>
          <span className="lbl">{t("更新日志", "Release notes")}</span>
          <ChevronRight size={14} className="muted" />
        </button>
      </section>

      <section className="grp" aria-label={t("高级", "Advanced")}>
        <button className="row clickable" aria-expanded={adv} onClick={() => setAdv(!adv)}>
          <span className="lbl">
            {t("高级", "Advanced")}
            <small>{t("按住时长、最长等待、诊断包、重置校准", "Click length, max wait, diagnostics, reset calibration")}</small>
          </span>
          {adv ? <ChevronDown size={14} className="muted" /> : <ChevronRight size={14} className="muted" />}
        </button>
        {adv && (
          <>
            <div className="row">
              <span className="lbl">
                {t("甩竿按住时长", "Click hold time")}
                <small>{t("右键按下到松开", "Right button down to up")}</small>
              </span>
              <NumberField value={s.advanced.clickHoldMs} min={30} max={500} suffix={t("毫秒", "ms")} onCommit={(v) => p({ advanced: { clickHoldMs: v } })} />
            </div>
            <div className="row">
              <span className="lbl">
                {t("最长等待", "Max wait")}
                <small>{t("甩出后这么久没收回，就收竿重甩", "Reel in and recast if nothing bites for this long")}</small>
              </span>
              <NumberField value={s.advanced.maxWaitS} min={10} max={600} suffix={sec} onCommit={(v) => p({ advanced: { maxWaitS: v } })} />
            </div>
            <div className="row">
              <span className="lbl">
                {t("仅游戏在前台时点击", "Click only when the game is in front")}
                <small>{t("建议保持开启，防止点到别的窗口", "Keep this on so clicks never land in other windows")}</small>
              </span>
              <Switch
                label={t("仅游戏在前台时点击", "Click only when the game is in front")}
                checked={s.advanced.focusGuard}
                onChange={(v) => p({ advanced: { focusGuard: v } })}
              />
            </div>
            <div className="row">
              <span className="lbl">
                {t("你操作时让出控制", "Step aside while you play")}
                <small>{t("在游戏里动鼠标、按键就先暂停，停手后自动继续", "Pauses when you move the mouse or press keys in game; resumes when you stop")}</small>
              </span>
              <Switch label={t("你操作时让出控制", "Step aside while you play")} checked={s.advanced.takeover} onChange={(v) => p({ advanced: { takeover: v } })} />
            </div>
            {s.advanced.takeover && (
              <div className="row">
                <span className="lbl">{t("停手多久后继续", "Resume after idle for")}</span>
                <NumberField value={s.advanced.takeoverIdleS} min={1} max={60} suffix={sec} onCommit={(v) => p({ advanced: { takeoverIdleS: v } })} />
              </div>
            )}
            <button className="row clickable" onClick={() => setPicker(true)}>
              <span className="lbl">
                {t("游戏窗口", "Game window")}
                <small>{s.advanced.window ? `${s.advanced.window.title} · ${s.advanced.window.process}` : t("自动识别", "Automatic")}</small>
              </span>
              <ChevronRight size={14} className="muted" />
            </button>
            <div className="row">
              <span className="lbl">
                {t("调试浮层", "Debug overlay")}
                <small>{t("在游戏上画出识别区域", "Draws the detection area over the game")}</small>
              </span>
              <Switch label={t("调试浮层", "Debug overlay")} checked={s.advanced.debugOverlay} onChange={(v) => p({ advanced: { debugOverlay: v } })} />
            </div>
            {status?.signal && (
              <div className="row">
                <span className="lbl">
                  {t("实时信号", "Live signal")}
                  <small className="mono">
                    {t("离收回", "to reeled-in")} {status.signal.dIn.toFixed(1)} · {t("离甩出", "to cast-out")} {status.signal.dOut.toFixed(1)}
                    {status.signal.audioDb != null &&
                      ` · ${t("声音", "sound")} ${status.signal.audioDb.toFixed(0)}/${status.signal.audioGateDb?.toFixed(0) ?? "—"} dB`}
                  </small>
                </span>
              </div>
            )}
            <div className="row">
              <span className="lbl">
                {t("导出诊断包", "Export diagnostics")}
                <small>{t("日志、校准截图、系统信息，反馈问题时附上", "Logs, calibration images and system info to attach to a bug report")}</small>
              </span>
              <button className="ghost" onClick={exportDiag}>
                {t("导出", "Export")}
              </button>
            </div>
            <div className="row">
              <span className="lbl">{t("打开数据目录", "Open data folder")}</span>
              <button className="ghost" onClick={() => info && void revealPath(info.dataDir)}>
                {t("打开", "Open")}
              </button>
            </div>
            <div className="row">
              <span className="lbl">
                {t("重置校准", "Reset calibration")}
                <small>{t("删除所有窗口尺寸的校准结果", "Deletes calibration for every window size")}</small>
              </span>
              <button
                className="ghost danger"
                disabled={running}
                onClick={() => {
                  void api.calReset();
                  toast(t("已重置，下次开始前会重新校准", "Reset. You'll calibrate again before the next start"));
                }}
              >
                {t("重置", "Reset")}
              </button>
            </div>
          </>
        )}
      </section>

      <section className="grp" aria-label={t("关于", "About")}>
        <div className="grp-h">{t("关于", "About")}</div>
        <button className="row clickable" onClick={() => void openUrl(REPO)}>
          <span className="lbl">
            AutoMoyu {info?.version}
            <small>GitHub · MIT License</small>
          </span>
          <ChevronRight size={14} className="muted" />
        </button>
        <button
          className="row clickable"
          onClick={() =>
            void openUrl(
              `${REPO}/issues/new?title=${encodeURIComponent(t("[反馈] ", "[Bug] "))}&body=${encodeURIComponent(
                t(
                  `版本：${info?.version}\n模式：${s.mode}\n窗口：${status?.window ? `${status.window.w}x${status.window.h}` : "未找到"}\n\n遇到的问题：\n\n（可以在 设置 → 高级 → 导出诊断包，把 zip 拖进来）`,
                  `Version: ${info?.version}\nMode: ${s.mode}\nWindow: ${status?.window ? `${status.window.w}x${status.window.h}` : "not found"}\n\nWhat happened:\n\n(Settings → Advanced → Export diagnostics, then drag the zip in here)`,
                ),
              )}`,
            )
          }
        >
          <span className="lbl">
            {t("反馈问题", "Report a problem")}
            <small>{t("会预填版本和窗口信息", "Pre-fills version and window info")}</small>
          </span>
          <ChevronRight size={14} className="muted" />
        </button>
      </section>
      <p className="fine">
        {t(
          "AutoMoyu 只在游戏窗口里模拟鼠标右键，不读写游戏内存、不修改游戏文件。挂机钓鱼可能违反部分服务器的规则，使用前请确认。本项目与 Mojang、Microsoft 无关。",
          "AutoMoyu only simulates right-clicks in the game window. It doesn't read or write game memory or change game files. AFK fishing may break some servers' rules, so check first. Not affiliated with Mojang or Microsoft.",
        )}
      </p>
      {picker && <WindowPicker onClose={() => setPicker(false)} />}
    </>
  );
}
