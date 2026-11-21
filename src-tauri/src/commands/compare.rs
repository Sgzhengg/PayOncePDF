//! F20 命令层：文档比对（只读查询）+ 导出对比报告（走导出授权）。

use crate::commands::extras::{commit_one, require_file};
use crate::commands::jobs::execute;
use crate::engine::compare;
use crate::error::AppError;
use crate::state;
use tauri::AppHandle;

use super::CompareExportRequest;

/// 只读查询：比对两个 PDF 的文字层，返回结构化差异。
/// 不写文件、不占导出授权、不发进度事件——与 list_bookmarks 同形态。
#[tauri::command]
pub fn compare_documents(
    payload: compare::CompareQuery,
    app: AppHandle,
) -> Result<compare::CompareReport, AppError> {
    let old_path = require_file(&payload.old_file)?;
    let new_path = require_file(&payload.new_file)?;
    compare::compare(
        &app,
        &old_path,
        payload.old_password.as_deref(),
        &new_path,
        payload.new_password.as_deref(),
        payload.pages.as_deref(),
        payload.sensitivity.as_ref().unwrap_or(&compare::CompareSensitivity::default()),
    )
}

/// 导出对比报告（纯文本 .txt）：走 execute 外壳，占导出授权、原子提交。
#[tauri::command]
pub fn export_compare_report(
    payload: CompareExportRequest,
    app: AppHandle,
) -> Result<super::JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "compareReport", move |app, jid, cancel| {
        let old_path = require_file(&payload.query.old_file)?;
        let new_path = require_file(&payload.query.new_file)?;
        let ctx = crate::engine::JobCtx {
            app: Some(app),
            job_id: jid,
            cancel,
            emit: true,
        };
        ctx.tick(1, 3, "读取并比对两个文档…")?;
        let report = compare::compare(
            app,
            &old_path,
            payload.query.old_password.as_deref(),
            &new_path,
            payload.query.new_password.as_deref(),
            payload.query.pages.as_deref(),
            payload
                .query
                .sensitivity
                .as_ref()
                .unwrap_or(&compare::CompareSensitivity::default()),
        )?;
        ctx.tick(2, 3, "生成对比报告…")?;
        let text = compare::render_report_text(&report);
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("compare-report.txt");
        std::fs::write(&tmp_out, text).map_err(AppError::from)?;
        ctx.tick(3, 3, "保存报告…")?;
        // 报告不是单个输入的改写，用第一个输入做「不覆盖输入」检查即可。
        let (_, out_str) = commit_one(&old_path, &payload.output, &tmp_out)?;
        Ok(super::JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "比对完成：新增 {} / 删除 {} / 修改 {} 处（仅文字层，报告已导出）",
                report.added, report.removed, report.changed
            ),
        ))
    })
}
