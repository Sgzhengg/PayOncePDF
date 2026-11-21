//! 任务命令：F1–F7 + 取消。所有导出命令统一经过 `execute()`，
//! 其第一行即 `license::ensure_export_allowed()`（CONTRACT §1 硬性要求）。

use crate::commands::{
    BatchRequest, CompressRequest, ImagesToPdfRequest, JobResult, MergeRequest, OcrRequest,
    SignRequest, SplitRequest, ToImagesRequest,
};
use crate::engine::{self, extras, ocr, objects, pdf, qpdf, render, JobCtx};
use crate::error::{AppError, AppResult};
use crate::state::{self, AppState};
use crate::license;
use crate::logging;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tauri::{AppHandle, Manager};

pub(crate) fn p2s(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

fn emit_failure(app: &AppHandle, job_id: &str, e: &AppError) {
    state::emit_finished(
        app,
        JobResult {
            job_id: job_id.to_string(),
            ok: false,
            outputs: vec![],
            message: e.to_string(),
            report_path: None,
            elapsed_ms: Some(0),
        },
    );
}

/// 所有导出命令的统一外壳：
/// - 授权校验（到期 E2005，绝不触碰用户文件）
/// - 注册取消标志 + 建立临时目录
/// - 成功/失败都推送 `payoncepdf://finished`
/// - 清理临时目录、注销任务、写本地日志（时间/操作/错误码）
pub(crate) fn execute<F>(app: &AppHandle, job_id: &str, op: &str, work: F) -> Result<JobResult, AppError>
where
    F: FnOnce(&AppHandle, &str, &AtomicBool) -> AppResult<JobResult>,
{
    if let Err(e) = license::ensure_export_allowed(app) {
        emit_failure(app, job_id, &e);
        logging::record(op, &e.code);
        return Err(e);
    }

    let started = Instant::now();
    let flag = {
        let st = app.state::<AppState>();
        st.register(job_id)
    };
    let _ = state::create_temp_dir(job_id);

    let res = work(app, job_id, &flag);
    let elapsed = started.elapsed().as_millis() as u64;

    let code: String;
    let final_res = match res {
        Ok(mut r) => {
            r.elapsed_ms = Some(elapsed);
            code = "OK".to_string();
            state::emit_finished(app, r.clone());
            Ok(r)
        }
        Err(e) => {
            code = e.code.clone();
            emit_failure(app, job_id, &e);
            Err(e)
        }
    };

    state::cleanup_temp_dir(job_id);
    {
        let st = app.state::<AppState>();
        st.finish(job_id);
    }
    logging::record(op, &code);
    final_res
}

pub(crate) fn ctx_new<'a>(
    app: &'a AppHandle,
    job_id: &'a str,
    cancel: &'a AtomicBool,
    emit: bool,
) -> JobCtx<'a> {
    JobCtx {
        app: Some(app),
        job_id,
        cancel,
        emit,
    }
}

/// 渲染拆分/图片输出的文件名模板。
fn render_name(template: &str, name: &str, index: usize) -> String {
    let t = template
        .replace("{name}", name)
        .replace("{index}", &index.to_string());
    let t = t
        .replace('/', "_")
        .replace('\\', "_")
        .replace("..", "_");
    if t.to_ascii_lowercase().ends_with(".pdf") {
        t
    } else {
        format!("{}.pdf", t)
    }
}

