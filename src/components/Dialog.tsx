import { useEffect, useRef } from "react";
import type { ReactNode } from "react";

interface DialogProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
}

// Esc 关闭由 App 统一处理（避免与任务取消冲突）；此处只负责焦点与点击遮罩关闭。
export function Dialog({ title, onClose, children }: DialogProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, []);
  return (
    <div
      className="dialog-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        ref={ref}
      >
        <header className="dialog-header">
          <h2>{title}</h2>
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            onClick={onClose}
            aria-label="关闭对话框"
          >
            关闭
          </button>
        </header>
        <div className="dialog-body">{children}</div>
      </div>
    </div>
  );
}
