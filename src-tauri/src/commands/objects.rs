//! F15 命令层：表单 / 脱敏 / 书签。导出类命令统一复用 `execute()` 外壳
//! （授权校验、取消、临时目录、进度事件、原子提交、错误码、本地日志）。

use crate::commands::extras::{commit_one, require_file};
use crate::commands::jobs::{ctx_new, execute};
use crate::commands::{
    BookmarkQuery, BookmarkRequest, FormRequest, JobResult, RedactRequest,
};
use crate::engine::{objects, render};
use crate::error::AppError;
use crate::state;
use tauri::AppHandle;

#[tauri::command]
pub fn create_form_pdf(payload: FormRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "createForm", move |_app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("form.pdf");
        let out = objects::create_form(
            &input,
            payload.password.as_deref(),
            &payload.fields,
            &tmp_out,
        )?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "已创建 {} 个文本域，涉及 {} 页。字段为标准 AcroForm Widget，Adobe Reader / PDFgear 可填写；阅读器会自动生成外观。不覆盖既有表单。",
                out.fields, out.pages
            ),
        ))
    })
}

#[tauri::command]
pub fn redact_pdf(payload: RedactRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "redact", move |app, jid, flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("redacted.pdf");
        let ctx = ctx_new(app, jid, flag, true);
        let pdfium = render::load_pdfium(app)?;
        let out = objects::redact(
            &ctx,
            pdfium,
            &input,
            payload.password.as_deref(),
            &payload.terms,
            payload.match_case,
            payload.pages.as_deref(),
            &tmp_out,
            &tmp_dir,
        )?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "脱敏完成：{} 个词 / {} 处命中，{} 个黑框覆盖 {} 页。文字对象已真删除（复制粘贴也取不到原文），不是只盖一层黑框。",
                out.terms_removed, out.matches, out.boxes, out.pages
            ),
        ))
    })
}

/// 只读查询，不产生输出文件，因此不占用导出授权、也不发进度事件。
#[tauri::command]
pub fn list_bookmarks(
    payload: BookmarkQuery,
    _app: AppHandle,
) -> Result<Vec<objects::BookmarkEntry>, AppError> {
    let input = require_file(&payload.file)?;
    objects::list_bookmarks(&input, payload.password.as_deref())
}

#[tauri::command]
pub fn manage_bookmarks(payload: BookmarkRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "bookmarks", move |_app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("bookmarked.pdf");
        let (count, summary) = objects::manage_bookmarks(
            &input,
            payload.password.as_deref(),
            &payload.action,
            &payload.spec,
            &payload.target,
            &payload.new_title,
            &tmp_out,
        )?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!("{}。当前书签 {} 条；页面增删后书签不会自动跟随页号。", summary, count),
        ))
    })
}