// ---------------------------------------------------------------------------
// F1 合并
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn merge_pdfs(payload: MergeRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "merge", move |app, jid, flag| {
        let inputs: Vec<PathBuf> = payload.files.iter().map(PathBuf::from).collect();
        if inputs.is_empty() {
            return Err(AppError::internal("没有待合并的文件"));
        }
        for f in &inputs {
            if !f.exists() {
                return Err(AppError::parse(format!("输入文件不存在：{}", f.display())));
            }
        }
        let pwd = payload.password.as_deref();

        // 逐文件页数校验（含 >5000 拒绝）+ 100 文件进度事件。
        let total = inputs.len();
        let mut expected: u32 = 0;
        for (i, f) in inputs.iter().enumerate() {
            state::check_cancel(flag)?;
            state::emit_progress(
                app,
                jid,
                i + 1,
                total,
                format!("校验页数 {}/{}", i + 1, total),
            );
            let n = qpdf::page_count(app, f, pwd)?; // qpdf 缺失 → E2001
            if n > 5000 {
                return Err(AppError::page_limit("单文件超过 5000 页，已拒绝"));
            }
            expected = expected.saturating_add(n);
        }

        let requested = PathBuf::from(&payload.output);
        state::assert_not_overwriting_inputs(
            &requested,
            &inputs.iter().map(|p| p.as_path()).collect::<Vec<_>>(),
        )?;

        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("merge_out.pdf");
        qpdf::merge(app, &inputs, pwd, &tmp_out, &tmp_dir)?;

        // 输出页数校验：必须等于输入页数之和。
        let actual = qpdf::page_count(app, &tmp_out, None)?;
        if actual != expected {
            return Err(AppError::internal(format!(
                "合并后页数 {} 与输入页数之和 {} 不一致",
                actual, expected
            )));
        }

        let final_path = state::resolve_output_path(&requested);

        // qpdf 合并产物没有 /Outlines。勾选了「保留书签」就必须真把各源文件书签
        // 按页数偏移重建进去，失败要报错，不能给个无书签的文件还报成功。
        let (staged, bookmark_note) = if payload.keep_bookmarks {
            let bm_out = tmp_dir.join("merge_bookmarked.pdf");
            let (_, summary) = objects::merge_bookmarks(&inputs, pwd, &tmp_out, &bm_out)?;
            (bm_out, format!("；{}", summary))
        } else {
            (tmp_out, String::new())
        };
        state::atomic_commit(&staged, &final_path)?;

        let msg = format!(
            "合并完成：{} 个文件，共 {} 页{}",
            inputs.len(),
            actual,
            bookmark_note
        );
        Ok(JobResult::ok(jid, vec![p2s(&final_path)], msg))
    })
}

// ---------------------------------------------------------------------------
// F2 拆分
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn split_pdf(payload: SplitRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "split", move |app, jid, flag| {
        let input = PathBuf::from(&payload.file);
        if !input.exists() {
            return Err(AppError::parse("输入文件不存在"));
        }
        let pwd = payload.password.as_deref();
        let total = qpdf::page_count(app, &input, pwd)?;
        if total > 5000 {
            return Err(AppError::page_limit("单文件超过 5000 页，已拒绝"));
        }

        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output")
            .to_string();
        let template = if payload.name_template.trim().is_empty() {
            "{name}_{index}.pdf".to_string()
        } else {
            payload.name_template.clone()
        };
        let out_dir = PathBuf::from(&payload.output_dir);
        std::fs::create_dir_all(&out_dir).map_err(AppError::from)?;

        // 每个元素是一个输出文件对应的 --pages 页码 token 组。
        let mut chunks: Vec<Vec<String>> = Vec::new();
        match payload.mode.as_str() {
            "everyN" => {
                let n = payload
                    .n
                    .ok_or_else(|| AppError::internal("everyN 模式缺少参数 n"))?;
                if n == 0 {
                    return Err(AppError::internal("n 必须大于 0"));
                }
                let mut s: u32 = 1;
                while s <= total {
                    let e = (s + n - 1).min(total);
                    chunks.push(vec![format!("{}-{}", s, e)]);
                    s = e + 1;
                }
            }
            "ranges" => {
                // 约定：逗号分隔的每个范围单独输出一个文件。
                // 例如 "1-5,8,11-13" → {name}_1.pdf(1-5) / _2.pdf(8) / _3.pdf(11-13)。
                let spec = payload
                    .ranges
                    .as_deref()
                    .ok_or_else(|| AppError::internal("ranges 模式缺少参数 ranges"))?;
                for t in qpdf::parse_range_tokens(spec)? {
                    chunks.push(vec![t]);
                }
            }
            "bookmarks" => {
                // 用 lopdf 读 /Outlines 顶层标题作为切点；读不到明确报错。
                let points = pdf::outline_split_points(&input, pwd)?;
                for i in 0..points.len() {
                    let start = points[i].1;
                    let end = if i + 1 < points.len() {
                        points[i + 1].1.saturating_sub(1)
                    } else {
                        total
                    };
                    if start <= end {
                        chunks.push(vec![format!("{}-{}", start, end)]);
                    }
                }
            }
            other => {
                return Err(AppError::internal(format!("未知拆分模式：{}", other)));
            }
        }
        if chunks.is_empty() {
            return Err(AppError::internal("没有产生任何拆分片段"));
        }

        let tmp_dir = state::temp_dir_for(jid);
        let mut outputs: Vec<String> = Vec::new();
        let n = chunks.len();
        for (i, ranges) in chunks.iter().enumerate() {
            state::check_cancel(flag)?;
            state::emit_progress(app, jid, i + 1, n, format!("拆分 {}/{}", i + 1, n));
            let filename = render_name(&template, &stem, i + 1);
            let final_path = state::resolve_output_path(&out_dir.join(&filename));
            let tmp_out = tmp_dir.join(format!("split_{}.pdf", i + 1));
            qpdf::slice(app, &input, pwd, ranges, &tmp_out, &tmp_dir)?;
            state::atomic_commit(&tmp_out, &final_path)?;
            outputs.push(p2s(&final_path));
        }
        Ok(JobResult::ok(
            jid,
            outputs,
            format!("拆分完成：{} 个文件（源 {} 页）", n, total),
        ))
    })
}

