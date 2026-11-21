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
