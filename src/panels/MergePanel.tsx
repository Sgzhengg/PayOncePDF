import { CheckboxField, PathField, TextField } from "../components/Field";
import { PDF_FILTERS, pickSave } from "../lib/backend";
import type { MergeParams } from "../lib/params";

interface MergePanelProps {
  value: MergeParams;
  onChange: (patch: Partial<MergeParams>) => void;
  fileCount: number;
  fileNames: string[];
  onSortByName: () => void;
}

export function MergePanel({
  value,
  onChange,
  fileCount,
  fileNames,
  onSortByName,
}: MergePanelProps) {
  return (
    <div className="panel-form">
      <div className="field-row">
        <span className="field-label">合并顺序（{fileCount} 个文件）</span>
        <ol className="order-list">
          {fileNames.length === 0 ? (
            <li>未添加文件</li>
          ) : (
            fileNames.map((name, i) => <li key={`${name}-${i}`}>{`${i + 1}. ${name}`}</li>)
          )}
        </ol>
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={onSortByName}
          disabled={fileCount === 0}
        >
          按文件名排序
        </button>
        <p className="field-hint">输出页数等于输入页数之和；可在中间列表用「上移 / 下移」调整顺序。</p>
      </div>

      <CheckboxField
        label="保留书签"
        checked={value.keepBookmarks}
        onChange={(v) => onChange({ keepBookmarks: v })}
        hint="合并后保留各文件的书签层级。"
      />

      <TextField
        label="加密文件密码"
        value={value.password}
        onChange={(v) => onChange({ password: v })}
        placeholder="留空表示无密码"
        hint="仅用于读取已知密码的文件，不破解密码。"
      />

      <PathField
        label="输出文件"
        value={value.output}
        onChange={(v) => onChange({ output: v })}
        onPick={() => pickSave({ defaultPath: value.output, filters: PDF_FILTERS, title: "合并输出为" })}
        hint="不会原地覆盖任何输入文件。"
      />
    </div>
  );
}
