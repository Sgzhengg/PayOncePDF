/**
 * 试用 → 缴费 → 买断 流程的常量与纯判定（PRD F8、官网 site-cloudflare/README.md）。
 *
 * 两条入口装的是同一个安装包，安装后都先进入 7 天全功能试用：
 *  - 下载入口：未付款。试用到期 → 软件内弹缴费窗口，点击缴费 → Gumroad 一次买断
 *    → 输入激活码 → 永久使用。
 *  - 购买入口：先在 Gumroad 付款拿到激活码。试用到期 → 已激活状态自动转入
 *    买断永久模式，不弹缴费窗口。
 *
 * 因此判定只取决于授权状态：只有 expired（未激活且不可导出）才弹缴费窗口，
 * active（已买断）永远不弹。这里只放纯逻辑，便于 node --test 直接验证。
 */
import type { LicenseState } from "./types";

/**
 * 缴费地址。Gumroad 商品创建后替换为真实结账直链；
 * 当前指向价格页，不伪造无效的 gumroad.com/l/ 链接。
 */
export const PAYMENT_URL = "https://payoncepdf.com/pricing";

/** 启动或导出被拦时，是否需要弹出缴费窗口。 */
export function shouldShowPaywall(state: LicenseState | null): boolean {
  return state !== null && state.status === "expired" && !state.canExport;
}

/** 缴费窗口与授权面板共用的状态标题。 */
export function licenseHeadline(state: LicenseState | null): string {
  if (!state) return "正在读取授权状态…";
  if (state.status === "active") return "买断授权 · 永久使用";
  if (state.status === "expired") return "7 天试用已结束 · 文件仍在";
  return `7 天试用中 · 剩余 ${state.trialDaysLeft} 天`;
}

/** 打开缴费页：Tauri 用 opener 插件，浏览器预览退回新标签页。 */
export async function openPaymentPage(): Promise<void> {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(PAYMENT_URL);
  } catch {
    if (typeof window !== "undefined") {
      window.open(PAYMENT_URL, "_blank", "noopener,noreferrer");
    }
  }
}
