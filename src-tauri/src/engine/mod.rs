//! 引擎层：qpdf / PDFium / lopdf / tesseract 的封装。

pub mod compare;
pub mod extras;
pub mod geometry;
pub mod more;
pub mod objects;
pub mod ocr;
pub mod pdf;
pub mod qpdf;
pub mod render;
pub mod sidecar;

use crate::error::{AppError, AppResult};
use crate::state;
use std::sync::atomic::AtomicBool;
use tauri::AppHandle;

/// 传给引擎函数的任务上下文：用于逐页/逐文件进度上报与取消检查。
/// `emit=false` 时（批量任务）由上层统一发进度，引擎内部静默。
pub struct JobCtx<'a> {
    /// 真实运行必为 Some；引擎级离线测试（无 Tauri runtime）为 None，
    /// 此时只能 `emit=false`，tick 依然会检查取消标志。
    pub app: Option<&'a AppHandle>,
    pub job_id: &'a str,
    pub cancel: &'a AtomicBool,
    pub emit: bool,
}

impl<'a> JobCtx<'a> {
    /// 需要 AppHandle 的引擎（OCR / qpdf / 渲染）通过此方法取句柄；
    /// 离线上下文返回明确错误，绝不解引用空指针。
    pub fn app(&self) -> AppResult<&'a AppHandle> {
        self.app
            .ok_or_else(|| AppError::internal("任务上下文缺少应用句柄（离线模式不支持此操作）"))
    }

    /// 每步开始：先检查取消（命中返回 E2003），再发进度。
    pub fn tick(
        &self,
        done: usize,
        total: usize,
        message: impl Into<String>,
    ) -> AppResult<()> {
        state::check_cancel(self.cancel)?;
        if self.emit {
            if let Some(app) = self.app {
                state::emit_progress(app, self.job_id, done, total, message);
            }
        }
        Ok(())
    }

    pub fn progress(&self, done: usize, total: usize, message: impl Into<String>) {
        if self.emit {
            if let Some(app) = self.app {
                state::emit_progress(app, self.job_id, done, total, message);
            }
        }
    }
}

/// 把页码选择字符串（如 "1-5,8,11-13"）展开为 1-based 页码列表；
/// 为 None / 空则返回全部页。越界或格式错误返回 E2004。
pub fn page_selection(spec: Option<&str>, total: u32) -> AppResult<Vec<u32>> {
    let all = || (1..=total).collect::<Vec<u32>>();
    let Some(raw) = spec else { return Ok(all()); };
    let s = raw.trim();
    if s.is_empty() {
        return Ok(all());
    }
    let mut out: Vec<u32> = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Some((a, b)) = p.split_once('-') {
            let lo: u32 = a
                .trim()
                .parse()
                .map_err(|_| AppError::internal(format!("非法页码范围：{}", p)))?;
            let hi: u32 = b
                .trim()
                .parse()
                .map_err(|_| AppError::internal(format!("非法页码范围：{}", p)))?;
            if lo == 0 || hi < lo || hi > total {
                return Err(AppError::internal(format!(
                    "页码范围 {} 超出 1..{}",
                    p, total
                )));
            }
            for x in lo..=hi {
                out.push(x);
            }
        } else {
            let x: u32 = p
                .parse()
                .map_err(|_| AppError::internal(format!("非法页码：{}", p)))?;
            if x == 0 || x > total {
                return Err(AppError::internal(format!(
                    "页码 {} 超出 1..{}",
                    x, total
                )));
            }
            out.push(x);
        }
    }
    if out.is_empty() {
        return Err(AppError::internal("没有选中任何页"));
    }
    Ok(out)
}
