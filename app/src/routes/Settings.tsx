// 设置：Win11 设置页式的分组列表。没有毫秒/阈值输入框；少数真正需要的旋钮收在「高级」里。

import { ChevronDown, ChevronRight } from "lucide-react";
import { useEffect, useState } from "react";

import { Select, Switch, WindowPicker } from "../components/ui";
import { api, openUrl, revealPath, saveDialog } from "../lib/api";
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
    <button className={`kbd ${rec ? "rec" : ""}`} aria-label={`${label}：${value}，点击后按新按键`} onClick={() => setRec(!rec)}>
      {rec ? "按下新按键…" : value}
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

const autoStopOptions: { value: AutoStopKind; label: string }[] = [
  { value: "none", label: "不限" },
  { value: "minutes", label: "运行一段时间" },
  { value: "catches", label: "钓到一定条数" },
  { value: "at", label: "到某个时刻" },
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

  const at = s.autoStop.value;
  const timeStr = `${String(Math.floor(at / 60)).padStart(2, "0")}:${String(at % 60).padStart(2, "0")}`;

  const exportDiag = async () => {
    const d = new Date();
    const name = `AutoMoyu-诊断-${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}.zip`;
    const path = await saveDialog(name, "诊断包", ["zip"]);
    if (!path) return;
    try {
      await api.exportDiagnostics(path);
      toast("诊断包已导出");
      void revealPath(path);
    } catch (e) {
      toast(`导出失败：${e}`);
    }
  };

  const manualCheck = async () => {
    setChecking(true);
    const u = await checkForUpdate(true);
    setChecking(false);
    if (u.state === "none") toast("已是最新版本");
    if (u.state === "error") toast(`检查更新失败：${u.message}`);
  };

  const updateLine =
    update.state === "ready"
      ? `新版本 ${update.info.version} 已下载，停止钓鱼后可重启更新`
      : update.state === "downloading"
        ? `正在下载 ${update.info.version}…`
        : update.state === "available"
          ? `发现新版本 ${update.info.version}`
          : "点右边检查";

  return (
    <>
      <section className="grp" aria-label="常规">
        <div className="grp-h">常规</div>
        <div className="row">
          <span className="lbl">开始 / 暂停</span>
          <HotkeyField label="开始 / 暂停" value={s.hotkeys.toggle} onChange={(v) => p({ hotkeys: { toggle: v } })} />
        </div>
        <div className="row">
          <span className="lbl">
            显示游戏内浮层
            <small>透明小条盖在游戏左上角，不会被识别截到</small>
          </span>
          <HotkeyField label="浮层热键" value={s.hotkeys.overlay} onChange={(v) => p({ hotkeys: { overlay: v } })} />
          <Switch label="显示游戏内浮层" checked={s.ui.overlay} onChange={(v) => p({ ui: { overlay: v } })} />
        </div>
        <div className="row">
          <span className="lbl">关闭窗口时最小化到托盘</span>
          <Switch label="关闭窗口时最小化到托盘" checked={s.ui.closeToTray} onChange={(v) => p({ ui: { closeToTray: v } })} />
        </div>
        <div className="row">
          <span className="lbl">主题</span>
          <Select
            label="主题"
            value={s.ui.theme}
            onChange={(v) => p({ ui: { theme: v } })}
            options={[
              { value: "system", label: "跟随系统" },
              { value: "dark", label: "深色" },
              { value: "light", label: "浅色" },
            ]}
          />
        </div>
      </section>

      <section className="grp" aria-label="钓鱼">
        <div className="grp-h">钓鱼</div>
        <div className="row">
          <span className="lbl">模式</span>
          <Select<S["mode"]>
            label="模式"
            value={s.mode}
            disabled={running}
            onChange={(v) => p({ mode: v })}
            options={[
              { value: "rodOnly", label: "自动甩竿（有钓鱼机）" },
              { value: "full", label: "全自动（无钓鱼机）" },
            ]}
          />
        </div>
        {s.mode === "full" && (
          <div className="row">
            <span className="lbl">
              咬钩识别
              <small>只听 Minecraft 自己的声音；音乐、语音不影响</small>
            </span>
            <Select
              label="咬钩灵敏度"
              value={s.advanced.biteSensitivity}
              onChange={(v) => p({ advanced: { biteSensitivity: v } })}
              options={[
                { value: "low", label: "灵敏度 低" },
                { value: "normal", label: "灵敏度 标准" },
                { value: "high", label: "灵敏度 高" },
              ]}
            />
          </div>
        )}
        <button className="row clickable" onClick={() => void api.calOpen()}>
          <span className="lbl">
            重新校准
            <small>
              {status?.calibration
                ? `已校准 ${status.calibration.w}×${status.calibration.h}`
                : "当前窗口尺寸还没校准"}
            </small>
          </span>
          <ChevronRight size={14} className="muted" />
        </button>
      </section>

      <section className="grp" aria-label="自动停止">
        <div className="grp-h">自动停止</div>
        <div className="row">
          <span className="lbl">
            停止条件<small>停下时发系统通知</small>
          </span>
          <Select
            label="停止条件"
            value={s.autoStop.kind}
            onChange={(v) => p({ autoStop: { kind: v, value: v === "minutes" ? 60 : v === "catches" ? 200 : v === "at" ? 7 * 60 : 0 } })}
            options={autoStopOptions}
          />
        </div>
        {s.autoStop.kind === "minutes" && (
          <div className="row">
            <span className="lbl">运行时长</span>
            <NumberField value={s.autoStop.value} min={1} max={24 * 60} suffix="分钟" onCommit={(v) => p({ autoStop: { value: v } })} />
          </div>
        )}
        {s.autoStop.kind === "catches" && (
          <div className="row">
            <span className="lbl">钓到</span>
            <NumberField value={s.autoStop.value} min={1} max={100000} suffix="条" onCommit={(v) => p({ autoStop: { value: v } })} />
          </div>
        )}
        {s.autoStop.kind === "at" && (
          <div className="row">
            <span className="lbl">到这个时刻停</span>
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

      <section className="grp" aria-label="通知">
        <div className="grp-h">通知</div>
        <div className="row">
          <span className="lbl">自动停止、游戏关闭时通知</span>
          <Switch label="停止时通知" checked={s.notify.onStop} onChange={(v) => p({ notify: { onStop: v } })} />
        </div>
        <div className="row">
          <span className="lbl">
            出问题时通知<small>没甩出去、认不出鱼竿</small>
          </span>
          <Switch label="出问题时通知" checked={s.notify.onError} onChange={(v) => p({ notify: { onError: v } })} />
        </div>
        <div className="row">
          <span className="lbl">钓到时提示音</span>
          <Switch label="钓到时提示音" checked={s.ui.catchSound} onChange={(v) => p({ ui: { catchSound: v } })} />
        </div>
      </section>

      <section className="grp" aria-label="更新">
        <div className="grp-h">更新</div>
        <div className="row">
          <span className="lbl">自动检查更新</span>
          <Switch label="自动检查更新" checked={s.update.auto} onChange={(v) => p({ update: { auto: v } })} />
        </div>
        <div className="row">
          <span className="lbl">
            更新通道<small>测试版更新更快，偶尔不稳定</small>
          </span>
          <Select
            label="更新通道"
            value={s.update.channel}
            onChange={(v) => p({ update: { channel: v } })}
            options={[
              { value: "stable", label: "稳定版" },
              { value: "beta", label: "测试版" },
            ]}
          />
        </div>
        <div className="row">
          <span className="lbl">
            版本 {info?.version ?? ""}
            <small>{updateLine}</small>
          </span>
          <button className="ghost" disabled={checking} onClick={manualCheck}>
            {checking ? "检查中…" : "检查更新"}
          </button>
        </div>
        <button className="row clickable" onClick={() => void openUrl(`${REPO}/releases`)}>
          <span className="lbl">更新日志</span>
          <ChevronRight size={14} className="muted" />
        </button>
      </section>

      <section className="grp" aria-label="高级">
        <button className="row clickable" aria-expanded={adv} onClick={() => setAdv(!adv)}>
          <span className="lbl">
            高级<small>按住时长、最长等待、诊断包、重置校准</small>
          </span>
          {adv ? <ChevronDown size={14} className="muted" /> : <ChevronRight size={14} className="muted" />}
        </button>
        {adv && (
          <>
            <div className="row">
              <span className="lbl">
                甩竿按住时长<small>右键按下到松开</small>
              </span>
              <NumberField value={s.advanced.clickHoldMs} min={30} max={500} suffix="毫秒" onCommit={(v) => p({ advanced: { clickHoldMs: v } })} />
            </div>
            <div className="row">
              <span className="lbl">
                最长等待<small>甩出后这么久没收回，就收竿重甩</small>
              </span>
              <NumberField value={s.advanced.maxWaitS} min={10} max={600} suffix="秒" onCommit={(v) => p({ advanced: { maxWaitS: v } })} />
            </div>
            <div className="row">
              <span className="lbl">
                仅游戏在前台时点击<small>建议保持开启，防止点到别的窗口</small>
              </span>
              <Switch label="仅游戏在前台时点击" checked={s.advanced.focusGuard} onChange={(v) => p({ advanced: { focusGuard: v } })} />
            </div>
            <button className="row clickable" onClick={() => setPicker(true)}>
              <span className="lbl">
                游戏窗口<small>{s.advanced.window ? `${s.advanced.window.title} · ${s.advanced.window.process}` : "自动识别"}</small>
              </span>
              <ChevronRight size={14} className="muted" />
            </button>
            <div className="row">
              <span className="lbl">
                调试浮层<small>在游戏上画出识别区域</small>
              </span>
              <Switch label="调试浮层" checked={s.advanced.debugOverlay} onChange={(v) => p({ advanced: { debugOverlay: v } })} />
            </div>
            {status?.signal && (
              <div className="row">
                <span className="lbl">
                  实时信号
                  <small className="mono">
                    离收回 {status.signal.dIn.toFixed(1)} · 离甩出 {status.signal.dOut.toFixed(1)}
                    {status.signal.audioDb != null &&
                      ` · 声音 ${status.signal.audioDb.toFixed(0)}/${status.signal.audioGateDb?.toFixed(0) ?? "—"} dB`}
                  </small>
                </span>
              </div>
            )}
            <div className="row">
              <span className="lbl">
                导出诊断包<small>日志、校准截图、系统信息，反馈问题时附上</small>
              </span>
              <button className="ghost" onClick={exportDiag}>
                导出
              </button>
            </div>
            <div className="row">
              <span className="lbl">打开数据目录</span>
              <button className="ghost" onClick={() => info && void revealPath(info.dataDir)}>
                打开
              </button>
            </div>
            <div className="row">
              <span className="lbl">
                重置校准<small>删除所有窗口尺寸的校准结果</small>
              </span>
              <button
                className="ghost danger"
                disabled={running}
                onClick={() => {
                  void api.calReset();
                  toast("已重置，下次开始前会重新校准");
                }}
              >
                重置
              </button>
            </div>
          </>
        )}
      </section>

      <section className="grp" aria-label="关于">
        <div className="grp-h">关于</div>
        <button className="row clickable" onClick={() => void openUrl(REPO)}>
          <span className="lbl">
            AutoMoyu {info?.version}
            <small>GitHub</small>
          </span>
          <ChevronRight size={14} className="muted" />
        </button>
        <button
          className="row clickable"
          onClick={() =>
            void openUrl(
              `${REPO}/issues/new?title=${encodeURIComponent("[反馈] ")}&body=${encodeURIComponent(
                `版本：${info?.version}\n模式：${s.mode}\n窗口：${status?.window ? `${status.window.w}x${status.window.h}` : "未找到"}\n\n遇到的问题：\n\n（可以在 设置 → 高级 → 导出诊断包，把 zip 拖进来）`,
              )}`,
            )
          }
        >
          <span className="lbl">
            反馈问题<small>会预填版本和窗口信息</small>
          </span>
          <ChevronRight size={14} className="muted" />
        </button>
      </section>
      <p className="fine">
        AutoMoyu 只在游戏窗口里模拟鼠标右键，不读写游戏内存、不修改游戏文件。多人服务器请遵守服务器规则。
      </p>
      {picker && <WindowPicker onClose={() => setPicker(false)} />}
    </>
  );
}
