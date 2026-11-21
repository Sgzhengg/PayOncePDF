//! F10–F14 命令层：与 F1–F9 完全共用 `execute()` 外壳（授权校验、取消、
//! 临时目录、进度/结束事件、原子提交、本地日志），不绕过任何既有安全约束。

use crate::commands::{
    AnnotateRequest, DecorateRequest, EditTextRequest, ExportRequest, JobResult, PageOpsRequest,
    RectEditRequest,
};
use crate::commands::jobs::{ctx_new, execute, p2s};
use crate::engine::{extras, more, qpdf, render};
use crate::error::{AppError, AppResult};
use crate::state;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// 单文件输入 → 单文件输出的统一收尾：不覆盖输入、临时文件原子提交。
pub(crate) fn commit_one(
    input: &Path,
    requested: &str,
    tmp_out: &Path,
) -> AppResult<(PathBuf, String)> {
    state::assert_not_overwriting_inputs(Path::new(requested), &[input])?;
    let final_path = state::resolve_output_path(Path::new(requested));
    state::atomic_commit(tmp_out, &final_path)?;
    Ok((final_path.clone(), p2s(&final_path)))
}

/// 文件名是否像本工具生成的连续编辑结果：`doc_edited`、`doc_edited_1`（重名自增）。
/// 只做「够用」的识别，真正的把关是请求里的 `in_place` 标志由前端按本次会话的产物设置。
fn looks_like_edited_stem(stem: &str) -> bool {
    if let Some(rest) = stem.strip_suffix("_edited") {
        return !rest.is_empty();
    }
    if let Some(pos) = stem.rfind("_edited_") {
        let suffix = &stem[pos + "_edited_".len()..];
        return !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// F19 连续编辑专用收尾：允许把结果写回「上一轮的结果文件」自身，实现单文件累积。
///
/// 为什么需要单独一个函数：`commit_one` 有 `assert_not_overwriting_inputs`，
/// 输入和输出同一路径会被拒。但连续改字的正常用法就是「接着改刚生成的那份
/// `*_edited.pdf`」，此时输入 == 输出。这里开一个**极窄**的口子：
/// 只有输入与输出是同一个文件、且文件名是本工具的 `_edited` 产物时，
/// 才允许原地替换。用户自己选的原始文件不会命中这条分支；即便前端的
/// `in_place` 判断错了，文件名这一关仍会挡住。
pub(crate) fn commit_one_chained(
    input: &Path,
    requested: &str,
    tmp_out: &Path,
) -> AppResult<(PathBuf, String)> {
    let req = Path::new(requested);
    let same = match (std::fs::canonicalize(req), std::fs::canonicalize(input)) {
        (Ok(a), Ok(b)) => a == b,
        _ => req == input,
    };
    if !same {
        return commit_one(input, requested, tmp_out);
    }
    let stem = req.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if !looks_like_edited_stem(stem) {
        return Err(AppError::internal(format!(
            "输出路径与输入文件相同（{}），且不是本工具生成的 _edited 结果文件，拒绝原地覆盖用户原件。",
            p2s(req)
        )));
    }
    state::atomic_commit(tmp_out, req)?;
    Ok((req.to_path_buf(), p2s(req)))
}

pub(crate) fn require_file(path: &str) -> AppResult<PathBuf> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(AppError::parse(format!("输入文件不存在：{}", path)));
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// F10 就地替换文字
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn replace_text(payload: EditTextRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "editText", move |app, jid, flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("edited.pdf");
        let ctx = ctx_new(app, jid, flag, true);
        let pdfium = render::load_pdfium(app)?;
        let outcome = extras::replace_text(
            &ctx,
            pdfium,
            &input,
            payload.password.as_deref(),
            &payload.find,
            &payload.replace,
            payload.match_case,
            payload.whole_word,
            payload.pages.as_deref(),
            &tmp_out,
        )?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "就地替换完成：{} 处，涉及 {} 页 / {} 个文本对象。原文件未改动。限制：不重排、不回流；替换文字变长时可能覆盖右侧内容，请用阅读器目视确认。",
                outcome.replacements, outcome.pages_touched, outcome.objects_modified
            ),
        ))
    })
}

