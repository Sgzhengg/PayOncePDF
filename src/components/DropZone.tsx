import { useEffect, useState } from "react";
import type { DragEvent } from "react";
import {
  INPUT_FILTERS,
  IS_TAURI,
  pickFiles,
  pickFolder,
  subscribeDragDrop,
} from "../lib/backend";
import type { InputItem } from "../lib/params";
import type { ToolId } from "../lib/types";

interface DropZoneProps {
  tool: ToolId;
  fileCount: number;
  onAddPaths: (items: InputItem[]) => void;
  onFolder: (path: string) => void;
  /** 紧凑模式：放在底部可折叠的文件区里，一行高度，不占画布的地方。 */
  compact?: boolean;
}

export function DropZone({ tool, fileCount, onAddPaths, onFolder, compact = false }: DropZoneProps) {
  const [dragOver, setDragOver] = useState(false);

  // Tauri 拖放走窗口级事件；预览模式走下面的 HTML5 onDrop。
  useEffect(() => {
    if (!IS_TAURI) return;
    let unlisten: (() => void) | null = null;
    let disposed = false;
    subscribeDragDrop((paths) =>
      onAddPaths(paths.map((path) => ({ path, size: 0 }))),
    ).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onAddPaths]);

  const addFiles = async () => {
    const paths = await pickFiles({
      multiple: true,
      filters: INPUT_FILTERS,
      title: "添加文件",
    });
    if (paths.length > 0) onAddPaths(paths.map((path) => ({ path, size: 0 })));
  };

  const addFolder = async () => {
    const dir = await pickFolder({ title: "选择文件夹" });
    if (dir) onFolder(dir);
  };

  const onDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragOver(false);
    if (IS_TAURI) return;
    const items = Array.from(e.dataTransfer.files).map((f) => ({ path: f.name, size: f.size }));
    if (items.length > 0) onAddPaths(items);
  };

  return (
    <section
      className={`dropzone-section${compact ? " is-compact" : ""}`}
      aria-label="文件拖放区"
    >
      <div
        className={`dropzone ${dragOver ? "is-over" : ""}`}
        role="button"
        tabIndex={0}
        aria-label="拖放文件到这里，或按回车选择文件"
        onDragOver={(e) => {
          if (!IS_TAURI) {
            e.preventDefault();
            setDragOver(true);
          }
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={onDrop}
        onClick={addFiles}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            void addFiles();
          }
        }}
      >
        {compact ? (
          <span className="dropzone-compact-text">
            拖入文件，或点击选择（支持 PDF 与图片）
          </span>
        ) : (
          <>
            <p className="dropzone-title">把文件拖到这里</p>
            <p className="dropzone-sub">
              {IS_TAURI ? "或点击选择文件 · 支持多个 PDF 与图片" : "或点击选择文件 · 预览模式只能取得文件名"}
            </p>
          </>
        )}
        <p className="dropzone-count">当前 {fileCount} 个文件</p>
      </div>

      <div className="dropzone-actions">
        <button type="button" className="btn" onClick={addFiles}>
          添加文件
        </button>
        <button type="button" className="btn btn-ghost" onClick={addFolder} aria-label="选择文件夹">
          选择文件夹
        </button>
        {tool === "batch" && !compact ? (
          <span className="inline-note">批量功能请在右侧参数面板指定输入 / 输出目录</span>
        ) : null}
      </div>
    </section>
  );
}
