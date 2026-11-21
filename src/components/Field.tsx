import { useId, useState } from "react";
import type { ReactNode } from "react";
import { IS_TAURI } from "../lib/backend";

// 统一的表单行容器：label + 控件 + 提示。所有控件均为原生元素，键盘可直接操作。
interface FieldRowProps {
  label: string;
  htmlFor?: string;
  hint?: string;
  children: ReactNode;
}

export function FieldRow({ label, htmlFor, hint, children }: FieldRowProps) {
  return (
    <div className="field-row">
      <label className="field-label" htmlFor={htmlFor}>
        {label}
      </label>
      {children}
      {hint ? <p className="field-hint">{hint}</p> : null}
    </div>
  );
}

interface TextFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  hint?: string;
  disabled?: boolean;
}

export function TextField({ label, value, onChange, placeholder, hint, disabled }: TextFieldProps) {
  const id = useId();
  return (
    <FieldRow label={label} htmlFor={id} hint={hint}>
      <input
        id={id}
        className="text-input"
        type="text"
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        aria-label={label}
        onChange={(e) => onChange(e.target.value)}
      />
    </FieldRow>
  );
}

interface NumberFieldProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  hint?: string;
  disabled?: boolean;
}

export function NumberField({
  label,
  value,
  onChange,
  min,
  max,
  step,
  hint,
  disabled,
}: NumberFieldProps) {
  const id = useId();
  return (
    <FieldRow label={label} htmlFor={id} hint={hint}>
      <input
        id={id}
        className="text-input"
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </FieldRow>
  );
}

interface Option<T extends string> {
  value: T;
  label: string;
}

interface SelectFieldProps<T extends string> {
  label: string;
  value: T;
  options: readonly Option<T>[];
  onChange: (value: T) => void;
  hint?: string;
  disabled?: boolean;
}

export function SelectField<T extends string>({
  label,
  value,
  options,
  onChange,
  hint,
  disabled,
}: SelectFieldProps<T>) {
  const id = useId();
  return (
    <FieldRow label={label} htmlFor={id} hint={hint}>
      <select
        id={id}
        className="text-input"
        value={value}
        disabled={disabled}
        aria-label={label}
        onChange={(e) => onChange(e.target.value as T)}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </FieldRow>
  );
}

interface RadioFieldProps<T extends string> {
  label: string;
  value: T;
  options: readonly Option<T>[];
  onChange: (value: T) => void;
  hint?: string;
}

export function RadioField<T extends string>({
  label,
  value,
  options,
  onChange,
  hint,
}: RadioFieldProps<T>) {
  const name = useId();
  return (
    <fieldset className="field-row fieldset-row">
      <legend className="field-label">{label}</legend>
      <div className="choice-group">
        {options.map((o) => (
          <label className="choice-item" key={o.value}>
            <input
              type="radio"
              name={name}
              value={o.value}
              checked={value === o.value}
              aria-label={`${label}：${o.label}`}
              onChange={() => onChange(o.value)}
            />
            <span>{o.label}</span>
          </label>
        ))}
      </div>
      {hint ? <p className="field-hint">{hint}</p> : null}
    </fieldset>
  );
}

interface CheckboxFieldProps {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  hint?: string;
}

export function CheckboxField({ label, checked, onChange, hint }: CheckboxFieldProps) {
  const id = useId();
  return (
    <div className="field-row checkbox-row">
      <label className="check-item" htmlFor={id}>
        <input
          id={id}
          type="checkbox"
          checked={checked}
          aria-label={label}
          onChange={(e) => onChange(e.target.checked)}
        />
        <span>{label}</span>
      </label>
      {hint ? <p className="field-hint">{hint}</p> : null}
    </div>
  );
}

interface CheckboxGroupFieldProps<T extends string> {
  label: string;
  values: T[];
  options: readonly Option<T>[];
  onChange: (values: T[]) => void;
  hint?: string;
}

