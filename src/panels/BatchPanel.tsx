import { KeyValueEditor, PathField, SelectField } from "../components/Field";
import { pickFolder } from "../lib/backend";
import { defaultBatchOptions } from "../lib/params";
import type { BatchParams } from "../lib/params";

interface BatchPanelProps {
  value: BatchParams;
  onChange: (patch: Partial<BatchParams>) => void;
  fileCount: number;
}

const OPERATION_OPTIONS: readonly { value: BatchParams["operation"]; label: string }[] = [
  { value: "compress", label: "压缩" },
  { value: "ocr", label: "OCR" },
  { value: "toImages", label: "PDF 转图片" },
  { value: "merge", label: "合并" },
  { value: "edit", label: "就地改字" },
  { value: "annotate", label: "批注" },
  { value: "export", label: "导出文字层" },
  { value: "decorate", label: "水印/页码" },
  { value: "pages", label: "页面操作" },
];

export function BatchPanel({ value, onChange, fileCount }: BatchPanelProps) {
  return (
    <div className="panel-form">
      <PathField
        label="输入目录"
        value={value.inputDir}
        onChange={(v) => onChange({ inputDir: v })}
        onPick={() => pickFolder({ title: "选择输入目录" })}
        hint="对该目录内的 PDF 应用同一操作。"
      />

      <PathField
        label="输出目录"
        value={value.outputDir}
        onChange={(v) => onChange({ outputDir: v })}
        onPick={() => pickFolder({ title: "选择输出目录" })}
        hint="输出到新目录，不改写输入文件；同时生成 report.csv 变更报告。"
      />

      <SelectField
        label="操作类型"
        value={value.operation}
        options={OPERATION_OPTIONS}
        onChange={(operation) => onChange({ operation, options: defaultBatchOptions(operation) })}
        hint="切换操作会载入该操作的默认选项。"
      />

      <KeyValueEditor
        label="批量选项"
        value={value.options}
        onChange={(options) => onChange({ options })}
        keyLabel="选项名"
        valueLabel="选项值"
        hint="如 targetMB、quality、dpi、languages、pages 等；失败文件会记录错误码且不中断队列。"
      />

      <p className="inline-note">
        批量按目录运行，与中间文件列表无关（当前列表 {fileCount} 项，仅作参考）。
      </p>
    </div>
  );
}
