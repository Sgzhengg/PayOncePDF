export type JobStatus = "idle" | "running" | "done" | "error" | "cancelled";

export interface ProgressState {
  done: number;
  total: number;
  message: string;
}

interface ProgressBarProps {
  status: JobStatus;
  progress: ProgressState;
  statusText: string;
  outputs: string[];
  canExport: boolean;
  preview: boolean;
  hasInput: boolean;
  onRun: () => void;
  onCancel: () => void;
  onOpenFolder: () => void;
}

export function ProgressBar({
  status,
  progress,
  statusText,
  outputs,
  canExport,
  preview,
  hasInput,
  onRun,
  onCancel,
  onOpenFolder,
}: ProgressBarProps) {
  const running = status === "running";
  const total = progress.total > 0 ? progress.total : 1;
  return (
    <footer className="actionbar" aria-label="任务进度与操作">
      <div className="progress-line">
        <progress
          className="progress-el"
          value={Math.min(progress.done, total)}
          max={total}
          aria-label="处理进度"
        />
        <span className="progress-count">
          {progress.total > 0 ? `${progress.done} / ${progress.total}` : "0 / 0"}
        </span>
      </div>

      <div className="action-row">
        <button
          type="button"
          className="btn btn-primary"
          onClick={onRun}
          disabled={running || !canExport || !hasInput}
          aria-label="开始处理并导出"
        >
          开始处理
        </button>
        <button type="button" className="btn" onClick={onCancel} disabled={!running}>
          取消
        </button>
        <button type="button" className="btn btn-ghost" onClick={onOpenFolder}>
          打开输出文件夹
        </button>
        <span
          className={`status-text ${status === "error" ? "is-error" : ""}`}
          role="status"
          aria-live="polite"
        >
          {status === "running" ? progress.message || statusText : statusText}
        </span>
      </div>

      {!canExport ? (
        <p className="warning-line" role="alert">
          试用已结束；文件仍在，激活后可导出。仅限制导出，不删除文件。
        </p>
      ) : null}

      {outputs.length > 0 ? (
        <div className="output-block">
          <span className="output-title">输出路径</span>
          <ul className="output-list">
            {outputs.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </div>
      ) : status === "done" && preview ? (
        <p className="inline-note">预览模式未产生任何输出文件。</p>
      ) : null}
    </footer>
  );
}
