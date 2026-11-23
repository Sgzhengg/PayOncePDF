//! 命令层：请求/响应 DTO（与前端 `src/lib/types.ts` 镜像）+ 子模块。
//!
//! 所有结构体统一 `#[serde(rename_all = "camelCase")]`：
//! Rust 侧用 snake_case 字段，前端自动收发 camelCase（CONTRACT §4 硬性约定）。

pub mod app;
pub mod compare;
pub mod extras;
pub mod jobs;
pub mod more;
pub mod objects;
pub mod selfcheck;
pub mod viewer;

// F17 内置阅读视图 / F18 画布框选的 DTO 定义在 viewer.rs（响应体再导出自 engine::render），
// 这里再导出以保持 `crate::commands::*` 的契约风格。
pub use viewer::{
    PageTextLayout, PageTextLayoutRequest, PdfInfo, PdfInfoRequest, PreviewImage, PreviewRequest,
    TextCharBox,
};
// F20 文档比对响应再导出，保持 `crate::commands::*` 的契约风格。
pub use crate::engine::compare::{CompareDifference, CompareReport, CompareSensitivity};

/// F20 导出对比报告请求（比对查询 + jobId + 输出路径）。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareExportRequest {
    pub job_id: String,
    pub query: crate::engine::compare::CompareQuery,
    pub output: String,
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 空 payload（用于 `get_app_info` / `get_license_state`，前端仍以 `invoke(cmd, { payload: {} })` 调用）。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyPayload {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequest {
    pub job_id: String,
    pub files: Vec<String>,
    pub output: String,
    #[serde(default)]
    pub password: Option<String>,
    pub keep_bookmarks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output_dir: String,
    /// "everyN" | "ranges" | "bookmarks"
    pub mode: String,
    #[serde(default)]
    pub n: Option<u32>,
    #[serde(default)]
    pub ranges: Option<String>,
    /// 默认 `{name}_{index}.pdf`
    pub name_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub target_bytes: u64,
    /// "auto" | "high" | "balanced" | "small"
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToImagesRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output_dir: String,
    /// "png" | "jpg"
    pub format: String,
    /// 72 | 150 | 300
    pub dpi: u32,
    #[serde(default)]
    pub pages: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagesToPdfRequest {
    pub job_id: String,
    pub files: Vec<String>,
    pub output: String,
    /// "auto" | "a4" | "letter"
    pub page_size: String,
    pub margin: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    /// "pdf" | "txt"
    pub output_format: String,
    pub languages: Vec<String>,
    #[serde(default)]
    pub pages: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub signature_path: String,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub fields: HashMap<String, String>,
    pub flatten_fields: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub job_id: String,
    pub input_dir: String,
    pub output_dir: String,
    /// "compress" | "ocr" | "toImages" | "merge" | "edit" | "annotate" | "export" | "decorate" | "pages"
    pub operation: String,
    #[serde(default)]
    pub options: serde_json::Value,
}

// ---------------------------------------------------------------------------
// F10–F14：对标 PDFgear 桌面版新增的请求体（全部 camelCase，见 CONTRACT §4）
// ---------------------------------------------------------------------------

/// F10 就地替换文字：不重排、不回流、只改文本对象。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditTextRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub find: String,
    pub replace: String,
    pub match_case: bool,
    pub whole_word: bool,
    #[serde(default)]
    pub pages: Option<String>,
}

/// F19 按矩形精确替换：rect 为 PDF point、左下原点，只改框选那一处。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectEditRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub page: u32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
    pub replace: String,
    /// 前端预览到的选中文字；用于发现「预览后被改过」的错位，不参与定位。
    #[serde(default)]
    pub expected: Option<String>,
    /// 连续编辑：前端确认「输出就是本次会话自己生成的 *_edited 结果文件」时才置真，
    /// 允许把新结果原地写回该文件。默认 false，走「绝不覆盖输入」的常规路径。
    #[serde(default)]
    pub in_place: bool,
}