// ---------------------------------------------------------------------------
// F3 压缩
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn compress_pdf(payload: CompressRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "compress", move |app, jid, flag| {
        let input = PathBuf::from(&payload.file);
        if !input.exists() {
            return Err(AppError::parse("输入文件不存在"));
        }
        let tmp_dir = state::temp_dir_for(jid);
        let ctx = ctx_new(app, jid, flag, true);
        let outcome = pdf::compress(
            &ctx,
            &input,
            payload.password.as_deref(),
            payload.target_bytes,
            &payload.quality,
            &tmp_dir,
        )?;

        let requested = PathBuf::from(&payload.output);
        state::assert_not_overwriting_inputs(&requested, &[input.as_path()])?;
        let final_path = state::resolve_output_path(&requested);
        state::atomic_commit(&outcome.path, &final_path)?;

        let msg = if outcome.reached {
            format!(
                "压缩完成：实际 {} 字节（目标 {} 字节），{}，重编码 {} 张图像；文字层与书签保持不变",
                outcome.actual_bytes,
                payload.target_bytes,
                outcome.quality_label,
                outcome.image_count
            )
        } else {
            format!(
                "未能压到目标：目标 {} 字节，实际最小 {} 字节，{}；文字层与书签保持不变",
                payload.target_bytes, outcome.actual_bytes, outcome.quality_label
            )
        };
        Ok(JobResult::ok(jid, vec![p2s(&final_path)], msg))
    })
}

// ---------------------------------------------------------------------------
// F4 PDF ⇄ 图片
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn pdf_to_images(payload: ToImagesRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "toImages", move |app, jid, flag| {
        let input = PathBuf::from(&payload.file);
        if !input.exists() {
            return Err(AppError::parse("输入文件不存在"));
        }
        let doc = render::open(app, &input, payload.password.as_deref())?; // 缺 PDFium → E2002
        let total = doc.page_count();
        if total > 5000 {
            return Err(AppError::page_limit("单文件超过 5000 页，已拒绝"));
        }
        let selection = engine::page_selection(payload.pages.as_deref(), total)?;
        let out_dir = PathBuf::from(&payload.output_dir);
        std::fs::create_dir_all(&out_dir).map_err(AppError::from)?;
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("page")
            .to_string();
        let ext = if payload.format.eq_ignore_ascii_case("jpg") {
            "jpg"
        } else {
            "png"
        };
        let tmp_dir = state::temp_dir_for(jid);

        let n = selection.len();
        let mut outputs: Vec<String> = Vec::new();
        for (i, page_no) in selection.iter().enumerate() {
            state::check_cancel(flag)?;
            state::emit_progress(app, jid, i + 1, n, format!("渲染 {}/{}", i + 1, n));
            let filename = format!("{}_p{}.{}", stem, page_no, ext);
            let final_path = state::resolve_output_path(&out_dir.join(&filename));
            let tmp_out = tmp_dir.join(format!("page_{}.{}", page_no, ext));
            doc.render_page(*page_no, payload.dpi, &tmp_out, &payload.format, 85)?;
            state::atomic_commit(&tmp_out, &final_path)?;
            outputs.push(p2s(&final_path));
        }
        Ok(JobResult::ok(
            jid,
            outputs,
            format!("导出 {} 张图片（{} DPI，{}）", n, payload.dpi, payload.format),
        ))
    })
}