export function CheckboxGroupField<T extends string>({
  label,
  values,
  options,
  onChange,
  hint,
}: CheckboxGroupFieldProps<T>) {
  return (
    <fieldset className="field-row fieldset-row">
      <legend className="field-label">{label}</legend>
      <div className="choice-group">
        {options.map((o) => (
          <label className="choice-item" key={o.value}>
            <input
              type="checkbox"
              checked={values.includes(o.value)}
              aria-label={`${label}：${o.label}`}
              onChange={(e) =>
                onChange(
                  e.target.checked
                    ? [...values, o.value]
                    : values.filter((v) => v !== o.value),
                )
              }
            />
            <span>{o.label}</span>
          </label>
        ))}
      </div>
      {hint ? <p className="field-hint">{hint}</p> : null}
    </fieldset>
  );
}

interface PathFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onPick: () => Promise<string | null>;
  hint?: string;
  placeholder?: string;
}

// 文本框 + "选择…"按钮。预览模式无法选择真实保存路径时给出明确提示。
export function PathField({ label, value, onChange, onPick, hint, placeholder }: PathFieldProps) {
  const id = useId();
  const [note, setNote] = useState("");
  const pick = async () => {
    const picked = await onPick();
    if (picked === null) {
      setNote(IS_TAURI ? "已取消选择" : "预览模式不支持选择保存路径，保留默认值");
      return;
    }
    setNote("");
    onChange(picked);
  };
  return (
    <FieldRow label={label} htmlFor={id} hint={note || hint}>
      <div className="path-input">
        <input
          id={id}
          className="text-input"
          type="text"
          value={value}
          placeholder={placeholder}
          aria-label={label}
          onChange={(e) => onChange(e.target.value)}
        />
        <button type="button" className="btn btn-ghost" onClick={pick} aria-label={`选择${label}`}>
          选择…
        </button>
      </div>
    </FieldRow>
  );
}

interface KeyValueEditorProps {
  label: string;
  value: Record<string, string>;
  onChange: (value: Record<string, string>) => void;
  keyLabel?: string;
  valueLabel?: string;
  hint?: string;
}

// 用于签名字段与批量选项的键值对编辑：可增删行、可改键与值。
export function KeyValueEditor({
  label,
  value,
  onChange,
  keyLabel = "键",
  valueLabel = "值",
  hint,
}: KeyValueEditorProps) {
  const entries = Object.entries(value);
  const apply = (
    mutate: (index: number, key: string, val: string) => [string, string] | null,
  ) => {
    const next: Record<string, string> = {};
    entries.forEach(([k, v], i) => {
      const r = mutate(i, k, v);
      if (r) next[r[0]] = r[1];
    });
    onChange(next);
  };
  const add = () => {
    let key = "field";
    let i = 1;
    while (Object.prototype.hasOwnProperty.call(value, key)) {
      key = `field${i}`;
      i += 1;
    }
    onChange({ ...value, [key]: "" });
  };
  return (
    <div className="field-row">
      <div className="kv-header">
        <span className="field-label">{label}</span>
        <button type="button" className="btn btn-ghost btn-sm" onClick={add} aria-label={`添加${label}`}>
          添加
        </button>
      </div>
      {entries.length === 0 ? <p className="field-hint">暂无条目</p> : null}
      {entries.map(([k, v], i) => (
        <div className="kv-row" key={i}>
          <input
            className="text-input"
            type="text"
            value={k}
            placeholder={keyLabel}
            aria-label={`${keyLabel} ${i + 1}`}
            onChange={(e) => {
              const val = e.target.value;
              apply((idx, ok, ov) => (idx === i ? [val, ov] : [ok, ov]));
            }}
          />
          <input
            className="text-input"
            type="text"
            value={v}
            placeholder={valueLabel}
            aria-label={`${valueLabel} ${i + 1}`}
            onChange={(e) => {
              const val = e.target.value;
              apply((idx, ok, ov) => (idx === i ? [ok, val] : [ok, ov]));
            }}
          />
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            aria-label={`删除条目 ${i + 1}`}
            onClick={() => apply((idx, ok, ov) => (idx === i ? null : [ok, ov]))}
          >
            删除
          </button>
        </div>
      ))}
      {hint ? <p className="field-hint">{hint}</p> : null}
    </div>
  );
}
