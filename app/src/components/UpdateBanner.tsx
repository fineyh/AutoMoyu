// 首页顶部的更新横幅。钓鱼进行中不出现。

import { ArrowDownCircle } from "lucide-react";

import { api } from "../lib/api";
import { useStore } from "../lib/store";
import { download } from "../lib/updater";

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
            {update.state === "ready" ? `新版本 ${info.version} 已下载` : `发现新版本 ${info.version}`}
          </b>
          {info.notes && <span className="notes">{info.notes}</span>}
        </div>
      </div>
      {update.state === "downloading" && (
        <div className="bar" aria-label="下载进度">
          <i style={{ width: `${Math.round((update.progress ?? 0.05) * 100)}%` }} />
        </div>
      )}
      <div className="acts">
        <button className="link" style={{ marginRight: "auto", fontSize: 12 }} onClick={skip}>
          跳过此版本
        </button>
        <button className="ghost" onClick={() => setUpdate({ state: "none" })}>
          稍后
        </button>
        {update.state === "available" && (
          <button className="primary sm" onClick={() => void download()}>
            下载
          </button>
        )}
        {update.state === "ready" && (
          <button className="primary sm" onClick={() => void api.updateInstall()}>
            重启并更新
          </button>
        )}
      </div>
    </div>
  );
}