#[tauri::command]
pub fn images_to_pdf(payload: ImagesToPdfRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "imagesToPdf", move |app, jid, flag| {
        let files: Vec<PathBuf> = payload.files.iter().map(PathBuf::from).collect();
        if files.is_empty() {
            return Err(AppError::internal("没有待转换的图片"));
        }
        for f in &files {
            if !f.exists() {
                return Err(AppError::parse(format!("图片不存在：{}", f.display())));
            }
        }
        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("images_to_pdf_out.pdf");
        let ctx = ctx_new(app, jid, flag, true);
        pdf::images_to_pdf(&ctx, &files, &payload.page_size, payload.margin, &tmp_out)?;

        let requested = PathBuf::from(&payload.output);
        state::assert_not_overwriting_inputs(
            &requested,
            &files.iter().map(|p| p.as_path()).collect::<Vec<_>>(),
        )?;
        let final_path = state::resolve_output_path(&requested);
        state::atomic_commit(&tmp_out, &final_path)?;
        Ok(JobResult::ok(
            jid,
            vec![p2s(&final_path)],
            format!("图片转 PDF 完成：{} 页（按文件名排序）", files.len()),
        ))
    })
}

// ---------------------------------------------------------------------------
// F5 OCR
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn ocr_pdf(payload: OcrRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "ocr", move |app, jid, flag| {
        let input = PathBuf::from(&payload.file);
        if !input.exists() {
            return Err(AppError::parse("输入文件不存在"));
        }
        let tmp_dir = state::temp_dir_for(jid);
        let ctx = ctx_new(app, jid, flag, true);
        let result_tmp = ocr::run(
            &ctx,
            &input,
            payload.password.as_deref(),
            &payload.output_format,
            &payload.languages,
            payload.pages.as_deref(),
            &tmp_dir,
        )?;

        let requested = PathBuf::from(&payload.output);
        state::assert_not_overwriting_inputs(&requested, &[input.as_path()])?;
        let final_path = state::resolve_output_path(&requested);
        state::atomic_commit(&result_tmp, &final_path)?;
        Ok(JobResult::ok(
            jid,
            vec![p2s(&final_path)],
            format!("OCR 完成（输出格式 {}）", payload.output_format),
        ))
    })
}

// ---------------------------------------------------------------------------
// F6 签名 / 表单
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn sign_pdf(payload: SignRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "sign", move |app, jid, flag| {
        let input = PathBuf::from(&payload.file);
        let sig = PathBuf::from(&payload.signature_path);
        if !input.exists() {
            return Err(AppError::parse("输入文件不存在"));
        }
        if !sig.exists() {
            return Err(AppError::parse("签名图片不存在"));
        }
        let ctx = ctx_new(app, jid, flag, true);
        ctx.tick(1, 1, "签名与表单处理")?;

        let tmp_dir = state::temp_dir_for(jid);
        let tmp_out = tmp_dir.join("signed.pdf");
        pdf::sign(
            &input,
            payload.password.as_deref(),
            &sig,
            payload.page,
            payload.x,
            payload.y,
            payload.width,
            payload.height,
            &payload.fields,
            payload.flatten_fields,
            &tmp_out,
        )?;

        let requested = PathBuf::from(&payload.output);
        state::assert_not_overwriting_inputs(&requested, &[input.as_path(), sig.as_path()])?;
        let final_path = state::resolve_output_path(&requested);
        state::atomic_commit(&tmp_out, &final_path)?;

        let msg = if payload.flatten_fields {
            "签名完成；表单已扁平化。外观流用标准 Helvetica，支持英文/数字/Latin-1 符号；含中文等值会明确拒绝，不会静默变成问号"
        } else {
            "签名完成；表单字段已写入 /V"
        };
        Ok(JobResult::ok(jid, vec![p2s(&final_path)], msg))
    })
}

