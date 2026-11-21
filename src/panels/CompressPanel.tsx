import { NumberField, PathField, SelectField, TextField } from "../components/Field";
import { PDF_FILTERS, pickSave } from "../lib/backend";
import type { CompressParams } from "../lib/params";

interface CompressPanelProps {
  value: CompressParams;
  onChange: (patch: Partial<CompressParams>) => void;
  fileCount: number;
}

const QUALITY_OPTIONS: readonly { value: CompressParams["quality"]; label: string }[] = [
  { value: "auto", label: "自动" },
  { value: "high", label: "高画质" },
  { value: "balanced", label: "均衡" },
  { value: "small", label: "最小体积" },
];

export function CompressPanel({ value, onChange, fileCount }: CompressPanelProps) {
  const targetBytes = Math.max(0, Math.round(value.targetMB * 1024 * 1024));
  return (
    <div className="panel-form">
      <NumberField
        label="目标大小（MB）"
        value={value.targetMB}
        min={0.1}
        step={0.5}
        onChange={(v) => onChange({ targetMB: v })}
        hint="自动选择图像降采样与重编码参数，尽量贴近目标大小。"
      />

      <SelectField
        label="画质档位"
        value={value.quality}
        options={QUALITY_OPTIONS}
        onChange={(quality) => onChange({ quality })}
        hint="档位越高越清晰，文件也越大。"
      />

      <dl className="estimate-list">
        <div>
          <dt>预计大小</dt>
          <dd>{`${value.targetMB} MB（约 ${targetBytes.toLocaleString()} 字节）`}</dd>
        </div>
        <div>
          <dt>画质档位</dt>
          <dd>{QUALITY_OPTIONS.find((o) => o.value === value.quality)?.label ?? value.quality}</dd>
        </div>
        <div>
          <dt>实际大小</dt>
          <dd>任务完成后由结果给出</dd>
        </div>
      </dl>

      <TextField
        label="加密文件密码"
        value={value.password}
        onChange={(v) => onChange({ password: v })}
        placeholder="留空表示无密码"
        hint="压缩不改变文字层与书签。"
      />

      <PathField
        label="输出文件"
        value={value.output}
        onChange={(v) => onChange({ output: v })}
        onPick={() => pickSave({ defaultPath: value.output, filters: PDF_FILTERS, title: "压缩输出为" })}
        hint={`将压缩文件列表中的第一个 PDF（当前 ${fileCount} 个文件）。`}
      />
    </div>
  );
}
