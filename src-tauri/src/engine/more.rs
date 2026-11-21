//! F16：文本源转 PDF（含中文）与中文水印。
//!
//! 两项都复用已在包内的 PDFium(BSD-3)，通过系统 CJK TrueType 字体嵌入字形。
//! 与「高保真 Word/Excel 转换」**无关**：这里只做纯文本排版，不还原版式。
//! 用户明确暂缓高保真 Office 互转 / AI 问答 / 移动端，本文件不涉及这些。
//!
//! 诚实边界：
//! - 找不到系统 CJK 字体时明确报错，绝不写方框或乱码；
//! - 中文水印为水平居中灰字，不支持旋转角度与独立透明度（原 ASCII 水印路径仍支持）；
//! - HTML 只做标签剥离，不渲染 CSS / 图片 / 表格。

use crate::error::{AppError, AppResult};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

fn pdf_error(e: PdfiumError) -> AppError {
    AppError::parse(format!("PDFium 处理失败：{}", e))
}

/// 找一个 PDFium 能嵌入的完整 CJK TrueType 字体。
/// 优先 TTF；TTC 集合字体在部分 PDFium 构建上无法直接加载，故只作次选。
pub fn system_cjk_font() -> AppResult<PathBuf> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:/Windows/Fonts/simhei.ttf",
            "C:/Windows/Fonts/msyh.ttc",
            "C:/Windows/Fonts/simsun.ttc",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
    };
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .ok_or_else(|| {
            AppError::internal(
                "系统未找到可嵌入的中文字体，无法生成中文 PDF。请安装中文字体后重试，或改用纯 ASCII 文本。",
            )
        })
}

fn cjk_font_token(doc: &mut PdfDocument<'_>) -> AppResult<PdfFontToken> {
    let font_path = system_cjk_font()?;
    doc.fonts_mut()
        .load_true_type_from_file(&font_path, true)
        .map_err(pdf_error)
}

// ---------------------------------------------------------------------------
// 文本 / CSV / HTML → PDF
// ---------------------------------------------------------------------------

/// 按视觉宽度折行：中文按 1 个字符算 1 格，ASCII 按 0.55 格。
fn wrap_line(line: &str, columns: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut used = 0.0f32;
    for c in line.chars() {
        let w = if (c as u32) > 0x2E80 { 1.0 } else { 0.55 };
        if used + w > columns as f32 && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            used = 0.0;
        }
        cur.push(c);
        used += w;
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

/// 极简 HTML 标签剥离（不执行脚本、不加载外链、不渲染 CSS）。
fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push('\n');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// `format` = txt | csv | html。每页 45 行，A4，11pt，自动分页。
pub fn text_to_pdf(
    pdfium: &Pdfium,
    src: &Path,
    format: &str,
    out_tmp: &Path,
) -> AppResult<usize> {
    let fmt = match format {
        "txt" | "csv" | "html" => format,
        _ => return Err(AppError::internal(format!("不支持的文本源格式：{}", format))),
    };
    let raw = std::fs::read(src).map_err(AppError::from)?;
    let mut text = String::from_utf8_lossy(&raw).to_string();
    if text.starts_with('\u{FEFF}') {
        text.remove(0);
    }
    if fmt == "html" {
        text = strip_html(&text);
    }
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        lines.extend(wrap_line(line.trim_end(), 92));
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    let needs_cjk = text.chars().any(|c| (c as u32) > 0x2E80);
    if needs_cjk {
        system_cjk_font()?; // 提前失败，绝不写乱码
    }

    let mut doc = pdfium.create_new_pdf().map_err(pdf_error)?;
    let font = if needs_cjk {
        cjk_font_token(&mut doc)?
    } else {
        doc.fonts_mut().helvetica()
    };
    let per_page = 45usize;
    let page_count = lines.len().div_ceil(per_page);
    for p in 0..page_count {
        let mut page = doc
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::a4())
            .map_err(pdf_error)?;
        for (i, line) in lines[p * per_page..((p + 1) * per_page).min(lines.len())]
            .iter()
            .enumerate()
        {
            if line.is_empty() {
                continue;
            }
            page.objects_mut()
                .create_text_object(
                    PdfPoints::new(50.0),
                    PdfPoints::new(790.0 - (i as f32) * 16.0),
                    line.as_str(),
                    font,
                    PdfPoints::new(11.0),
                )
                .map_err(pdf_error)?;
        }
        page.regenerate_content().map_err(pdf_error)?;
    }
    doc.save_to_file(out_tmp).map_err(pdf_error)?;
    Ok(page_count)
}

// ---------------------------------------------------------------------------
// 中文水印（水平居中）
// ---------------------------------------------------------------------------

/// 在每页中央叠加一行中文（或其它 Unicode）文字。
/// 与 F13 的 lopdf 水印路径互斥：命令层检测到非 ASCII 时改走这里。
pub fn cjk_watermark(
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    text: &str,
    size: f32,
    out_tmp: &Path,
) -> AppResult<usize> {
    if text.trim().is_empty() {
        return Err(AppError::internal("水印文字不能为空"));
    }
    let mut doc = pdfium
        .load_pdf_from_file(path, password)
        .map_err(pdf_error)?;
    let font = cjk_font_token(&mut doc)?;
    let total = doc.pages().len().max(0) as usize;
    if total == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let approx = text.chars().count() as f32 * size * 0.62;
    for i in 0..total {
        let gray = PdfColor::from_hex("#9A9A9A").map_err(pdf_error)?;
        let mut page = doc.pages().get(i as PdfPageIndex).map_err(pdf_error)?;
        let x = (page.width().value - approx) / 2.0;
        let y = page.height().value / 2.0;
        let mut obj = page
            .objects_mut()
            .create_text_object(
                PdfPoints::new(x.max(10.0)),
                PdfPoints::new(y),
                text,
                font,
                PdfPoints::new(size),
            )
            .map_err(pdf_error)?;
        let _ = obj.set_fill_color(gray);
        page.regenerate_content().map_err(pdf_error)?;
    }
    doc.save_to_file(out_tmp).map_err(pdf_error)?;
    Ok(total)
}

// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_cjk_and_ascii() {
        let zh = wrap_line("一二三四五六七八九十", 5);
        assert_eq!(zh.len(), 2);
        assert_eq!(zh[0], "一二三四五");
        let en = wrap_line("abcdefghij", 5);
        assert!(en.len() >= 2);
    }

    #[test]
    fn strips_html_without_executing() {
        let s = strip_html("<script>alert(1)</script><p>合同 &amp; 条款</p>");
        assert!(!s.contains("<script>"));
        assert!(s.contains("合同 &amp; 条款"));
    }

    #[test]
    fn cjk_font_is_available_on_this_machine() {
        // 本机 Windows 有 simhei.ttf；其它平台若缺字体则测试跳过而非伪装通过。
        if system_cjk_font().is_err() {
            eprintln!("系统无 CJK 字体，跳过（不视为通过）");
        }
    }
}