/// F11 批注：按查找文字生成标准 PDF 注解字典。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotateRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub find: String,
    /// "highlight" | "underline" | "strikeout" | "squiggly" | "note"
    pub kind: String,
    /// #RRGGBB
    pub color: String,
    pub author: String,
    pub note: String,
    pub match_case: bool,
    #[serde(default)]
    pub pages: Option<String>,
}

/// F12 页面管理。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageOpsRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    /// "rotate" | "delete" | "extract" | "insert" | "crop"
    pub mode: String,
    pub pages: String,
    pub degrees: i64,
    pub insert_at: u32,
    pub insert_count: u32,
    /// 裁剪边距 [左, 上, 右, 下]，单位 pt
    pub margins: Vec<f64>,
}

/// F13 水印 / 页眉页脚 / 页码 / 元数据 / AES-256 加密。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorateRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub watermark: String,
    pub opacity: f64,
    pub angle: f64,
    pub watermark_size: f64,
    pub header: String,
    pub footer: String,
    pub page_numbers: bool,
    /// "footerCenter" | "footerRight" | "headerRight"
    pub number_position: String,
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
    /// 留空表示不加密
    pub user_password: String,
    pub owner_password: String,
    pub allow_print: bool,
    pub allow_copy: bool,
    pub allow_modify: bool,
}

/// F14 导出文字层（不还原版式）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    /// "txt" | "html" | "docx" | "xlsx"
    pub format: String,
    #[serde(default)]
    pub pages: Option<String>,
}

// ---------------------------------------------------------------------------
// F15：文档对象（表单创建 / 脱敏 / 书签）
// ---------------------------------------------------------------------------

/// 创建 AcroForm 文本域；字段列表由前端逐行录入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub fields: Vec<crate::engine::objects::FormField>,
}

/// 密文脱敏：真删除文字对象 + 盖黑框。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    /// 要抹掉的词，一行一个
    pub terms: Vec<String>,
    pub match_case: bool,
    #[serde(default)]
    pub pages: Option<String>,
}

/// 只读查询：列出现有顶层书签。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkQuery {
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
}

/// "set" | "add" | "rename" | "delete"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkRequest {
    pub job_id: String,
    pub file: String,
    #[serde(default)]
    pub password: Option<String>,
    pub output: String,
    pub action: String,
    /// 每行「页码 标题」，set/add 使用
    pub spec: String,
    /// rename/delete 的原标题
    pub target: String,
    /// rename 的新标题
    pub new_title: String,
}

// ---------------------------------------------------------------------------
// F16：文本转 PDF / 打印与邮件 / OCR 语言包
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextToPdfRequest {
    pub job_id: String,
    pub file: String,
    pub output: String,
    /// "txt" | "csv" | "html"
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareRequest {
    pub job_id: String,
    pub file: String,
    /// "print" | "email"
    pub action: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageRequest {
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateRequest {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLoggingRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenFolderRequest {
    pub path: String,
}

/// 所有命令的统一返回结构。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResult {
    pub job_id: String,
    pub ok: bool,
    pub outputs: Vec<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

impl JobResult {
    pub fn ok(job_id: &str, outputs: Vec<String>, message: impl Into<String>) -> Self {
        Self {
            job_id: job_id.to_string(),
            ok: true,
            outputs,
            message: message.into(),
            report_path: None,
            elapsed_ms: None,
        }
    }
}

/// sidecar 自检条目。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarInfo {
    pub name: String,
    pub ready: bool,
    pub path: String,
    pub arch: String,
}

/// `get_app_info` 返回体。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub sidecars: Vec<SidecarInfo>,
    pub offline_capable: bool,
    /// 恒为 false：无遥测。
    pub telemetry: bool,
    /// 恒为 false：这是本地引擎，不是浏览器预览。
    pub preview: bool,
}

/// `get_license_state` / `activate_license` 返回体。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseState {
    /// "trial" | "active" | "expired"
    pub status: String,
    pub trial_days_left: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<i64>,
    pub can_export: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_hint: Option<String>,
}
