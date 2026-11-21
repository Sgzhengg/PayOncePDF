import { IS_TAURI, PREVIEW_BANNER_TEXT } from "../lib/backend";

// 预览模式下顶部常驻的明显横幅（CONTRACT §5 硬性要求）。
export function PreviewBanner() {
  if (IS_TAURI) return null;
  return (
    <div className="preview-banner" role="status" aria-label="预览模式提示">
      <span className="preview-banner-tag">预览模式</span>
      <span>{PREVIEW_BANNER_TEXT}</span>
    </div>
  );
}
