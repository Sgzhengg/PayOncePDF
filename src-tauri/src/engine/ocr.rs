//! F5 本地 OCR（PRD §7 F5；Tesseract Apache-2.0）。
//!
//! 流程：PDFium 渲染页 → 300DPI 临时 PNG → tesseract 逐页产出 txt / 可搜索 PDF
//! → 多页 PDF 用 qpdf 合并成单个可搜索 PDF（文字层叠加），txt 则按页拼接。
//! 缺语言包 → E1006；缺 tesseract sidecar → E2001；缺 PDFium → E2002。
//! 全程本地，绝不上传任何文件内容。

use crate::engine::{qpdf, render, sidecar, JobCtx};
use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Windows 上 `app.path().resource_dir()` 可能返回 `\\?\` 扩展长度路径；
/// tesseract 内部用 `/` 拼接 `<tessdata>/<lang>.traineddata`，而在 `\\?\` 路径下
/// Windows 不接受正斜杠，会误报 E1006「语言包缺失」。传给外部工具前去掉前缀，
/// 路径本身仍然指向同一目录（普通 Win32 绝对路径）。F21 自检首次运行发现此问题。
#[cfg(target_os = "windows")]
fn tessdata_arg_for_external(p: &Path) -> PathBuf {
    let raw = p.to_string_lossy();
    PathBuf::from(raw.strip_prefix(r"\\?\").unwrap_or(raw.as_ref()))
}

#[cfg(not(target_os = "windows"))]
fn tessdata_arg_for_external(p: &Path) -> PathBuf {
    p.to_path_buf()
}

fn run_tesseract(
    app: &tauri::AppHandle,
    bin: &Path,
    tessdata: &Path,
    img: &Path,
    outbase: &Path,
    languages: &[String],
    want_pdf: bool,
) -> AppResult<PathBuf> {
    let langs = languages.join("+");
    let tessdata_arg = tessdata_arg_for_external(tessdata);
    let mut cmd = Command::new(bin);
    cmd.arg(img)
        .arg(outbase)
        .arg("--tessdata-dir")
        .arg(&tessdata_arg)
        .arg("-l")
        .arg(&langs);
    if want_pdf {
        cmd.arg("pdf");
    }
    // txt 不传 config，tesseract 默认即输出 .txt。
    sidecar::apply_child_env(app, &mut cmd);
    let output = cmd
        .output()
        .map_err(|e| AppError::internal(format!("无法启动 tesseract：{}", e)))?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        if stderr.to_ascii_lowercase().contains("traineddata")
            || stderr.to_ascii_lowercase().contains("language")
        {
            return Err(AppError::ocr_lang(format!(
                "OCR 语言包缺失或不可用：{}",
                stderr.trim()
            )));
        }
        return Err(AppError::internal(format!(
            "tesseract 执行失败：{}",
            stderr.trim()
        )));
    }
    let produced = if want_pdf {
        outbase.with_extension("pdf")
    } else {
        outbase.with_extension("txt")
    };
    if !produced.exists() {
        return Err(AppError::internal(format!(
            "tesseract 未生成预期输出 {}",
            produced.display()
        )));
    }
    Ok(produced)
}

/// 执行 OCR，返回写入到 `tmp_dir` 的临时结果文件路径（由上层做原子提交）。
pub fn run(
    ctx: &JobCtx<'_>,
    pdf: &Path,
    password: Option<&str>,
    output_format: &str,
    languages: &[String],
    pages: Option<&str>,
    tmp_dir: &Path,
) -> AppResult<PathBuf> {
    let want_pdf = output_format.eq_ignore_ascii_case("pdf");
    let app = ctx.app()?;

    // 语言包校验（缺任意一个 → E1006）。
    let langs: Vec<String> = if languages.is_empty() {
        vec!["eng".to_string()]
    } else {
        languages.to_vec()
    };
    for l in &langs {
        sidecar::check_language(app, l)?;
    }
    let tessdata = sidecar::tessdata_dir(app)?;
    let tesseract = sidecar::tesseract_path(app)?;

    // 打开文档、确定页数。
    let doc = render::open(app, pdf, password)?;
    let total = doc.page_count();
    if total > 5000 {
        return Err(AppError::page_limit("单文件超过 5000 页，已拒绝"));
    }
    let selection = crate::engine::page_selection(pages, total)?;
    let n = selection.len();

    let mut per_page: Vec<PathBuf> = Vec::with_capacity(n);
    for (i, page_no) in selection.iter().enumerate() {
        ctx.tick(i + 1, n, format!("OCR 第 {}/{} 页", i + 1, n))?;

        let png = tmp_dir.join(format!("ocr_page_{}.png", page_no));
        doc.render_page(*page_no, 300, &png, "png", 90)?;

        let outbase = tmp_dir.join(format!("ocr_page_{}", page_no));
        let produced = run_tesseract(
            app,
            &tesseract,
            &tessdata,
            &png,
            &outbase,
            &langs,
            want_pdf,
        )?;
        per_page.push(produced);
    }

    if want_pdf {
        // 多页可搜索 PDF：用 qpdf 把逐页 PDF 合并（文字层随页保留）。
        let merged = tmp_dir.join("ocr_result.pdf");
        qpdf::merge(app, &per_page, None, &merged, tmp_dir)?;
        Ok(merged)
    } else {
        // 多页 TXT：按页拼接，带分页标记。
        let merged = tmp_dir.join("ocr_result.txt");
        let mut buf = String::new();
        for (i, p) in per_page.iter().enumerate() {
            let text = std::fs::read_to_string(p).map_err(AppError::from)?;
            buf.push_str(&format!("----- 第 {} 页 -----\n", i + 1));
            buf.push_str(&text);
            buf.push_str("\n\n");
        }
        std::fs::write(&merged, buf).map_err(AppError::from)?;
        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn tessdata_arg_drops_verbatim_prefix() {
        let p = PathBuf::from(r"\\?\E:\app\resources\tessdata");
        let got = tessdata_arg_for_external(&p);
        assert_eq!(got, PathBuf::from(r"E:\app\resources\tessdata"));
    }

    #[test]
    fn tessdata_arg_keeps_normal_path() {
        let p = PathBuf::from("resources/tessdata");
        assert_eq!(tessdata_arg_for_external(&p), p);
    }
}
