import { useState } from "react";
import { IS_TAURI, openOutputFolder, runSelfCheck } from "../lib/backend";
import type { SelfCheckReport } from "../lib/selfcheck";
import {
  caseMark,
  formatSelfCheckText,
  groupSelfCheckCases,
  selfCheckReportDir,
  summarizeSelfCheck,
  verdictText,
} from "../lib/selfcheck";
import { Dialog } from "./Dialog";

interface SelfCheckDialogProps {
  onClose: () => void;
}

/**
 * F21 自检面板（参考 Grabber 设置里的「运行自检」）：
 * 一键跑完本地引擎能自证的项目，失败逐条可定位，报告可复制 / 可打开所在目录；
 * 自检覆盖不到的项目单独列成「仍需真机验证」，不把跳过伪装成通过。
 */
export function SelfCheckDialog({ onClose }: SelfCheckDialogProps) {
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<SelfCheckReport | null>(null);
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const [showRaw, setShowRaw] = useState(false);

  const run = async () => {
    setBusy(true);
    setError("");
    setNote("");
    setReport(null);
    setShowRaw(false);
    try {
      const r = await runSelfCheck();
      setReport(r);
      if (r.write_error) setError(`报告文件未写出：${r.write_error}（结果仍可复制）`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const copy = async () => {
    if (!report) return;
    const text = formatSelfCheckText(report);
    try {
      if (!navigator.clipboard) throw new Error("clipboard unavailable");
      await navigator.clipboard.writeText(text);
      setNote("已复制全部结果，可直接发给支持定位");
      setError("");
    } catch {
      setShowRaw(true);
      setError("自动复制失败，已展开报告文本，请手动全选复制");
    }
  };

  const openDir = async () => {
    if (!report) return;
    const dir = selfCheckReportDir(report.path ?? "");
    if (!dir) {
      setError("本次报告未写出文件，无法打开目录；请点「复制全部结果」");
      return;
    }
    const msg = await openOutputFolder(dir);
    if (msg) setError(msg);
    else setNote("已打开报告所在文件夹");
  };

  const summary = report ? summarizeSelfCheck(report) : null;
  const groups = report ? groupSelfCheckCases(report.cases) : [];

  return (
    <Dialog title="运行自检" onClose={onClose}>
      <p className="dialog-lead">
        一次跑完不依赖人眼的检查：环境依赖、纯逻辑、真实引擎处理（用内置样本 PDF，不碰你的文件、不联网、不占导出授权）。
        结果自动写到下载目录的 <code>selftest-*.json</code>。
      </p>

      {!IS_TAURI ? (
        <p className="error-line" role="alert">
          浏览器预览模式不调用本地引擎，自检不可用；请在桌面版 PayOnce PDF 中运行。
        </p>
      ) : null}

      <div className="dialog-actions">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void run()}
          disabled={busy || !IS_TAURI}
        >
          {busy ? "自检运行中…" : report ? "重新运行自检" : "运行自检"}
        </button>
        {report ? (
          <>
            <button type="button" className="btn" onClick={() => void copy()}>
              复制全部结果
            </button>
            <button type="button" className="btn" onClick={() => void openDir()} disabled={!report.path}>
              打开报告文件夹
            </button>
            <button type="button" className="btn btn-ghost" onClick={() => setShowRaw((v) => !v)}>
              {showRaw ? "收起报告文本" : "查看报告文本"}
            </button>
          </>
        ) : null}
      </div>

      {busy ? (
        <p className="inline-note" role="status">
          正在真实调用 qpdf / lopdf / PDFium 处理内置样本，通常几秒内完成。
        </p>
      ) : null}
      {error ? <p className="error-line" role="alert">{error}</p> : null}
      {note ? <p className="inline-note" role="status">{note}</p> : null}

      {report && summary ? (
        <>
          <div className={`selfcheck-summary selfcheck-${summary.verdict}`}>
            <span className="selfcheck-chip">
              <b>{summary.passed}/{summary.total}</b> 通过
            </span>
            <span className={`selfcheck-chip ${summary.failed ? "is-bad" : ""}`}>
              失败 {summary.failed}
            </span>
            <span className={`selfcheck-chip ${summary.skipped ? "is-skip" : ""}`}>
              跳过 {summary.skipped}
            </span>
            <span className="selfcheck-chip">耗时 {report.elapsed_ms}ms</span>
            <span className="selfcheck-verdict">{verdictText(summary.verdict)}</span>
          </div>

          <h3 className="dialog-section-title">逐项结果</h3>
          <div className="selfcheck-list">
            {groups.map((g) => (
              <div className="selfcheck-group" key={g.group}>
                <div className="selfcheck-group-title">{g.group}</div>
                <ul>
                  {g.cases.map((c) => (
                    <li
                      key={c.case}
                      className={!c.ok ? "is-fail" : c.skipped ? "is-skip" : "is-pass"}
                    >
                      <span className="selfcheck-mark" aria-hidden="true">{caseMark(c)}</span>
                      <code>{c.case}</code>
                      <span className="selfcheck-feature">{c.feature}</span>
                      <span className="selfcheck-ms">{c.ms}ms</span>
                      <div className="selfcheck-detail">{c.detail}</div>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>

          <div className="selfcheck-coverage">
            <b>已被自检覆盖</b>
            <p>{report.coverage?.self_checked?.join("、") || "（无）"}</p>
          </div>

          <details className="selfcheck-manual">
            <summary>
              仍需真机验证（{report.coverage?.needs_real_device?.length ?? 0} 项）—— 自检不能替代的部分
            </summary>
            <ul>
              {(report.coverage?.needs_real_device ?? []).map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </details>

          {showRaw ? (
            <textarea
              className="selfcheck-raw"
              readOnly
              rows={12}
              aria-label="自检报告纯文本"
              value={formatSelfCheckText(report)}
            />
          ) : null}
        </>
      ) : null}
    </Dialog>
  );
}