// ---------------------------------------------------------------------------
// F19 按矩形精确替换（框选的那一处）
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn edit_text_at_rect(payload: RectEditRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "editTextRect", move |app, jid, flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("rect_edited.pdf");
        let ctx = ctx_new(app, jid, flag, true);
        let pdfium = render::load_pdfium(app)?;
        let rect = (
            payload.left.min(payload.right),
            payload.bottom.min(payload.top),
            payload.left.max(payload.right),
            payload.bottom.max(payload.top),
        );
        let outcome = extras::replace_text_at_rect(
            &ctx,
            pdfium,
            &input,
            payload.password.as_deref(),
            payload.page,
            rect,
            &payload.replace,
            payload.expected.as_deref(),
            &tmp_out,
        )?;
        let (_, out_str) = if payload.in_place {
            commit_one_chained(&input, &payload.output, &tmp_out)?
        } else {
            commit_one(&input, &payload.output, &tmp_out)?
        };
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "已按框选位置替换第 {} 页的「{}」→「{}」（{} 个字符 / {} 个文本对象）。这是按坐标定位，同页其它相同文字不会被改；仍不重排，替换文字变长可能覆盖右侧内容，请目视确认。原文件未改动，可继续框选别处接着改，结果会累积到同一份 _edited 文件。",
                outcome.page,
                outcome.old_text,
                payload.replace,
                outcome.selected_chars,
                outcome.objects_modified
            ),
        ))
    })
}

// ---------------------------------------------------------------------------
// F11 批注
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn annotate_pdf(payload: AnnotateRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "annotate", move |app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("annotated.pdf");
        let pdfium = render::load_pdfium(app)?;
        let outcome = extras::annotate(
            pdfium,
            &input,
            payload.password.as_deref(),
            &payload.find,
            &payload.kind,
            &payload.color,
            &payload.author,
            &payload.note,
            payload.match_case,
            payload.pages.as_deref(),
            &tmp_out,
        )?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "批注完成：{} 处匹配 / {} 个四边形 / {} 页，类型为 {}。注解为标准 PDF 注解字典，Adobe Reader / PDFgear 均可读取。",
                outcome.matches, outcome.quads, outcome.pages_touched, payload.kind
            ),
        ))
    })
}

// ---------------------------------------------------------------------------
// F12 页面管理
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn manage_pages(payload: PageOpsRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "pageOps", move |app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("pages.pdf");
        let mut margins = [0f32; 4];
        for (i, v) in payload.margins.iter().take(4).enumerate() {
            if *v < 0.0 {
                return Err(AppError::internal("裁剪边距不能为负数"));
            }
            margins[i] = *v as f32;
        }
        let summary = extras::page_ops(
            Some(app),
            &input,
            payload.password.as_deref(),
            &payload.mode,
            &payload.pages,
            payload.degrees,
            payload.insert_at,
            payload.insert_count,
            margins,
            &tmp_out,
            &tmp_dir,
        )?;
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "页面操作完成：{}。注意：重建页树后嵌套书签可能不随页号更新。原文件未改动。",
                summary
            ),
        ))
    })
}

// ---------------------------------------------------------------------------
// F13 水印 / 页眉页脚 / 页码 / 元数据 / 加密
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn decorate_pdf(payload: DecorateRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "decorate", move |app, jid, _flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_base = tmp_dir.join("decorated.pdf");
        // 中文水印走 PDFium + 系统 CJK 字体；页眉/页脚仍限 WinAnsi（明确拒绝）。
        let cjk_wm = !payload.watermark.trim().is_empty() && !payload.watermark.is_ascii();
        if !payload.header.is_ascii() || !payload.footer.is_ascii() {
            return Err(AppError::internal(
                "页眉/页脚仍只支持拉丁字符；中文水印请使用「水印文字」字段。",
            ));
        }
        let wm_for_lopdf = if cjk_wm { "" } else { payload.watermark.as_str() };
        let count = extras::decorate(
            &input,
            payload.password.as_deref(),
            wm_for_lopdf,
            payload.opacity as f32,
            payload.angle as f32,
            payload.watermark_size as f32,
            &payload.header,
            &payload.footer,
            payload.page_numbers,
            &payload.number_position,
            &payload.title,
            &payload.author,
            &payload.subject,
            &payload.keywords,
            &tmp_base,
        )?;

        let mut tmp_out = tmp_base;
        if cjk_wm {
            let pdfium = render::load_pdfium(app)?;
            let tmp_cjk = tmp_dir.join("cjk_watermark.pdf");
            more::cjk_watermark(
                pdfium,
                &tmp_out,
                None,
                &payload.watermark,
                payload.watermark_size as f32,
                &tmp_cjk,
            )?;
            tmp_out = tmp_cjk;
        }
        if !payload.user_password.trim().is_empty() {
            let tmp_enc = tmp_dir.join("encrypted.pdf");
            qpdf::encrypt(
                app,
                &tmp_out,
                &payload.user_password,
                &payload.owner_password,
                payload.allow_print,
                payload.allow_copy,
                payload.allow_modify,
                &tmp_enc,
            )?;
            tmp_out = tmp_enc;
        }
        let (_, out_str) = commit_one(&input, &payload.output, &tmp_out)?;
        let enc_note = if payload.user_password.trim().is_empty() {
            "未加密"
        } else {
            "已启用 AES-256 打开密码与权限位（请自行保存密码，本机不存储）"
        };
        let wm_note = if cjk_wm {
            "中文水印用系统 CJK 字体嵌入，水平居中灰字（不支持旋转/独立透明度）"
        } else {
            "叠加文字为 Helvetica/WinAnsi，仅支持拉丁字符"
        };
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "装饰完成：{} 页；{}。{}；不重排原版面。",
                count, enc_note, wm_note
            ),
        ))
    })
}

