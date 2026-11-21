import { NumberField, PathField, RadioField, TextField } from "../components/Field";
import { pickFolder } from "../lib/backend";
import type { SplitParams } from "../lib/params";

interface SplitPanelProps {
  value: SplitParams;
  onChange: (patch: Partial<SplitParams>) => void;
  fileCount: number;
}

const MODE_OPTIONS: readonly { value: SplitParams["mode"]; label: string }[] = [
  { value: "everyN", label: "每 N 页一份" },
  { value: "ranges", label: "指定页码范围" },
  { value: "bookmarks", label: "按书签层级" },
];

export function SplitPanel({ value, onChange, fileCount }: SplitPanelProps) {
  return (
    <div className="panel-form">
      <RadioField
        label="拆分模式"
        value={value.mode}
        options={MODE_OPTIONS}
        onChange={(mode) => onChange({ mode })}
      />

      {value.mode === "everyN" ? (
        <NumberField
          label="每份页数 N"
          value={value.n}
          min={1}
          step={1}
          onChange={(n) => onChange({ n })}
          hint="例如 500 页、每 10 页拆一份，将得到 50 个文件。"
        />
      ) : null}

      {value.mode === "ranges" ? (
        <TextField
          label="页码范围"
          value={value.ranges}
          onChange={(v) => onChange({ ranges: v })}
          placeholder="1-5,8,10-12"
          hint="用逗号分隔多个范围，每份对应一个输出文件。"
        />
      ) : null}

      {value.mode === "bookmarks" ? (
        <p className="inline-note">按顶层书签切分，每个书签章节输出一个文件。</p>
      ) : null}

      <TextField
        label="文件名模板"
        value={value.nameTemplate}
        onChange={(v) => onChange({ nameTemplate: v })}
        placeholder="{name}_{index}.pdf"
        hint="默认 {name}_{index}.pdf；{name} 为原名，{index} 为序号。"
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
        onPick={() => pickFolder({ title: "拆分输出文件夹" })}
        hint={`将使用文件列表中的第一个 PDF（当前 ${fileCount} 个文件）。`}
      />
    </div>
  );
}
