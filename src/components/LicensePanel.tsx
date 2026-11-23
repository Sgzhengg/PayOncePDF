import { useState } from "react";
import type { LicenseState } from "../lib/types";
import { describeError } from "../lib/errors";
import { openPaymentPage } from "../lib/commerce";
import { TextField } from "./Field";
import { Dialog } from "./Dialog";

interface LicensePanelProps {
  state: LicenseState | null;
  onActivate: (key: string) => Promise<void>;
  onClose: () => void;
}

function statusText(state: LicenseState | null): string {
  if (!state) return "读取中…";
  if (state.status === "trial") return `试用中，剩余 ${state.trialDaysLeft} 天`;
  if (state.status === "active") return "已激活（买断）· 永久使用";
  return "试用已结束";
}

export function LicensePanel({ state, onActivate, onClose }: LicensePanelProps) {
  const [key, setKey] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setError("");
    setBusy(true);
    try {
      await onActivate(key.trim());
      setKey("");
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog title="授权" onClose={onClose}>
      <dl className="info-list">
        <div>
          <dt>当前状态</dt>
          <dd>{statusText(state)}</dd>
        </div>
        <div>
          <dt>可导出</dt>
          <dd>{state?.canExport ? "是" : "否"}</dd>
        </div>
        <div>
          <dt>密钥提示</dt>
          <dd>{state?.keyHint ?? "未激活"}</dd>
        </div>
        {state?.activatedAt ? (
          <div>
            <dt>激活时间</dt>
            <dd>{new Date(state.activatedAt).toLocaleString()}</dd>
          </div>
        ) : null}
      </dl>

      <p className="inline-note">
        试用到期后只限制导出，不会删除或改写任何文件；点击缴费一次买断并输入激活码后即可永久使用。已激活的用户到期自动转入买断模式，不会再弹缴费窗口。
      </p>

      {state?.status !== "active" ? (
        <div className="license-form">
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void openPaymentPage()}
          >
            点击缴费 · $39 一次买断 →
          </button>
        </div>
      ) : null}

      <div className="license-form">
        <TextField
          label="买断密钥"
          value={key}
          onChange={setKey}
          placeholder="PAYONCE-XXXX-XXXX-XXXX"
          hint="格式：PAYONCE-XXXX-XXXX-XXXX"
        />
        <button
          type="button"
          className="btn btn-primary"
          onClick={submit}
          disabled={busy || key.trim().length === 0}
        >
          激活
        </button>
      </div>
      {error ? (
        <p className="error-line" role="alert">
          {error}
        </p>
      ) : null}
    </Dialog>
  );
}