// ---------------------------------------------------------------------------
// F14 导出文字层
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn export_pdf(payload: ExportRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "export", move |app, jid, flag| {
        let input = require_file(&payload.file)?;
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join(format!("export.{}", payload.format));
        let ctx = ctx_new(app, jid, flag, true);
        let pdfium = render::load_pdfium(app)?;
        let (pages, chars) = extras::export_text(
            &ctx,
            pdfium,
            &input,
            payload.password.as_deref(),
            &payload.format,
            payload.pages.as_deref(),
            &tmp_out,
        )?;

        // 后缀与格式保持一致，避免用户拿到 .pdf 但实际是 docx。
        let mut requested = PathBuf::from(&payload.output);
        let wanted = payload.format.as_str();
        let ok_ext = requested
            .extension()
            .and_then(|s| s.to_str())
            .map(|e| e.eq_ignore_ascii_case(wanted))
            .unwrap_or(false);
        if !ok_ext {
            requested.set_extension(wanted);
        }
        let (_, out_str) = commit_one(&input, &requested.to_string_lossy(), &tmp_out)?;
        Ok(JobResult::ok(
            jid,
            vec![out_str],
            format!(
                "导出完成：{} 页 / {} 字符，格式 {}。诚实边界：仅导出文字层，不还原版式、表格与图片；复杂排版请人工校对。扫描件请先走 F5 OCR。",
                pages, chars, wanted.to_ascii_uppercase()
            ),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "payoncepdf_commit_{}_{}",
            std::process::id(),
            tag
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 连续编辑的正常路径：输入就是上一轮的 `*_edited.pdf`，允许原地替换。
    #[test]
    fn chained_commit_overwrites_own_edited_output() {
        let dir = tmpdir("chained_ok");
        let edited = dir.join("doc_edited.pdf");
        std::fs::write(&edited, b"round-1").unwrap();
        let tmp = dir.join("tmp_ok.pdf");
        std::fs::write(&tmp, b"round-2").unwrap();

        let (out, _) = commit_one_chained(&edited, &p2s(&edited), &tmp).unwrap();
        assert_eq!(out, edited);
        assert_eq!(std::fs::read(&edited).unwrap(), b"round-2");
    }

    /// 安全边界：即使用户把自己的原件当成输出，也绝不允许覆盖。
    #[test]
    fn chained_commit_refuses_user_original() {
        let dir = tmpdir("chained_deny");
        let orig = dir.join("report.pdf");
        std::fs::write(&orig, b"user-file").unwrap();
        let tmp = dir.join("tmp_deny.pdf");
        std::fs::write(&tmp, b"new").unwrap();

        let e = commit_one_chained(&orig, &p2s(&orig), &tmp).unwrap_err();
        assert!(
            format!("{e}").contains("_edited"),
            "拒绝原因应指明只允许覆盖 _edited 产物：{e}"
        );
        assert_eq!(std::fs::read(&orig).unwrap(), b"user-file", "原件必须一字未动");
    }

    /// 文件名识别的边界：只认本工具的 `_edited` / `_edited_N` 形态。
    #[test]
    fn edited_stem_detection() {
        for ok in ["doc_edited", "报告_edited", "doc_edited_1", "doc_edited_12"] {
            assert!(looks_like_edited_stem(ok), "应识别为产物：{ok}");
        }
        for no in ["report", "doc", "_edited", "doc_edited_x", "doc_edited_", "edited"] {
            assert!(!looks_like_edited_stem(no), "不应识别为产物：{no}");
        }
    }

    /// 普通路径不受影响：输入输出不同名时仍按既有规则另存（重名自增）。
    #[test]
    fn chained_commit_falls_back_for_new_output() {
        let dir = tmpdir("chained_diff");
        let src = dir.join("doc.pdf");
        std::fs::write(&src, b"src").unwrap();
        let tmp = dir.join("tmp_diff.pdf");
        std::fs::write(&tmp, b"out").unwrap();
        let wanted = dir.join("doc_edited.pdf");

        let (out, _) = commit_one_chained(&src, &p2s(&wanted), &tmp).unwrap();
        assert_eq!(out, wanted);
        assert_eq!(std::fs::read(&src).unwrap(), b"src");
    }
}