// ---------------------------------------------------------------------------
// F7 批量
// ---------------------------------------------------------------------------

struct ReportRow {
    filename: String,
    elapsed_ms: u64,
    result: String,
    error_code: String,
    output: String,
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn opt_str<'a>(options: &'a serde_json::Value, key: &str, default: &'a str) -> &'a str {
    options.get(key).and_then(|v| v.as_str()).unwrap_or(default)
}

fn opt_u64(options: &serde_json::Value, key: &str, default: u64) -> u64 {
    options.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
}

fn opt_bool(options: &serde_json::Value, key: &str, default: bool) -> bool {
    options
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

fn opt_f64(options: &serde_json::Value, key: &str, default: f64) -> f64 {
    options.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
}

/// 单个文件的批量操作，返回该文件的输出路径（或错误码，由调用方记录）。
#[allow(clippy::too_many_arguments)]
fn run_batch_one(
    app: &AppHandle,
    jid: &str,
    flag: &AtomicBool,
    operation: &str,
    options: &serde_json::Value,
    file: &Path,
    output_dir: &Path,
    work_dir: &Path,
) -> AppResult<String> {
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ctx = ctx_new(app, jid, flag, false); // 批量进度由上层统一发

    match operation {
        "compress" => {
            let target = opt_u64(options, "targetBytes", 5 * 1024 * 1024);
            let quality = opt_str(options, "quality", "auto");
            let outcome = pdf::compress(&ctx, file, None, target, quality, work_dir)?;
            let final_path =
                state::resolve_output_path(&output_dir.join(format!("{}_compressed.pdf", stem)));
            state::atomic_commit(&outcome.path, &final_path)?;
            Ok(p2s(&final_path))
        }
        "ocr" => {
            let of = opt_str(options, "outputFormat", "pdf");
            let langs: Vec<String> = options
                .get("languages")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| vec!["eng".to_string()]);
            let pages = options.get("pages").and_then(|v| v.as_str());
            let res = ocr::run(&ctx, file, None, of, &langs, pages, work_dir)?;
            let ext = if of.eq_ignore_ascii_case("pdf") { "pdf" } else { "txt" };
            let final_path =
                state::resolve_output_path(&output_dir.join(format!("{}_ocr.{}", stem, ext)));
            state::atomic_commit(&res, &final_path)?;
            Ok(p2s(&final_path))
        }
        "toImages" => {
            let format = opt_str(options, "format", "png");
            let dpi = opt_u64(options, "dpi", 150) as u32;
            let pages = options.get("pages").and_then(|v| v.as_str());
            let doc = render::open(app, file, None)?;
            let total = doc.page_count();
            let selection = engine::page_selection(pages, total)?;
            let img_dir = output_dir.join(&stem);
            std::fs::create_dir_all(&img_dir).map_err(AppError::from)?;
            let ext = if format.eq_ignore_ascii_case("jpg") { "jpg" } else { "png" };
            for p in selection {
                state::check_cancel(flag)?;
                let final_path = state::resolve_output_path(
                    &img_dir.join(format!("{}_p{}.{}", stem, p, ext)),
                );
                let tmp = work_dir.join(format!("p{}.{}", p, ext));
                doc.render_page(p, dpi, &tmp, format, 85)?;
                state::atomic_commit(&tmp, &final_path)?;
            }
            Ok(p2s(&img_dir))
        }
        "merge" => {
            // 合并由上层统一执行，这里只做存在性登记。
            let _ = ctx;
            Ok("__MERGE_PENDING__".to_string())
        }
        // F10–F14：批量化是相对 PDFgear 桌面版的真实优势（第三方评测明确列其批量/自动化偏弱）。
        // 单文件失败只记错误码，不中断队列，仍写 report.csv。
        "edit" => {
            let tmp = work_dir.join("edited.pdf");
            let pdfium = render::load_pdfium(app)?;
            extras::replace_text(
                &ctx,
                pdfium,
                file,
                None,
                opt_str(options, "find", ""),
                opt_str(options, "replace", ""),
                opt_bool(options, "matchCase", false),
                opt_bool(options, "wholeWord", false),
                options.get("pages").and_then(|v| v.as_str()),
                &tmp,
            )?;
            let final_path =
                state::resolve_output_path(&output_dir.join(format!("{}_edited.pdf", stem)));
            state::atomic_commit(&tmp, &final_path)?;
            Ok(p2s(&final_path))
        }
        "annotate" => {
            let tmp = work_dir.join("annotated.pdf");
            let pdfium = render::load_pdfium(app)?;
            extras::annotate(
                pdfium,
                file,
                None,
                opt_str(options, "find", ""),
                opt_str(options, "kind", "highlight"),
                opt_str(options, "color", "#FFD54F"),
                opt_str(options, "author", ""),
                opt_str(options, "note", ""),
                opt_bool(options, "matchCase", false),
                options.get("pages").and_then(|v| v.as_str()),
                &tmp,
            )?;
            let final_path =
                state::resolve_output_path(&output_dir.join(format!("{}_annotated.pdf", stem)));
            state::atomic_commit(&tmp, &final_path)?;
            Ok(p2s(&final_path))
        }
        "export" => {
            let format = opt_str(options, "format", "txt");
            let tmp = work_dir.join(format!("export.{}", format));
            let pdfium = render::load_pdfium(app)?;
            extras::export_text(
                &ctx,
                pdfium,
                file,
                None,
                format,
                options.get("pages").and_then(|v| v.as_str()),
                &tmp,
            )?;
            let final_path = state::resolve_output_path(
                &output_dir.join(format!("{}_export.{}", stem, format)),
            );
            state::atomic_commit(&tmp, &final_path)?;
            Ok(p2s(&final_path))
        }
        "decorate" => {
            let tmp = work_dir.join("decorated.pdf");
            extras::decorate(
                file,
                None,
                opt_str(options, "watermark", ""),
                opt_f64(options, "opacity", 0.25) as f32,
                opt_f64(options, "angle", 45.0) as f32,
                opt_f64(options, "watermarkSize", 48.0) as f32,
                opt_str(options, "header", ""),
                opt_str(options, "footer", ""),
                opt_bool(options, "pageNumbers", false),
                opt_str(options, "numberPosition", "footerCenter"),
                opt_str(options, "title", ""),
                opt_str(options, "author", ""),
                opt_str(options, "subject", ""),
                opt_str(options, "keywords", ""),
                &tmp,
            )?;
            let final_path =
                state::resolve_output_path(&output_dir.join(format!("{}_decorated.pdf", stem)));
            state::atomic_commit(&tmp, &final_path)?;
            Ok(p2s(&final_path))
        }
        "pages" => {
            let tmp = work_dir.join("pages.pdf");
            extras::page_ops(
                Some(app),
                file,
                None,
                opt_str(options, "mode", "rotate"),
                opt_str(options, "pages", "1"),
                opt_u64(options, "degrees", 90) as i64,
                opt_u64(options, "insertAt", 1) as u32,
                opt_u64(options, "insertCount", 1) as u32,
                [0.0, 0.0, 0.0, 0.0],
                &tmp,
                work_dir,
            )?;
            let final_path =
                state::resolve_output_path(&output_dir.join(format!("{}_pages.pdf", stem)));
            state::atomic_commit(&tmp, &final_path)?;
            Ok(p2s(&final_path))
        }
        other => Err(AppError::internal(format!("未知批量操作：{}", other))),
    }
}

#[tauri::command]
pub fn batch_run(payload: BatchRequest, app: AppHandle) -> Result<JobResult, AppError> {
    let job_id = payload.job_id.clone();
    execute(&app, &job_id, "batch", move |app, jid, flag| {
        let input_dir = PathBuf::from(&payload.input_dir);
        let output_dir = PathBuf::from(&payload.output_dir);
        if !input_dir.is_dir() {
            return Err(AppError::internal("输入目录不存在"));
        }
        std::fs::create_dir_all(&output_dir).map_err(AppError::from)?;
        let recursive = payload
            .options
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 收集 PDF：非递归时 max_depth(1)，递归时不限制；按路径排序保证确定性。
        let mut files: Vec<PathBuf> = Vec::new();
        let mut walker = walkdir::WalkDir::new(&input_dir);
        if !recursive {
            walker = walker.max_depth(1);
        }
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let p = entry.into_path();
                let is_pdf = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("pdf"))
                    .unwrap_or(false);
                if is_pdf {
                    files.push(p);
                }
            }
        }
        files.sort();

