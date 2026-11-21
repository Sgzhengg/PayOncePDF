//! 统一错误类型与错误码（PRD §13 + CONTRACT §2 扩展码）。
//!
//! 所有命令失败时返回 `Err(AppError)`，Tauri 会把它序列化为
//! `{ "code": "E1001", "message": "..." }` 传给前端。

use serde::Serialize;
use std::fmt;

/// 前后端约定的错误结构。字段用 camelCase（此处本身即小写，无需 rename）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    /// E1001 文件损坏 / 无法解析
    pub fn parse(message: impl Into<String>) -> Self {
        Self::new("E1001", message)
    }
    /// E1002 需要密码
    pub fn need_password(message: impl Into<String>) -> Self {
        Self::new("E1002", message)
    }
    /// E1003 密码错误
    pub fn bad_password(message: impl Into<String>) -> Self {
        Self::new("E1003", message)
    }
    /// E1004 磁盘空间不足
    pub fn disk_full(message: impl Into<String>) -> Self {
        Self::new("E1004", message)
    }
    /// E1005 页数超限（>5000）
    pub fn page_limit(message: impl Into<String>) -> Self {
        Self::new("E1005", message)
    }
    /// E1006 OCR 语言包缺失
    pub fn ocr_lang(message: impl Into<String>) -> Self {
        Self::new("E1006", message)
    }
    /// E1007 输出被占用 / 已存在
    pub fn output_busy(message: impl Into<String>) -> Self {
        Self::new("E1007", message)
    }
    /// E2001 sidecar 缺失或架构不匹配
    pub fn sidecar_missing(message: impl Into<String>) -> Self {
        Self::new("E2001", message)
    }
    /// E2002 PDFium 渲染引擎未加载
    pub fn pdfium_missing(message: impl Into<String>) -> Self {
        Self::new("E2002", message)
    }
    /// E2003 用户取消
    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new("E2003", message)
    }
    /// E2004 内部错误（含参数非法）
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("E2004", message)
    }
    /// E2005 试用到期，导出被限制
    pub fn trial_expired(message: impl Into<String>) -> Self {
        Self::new("E2005", message)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// 命令返回类型别名。
pub type AppResult<T> = Result<T, AppError>;

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            // 磁盘满 / 配额不足在部分平台并不稳定映射到 StorageFull，
            // 因此这里只做通用内部错误映射，磁盘空间由调用点显式检查。
            ErrorKind::NotFound => Self::internal(format!("IO error: {}", err)),
            ErrorKind::PermissionDenied => Self::output_busy(format!("IO error: {}", err)),
            _ => Self::internal(format!("IO error: {}", err)),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::internal(format!("JSON 处理失败：{}", err))
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::internal(message)
    }
}
