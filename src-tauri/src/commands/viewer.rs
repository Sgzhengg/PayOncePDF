//! F17 内置阅读视图：PDF 元信息 + 单页 PNG 渲染（UPDF 对标 L1，见 `docs/updf-parity-decision.md` §2）。
//! F18 画布框选修订联动：单页字符级文字布局（`docs/primary-use-case-decision.md` §5 P0）。
//!
//! 纯本地：PDFium 进程内渲染 → PNG 字节 / 字符包围盒 → base64 或 JSON 经 IPC 回传。
//! **不写临时文件、不联网、不上传文件内容或文件名**；失败走既有错误码体系。
//! 这些是查询命令，不产生 JobResult、不进入长任务队列（CONTRACT §3）。

use crate::engine::render;
pub use crate::engine::render::{PageTextLayout, TextCharBox};
use crate::error::{AppError, AppResult};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfoRequest {
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub pages: u32,
    pub page_width_pt: f32,
    pub page_height_pt: f32,
    /// 是否使用了密码打开（不表示文档本身一定加密）。
    pub opened_with_password: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequest {
    pub file: String,
    pub page: u32,
    #[serde(default)]
    pub dpi: Option<u32>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImage {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
    pub page: u32,
    pub pages: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTextLayoutRequest {
    pub file: String,
    pub page: u32,
    #[serde(default)]
    pub password: Option<String>,
}

fn require_pdf(path: &str) -> AppResult<&Path> {
    let p = Path::new(path);
    let is_pdf = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false);
    if !is_pdf {
        return Err(AppError::internal("内置阅读视图仅支持 PDF 文件"));
    }
    if !p.is_file() {
        return Err(AppError::internal("预览文件不存在或路径不可读"));
    }
    Ok(p)
}

/// 查询页数与首页纸张尺寸；加密文档会返回 E1002（由 `render::open` 透出），前端据此提示输入密码。
#[tauri::command]
pub fn get_pdf_info(payload: PdfInfoRequest, app: AppHandle) -> Result<PdfInfo, AppError> {
    let path = require_pdf(&payload.file)?;
    let doc = render::open(&app, path, payload.password.as_deref())?;
    let pages = doc.page_count();
    let (w, h) = if pages > 0 {
        doc.page_size_pt(1)?
    } else {
        (0.0, 0.0)
    };
    Ok(PdfInfo {
        pages,
        page_width_pt: w,
        page_height_pt: h,
        opened_with_password: payload.password.is_some(),
    })
}

/// 渲染指定页（1-based）为 PNG。页码越界自动夹到合法范围，不报错。
#[tauri::command]
pub fn render_page_preview(
    payload: PreviewRequest,
    app: AppHandle,
) -> Result<PreviewImage, AppError> {
    let path = require_pdf(&payload.file)?;
    let doc = render::open(&app, path, payload.password.as_deref())?;
    let pages = doc.page_count();
    if pages == 0 {
        return Err(AppError::parse("该 PDF 没有可渲染的页面"));
    }
    let page = payload.page.clamp(1, pages);
    let dpi = payload.dpi.unwrap_or(96).clamp(48, 200);
    let (png, width, height) = doc.render_page_png(page, dpi)?;
    Ok(PreviewImage {
        png_base64: base64::engine::general_purpose::STANDARD.encode(&png),
        width,
        height,
        page,
        pages,
    })
}

/// F18：返回指定页的字符级文字布局，供预览画布框选求交。
/// 旋转页 / 扫描件返回 `hasTextLayer=false` 且 `chars` 为空，前端必须禁用框选并提示。
#[tauri::command]
pub fn get_page_text_layout(
    payload: PageTextLayoutRequest,
    app: AppHandle,
) -> Result<PageTextLayout, AppError> {
    let path = require_pdf(&payload.file)?;
    let doc = render::open(&app, path, payload.password.as_deref())?;
    doc.page_text_layout(payload.page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pdf_is_rejected() {
        let err = require_pdf("Cargo.toml").unwrap_err();
        assert_eq!(err.code, "E2004");
    }

    #[test]
    fn missing_pdf_is_rejected() {
        let err = require_pdf("__definitely_missing__.pdf").unwrap_err();
        assert_eq!(err.code, "E2004");
    }

    #[test]
    fn layout_dto_is_camel_case() {
        let req: PageTextLayoutRequest =
            serde_json::from_str(r#"{"file":"a.pdf","page":2,"password":"pw"}"#).unwrap();
        assert_eq!(req.file, "a.pdf");
        assert_eq!(req.page, 2);
        assert_eq!(req.password.as_deref(), Some("pw"));
        let json = serde_json::to_string(&PageTextLayout {
            page: 2,
            pages: 3,
            page_width_pt: 595.0,
            page_height_pt: 842.0,
            chars: Vec::new(),
            text: String::new(),
            truncated: false,
            has_text_layer: false,
            rotation: 90,
            unsupported_reason: Some("旋转页".to_string()),
        })
        .unwrap();
        assert!(json.contains("\"hasTextLayer\":false"), "{json}");
        assert!(json.contains("\"pageWidthPt\""), "{json}");
        assert!(json.contains("\"rotation\":90"), "{json}");
        assert!(json.contains("\"unsupportedReason\""), "{json}");
    }
}