        let total = files.len();
        let mut rows: Vec<ReportRow> = Vec::new();
        let mut ok_outputs: Vec<String> = Vec::new();
        let tmp_root = state::temp_dir_for(jid);

        for (i, f) in files.iter().enumerate() {
            state::check_cancel(flag)?;
            state::emit_progress(app, jid, i + 1, total, format!("批量 {}/{}", i + 1, total));
            let filename = f
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let work_dir = tmp_root.join(format!("item_{}", i));
            let _ = std::fs::create_dir_all(&work_dir);
            let start = Instant::now();

            // 单个失败只记录错误码，不中断队列。
            let res = run_batch_one(
                app,
                jid,
                flag,
                &payload.operation,
                &payload.options,
                f,
                &output_dir,
                &work_dir,
            );
            let elapsed = start.elapsed().as_millis() as u64;
            match res {
                Ok(out) => {
                    // merge 操作逐文件只是登记占位，等下面统一合并后回填输出。
                    if out != "__MERGE_PENDING__" {
                        ok_outputs.push(out.clone());
                    }
                    rows.push(ReportRow {
                        filename,
                        elapsed_ms: elapsed,
                        result: "ok".to_string(),
                        error_code: String::new(),
                        output: out,
                    });
                }
                Err(e) => rows.push(ReportRow {
                    filename,
                    elapsed_ms: elapsed,
                    result: "failed".to_string(),
                    error_code: e.code.clone(),
                    output: String::new(),
                }),
            }
        }

