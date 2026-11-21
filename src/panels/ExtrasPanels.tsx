// F10–F14 参数面板。所有控件复用 components/Field，保持 §16 右栏布局一致。
// 边界说明全部写在 hint 里：不夸大能力，做不到的事明确告诉用户（PRD §6 诚实原则）。

import {
  CheckboxField,
  NumberField,
  PathField,
  SelectField,
  TextField,
} from "../components/Field";
import { PDF_FILTERS, pickSave } from "../lib/backend";
import type {
  AnnotateParams,
  DecorateParams,
  EditParams,
  ExportParams,
  PagesParams,
} from "../lib/params";

interface PanelProps<T> {
  value: T;
  onChange: (patch: Partial<T>) => void;
  fileCount: number;
}

const pdfSave = (defaultPath: string, title: string) =>
  pickSave({ defaultPath, filters: PDF_FILTERS, title });

// ---------------------------------------------------------------------------
// F10 就地改字
// ---------------------------------------------------------------------------

export function EditPanel({ value, onChange, fileCount }: PanelProps<EditParams>) {
  return (
    <div className="panel-form">
      <p className="inline-note">
        看 PDF 时发现几个字要改？不用转 Word，不用订 Acrobat。就地改，一次买断。
        当前文件列表 {fileCount} 项，仅处理第一个 PDF。
      </p>

      <TextField
        label="查找"
        value={value.find}
        onChange={(v) => onChange({ find: v })}
        placeholder="要被替换的原文字"
        hint="必须是有文字层的可选中文本；扫描件请先走 OCR。"
      />
      <TextField
        label="替换为"
        value={value.replace}
        onChange={(v) => onChange({ replace: v })}
        placeholder="新文字（可留空表示删除）"
        hint="中文可用：沿用原 PDF 字体写入；若原字体缺字形会明确报错，绝不写乱码。"
      />
      <CheckboxField
        label="区分大小写"
        checked={value.matchCase}
        onChange={(v) => onChange({ matchCase: v })}
      />
      <CheckboxField
        label="全词匹配"
        checked={value.wholeWord}
        onChange={(v) => onChange({ wholeWord: v })}
        hint="只匹配被空白或标点包围的完整词。"
      />
      <TextField
        label="页码范围"
        value={value.pages}
        onChange={(v) => onChange({ pages: v })}
        placeholder="留空 = 全部页，如 1-3,5"
      />
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
        onPick={() => pdfSave(value.output, "改字结果另存为")}
        hint="不重排、不回流；替换文字变长时可能压到右侧内容，请目视确认。原文件绝不改动。"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// F11 批注
// ---------------------------------------------------------------------------

const ANNOT_KINDS: readonly { value: AnnotateParams["kind"]; label: string }[] = [
  { value: "highlight", label: "高亮" },
  { value: "underline", label: "下划线" },
  { value: "strikeout", label: "删除线" },
  { value: "squiggly", label: "波浪线" },
  { value: "note", label: "便签备注" },
];

export function AnnotatePanel({ value, onChange, fileCount }: PanelProps<AnnotateParams>) {
  return (
    <div className="panel-form">
      <p className="inline-note">
        按文字查找后生成标准 PDF 注解，Adobe Reader / PDFgear / 预览 都能读取。
        当前文件列表 {fileCount} 项，仅处理第一个 PDF。
      </p>
      <TextField
        label="标注哪些文字"
        value={value.find}
        onChange={(v) => onChange({ find: v })}
        placeholder="如：重要条款"
        hint="命中全部匹配处；扫描件请先走 OCR。"
      />
      <SelectField
        label="批注类型"
        value={value.kind}
        options={ANNOT_KINDS}
        onChange={(v) => onChange({ kind: v })}
      />
      <TextField
        label="颜色"
        value={value.color}
        onChange={(v) => onChange({ color: v })}
        placeholder="#FFD54F"
        hint="#RRGGBB 格式。"
      />
      <TextField
        label="作者"
        value={value.author}
        onChange={(v) => onChange({ author: v })}
        placeholder="可留空"
      />
      <TextField
        label="备注内容"
        value={value.note}
        onChange={(v) => onChange({ note: v })}
        placeholder="写在注解 /Contents 里"
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
        onPick={() => pdfSave(value.output, "批注结果另存为")}
        hint="输出新文件，原文件不变。"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// F12 页面管理
// ---------------------------------------------------------------------------

const PAGE_MODES: readonly { value: PagesParams["mode"]; label: string }[] = [
  { value: "rotate", label: "旋转" },
  { value: "delete", label: "删除" },
  { value: "extract", label: "抽取" },
  { value: "insert", label: "插入空白页" },
  { value: "crop", label: "裁剪" },
];

export function PagesPanel({ value, onChange, fileCount }: PanelProps<PagesParams>) {
  const modeHint: Record<PagesParams["mode"], string> = {
    rotate: "把选中页旋转 90/180/270 度。",
    delete: "删除选中页，页树重建，书签可能不随页号更新。",
    extract: "只保留指定页码范围，输出一个新 PDF。",
    insert: "在指定页前插入若干张同尺寸空白页。",
    crop: "只改 /CropBox，不裁掉页面内容，随时可还原。",
  };
  return (
    <div className="panel-form">
      <p className="inline-note">当前文件列表 {fileCount} 项，仅处理第一个 PDF。</p>
      <SelectField
        label="操作"
        value={value.mode}
        options={PAGE_MODES}
        onChange={(v) => onChange({ mode: v })}
        hint={modeHint[value.mode]}
      />
      <TextField
        label="页码范围"
        value={value.pages}
        onChange={(v) => onChange({ pages: v })}
        placeholder="1-3,5 或 all"
        hint="旋转/删除/裁剪/抽取使用；插入模式使用下方的「插入到第几页前」。"
      />
      {value.mode === "rotate" ? (
        <NumberField
          label="旋转角度"
          value={value.degrees}
          onChange={(v) => onChange({ degrees: v })}
          min={-270}
          max={270}
          step={90}
          hint="必须是 90 的倍数。"
        />
      ) : null}
      {value.mode === "insert" ? (
        <>
          <NumberField
            label="插入到第几页前"
            value={value.insertAt}
            onChange={(v) => onChange({ insertAt: v })}
            min={1}
            max={9999}
          />
          <NumberField
            label="插入张数"
            value={value.insertCount}
            onChange={(v) => onChange({ insertCount: v })}
            min={1}
            max={200}
          />
        </>
      ) : null}
      {value.mode === "crop" ? (
        <>
          <NumberField label="左边距 (pt)" value={value.marginLeft} onChange={(v) => onChange({ marginLeft: v })} min={0} />
          <NumberField label="上边距 (pt)" value={value.marginTop} onChange={(v) => onChange({ marginTop: v })} min={0} />
          <NumberField label="右边距 (pt)" value={value.marginRight} onChange={(v) => onChange({ marginRight: v })} min={0} />
          <NumberField label="下边距 (pt)" value={value.marginBottom} onChange={(v) => onChange({ marginBottom: v })} min={0} />
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
        onPick={() => pdfSave(value.output, "页面操作结果另存为")}
        hint="输出新文件，原文件不变。"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// F13 水印 / 页眉页脚 / 页码 / 元数据 / 加密
// ---------------------------------------------------------------------------

const NUMBER_POSITIONS: readonly {
  value: DecorateParams["numberPosition"];
  label: string;
}[] = [
  { value: "footerCenter", label: "页脚居中" },
  { value: "footerRight", label: "页脚右侧" },
  { value: "headerRight", label: "页眉右侧" },
];

export function DecoratePanel({ value, onChange, fileCount }: PanelProps<DecorateParams>) {
  return (
    <div className="panel-form">
      <p className="inline-note">
        <strong>水印文字支持中文</strong>：含非拉丁字符时自动用系统 CJK 字体嵌入，写进真实文本层（水平居中灰字，不支持旋转/独立透明度，此时字号生效、角度与不透明度不生效）。
        页眉 / 页脚仍用 Helvetica/WinAnsi，<strong>仅支持拉丁字符</strong>，输入中文会明确报错而不是写出乱码。
        当前文件列表 {fileCount} 项，仅处理第一个 PDF。
      </p>
      <TextField
        label="水印文字"
        value={value.watermark}
        onChange={(v) => onChange({ watermark: v })}
        placeholder="DRAFT / CONFIDENTIAL"
      />
      <NumberField label="水印字号 (pt)" value={value.watermarkSize} onChange={(v) => onChange({ watermarkSize: v })} min={6} max={200} />
      <NumberField label="水印角度" value={value.angle} onChange={(v) => onChange({ angle: v })} min={-90} max={90} />
      <NumberField label="不透明度" value={value.opacity} onChange={(v) => onChange({ opacity: v })} min={0.05} max={1} step={0.05} />
      <TextField label="页眉" value={value.header} onChange={(v) => onChange({ header: v })} placeholder="可留空" />
      <TextField label="页脚" value={value.footer} onChange={(v) => onChange({ footer: v })} placeholder="可留空" />
      <CheckboxField label="添加页码" checked={value.pageNumbers} onChange={(v) => onChange({ pageNumbers: v })} />
      <SelectField
        label="页码位置"
        value={value.numberPosition}
        options={NUMBER_POSITIONS}
        onChange={(v) => onChange({ numberPosition: v })}
      />
      <TextField label="文档标题" value={value.title} onChange={(v) => onChange({ title: v })} placeholder="写入 /Info" />
      <TextField label="作者" value={value.author} onChange={(v) => onChange({ author: v })} />
      <TextField label="主题" value={value.subject} onChange={(v) => onChange({ subject: v })} />
      <TextField label="关键词" value={value.keywords} onChange={(v) => onChange({ keywords: v })} />
      <TextField
        label="打开密码（留空不加密）"
        value={value.userPassword}
        onChange={(v) => onChange({ userPassword: v })}
        placeholder="AES-256，本机不保存"
        hint="仅设置你提供的密码，不破解任何已有密码。密码丢失无法找回。"
      />
      <TextField label="所有者密码" value={value.ownerPassword} onChange={(v) => onChange({ ownerPassword: v })} placeholder="留空则同打开密码" />
      <CheckboxField label="允许打印" checked={value.allowPrint} onChange={(v) => onChange({ allowPrint: v })} />
      <CheckboxField label="允许复制文字" checked={value.allowCopy} onChange={(v) => onChange({ allowCopy: v })} />
      <CheckboxField label="允许修改" checked={value.allowModify} onChange={(v) => onChange({ allowModify: v })} />
      <TextField
        label="源文件密码"
        value={value.password}
        onChange={(v) => onChange({ password: v })}
        placeholder="留空表示无密码"
      />
      <PathField
        label="输出文件"
        value={value.output}
        onChange={(v) => onChange({ output: v })}
        onPick={() => pdfSave(value.output, "结果另存为")}
        hint="叠加内容追加在页面内容流末尾，不重排原版面。"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// F14 导出文字层
// ---------------------------------------------------------------------------

const EXPORT_FORMATS: readonly { value: ExportParams["format"]; label: string }[] = [
  { value: "txt", label: "纯文本 TXT" },
  { value: "html", label: "网页 HTML" },
  { value: "docx", label: "Word DOCX" },
  { value: "xlsx", label: "Excel XLSX" },
];

const EXT_LABEL: Record<ExportParams["format"], string> = {
  txt: "txt",
  html: "html",
  docx: "docx",
  xlsx: "xlsx",
};

export function ExportPanel({ value, onChange, fileCount }: PanelProps<ExportParams>) {
  const changeFormat = (format: ExportParams["format"]) => {
    const stem = value.output.replace(/\.[^./\\]+$/, "") || "export";
    onChange({ format, output: `${stem}.${EXT_LABEL[format]}` });
  };
  return (
    <div className="panel-form">
      <p className="inline-note">
        <strong>诚实边界：</strong>只导出文字层，不还原版式、表格与图片；DOCX/XLSX 是可编辑的文字稿，
        不是商业 SDK 那种高保真转换。复杂排版请人工校对。当前文件列表 {fileCount} 项，仅处理第一个 PDF。
      </p>
      <SelectField
        label="导出格式"
        value={value.format}
        options={EXPORT_FORMATS}
        onChange={changeFormat}
        hint="OOXML 由本机直接生成（ZIP+XML），不上传、不调用云端服务。"
      />
      <TextField
        label="页码范围"
        value={value.pages}
        onChange={(v) => onChange({ pages: v })}
        placeholder="留空 = 全部页，如 1-3,5"
      />
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
        onPick={() =>
          pickSave({
            defaultPath: value.output,
            filters: [
              {
                name: `${EXT_LABEL[value.format].toUpperCase()} 文件`,
                extensions: [EXT_LABEL[value.format]],
              },
            ],
            title: "导出为",
          })
        }
        hint="扫描件无文字层时会导出空页提示，请先走 OCR。"
      />
    </div>
  );
}
