// F15 / F16 参数面板。边界全部写在 hint 里：做不到的事明确告诉用户。
// 书签列表与 OCR 语言包是只读/主动触发的查询，不经过任务队列。

import { useEffect, useState } from "react";
import {
  CheckboxField,
  FieldRow,
  PathField,
  SelectField,
  TextField,
} from "../components/Field";
import {
  PDF_FILTERS,
  installOcrLanguage,
  listBookmarks,
  listOcrLanguages,
  pickSave,
} from "../lib/backend";
import type { CreateParams, ObjectsParams, ShareParams } from "../lib/params";
import type { BookmarkEntry, OcrLanguageInfo } from "../lib/types";

interface PanelProps<T> {
  value: T;
  onChange: (patch: Partial<T>) => void;
  fileCount: number;
  filePaths?: string[];
}

const pdfSave = (defaultPath: string, title: string) =>
  pickSave({ defaultPath, filters: PDF_FILTERS, title });

function TextArea({
  label,
  value,
  onChange,
  hint,
  rows = 5,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  hint?: string;
  rows?: number;
}) {
  return (
    <FieldRow label={label} hint={hint}>
      <textarea
        className="text-input"
        aria-label={label}
        rows={rows}
        value={value}
        style={{ minHeight: `${rows * 22}px`, resize: "vertical" }}
        onChange={(e) => onChange(e.target.value)}
      />
    </FieldRow>
  );
}

// ---------------------------------------------------------------------------
// F15 表单 / 脱敏 / 书签
// ---------------------------------------------------------------------------

const OBJECT_MODES: readonly { value: ObjectsParams["mode"]; label: string }[] = [
  { value: "form", label: "创建可填写表单" },
  { value: "redact", label: "密文脱敏" },
  { value: "bookmarks", label: "书签管理" },
];

const BOOKMARK_ACTIONS: readonly { value: ObjectsParams["action"]; label: string }[] = [
  { value: "set", label: "整体替换书签" },
  { value: "add", label: "追加书签" },
  { value: "rename", label: "重命名书签" },
  { value: "delete", label: "删除书签" },
];

