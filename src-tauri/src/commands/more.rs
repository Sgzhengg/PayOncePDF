//! F16 命令层：文本转 PDF、打印/邮件、OCR 语言包。
//!
//! 语言包下载是全产品唯一运行时联网点：用户主动触发、URL 只含语言代码、
//! 不含任何用户数据；不调用云端处理或 AI 接口（见 sidecar.rs 注释）。

use crate::commands::extras::{commit_one, require_file};
use crate::commands::jobs::execute;
use crate::commands::{EmptyPayload, JobResult, LanguageRequest, ShareRequest, TextToPdfRequest};
use crate::engine::{more, render, sidecar};
use crate::error::{AppError, AppResult};
use crate::state;
use std::path::Path;
use tauri::AppHandle;

#[tauri::command]
pub fn text_to_pdf(payload: TextToPdfRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "textToPdf", move |app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("created.pdf");
        let pdfium = render::load_pdfium(app)?;
        let pages = more::text_to_pdf(pdfium, &input, &payload.format, &tmp_out)?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "已把 {} 文本排成 {} 页 A4 PDF（中文用系统字体嵌入）。只做纯文本排版，不还原 Word/Excel 版式、表格与图片。",
                payload.format, pages
            ),
        ))
    })
}

/// percent-encode（mailto URL 的 subject/body）。
fn pct(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_' | b'.' | b'~') {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn print_file(path: &Path) -> AppResult<String> {
    let p = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        let quoted = p.replace('\'', "''");
        let script = format!("Start-Process -FilePath '{}' -Verb Print", quoted);
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script.as_str(),
            ])
            .spawn()
            .map_err(AppError::from)?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("lp")
            .arg(&p)
            .spawn()
            .map_err(AppError::from)?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = p;
        return Err(AppError::internal(
            "当前平台暂不支持直接调用系统打印；请用「打开输出文件夹」后手动打印。",
        ));
    }
    Ok("已把文件交给系统默认 PDF 阅读器打印。打印对话框由阅读器显示；PayOnce PDF 不上传文件。".into())
}

fn email_file(path: &Path, subject: &str, body: &str) -> AppResult<String> {
    let hint = format!(
        "{}\n\n附件请手动添加：{}\n（由 PayOnce PDF 本地生成，未上传）",
        body,
        path.to_string_lossy()
    );
    let url = format!("mailto:?subject={}&body={}", pct(subject), pct(&hint));
    #[cfg(target_os = "windows")]
    {
        let quoted = url.replace('\'', "''");
        let script = format!("Start-Process '{}'", quoted);
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script.as_str(),
            ])
            .spawn()
            .map_err(AppError::from)?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(AppError::from)?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = url;
        return Err(AppError::internal(
            "当前平台暂不支持直接唤起邮件客户端；请用「打开输出文件夹」后手动添加附件。",
        ));
    }
    Ok("已唤起系统邮件客户端并填好主题与正文。mailto 无法可靠跨平台附加附件，请在客户端里手动添加 PDF；PayOnce PDF 不上传文件。".into())
}

#[tauri::command]
pub fn share_file(payload: ShareRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "share", move |_app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let message = match payload.action.as_str() {
            "print" => print_file(&input)?,
            "email" => email_file(&input, &payload.subject, &payload.body)?,
            other => {
                return Err(AppError::internal(format!(
                    "未知分享动作：{}（可用 print / email）",
                    other
                )))
            }
        };
        Ok(JobResult::ok(jid, vec![], message))
    })
}

#[tauri::command]
pub fn list_ocr_languages(
    _payload: EmptyPayload,
    app: AppHandle,
) -> Vec<sidecar::OcrLanguageInfo> {
    sidecar::list_languages(&app)
}

#[tauri::command]
pub fn install_ocr_language(
    payload: LanguageRequest,
    app: AppHandle,
) -> Result<String, AppError> {
    sidecar::install_language(&app, &payload.lang)
}
