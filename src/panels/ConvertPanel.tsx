import {
  NumberField,
  PathField,
  RadioField,
  SelectField,
  TextField,
} from "../components/Field";
import { PDF_FILTERS, pickFolder, pickSave } from "../lib/backend";
import type { ConvertParams } from "../lib/params";

interface ConvertPanelProps {
  value: ConvertParams;
  onChange: (patch: Partial<ConvertParams>) => void;
  fileCount: number;
}

const DIRECTION_OPTIONS: readonly { value: ConvertParams["direction"]; label: string }[] = [
  { value: "toImages", label: "PDF → 图片" },
  { value: "toPdf", label: "图片 → PDF" },
];

const FORMAT_OPTIONS: readonly { value: ConvertParams["format"]; label: string }[] = [
  { value: "png", label: "PNG" },
  { value: "jpg", label: "JPG" },
];

const DPI_OPTIONS: readonly { value: "72" | "150" | "300"; label: string }[] = [
  { value: "72", label: "72 DPI" },
  { value: "150", label: "150 DPI" },
  { value: "300", label: "300 DPI" },
];

const PAGE_SIZE_OPTIONS: readonly { value: ConvertParams["pageSize"]; label: string }[] = [
  { value: "auto", label: "跟随图片" },
  { value: "a4", label: "A4" },
  { value: "letter", label: "Letter" },
];

export function ConvertPanel({ value, onChange, fileCount }: ConvertPanelProps) {
  return (
    <div className="panel-form">
      <RadioField
        label="转换方向"
        value={value.direction}
        options={DIRECTION_OPTIONS}
        onChange={(direction) => onChange({ direction })}
      />

      {value.direction === "toImages" ? (
        <>
          <RadioField
            label="图片格式"
            value={value.format}
            options={FORMAT_OPTIONS}
            onChange={(format) => onChange({ format })}
          />
          <SelectField
            label="分辨率 DPI"
            value={(String(value.dpi) as "72" | "150" | "300")}
            options={DPI_OPTIONS}
            onChange={(v) => onChange({ dpi: Number(v) as 72 | 150 | 300 })}
            hint="300 DPI 适合打印与 OCR 前处理。"
          />
          <TextField
            label="页码范围"
            value={value.pages}
            onChange={(v) => onChange({ pages: v })}
            placeholder="留空表示全部页，如 1-3,7"
          />
          <TextField
            label="加密文件密码"
            value={value.password}
            onChange={(v) => onChange({ password: v })}
            placeholder="留空表示无密码"
          />
          <PathField
            label="输出文件夹"
            value={value.outputDir}
            onChange={(v) => onChange({ outputDir: v })}
            onPick={() => pickFolder({ title: "图片输出文件夹" })}
            hint={`按页导出，页序与原文件一致（当前 ${fileCount} 个文件，取第一个 PDF）。`}
          />
        </>
      ) : (
        <>
          <SelectField
            label="页面大小"
            value={value.pageSize}
            options={PAGE_SIZE_OPTIONS}
            onChange={(pageSize) => onChange({ pageSize })}
          />
          <NumberField
            label="页边距"
            value={value.margin}
            min={0}
            step={1}
            onChange={(margin) => onChange({ margin })}
            hint="单位为点（pt）。"
          />
          <PathField
            label="输出文件"
            value={value.output}
            onChange={(v) => onChange({ output: v })}
            onPick={() => pickSave({ defaultPath: value.output, filters: PDF_FILTERS, title: "图片转 PDF 输出为" })}
            hint={`使用文件列表中的全部图片，列表顺序即页序（当前 ${fileCount} 个文件）。`}
          />
        </>
      )}
    </div>
  );
}