export function ObjectsPanel({ value, onChange, fileCount, filePaths = [] }: PanelProps<ObjectsParams>) {
  const [bookmarks, setBookmarks] = useState<BookmarkEntry[]>([]);
  const [listMsg, setListMsg] = useState("");

  const refresh = async () => {
    const path = filePaths[0];
    if (!path) {
      setListMsg("请先在中间区域添加要查看的 PDF");
      return;
    }
    try {
      const items = await listBookmarks({ file: path, ...(value.password ? { password: value.password } : {}) });
      setBookmarks(items);
      setListMsg(items.length ? `读取到 ${items.length} 条顶层书签` : "该文档没有书签");
    } catch (e) {
      setListMsg(`读取失败：${String(e)}`);
    }
  };

  return (
    <div className="panel-form">
      <p className="inline-note">
        当前文件列表 {fileCount} 项，仅处理第一个 PDF。输出新文件，原文件不变。
      </p>
      <SelectField
        label="操作"
        value={value.mode}
        options={OBJECT_MODES}
        onChange={(v) => onChange({ mode: v })}
        hint={
          value.mode === "form"
            ? "创建 AcroForm 文本域；不做复选框/单选/下拉/签名域。"
            : value.mode === "redact"
              ? "先真删除文字对象再盖黑框，复制粘贴也取不到原文。"
              : "只做顶层书签；页面增删后书签不自动跟随页号。"
        }
      />

      {value.mode === "form" ? (
        <>
          <TextArea
            label="字段定义（每行一个）"
            value={value.fieldsSpec}
            onChange={(v) => onChange({ fieldsSpec: v })}
            rows={4}
            hint="格式：页码|名称|x|y|宽|高|默认值|多行(0/1)|必填(0/1)。坐标单位 pt，从左下原点算；默认值可留空。"
          />
        </>
      ) : null}

      {value.mode === "redact" ? (
        <>
          <TextArea
            label="要抹掉的词（每行一个）"
            value={value.terms}
            onChange={(v) => onChange({ terms: v })}
            rows={4}
            hint="也可用逗号分隔。未命中的词会跳过；扫描件需先走 OCR。"
          />
          <CheckboxField
            label="区分大小写"
            checked={value.matchCase}
            onChange={(v) => onChange({ matchCase: v })}
          />
          <TextField
            label="页码范围"
            value={value.pages}
            onChange={(v) => onChange({ pages: v })}
            placeholder="留空 = 全部页，如 1-3,5"
          />
        </>
      ) : null}

      {value.mode === "bookmarks" ? (
        <>
          <SelectField
            label="书签操作"
            value={value.action}
            options={BOOKMARK_ACTIONS}
            onChange={(v) => onChange({ action: v })}
          />
          {value.action === "set" || value.action === "add" ? (
            <TextArea
              label="书签定义（每行一个）"
              value={value.spec}
              onChange={(v) => onChange({ spec: v })}
              rows={5}
              hint="格式：页码 标题，如 `3 第三章 交付验收`。标题可含空格。中文标题用 UTF-16 写入。"
            />
          ) : null}
          {value.action === "rename" || value.action === "delete" ? (
            <TextField
              label="原标题"
              value={value.target}
              onChange={(v) => onChange({ target: v })}
              placeholder="要与现有书签标题完全一致"
            />
          ) : null}
          {value.action === "rename" ? (
            <TextField
              label="新标题"
              value={value.newTitle}
              onChange={(v) => onChange({ newTitle: v })}
            />
          ) : null}
          <div className="field-row">
            <button type="button" className="btn-secondary" onClick={refresh}>
              读取当前书签
            </button>
            <span className="field-hint">{listMsg}</span>
          </div>
          {bookmarks.length ? (
            <ul className="inline-note" style={{ marginTop: 4 }}>
              {bookmarks.map((b, i) => (
                <li key={`${b.title}-${i}`}>
                  第 {b.page} 页 · {b.title}
                </li>
              ))}
            </ul>
          ) : null}
        </>
      ) : null}

      <TextField
        label="加密文件密码"
        value={value.password}
        onChange={(v) => onChange({ password: v })}
        placeholder="留空表示无密码"
      />
      <PathField
        label="输出文件"
        value={value.output}
        onChange={(v) => onChange({ output: v })}
        onPick={() => pdfSave(value.output, "结果另存为")}
        hint="所有操作都另存为新 PDF，绝不改写输入文件。"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// F16 文本转 PDF
// ---------------------------------------------------------------------------

const CREATE_FORMATS: readonly { value: CreateParams["format"]; label: string }[] = [
  { value: "txt", label: "纯文本 TXT" },
  { value: "csv", label: "CSV 文本" },
  { value: "html", label: "HTML（剥离标签）" },
];

export function CreatePanel({ value, onChange, fileCount }: PanelProps<CreateParams>) {
  return (
    <div className="panel-form">
      <p className="inline-note">
        把 TXT / CSV / HTML 排成 A4 PDF，中文用系统字体嵌入。
        <strong>只做纯文本排版</strong>：不还原 Word/Excel 版式、表格样式与图片；
        HTML 只剥离标签，不渲染 CSS。当前文件列表 {fileCount} 项，仅处理第一个文件。
      </p>
      <SelectField
        label="源文件格式"
        value={value.format}
        options={CREATE_FORMATS}
        onChange={(v) => onChange({ format: v })}
        hint="请先通过中间区域或拖放添加对应的文本文件。"
      />
      <PathField
        label="输出 PDF"
        value={value.output}
        onChange={(v) => onChange({ output: v })}
        onPick={() => pdfSave(value.output, "新建 PDF 另存为")}
        hint="每页 45 行、11pt，自动分页；找不到系统 CJK 字体会明确报错而不是写方框。"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// F16 打印 / 邮件 / OCR 语言包
// ---------------------------------------------------------------------------

const SHARE_ACTIONS: readonly { value: ShareParams["action"]; label: string }[] = [
  { value: "print", label: "交给系统打印" },
  { value: "email", label: "唤起邮件客户端" },
];

export function SharePanel({ value, onChange, fileCount }: PanelProps<ShareParams>) {
  const [langs, setLangs] = useState<OcrLanguageInfo[]>([]);
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    listOcrLanguages()
      .then((items) => {
        if (alive) setLangs(items);
      })
      .catch((e) => {
        if (alive) setMsg(`语言列表读取失败：${String(e)}`);
      });
    return () => {
      alive = false;
    };
  }, []);

  const install = async () => {
    setBusy(true);
    setMsg("正在下载语言包…");
    try {
      const text = await installOcrLanguage(value.langCode.trim());
      setMsg(text);
      setLangs(await listOcrLanguages());
    } catch (e) {
      setMsg(`安装失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const installed = langs.filter((l) => l.installed).length;

  return (
    <div className="panel-form">
      <p className="inline-note">
        当前文件列表 {fileCount} 项，仅处理第一个文件。打印与邮件都调用系统程序，PayOnce PDF 不上传文件。
      </p>
      <SelectField
        label="动作"
        value={value.action}
        options={SHARE_ACTIONS}
        onChange={(v) => onChange({ action: v })}
        hint="打印对话框由系统默认 PDF 阅读器显示；邮件为 mailto 预填，附件需在客户端手动添加。"
      />
      {value.action === "email" ? (
        <>
          <TextField label="邮件主题" value={value.subject} onChange={(v) => onChange({ subject: v })} />
          <TextArea label="邮件正文" value={value.body} onChange={(v) => onChange({ body: v })} rows={3} />
        </>
      ) : null}

      <hr className="panel-divider" />
      <p className="inline-note">
        <strong>OCR 可选语言包</strong>：已安装 {installed} / {langs.length || "—"} 种。
        语言包下载是本产品<strong>唯一</strong>的运行时联网点：只在你点击安装时发生，
        请求只含语言代码，不含任何文件、文件名或用户数据；识别全程仍在本机完成。
      </p>
      <div className="field-row">
        <span className="field-label">已安装</span>
        <span className="field-hint">
          {langs.filter((l) => l.installed).map((l) => l.code).join(" / ") || "（无）"}
        </span>
      </div>
      <TextField
        label="安装语言代码"
        value={value.langCode}
        onChange={(v) => onChange({ langCode: v })}
        placeholder="如 jpn / fra / deu"
        hint="支持 42 种 tessdata_fast 语言；未列出的代码会被拒绝，避免拼错或路径穿越。"
      />
      <div className="field-row">
        <button type="button" className="btn-secondary" onClick={install} disabled={busy}>
          {busy ? "下载中…" : "安装语言包"}
        </button>
        <span className="field-hint">{msg}</span>
      </div>
    </div>
  );
}
