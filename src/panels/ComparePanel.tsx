// F20 文档比对面板：只读查询在面板内展示结构化差异；导出报告走任务队列。
// 边界写在 hint 与页脚：只比文字层，图片/版式/页序不在范围，扫描件需先 OCR。

import { useState } from "react";
import { CheckboxField, PathField, TextField } from "../components/Field";
import {
  compareDocuments,
  exportCompareReport,
  IS_TAURI,
  PDF_FILTERS,
  pickFiles,
  pickSave,
} from "../lib/backend";
import type { CompareParams } from "../lib/params";
import type { CompareReport } from "../lib/types";

interface PanelProps {
  value: CompareParams;
  onChange: (patch: Partial<CompareParams>) => void;
  filePaths?: string[];
}

const TXT_FILTERS = [{ name: "文本文件", extensions: ["txt"] }];

const pickPdf = async (): Promise<string | null> => {
  const picked = await pickFiles({ filters: PDF_FILTERS, title: "选择 PDF 文件" });
  return picked[0] ?? null;
};

export function ComparePanel({ value, onChange, filePaths = [] }: PanelProps) {
  const [report, setReport] = useState<CompareReport | null>(null);
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);

  const useListFiles = () => {
    if (filePaths.length >= 2) {
      onChange({ oldFile: filePaths[0], newFile: filePaths[1] });
      setMsg("已把文件列表前两项填为旧版 / 新版");
    } else if (filePaths.length === 1) {
      onChange({ oldFile: filePaths[0] });
      setMsg("文件列表只有一项：已填为旧版，请再选新版");
    } else {
      setMsg("请先在中间区域添加两个 PDF（旧版在前、新版在后）");
    }
  };

  const runCompare = async () => {
    setReport(null);
    if (!value.oldFile || !value.newFile) {
      setMsg("请选择旧文件与新文件");
      return;
    }
    if (!IS_TAURI) {
      setMsg("预览模式不调用本地引擎，无法真正比对；请在桌面版打开两个 PDF 后使用。");
      return;
    }
    setBusy(true);
    setMsg("正在比对文字层…");
    try {
      const r = await compareDocuments({
        oldFile: value.oldFile,
        newFile: value.newFile,
        ...(value.oldPassword ? { oldPassword: value.oldPassword } : {}),
        ...(value.newPassword ? { newPassword: value.newPassword } : {}),
        ...(value.pages.trim() ? { pages: value.pages.trim() } : {}),
        sensitivity: {
          ignoreCase: value.ignoreCase,
          ignoreWhitespace: value.ignoreWhitespace,
          ignorePunctuation: value.ignorePunctuation,
        },
      });
      if (!r) {
        setMsg("预览模式未返回比对结果");
        return;
      }
      setReport(r);
      const total = r.added + r.removed + r.changed;
      setMsg(
        total
          ? `发现 ${total} 处差异（新增 ${r.added} / 删除 ${r.removed} / 修改 ${r.changed}）`
          : "文字层未发现差异（图片与版式变化不在范围内）",
      );
    } catch (e) {
      setMsg(`比对失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const doExport = async () => {
    if (!report) {
      setMsg("请先执行一次比对，再导出报告");
      return;
    }
    if (!IS_TAURI) {
      setMsg("预览模式不写文件；请在桌面版导出报告");
      return;
    }
    setBusy(true);
    try {
      const result = await exportCompareReport({
        jobId: crypto.randomUUID(),
        query: {
          oldFile: value.oldFile,
          newFile: value.newFile,
          ...(value.oldPassword ? { oldPassword: value.oldPassword } : {}),
          ...(value.newPassword ? { newPassword: value.newPassword } : {}),
          ...(value.pages.trim() ? { pages: value.pages.trim() } : {}),
          sensitivity: {
            ignoreCase: value.ignoreCase,
            ignoreWhitespace: value.ignoreWhitespace,
            ignorePunctuation: value.ignorePunctuation,
          },
        },
        output: value.output,
      });
      setMsg(result.ok ? `报告已导出：${result.outputs[0] || value.output}` : `导出失败：${result.message}`);
    } catch (e) {
      setMsg(`导出失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel-form">
      <p className="inline-note">
        比对两个 PDF 的<strong>文字层</strong>差异，按页列出新增 / 删除 / 修改。只读，不改动原文件。
      </p>
      <div className="field-row">
        <button type="button" className="btn-secondary" onClick={useListFiles}>
          用文件列表前两项
        </button>
        <span className="field-hint">旧版在前、新版在后</span>
      </div>
      <PathField
        label="旧版文件"
        value={value.oldFile}
        onChange={(v) => onChange({ oldFile: v })}
        onPick={async () => {
          const p = await pickPdf();
          if (p) onChange({ oldFile: p });
          return p;
        }}
        placeholder="留空可用文件列表第一项"
      />
      <PathField
        label="新版文件"
        value={value.newFile}
        onChange={(v) => onChange({ newFile: v })}
        onPick={async () => {
          const p = await pickPdf();
          if (p) onChange({ newFile: p });
          return p;
        }}
        placeholder="留空可用文件列表第二项"
      />
      <TextField
        label="页码范围"
        value={value.pages}
        onChange={(v) => onChange({ pages: v })}
        placeholder="留空 = 全部页，如 1-3,5"
        hint="长文档可先框定范围；两版页数不同时，多出的页按整页新增/删除列出。"
      />
      <CheckboxField
        label="忽略大小写"
        checked={value.ignoreCase}
        onChange={(v) => onChange({ ignoreCase: v })}
      />
      <CheckboxField
        label="忽略空白差异"
        checked={value.ignoreWhitespace}
        onChange={(v) => onChange({ ignoreWhitespace: v })}
        hint="默认开启：换行 / 空格的排版变化不算内容变化"
      />
      <CheckboxField
        label="忽略标点差异"
        checked={value.ignorePunctuation}
        onChange={(v) => onChange({ ignorePunctuation: v })}
        hint="合同里一个标点可能改变含义，默认关闭（算作差异）"
      />
      <div className="field-row">
        <button type="button" className="btn-primary" onClick={runCompare} disabled={busy}>
          {busy ? "比对中…" : "开始比对"}
        </button>
        <span className="field-hint">{msg}</span>
      </div>

      {report ? (
        <div className="compare-result">
          <div className="compare-summary">
            <span className="cmp-added">新增 {report.added}</span>
            <span className="cmp-removed">删除 {report.removed}</span>
            <span className="cmp-changed">修改 {report.changed}</span>
            <span className="field-hint">
              旧 {report.oldPages} 页 / 新 {report.newPages} 页 / 比对 {report.comparedPages} 页
            </span>
          </div>
          {report.warnings.map((w, i) => (
            <p key={`w-${i}`} className="inline-note cmp-warning">
              {w}
            </p>
          ))}
          {report.differences.length ? (
            <ul className="compare-diff-list">
              {report.differences.map((d, i) => (
                <li key={`${d.page}-${i}`} className={`compare-diff cmp-${d.kind}`}>
                  <div className="cmp-diff-head">
                    第 {d.page} 页 · {d.kind === "added" ? "新增" : d.kind === "removed" ? "删除" : "修改"}
                  </div>
                  {d.kind !== "added" ? (
                    <div className="cmp-old">旧：{d.oldText}</div>
                  ) : null}
                  {d.kind !== "removed" ? (
                    <div className="cmp-new">新：{d.newText}</div>
                  ) : null}
                </li>
              ))}
            </ul>
          ) : (
            <p className="inline-note">未发现文字层差异。</p>
          )}
          <div className="field-row" style={{ marginTop: 8 }}>
            <button type="button" className="btn-secondary" onClick={doExport} disabled={busy}>
              导出报告
            </button>
          </div>
          <PathField
            label="报告输出"
            value={value.output}
            onChange={(v) => onChange({ output: v })}
            onPick={async () => {
              const p = await pickSave({ defaultPath: value.output, filters: TXT_FILTERS, title: "报告另存为" });
              if (p) onChange({ output: p });
              return p;
            }}
            hint="纯文本报告，含差异清单与边界说明"
          />
        </div>
      ) : null}

      <p className="inline-note" style={{ marginTop: 10 }}>
        <strong>边界</strong>：只比文字层文本，<strong>不识别图片、版式、页序、字体属性差异</strong>；扫描件请先走 OCR；
        旧新文件不要放反；结果需人工复核，尤其是合同、财务、法律文件。
      </p>
    </div>
  );
}
