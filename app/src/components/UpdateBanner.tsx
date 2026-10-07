// 首页顶部的更新横幅。钓鱼进行中不出现。

import { ArrowDownCircle } from "lucide-react";

import { api } from "../lib/api";
import { t } from "../lib/i18n";
import { useStore } from "../lib/store";
import { download } from "../lib/updater";

/** 发布说明是 CHANGELOG 的一节：中文在前，"**English**" 之后是英文，"---" 之后是安装说明。只留当前语言的正文。 */
function pickNotes(notes: string): string {
  const [body] = notes.split(/^---\s*$/m);
  const [zh, en] = body.split(/^\*\*English\*\*\s*$/m);
  return t(zh, en ?? zh)
    .split("\n")
    .filter((l) => !l.startsWith("#"))
    .join("\n")
    .replace(/\*\*/g, "")
    .trim();
}

export function UpdateBanner() {
  const update = useStore((s) => s.update);
  const session = useStore((s) => s.status?.session);
  const patch = useStore((s) => s.patchSettings);
  const setUpdate = useStore((s) => s.setUpdate);
  if (session) return null;
  if (update.state !== "available" && update.state !== "ready" && update.state !== "downloading") return null;
  const { info } = update;
  const skip = () => {
    void patch({ update: { skipVersion: info.version } });
    setUpdate({ state: "none" });
  };
  return (
    <div className="upd" role="status">
      <div className="h">
        <ArrowDownCircle size={18} />
        <div>
          <b>
            {update.state === "ready"
              ? t(`新版本 ${info.version} 已下载`, `Version ${info.version} downloaded`)
              : t(`发现新版本 ${info.version}`, `Version ${info.version} available`)}
          </b>
          {info.notes && <span className="notes">{pickNotes(info.notes)}</span>}
        </div>
      </div>
      {update.state === "downloading" && (
        <div className="bar" aria-label={t("下载进度", "Download progress")}>
          <i style={{ width: `${Math.round((update.progress ?? 0.05) * 100)}%` }} />
        </div>
      )}
      <div className="acts">
        <button className="link" style={{ marginRight: "auto", fontSize: 12 }} onClick={skip}>
          {t("跳过此版本", "Skip this version")}
        </button>
        <button className="ghost" onClick={() => setUpdate({ state: "none" })}>
          {t("稍后", "Later")}
        </button>
        {update.state === "available" && (
          <button className="primary sm" onClick={() => void download()}>
            {t("下载", "Download")}
          </button>
        )}
        {update.state === "ready" && (
          <button className="primary sm" onClick={() => void api.updateInstall()}>
            {t("重启并更新", "Restart & update")}
          </button>
        )}
      </div>
    </div>
  );
}
