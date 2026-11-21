import { useState } from "react";
import type { AppInfo, UpdateCheckResult, UpdateSettings } from "../lib/types";
import { checkUpdate, installUpdate } from "../lib/backend";
import { CheckboxField } from "./Field";
import { Dialog } from "./Dialog";

interface AboutDialogProps {
  info: AppInfo | null;
  logging: boolean;
  onLoggingChange: (value: boolean) => void;
  updateSettings: UpdateSettings;
  onUpdateEnabledChange: (value: boolean) => void;
  onClose: () => void;
}

export function AboutDialog({
  info,
  logging,
  onLoggingChange,
  updateSettings,
  onUpdateEnabledChange,
  onClose,
}: AboutDialogProps) {
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<UpdateCheckResult | null>(null);
  const [installNote, setInstallNote] = useState<string | null>(null);

  const handleCheck = async () => {
    setChecking(true);
    setInstallNote(null);
    try {
      setResult(await checkUpdate());
    } finally {
      setChecking(false);
    }
  };

  const handleInstall = async () => {
    setChecking(true);
    try {
      setInstallNote(await installUpdate());
    } finally {
      setChecking(false);
    }
  };

  return (
    <Dialog title="关于 PayOnce PDF" onClose={onClose}>
      <p className="dialog-lead">一次买断，打开即用，文件只在这台电脑上处理。</p>

      <dl className="info-list">
        <div>
          <dt>版本</dt>
          <dd>{info?.version ?? "读取中…"}</dd>
        </div>
        <div>
          <dt>平台</dt>
          <dd>{info ? `${info.platform} / ${info.arch}` : "读取中…"}</dd>
        </div>
        <div>
          <dt>离线可用</dt>
          <dd>{info?.offlineCapable ? "是" : "读取中…"}</dd>
        </div>
        <div>
          <dt>数据收集</dt>
          <dd>无</dd>
        </div>
      </dl>

      <h3 className="dialog-section-title">组件自检</h3>
      {info && info.sidecars.length > 0 ? (
        <table className="info-table">
          <thead>
            <tr>
              <th scope="col">组件</th>
              <th scope="col">架构</th>
              <th scope="col">状态</th>
            </tr>
          </thead>
          <tbody>
            {info.sidecars.map((s) => (
              <tr key={s.name}>
                <td>{s.name}</td>
                <td>{s.arch}</td>
                <td>{s.ready ? "就绪" : "缺失"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="inline-note">自检信息读取中…</p>
      )}

      <h3 className="dialog-section-title">更新（F9）</h3>
      <div className="dialog-settings">
        <CheckboxField
          label="允许检查更新（默认开启；关闭后不会发起任何更新请求）"
          checked={updateSettings.enabled}
          onChange={onUpdateEnabledChange}
        />
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={handleCheck} disabled={checking}>
            {checking ? "处理中…" : "检查更新"}
          </button>
          {result?.available ? (
            <button type="button" className="btn btn-primary" onClick={handleInstall} disabled={checking}>
              下载并安装 {result.version}
            </button>
          ) : null}
        </div>
        {result ? (
          <p className="inline-note" role="status">
            {result.error
              ? result.error
              : result.available
                ? `发现新版本 ${result.version}${result.notes ? `：${result.notes}` : ""}`
                : "当前已是最新版本"}
          </p>
        ) : null}
        {installNote ? (
          <p className="inline-note" role="status">{installNote}</p>
        ) : null}
        <p className="inline-note">
          更新包需通过 Tauri Updater 的签名校验；失败时保留旧版本，可回滚。除手动点击外不会联网。
        </p>
      </div>

      <div className="dialog-settings">
        <CheckboxField
          label="启用本地日志（默认关闭，仅记录操作类型与错误码）"
          checked={logging}
          onChange={onLoggingChange}
        />
      </div>
    </Dialog>
  );
}
