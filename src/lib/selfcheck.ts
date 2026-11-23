/**
 * 应用内功能自检（F21）的 DTO 与纯展示逻辑。
 *
 * 与 Rust `src-tauri/src/commands/selfcheck.rs` 的报告 JSON 镜像；
 * 参考 Grabber `SelftestCase = { case, ok, ms, detail }`，另加分组、功能编号与
 * 「跳过（可选组件缺失）」状态。这里的函数不碰 Tauri / DOM，可直接被 Node 单测导入。
 */

export interface SelfCheckCase {
  case: string;
  group: string;
  /** 对应的功能编号，用于汇总「已被自检覆盖的功能」。 */
  feature: string;
  ok: boolean;
  skipped: boolean;
  optional: boolean;
  ms: number;
  detail: string;
}

export interface SelfCheckReport {
  schema: string;
  started_at: number;
  elapsed_ms: number;
  env: Record<string, string>;
  summary: { total: number; passed: number; failed: number; skipped: number };
  coverage: { self_checked: string[]; needs_real_device: string[] };
  cases: SelfCheckCase[];
  path?: string;
  write_error?: string;
}

export type SelfCheckVerdict = "pass" | "warn" | "fail";

export interface SelfCheckSummary {
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  verdict: SelfCheckVerdict;
}

/** 从 cases 重算汇总，不直接信任后端 summary：报告本身也要能自证统计没算错。 */
export function summarizeSelfCheck(report: SelfCheckReport): SelfCheckSummary {
  const cases = Array.isArray(report?.cases) ? report.cases : [];
  const total = cases.length;
  const failed = cases.filter((c) => !c.ok).length;
  const skipped = cases.filter((c) => c.ok && c.skipped).length;
  const passed = total - failed - skipped;
  const verdict: SelfCheckVerdict = failed > 0 ? "fail" : skipped > 0 ? "warn" : "pass";
  return { total, passed, failed, skipped, verdict };
}

export function caseMark(c: SelfCheckCase): string {
  if (!c.ok) return "✗";
  return c.skipped ? "—" : "✓";
}

export function verdictText(v: SelfCheckVerdict): string {
  if (v === "fail") return "存在失败项，需按下方明细排查";
  if (v === "warn") return "必需项全部通过，可选组件有跳过";
  return "必需项全部通过";
}

export interface SelfCheckGroup {
  group: string;
  cases: SelfCheckCase[];
}

/** 按报告中的顺序分组（不排序，保留环境 → 逻辑 → 引擎 → 可选的诊断顺序）。 */
export function groupSelfCheckCases(cases: SelfCheckCase[]): SelfCheckGroup[] {
  const out: SelfCheckGroup[] = [];
  for (const c of Array.isArray(cases) ? cases : []) {
    const name = c.group || "其他";
    let g = out.find((x) => x.group === name);
    if (!g) {
      g = { group: name, cases: [] };
      out.push(g);
    }
    g.cases.push(c);
  }
  return out;
}

/** Windows / macOS 路径都能取到报告所在目录，供「打开报告文件夹」。 */
export function selfCheckReportDir(path: string): string {
  if (!path) return "";
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return i > 0 ? path.slice(0, i) : path;
}

/** 一键复制用的纯文本：环境、逐条结果、覆盖清单、真机剩余清单都要带上。 */
export function formatSelfCheckText(report: SelfCheckReport): string {
  const s = summarizeSelfCheck(report);
  const env = Object.entries(report?.env ?? {})
    .map(([k, v]) => `${k}=${v}`)
    .join(" · ");
  const lines: string[] = [
    `PayOnce PDF 自检报告（${report?.schema ?? "unknown"}）`,
    `时间：${new Date(report?.started_at ?? Date.now()).toLocaleString()} · 耗时 ${report?.elapsed_ms ?? 0}ms`,
    `结果：${s.passed}/${s.total} 通过，${s.failed} 失败，${s.skipped} 跳过（${verdictText(s.verdict)}）`,
    `环境：${env}`,
    report?.path ? `报告文件：${report.path}` : "报告文件：未写出（见界面提示）",
    "",
  ];
  for (const g of groupSelfCheckCases(report?.cases ?? [])) {
    lines.push(`【${g.group}】`);
    for (const c of g.cases) {
      lines.push(`${caseMark(c)} ${c.case} · ${c.feature} · ${c.ms}ms · ${c.detail}`);
    }
    lines.push("");
  }
  lines.push(`已自检覆盖：${(report?.coverage?.self_checked ?? []).join("、") || "（无）"}`);
  lines.push("仍需真机验证：");
  const rest = report?.coverage?.needs_real_device ?? [];
  if (rest.length === 0) lines.push("  （无）");
  for (const item of rest) lines.push(`  - ${item}`);
  return lines.join("\n");
}
