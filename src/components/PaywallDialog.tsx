import { useState } from "react";
import { Dialog } from "./Dialog";
import { PAYMENT_URL, licenseHeadline, openPaymentPage } from "../lib/commerce";
import type { LicenseState } from "../lib/types";

interface PaywallDialogProps {
  state: LicenseState | null;
  /** 切到授权面板，让已购买用户输入激活码。 */
  onOpenLicense: () => void;
  onClose: () => void;
}

/**
 * 7 天试用到期时的缴费窗口。
 * 只在「未激活且已过期」时出现；已买断用户不会看到本窗口，到期直接永久使用。
 * 文件不会被删除或改写：Rust 侧 ensure_export_allowed() 只拦截导出。
 */
export function PaywallDialog({ state, onOpenLicense, onClose }: PaywallDialogProps) {
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const pay = async () => {
    setError("");
    setBusy(true);
    try {
      await openPaymentPage();
    } catch {
      setError(`无法打开缴费页面，请手动访问 ${PAYMENT_URL}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog title="7 天试用已结束" onClose={onClose}>
      <p style={{ fontSize: 13, margin: "4px 0 10px" }}>{licenseHeadline(state)}</p>
      <p className="inline-note">
        试用期内处理过的文件都还在本机，不会被删除或改写；只有导出被暂停。点击缴费完成
        $39 / ¥199 一次买断，拿到激活码后即可继续导出，永久使用当前大版本，无订阅、无自动续费。
      </p>
      <p className="inline-note">
        已经在 Gumroad 购买过？输入激活码即可：激活后到期会自动转入买断永久模式，不会再弹出本窗口。
      </p>
      <div className="license-form">
        <button
          type="button"
          className="btn btn-primary"
          onClick={pay}
          disabled={busy}
        >
          点击缴费 · $39 一次买断 →
        </button>
        <button type="button" className="btn btn-ghost" onClick={onOpenLicense}>
          已有激活码？输入激活
        </button>
      </div>
      {error ? (
        <p className="error-line" role="alert">
          {error}
        </p>
      ) : null}
      <p className="inline-note">
        缴费地址：{PAYMENT_URL}（Gumroad 商品上线后替换为直链）
      </p>
    </Dialog>
  );
}
