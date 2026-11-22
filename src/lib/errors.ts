import type { AppError } from "./types";

// PRD §13 + CONTRACT §2（E2xxx 扩展码）。code → 中文用户提示。
export const ERROR_HINTS: Record<string, string> = {
  E1001: "文件无法读取，已跳过",
  E1002: "该文件有密码，请输入",
  E1003: "密码不正确",
  E1004: "请清理磁盘后重试",
  E1005: "单文件超过 5000 页，已拒绝",
  E1006: "请安装中文语言包",
  E1007: "目标文件正被其他程序使用",
  E2001: "组件缺失，请重新安装或运行自检",
  E2002: "渲染组件缺失，请重新安装",
  E2003: "已取消，原文件未改动",
  E2004: "操作失败，原文件未改动",
  E2005: "试用已结束；文件仍在，激活后可导出",
};

const INTERNAL_FALLBACK = "操作失败，原文件未改动";

export function hintForCode(code: string): string {
  return ERROR_HINTS[code] ?? INTERNAL_FALLBACK;
}

// 把 Tauri / mock 抛出的任意错误规范化为 AppError。
// Rust 侧 Err(AppError) 序列化为 { code, message }；Tauri 也可能抛出字符串或 Error。
export function normalizeError(err: unknown): AppError {
  if (err && typeof err === "object") {
    const o = err as Record<string, unknown>;
    if (typeof o.code === "string") {
      return {
        code: o.code,
        message: typeof o.message === "string" ? o.message : "",
      };
    }
    if (typeof o.message === "string") {
      return { code: "E2004", message: o.message };
    }
  }
  if (typeof err === "string") {
    try {
      const parsed: unknown = JSON.parse(err);
      if (parsed && typeof parsed === "object") {
        const o = parsed as Record<string, unknown>;
        if (typeof o.code === "string") {
          return {
            code: o.code,
            message: typeof o.message === "string" ? o.message : "",
          };
        }
      }
    } catch {
      // 非 JSON 字符串，按内部错误处理。
    }
    return { code: "E2004", message: err };
  }
  return { code: "E2004", message: INTERNAL_FALLBACK };
}

// UI 直接显示：按 code 取对应中文提示。
export function describeError(err: unknown): string {
  return hintForCode(normalizeError(err).code);
}

/**
 * 任务失败后状态栏展示的错误详情（P0-2，2026-11-22）。
 *
 * 后端 AppError 序列化为 { code, message }，message 里写了很有价值的中文业务原因
 * （如「选区里有大小不同的字」「文字层顺序与内容流不一致」），旧代码却只用 code
 * 取通用提示，把这些原因全丢了，用户只能反复试错。
 *
 * 策略（避免把内部实现/堆栈泄漏给用户）：
 * - 空 message / 多行堆栈 / Rust panic / JS 栈帧 / 超长 dump → 回退到稳定 code 文案；
 * - 纯英文技术串（如 "IO error: ..."）→ 同样回退，不暴露内部错误串；
 * - 含中文的业务原因 → 直接展示；若没自带「未改动」语义，补一句让用户安心。
 *
 * 不改变任何错误码语义，也不影响 describeError（许可激活场景）的既有行为。
 */
export function describeErrorDetail(err: unknown): string {
  const appErr = normalizeError(err);
  const hint = hintForCode(appErr.code);
  const detail = appErr.message.trim();
  if (!detail) return hint;
  if (
    /[\r\n]/.test(detail) ||
    detail.length > 240 ||
    /\bat\s+\S+\s*\([^)]*:\d+:\d+\)/.test(detail) ||
    /panicked at/i.test(detail)
  ) {
    return hint;
  }
  // 纯英文/技术细节：不向用户暴露（IO error、serde 错误串等）。
  if (!/[\u3400-\u9fff]/.test(detail)) return hint;
  // 中文业务原因：多数自带「已拒绝且未写出文件」口径，无需重复；否则补安全保证。
  return /未改动|未写出|未改变|未修改|已拒绝/.test(detail)
    ? detail
    : `${detail}（原文件未改动）`;
}
