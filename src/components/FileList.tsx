import type { FileItem } from "../lib/params";
import type { ToolId } from "../lib/types";

interface FileListProps {
  files: FileItem[];
  tool: ToolId;
  onRemove: (id: string) => void;
  onMove: (id: string, direction: -1 | 1) => void;
  onClear: () => void;
}

const STATUS_LABEL: Record<FileItem["status"], string> = {
  ready: "待处理",
  done: "完成",
  error: "失败",
  skipped: "已跳过",
};

function formatSize(bytes: number): string {
  if (bytes <= 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function FileList({ files, tool, onRemove, onMove, onClear }: FileListProps) {
  return (
    <section className="filelist" aria-label="文件列表">
      <div className="filelist-header">
        <span>文件列表（{files.length}）</span>
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={onClear}
          disabled={files.length === 0}
          aria-label="清空文件列表"
        >
          清空
        </button>
      </div>

      {tool === "batch" ? (
        <p className="inline-note filelist-batch-note">
          批量处理按右侧面板选择的目录运行，此列表仅作参考、不参与批处理。
        </p>
      ) : null}

      {files.length === 0 ? (
        <p className="filelist-empty">尚未添加文件。用上方拖放区或「添加文件」加入待处理文件。</p>
      ) : (
        <table className="file-table">
          <thead>
            <tr>
              <th scope="col">序号</th>
              <th scope="col">文件名</th>
              <th scope="col">大小</th>
              <th scope="col">状态</th>
              <th scope="col">操作</th>
            </tr>
          </thead>
          <tbody>
            {files.map((file, index) => (
              <tr key={file.id}>
                <td className="cell-index">{index + 1}</td>
                <td className="cell-name" title={file.path}>
                  <span>{file.name}</span>
                  {file.note ? <span className="cell-note">{file.note}</span> : null}
                </td>
                <td className="cell-size">{formatSize(file.size)}</td>
                <td className={`cell-status status-${file.status}`}>{STATUS_LABEL[file.status]}</td>
                <td className="cell-actions">
                  <button
                    type="button"
                    className="btn btn-icon"
                    aria-label={`上移 ${file.name}`}
                    disabled={index === 0}
                    onClick={() => onMove(file.id, -1)}
                  >
                    上移
                  </button>
                  <button
                    type="button"
                    className="btn btn-icon"
                    aria-label={`下移 ${file.name}`}
                    disabled={index === files.length - 1}
                    onClick={() => onMove(file.id, 1)}
                  >
                    下移
                  </button>
                  <button
                    type="button"
                    className="btn btn-icon btn-danger"
                    aria-label={`删除 ${file.name}`}
                    onClick={() => onRemove(file.id)}
                  >
                    删除
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