        // merge：把所有输入合成单个文件，并把结果回填到各文件行。
        if payload.operation == "merge" && !files.is_empty() {
            let merged_tmp = tmp_root.join("batch_merged.pdf");
            let merge_res = qpdf::merge(app, &files, None, &merged_tmp, &tmp_root);
            match merge_res {
                Ok(()) => {
                    let final_path =
                        state::resolve_output_path(&output_dir.join("batch_merged.pdf"));
                    state::atomic_commit(&merged_tmp, &final_path)?;
                    let out = p2s(&final_path);
                    ok_outputs.push(out.clone());
                    for r in rows.iter_mut() {
                        r.result = "ok".to_string();
                        r.error_code.clear();
                        r.output = out.clone();
                    }
                }
                Err(e) => {
                    for r in rows.iter_mut() {
                        r.result = "failed".to_string();
                        r.error_code = e.code.clone();
                        r.output.clear();
                    }
                }
            }
        }

        // 写 report.csv（列：filename,elapsed_ms,result,error_code,output）。
        let mut csv = String::from("filename,elapsed_ms,result,error_code,output\n");
        for r in &rows {
            csv.push_str(&format!(
                "{},{},{},{},{}\n",
                csv_escape(&r.filename),
                r.elapsed_ms,
                r.result,
                csv_escape(&r.error_code),
                csv_escape(&r.output)
            ));
        }
        let report_path = output_dir.join("report.csv");
        let tmp_report = tmp_root.join("report.csv");
        std::fs::write(&tmp_report, csv).map_err(AppError::from)?;
        // report.csv 是本工具生成的报告，允许覆盖自身上一份（非用户文件）。
        if report_path.exists() {
            let _ = std::fs::remove_file(&report_path);
        }
        state::atomic_commit(&tmp_report, &report_path)?;

        let failed = rows.iter().filter(|r| r.result == "failed").count();
        let mut result = JobResult::ok(
            jid,
            ok_outputs,
            format!(
                "批量完成：共 {} 个文件，成功 {}，失败 {}（详见 report.csv）",
                total,
                total.saturating_sub(failed),
                failed
            ),
        );
        result.report_path = Some(p2s(&report_path));
        Ok(result)
    })
}
