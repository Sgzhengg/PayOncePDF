import { CheckboxGroupField, PathField, RadioField, TextField } from "../components/Field";
import { PDF_FILTERS, pickSave } from "../lib/backend";
import type { OcrParams } from "../lib/params";

interface OcrPanelProps {
  value: OcrParams;
  onChange: (patch: Partial<OcrParams>) => void;
  fileCount: number;
}

const FORMAT_OPTIONS: readonly { value: OcrParams["outputFormat"]; label: string }[] = [
  { value: "pdf", label: "可搜索 PDF（叠加文字层）" },
  { value: "txt", label: "纯文本 TXT" },
];

const LANG_OPTIONS: readonly { value: string; label: string }[] = [
  { value: "eng", label: "英文 (eng)" },
  { value: "chi_sim", label: "简体中文 (chi_sim)" },
];

export function OcrPanel({ value, onChange, fileCount }: OcrPanelProps) {
  return (
    <div className="panel-form">
      <RadioField
        label="输出格式"
        value={value.outputFormat}
        options={FORMAT_OPTIONS}
        onChange={(outputFormat) => onChange({ outputFormat })}
      />

      <CheckboxGroupField
        label="识别语言"
        values={value.languages}
        options={LANG_OPTIONS}
        onChange={(languages) => onChange({ languages })}
        hint="可同时选择中英双语；语言包随应用本地提供。"
      />

      <TextField
        label="页码范围"
        value={value.pages}
        onChange={(v) => onChange({ pages: v })}
        placeholder="留空表示全部页，如 1-10"
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
            filters: value.outputFormat === "txt" ? [{ name: "TXT 文本", extensions: ["txt"] }] : PDF_FILTERS,
            title: "OCR 输出为",
          })
        }
        hint={`本地 Tesseract 引擎识别，不经过任何服务器（当前 ${fileCount} 个文件，取第一个 PDF）。`}
      />
    </div>
  );
}
