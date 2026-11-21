import {
  CheckboxField,
  KeyValueEditor,
  NumberField,
  PathField,
  TextField,
} from "../components/Field";
import { IMAGE_FILTERS, PDF_FILTERS, pickFiles, pickSave } from "../lib/backend";
import type { SignParams } from "../lib/params";

interface SignPanelProps {
  value: SignParams;
  onChange: (patch: Partial<SignParams>) => void;
  fileCount: number;
}

export function SignPanel({ value, onChange, fileCount }: SignPanelProps) {
  return (
    <div className="panel-form">
      <PathField
        label="签名图片"
        value={value.signaturePath}
        onChange={(v) => onChange({ signaturePath: v })}
        onPick={async () => {
          const picked = await pickFiles({
            multiple: false,
            filters: IMAGE_FILTERS,
            accept: "image/*",
            title: "选择签名图片",
          });
          return picked.length > 0 ? picked[0] : null;
        }}
        hint="选择一张本地签名图片作为图章。"
      />

      <NumberField
        label="目标页码"
        value={value.page}
        min={1}
        step={1}
        onChange={(page) => onChange({ page })}
      />

      <div className="field-grid">
        <NumberField label="X 坐标" value={value.x} step={1} onChange={(x) => onChange({ x })} />
        <NumberField label="Y 坐标" value={value.y} step={1} onChange={(y) => onChange({ y })} />
        <NumberField
          label="宽度"
          value={value.width}
          min={1}
          step={1}
          onChange={(width) => onChange({ width })}
        />
        <NumberField
          label="高度"
          value={value.height}
          min={1}
          step={1}
          onChange={(height) => onChange({ height })}
        />
      </div>

      <KeyValueEditor
        label="表单字段"
        value={value.fields}
        onChange={(fields) => onChange({ fields })}
        keyLabel="字段名"
        valueLabel="填写值"
        hint="按需添加表单字段的键与值。"
      />

      <CheckboxField
        label="扁平化表单（flatten）"
        checked={value.flattenFields}
        onChange={(flattenFields) => onChange({ flattenFields })}
        hint="扁平化后字段不可再编辑。"
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
        onPick={() => pickSave({ defaultPath: value.output, filters: PDF_FILTERS, title: "签名输出为" })}
        hint={`将签署文件列表中的第一个 PDF（当前 ${fileCount} 个文件）。`}
      />
    </div>
  );
}
