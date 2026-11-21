//! F10–F14：对标 PDFgear 桌面版补齐的功能层（2026-11-16 范围放宽后新增）。
//!
//! 许可与依赖：只用已在包内的 PDFium(BSD-3) / qpdf(Apache-2.0) / lopdf(MIT) /
//! image(MIT|Apache-2.0)；**不引入 AGPL 组件、不引入任何云端 API**。
//!
//! | 编号 | 能力 | 引擎 | 诚实边界 |
//! |---|---|---|---|
//! | F10 | 就地替换文字 | PDFium `FPDFText_SetText` | 不重排；跨文本对象合并；原字体缺字形时明确报错，绝不写乱码；扫描件无效 |
//! | F11 | 批注（高亮/下划线/删除线/波浪线/便签） | PDFium 查找 + lopdf 写注解字典 | 只标注可查找的可选中文本 |
//! | F12 | 页面管理（旋转/删除/抽取/插入空白/裁剪） | lopdf 重建页树 + qpdf 切页 | 不重建嵌套书签 |
//! | F13 | 水印/页眉页脚/页码/元数据/AES-256 密码 | lopdf 内容流 + qpdf 加密 | 叠加文字仅支持 WinAnsi（中文明确报错），不重排版面 |
//! | F14 | 导出 TXT/HTML/DOCX/XLSX | PDFium 文本层 + 自写 OOXML ZIP | **只导出文字层，不还原版式**；这是与 PDFgear 的商业 SDK 转换的已知差距 |
//!
//! 所有输出都写到调用方给的临时路径，由上层原子提交；绝不原地覆盖输入文件。

use crate::engine::{pdf, qpdf, JobCtx};
use crate::error::{AppError, AppResult};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 通用纯函数（可单测）
// ---------------------------------------------------------------------------

/// 解析页码表达式：`""` / `"all"` / `"1-3,5,8-9"` → 1-based 去重升序页码。
/// 复用 `engine::page_selection` 的既有校验，不重复实现解析规则。
pub fn parse_pages(spec: Option<&str>, total: u32) -> AppResult<Vec<u32>> {
    if spec
        .map(|s| s.trim().eq_ignore_ascii_case("all"))
        .unwrap_or(false)
    {
        return Ok((1..=total).collect());
    }
    let mut out = crate::engine::page_selection(spec, total)?;
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(AppError::internal("页码范围为空"));
    }
    Ok(out)
}

/// 把 "1-3,5" 变成 qpdf `--pages` 可接受的 token。
pub fn range_tokens(spec: &str) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        if !t
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c.is_whitespace())
        {
            return Err(AppError::internal(format!("非法页码范围：{}", t)));
        }
        out.push(t.chars().filter(|c| !c.is_whitespace()).collect());
    }
    if out.is_empty() {
        return Err(AppError::internal("页码范围为空"));
    }
    Ok(out)
}

/// `#RRGGBB` → 0..1 的 RGB。
pub fn parse_color(hex: &str) -> AppResult<[f64; 3]> {
    let s = hex.trim().trim_start_matches('#');
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::internal("颜色必须是 #RRGGBB 格式"));
    }
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
    Ok([byte(0), byte(2), byte(4)])
}

/// WinAnsi 字面量编码：中文等不可用字符**明确报错**，绝不降级成 `?` 冒充成功。
pub fn winansi_literal(s: &str) -> AppResult<String> {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' | '\t' => out.push(' '),
            c if (c as u32) < 128 && !c.is_control() => out.push(c),
            // WinAnsi 高位区（Latin-1 主体 + 常见欧文符号）
            c if (0xA0..=0xFF).contains(&(c as u32)) => out.push(c),
            c if matches!(c, '€' | '‘' | '’' | '“' | '”' | '–' | '—' | '…' | '•') => out.push(c),
            _ => {
                return Err(AppError::internal(format!(
                    "叠加文字「{}」含当前字体不支持的字符（{}）：水印/页眉页脚仅支持拉丁字符，请改用 ASCII 或使用 F10 就地改字（支持中文）",
                    s, c
                )))
            }
        }
    }
    Ok(out)
}

/// 查找所有不重叠匹配，返回 `(起始字符下标, 结束字符下标)`（左闭右开）。
pub fn find_all(
    haystack: &[char],
    needle: &[char],
    match_case: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let norm = |c: char| if match_case { c } else { c.to_lowercase().next().unwrap_or(c) };
    let h: Vec<char> = haystack.iter().map(|&c| norm(c)).collect();
    let n: Vec<char> = needle.iter().map(|&c| norm(c)).collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + n.len() <= h.len() {
        if h[i..i + n.len()] == n[..] {
            let before_ok = !whole_word
                || i == 0
                || !haystack[i - 1].is_alphanumeric();
            let after = i + n.len();
            let after_ok = !whole_word
                || after >= haystack.len()
                || !haystack[after].is_alphanumeric();
            if before_ok && after_ok {
                out.push((i, after));
                i = after;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// PDFium 实测（2026-11-16，随包版本）：页面 /Rotate 非 0 时 `FPDFText_*`
/// 返回空文本，但页面内容流与字体资源都完好。为保证 F11 查找、F14 导出在
/// 旋转过的页面上仍可用，生成一份**仅把 /Rotate 置 0**的临时副本供文字提取。
/// 内容流、坐标、注解一概不动；输出的正式文件仍保留原旋转值。
fn unrotated_copy(path: &Path, out_tmp: &Path) -> AppResult<Option<PathBuf>> {
    let mut doc = Document::load(path).map_err(lopdf_error)?;
    let pages = doc.get_pages();
    let mut rotated: Vec<ObjectId> = Vec::new();
    for (_no, id) in &pages {
        let r = doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"Rotate").ok())
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        if r != 0 {
            rotated.push(*id);
        }
    }
    if rotated.is_empty() {
        return Ok(None);
    }
    if doc.is_encrypted() {
        return Err(AppError::internal(
            "文档同时含加密与旋转页：无法在不破坏密码的前提下生成文字提取副本。请先移除密码再操作。",
        ));
    }
    for id in &rotated {
        doc.get_object_mut(*id)
            .map_err(lopdf_error)?
            .as_dict_mut()
            .map_err(lopdf_error)?
            .set("Rotate", 0i64);
    }
    let bytes = pdf::save_to_bytes(&mut doc)?;
    let copy = out_tmp.with_extension("unrotated.pdf");
    std::fs::write(&copy, bytes).map_err(AppError::from)?;
    Ok(Some(copy))
}

/// 无论查找成功、失败还是提前返回，都清理临时副本。
fn with_unrotated<T>(
    path: &Path,
    out_tmp: &Path,
    work: impl FnOnce(&Path) -> AppResult<T>,
) -> AppResult<T> {
    let copy = unrotated_copy(path, out_tmp)?;
    let effective = copy.as_deref().unwrap_or(path);
    let result = work(effective);
    if let Some(c) = &copy {
        let _ = std::fs::remove_file(c);
    }
    result
}

fn pdf_error(e: PdfiumError) -> AppError {
    let s = e.to_string().to_ascii_lowercase();
    if s.contains("password") {
        AppError::bad_password("密码不正确")
    } else {
        AppError::parse(format!("PDFium 处理失败：{}", e))
    }
}

fn lopdf_error(e: lopdf::Error) -> AppError {
    AppError::parse(format!("PDF 结构写入失败：{}", e))
}

fn write_doc(doc: &mut Document, out_tmp: &Path) -> AppResult<()> {
    let bytes = pdf::save_to_bytes(doc)?;
    std::fs::write(out_tmp, bytes).map_err(AppError::from)
}

fn now_pdf_date() -> String {
    // 本地时间，不含时区偏移；PDF 规范允许 D:YYYYMMDDHHmmSS。
    chrono::Local::now().format("D:%Y%m%d%H%M%S").to_string()
}

// ---------------------------------------------------------------------------
// F10：就地替换文字（核心缺口，对应用户「Acrobat Reader 里就地改几个字」场景）
// ---------------------------------------------------------------------------

pub struct EditOutcome {
    pub replacements: usize,
    pub pages_touched: usize,
    pub objects_modified: usize,
}

struct TextRun {
    object_index: usize,
    text: String,
}

/// 收集一页里全部文本对象，按内容流顺序拼成一个可跨对象查找的字符序列。
fn collect_runs(page: &PdfPage<'_>) -> (Vec<TextRun>, Vec<char>, Vec<(usize, usize)>) {
    let mut runs: Vec<TextRun> = Vec::new();
    let count = page.objects().len();
    for i in 0..count {
        if let Ok(obj) = page.objects().get(i) {
            if let Some(t) = obj.as_text_object() {
                let s = t.text();
                if !s.is_empty() {
                    runs.push(TextRun {
                        object_index: i,
                        text: s,
                    });
                }
            }
        }
    }
    let mut chars: Vec<char> = Vec::new();
    // 全局字符下标 → (run 下标, 该字符在 run 内的字节偏移)
    let mut owner: Vec<(usize, usize)> = Vec::new();
    for (ri, r) in runs.iter().enumerate() {
        for (bi, c) in r.text.char_indices() {
            owner.push((ri, bi));
            chars.push(c);
        }
    }
    (runs, chars, owner)
}

/// 把一次匹配按 run 分组，返回 `(run 下标, 起始字节, 结束字节, 是否是首个 run)`。
fn split_match(owner: &[(usize, usize)], runs: &[TextRun], start: usize, end: usize) -> Vec<(usize, usize, usize, bool)> {
    let mut groups: Vec<(usize, usize, usize)> = Vec::new(); // (ri, start_byte, end_byte)
    for gi in start..end {
        let (ri, bi) = owner[gi];
        let ch_len = runs[ri].text[bi..].chars().next().map(char::len_utf8).unwrap_or(1);
        let last = bi + ch_len;
        if let Some(g) = groups.last_mut() {
            if g.0 == ri {
                g.2 = last;
                continue;
            }
        }
        groups.push((ri, bi, last));
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(i, (ri, a, b))| (ri, a, b, i == 0))
        .collect()
}

/// 对一段文本应用替换区间（倒序应用，避免位移）。
fn apply_edits(original: &str, edits: &[(usize, usize, Option<&str>)]) -> String {
    let mut out = original.to_string();
    let mut sorted: Vec<(usize, usize, Option<&str>)> = edits.to_vec();
    sorted.sort_by_key(|e| std::cmp::Reverse(e.0));
    for (a, b, replacement) in sorted {
        if a > b || b > out.len() {
            continue;
        }
        out.replace_range(a..b, replacement.unwrap_or(""));
    }
    out
}

/// F10 主流程：不重排、不回流，只替换文本对象里的字符；结果另存为新文件。
#[allow(clippy::too_many_arguments)]
pub fn replace_text(
    ctx: &JobCtx<'_>,
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    find: &str,
    replace: &str,
    match_case: bool,
    whole_word: bool,
    pages: Option<&str>,
    out_tmp: &Path,
) -> AppResult<EditOutcome> {
    if find.is_empty() {
        return Err(AppError::internal("查找内容不能为空"));
    }
    let doc = pdfium
        .load_pdf_from_file(path, password)
        .map_err(pdf_error)?;
    let total = doc.pages().len() as u32;
    if total == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let targets = parse_pages(pages, total)?;
    let needle: Vec<char> = find.chars().collect();

    let mut outcome = EditOutcome {
        replacements: 0,
        pages_touched: 0,
        objects_modified: 0,
    };

    for (k, &page_no) in targets.iter().enumerate() {
        ctx.tick(k + 1, targets.len(), format!("查找第 {} 页", page_no))?;
        let mut page = doc
            .pages()
            .get((page_no - 1) as PdfPageIndex)
            .map_err(pdf_error)?;
        let (runs, chars, owner) = collect_runs(&page);
        if runs.is_empty() {
            continue; // 扫描件 / 纯图片页：明确跳过，不假装改成功
        }
        let matches = find_all(&chars, &needle, match_case, whole_word);
        if matches.is_empty() {
            continue;
        }
        outcome.pages_touched += 1;

        // 每个 run 上累积若干替换区间。
        let mut per_run: Vec<Vec<(usize, usize, Option<String>)>> = vec![Vec::new(); runs.len()];
        for (start, end) in &matches {
            for (ri, a, b, first) in split_match(&owner, &runs, *start, *end) {
                let repl = if first {
                    Some(replace.to_string())
                } else {
                    None
                };
                per_run[ri].push((a, b, repl));
            }
            outcome.replacements += 1;
        }

        let mut to_remove: Vec<usize> = Vec::new();
        for (ri, run) in runs.iter().enumerate() {
            if per_run[ri].is_empty() {
                continue;
            }
            let edits: Vec<(usize, usize, Option<&str>)> = per_run[ri]
                .iter()
                .map(|(a, b, r)| (*a, *b, r.as_deref()))
                .collect();
            let new_text = apply_edits(&run.text, &edits);
            if new_text.is_empty() {
                to_remove.push(run.object_index);
                continue;
            }
            {
                let mut obj = page
                    .objects()
                    .get(run.object_index)
                    .map_err(pdf_error)?;
                let Some(text_obj) = obj.as_text_object_mut() else {
                    continue;
                };
                // PDFium 的 FPDFText_SetText 对字体缺字形的字符会静默写空格/问号，
                // 并不会返回错误。标准 14 字体 / 未嵌入字体一律先行拒绝。
                if new_text.chars().any(|c| !c.is_ascii()) {
                    let font = text_obj.font();
                    if font.is_built_in() || !font.is_embedded().unwrap_or(false) {
                        return Err(AppError::internal(format!(
                            "第 {} 页文本对象使用内置/未嵌入字体（{}），无法写入中文等非 ASCII 文字；已中止且未写出任何文件。请改用 ASCII 替换，或换用已嵌入中文字体的 PDF。",
                            page_no, font.name()
                        )));
                    }
                }
                if text_obj.set_text(&new_text).is_err() {
                    return Err(AppError::internal(format!(
                        "第 {} 页有文本对象的字体不支持新文字（缺字形或字体未嵌入），已中止且未写出任何文件。可缩短替换内容或先用 F5 OCR / 换用同源字体文档。",
                        page_no
                    )));
                }
                outcome.objects_modified += 1;
            }
        }
        // 倒序删除被完全消费的文本对象，避免留下多余空格。
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            let _ = page.objects_mut().remove_object_at_index(idx);
        }
        // 关键：PDFium 规定保存前必须 FPDFPage_GenerateContent，否则对象级修改
        // 全部丢失（表现为「返回成功但打开还是原文」）。这里显式提交每页内容流。
        page.regenerate_content().map_err(pdf_error)?;
    }

    if outcome.replacements == 0 {
        return Err(AppError::internal(format!(
            "没有找到「{}」。请确认：文档是否含可选中文本（扫描件需先走 F5 OCR）、页码范围与区分大小写设置。",
            find
        )));
    }
    let bytes = doc.save_to_bytes().map_err(pdf_error)?;
    std::fs::write(out_tmp, bytes).map_err(AppError::from)?;

    // 写后读回：重新打开产物确认新文字真进了文本层。覆盖三类「假成功」：
    // PDFium 静默把缺字形字符写成空格、内容流没提交、字体子集只含原字形。
    if let Err(e) = verify_edit(pdfium, out_tmp, &targets, find, replace) {
        let _ = std::fs::remove_file(out_tmp);
        return Err(e);
    }
    Ok(outcome)
}

/// 忽略空白后的包含判断：PDFium 提取文本时可能在字符间插入空格。
fn compact(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn verify_edit(
    pdfium: &Pdfium,
    out_tmp: &Path,
    targets: &[u32],
    find: &str,
    replace: &str,
) -> AppResult<()> {
    let doc = pdfium
        .load_pdf_from_file(out_tmp, None)
        .map_err(pdf_error)?;
    let mut all = String::new();
    for &p in targets {
        let page = doc
            .pages()
            .get((p - 1) as PdfPageIndex)
            .map_err(pdf_error)?;
        // 不用 page.text()：PDFium 在旋转页上会返回空，改用文本对象拼接。
        let (_, chars, _) = collect_runs(&page);
        all.push_str(&chars.iter().collect::<String>());
        all.push('\n');
    }
    let hay = compact(&all);
    if replace.trim().is_empty() {
        if hay.contains(&compact(find)) {
            return Err(AppError::internal(
                "替换结果校验失败：原文仍可读取，未写出任何文件。",
            ));
        }
    } else if !hay.contains(&compact(replace)) {
        let _ = std::fs::remove_file(out_tmp);
        return Err(AppError::internal(format!(
            "替换结果校验失败：输出中读不到「{}」。多半是原字体缺字形（PDFium 会静默写空格），已删除输出且未改动原文件。",
            replace
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// F19：按矩形精确替换（只改框选的那一处，不做全文文本查找）
// ---------------------------------------------------------------------------

/// 单页按位置替换的字符上限，与 F18 的框选上限保持一致。
pub const MAX_RECT_CHARS: usize = 60_000;

pub struct RectEditOutcome {
    pub page: u32,
    pub old_text: String,
    pub objects_modified: usize,
    pub selected_chars: usize,
}

/// 字符框被选区覆盖的比例（与前端 `selection.ts::overlapRatio` 同一口径）。
fn char_cover_ratio(b: (f32, f32, f32, f32), sel: (f32, f32, f32, f32)) -> f32 {
    let (cl, cb, cr, ct) = b;
    let (sl, sb, sr, st) = sel;
    let area = (cr - cl).max(0.0) * (ct - cb).max(0.0);
    if area <= 0.0 {
        return 0.0;
    }
    let w = (cr.min(sr) - cl.max(sl)).max(0.0);
    let h = (ct.min(st) - cb.max(sb)).max(0.0);
    (w * h) / area
}

/// 大小写敏感、非重叠计数。
fn count_str(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    hay.match_indices(needle).count()
}

/// 用矩形在字符框里挑出选中字符（中心点落在框内，或字符框被覆盖 ≥ 40%）。
fn pick_chars_by_rect(boxes: &[(f32, f32, f32, f32)], sel: (f32, f32, f32, f32)) -> Vec<usize> {
    let (sl, sb, sr, st) = sel;
    let mut out = Vec::new();
    for (i, b) in boxes.iter().enumerate() {
        let (l, bo, r, t) = *b;
        let cx = (l + r) / 2.0;
        let cy = (bo + t) / 2.0;
        let center = cx >= sl && cx <= sr && cy >= sb && cy <= st;
        if center || char_cover_ratio(*b, sel) >= 0.4 {
            out.push(i);
        }
    }
    out
}

/// F19：按 PDF 坐标（pt、左下原点）定位并替换**那一处**文字。
///
/// 与 F10 的本质区别：不做文本查找，因此同页出现多次的相同文字也不会被一起改掉。
/// 安全闸门（任一不满足即报错、不写文件）：
/// - 页面 /Rotate 必须为 0（否则字符坐标与渲染图不一致，F18 已同样禁用）；
/// - 文字层字符序列必须与内容流文本对象序列逐字一致，否则字符下标无法映射到对象；
/// - 选区必须落在同一行内的连续字符上（跨行合并会破坏排版）。
#[allow(clippy::too_many_arguments)]
pub fn replace_text_at_rect(
    ctx: &JobCtx<'_>,
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    page_1based: u32,
    rect: (f32, f32, f32, f32),
    replace: &str,
    expected: Option<&str>,
    out_tmp: &Path,
) -> AppResult<RectEditOutcome> {
    let doc = pdfium.load_pdf_from_file(path, password).map_err(pdf_error)?;
    let total = doc.pages().len() as u32;
    if total == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let page_no = page_1based.clamp(1, total);
    ctx.tick(1, 2, format!("第 {} 页按框选位置定位", page_no))?;
    let mut page = doc
        .pages()
        .get((page_no - 1) as PdfPageIndex)
        .map_err(pdf_error)?;

    let rotation = match page.rotation() {
        Ok(PdfPageRenderRotation::Degrees90) => 90,
        Ok(PdfPageRenderRotation::Degrees180) => 180,
        Ok(PdfPageRenderRotation::Degrees270) => 270,
        _ => 0,
    };
    if rotation != 0 {
        return Err(AppError::internal(format!(
            "第 {} 页有 /Rotate {}，字符坐标与渲染图不一致，按位置替换已拒绝；请先用「页面」工具把旋转归零，或改用「按文本替换」。",
            page_no, rotation
        )));
    }

    // 1) 取字符与包围盒，按矩形挑出选中字符
    //    紧贴框用于框选命中（跟用户看到的墨迹一致）；宽松框由字体 ascent/descent
    //    决定，同字号恒等，用来判断选区内是否混了不同字号（上下标）。
    #[allow(clippy::type_complexity)]
    let (page_chars, boxes, lboxes, picked): (
        Vec<char>,
        Vec<(f32, f32, f32, f32)>,
        Vec<(f32, f32, f32, f32)>,
        Vec<usize>,
    ) = {
        let text = page.text().map_err(|e| {
            AppError::parse(format!("第 {} 页文字层读取失败：{}", page_no, e))
        })?;
        let n = text.len() as usize;
        if n == 0 {
            return Err(AppError::internal(format!(
                "第 {} 页没有可选中文字（扫描件或纯图片页）；如需修改请先走 OCR。",
                page_no
            )));
        }
        if n > MAX_RECT_CHARS {
            return Err(AppError::internal(format!(
                "第 {} 页有 {} 个字符，超过按位置替换的 {} 字符上限；请改用「按文本替换」。",
                page_no, n, MAX_RECT_CHARS
            )));
        }
        let mut chars = Vec::with_capacity(n);
        let mut bx = Vec::with_capacity(n);
        let mut lbx = Vec::with_capacity(n);
        for c in text.chars().iter() {
            let ch = c.unicode_char().unwrap_or('\u{FFFD}');
            // PDFium 会在文本页里插入换行符（内容流里并不存在）。它们
            // 既不属于任何文本对象，也不该被框选命中，统一剔除后再对齐。
            if ch == '\r' || ch == '\n' {
                continue;
            }
            let b = c
                .tight_bounds()
                .or_else(|_| c.loose_bounds())
                .unwrap_or_else(|_| PdfRect::zero());
            let lb = c.loose_bounds().unwrap_or(b);
            chars.push(ch);
            bx.push((
                b.left().value,
                b.bottom().value,
                b.right().value,
                b.top().value,
            ));
            lbx.push((
                lb.left().value,
                lb.bottom().value,
                lb.right().value,
                lb.top().value,
            ));
        }
        if chars.is_empty() {
            return Err(AppError::internal(format!(
                "第 {} 页没有可选中文字（扫描件或纯图片页）；如需修改请先走 OCR。",
                page_no
            )));
        }
        let pick = pick_chars_by_rect(&bx, rect);
        (chars, bx, lbx, pick)
    };

    if picked.is_empty() {
        return Err(AppError::internal(
            "框选范围内没有可选文字：请拖过文字本身（不要只框空白），或改用右侧「按文本替换」。",
        ));
    }
    let first = picked[0];
    let last = picked[picked.len() - 1];
    // 空格等字符的字符框是退化的（PDFium 只给一条细缝），真实拖选经常套不住
    // 它们的中心点。中间缺口只要全是空白字符就算连续，不把「没选中空格」当跨行。
    let picked_set: std::collections::HashSet<usize> = picked.iter().copied().collect();
    let mut gap_non_ws: Vec<char> = Vec::new();
    for i in first..=last {
        if !picked_set.contains(&i) && !page_chars[i].is_whitespace() {
            gap_non_ws.push(page_chars[i]);
        }
    }
    if !gap_non_ws.is_empty() {
        let shown: String = gap_non_ws.iter().take(12).collect();
        return Err(AppError::internal(format!(
            "框选不连续：中间有不属于选区的文字「{}」。请重新拖选同一行内的连续文字。",
            shown
        )));
    }
    let old_text: String = page_chars[first..=last].iter().collect();
    if old_text.contains('\n') || old_text.contains('\r') {
        return Err(AppError::internal(
            "框选跨了多行：按位置替换只支持同一行内的连续文字，请缩到一行再试（跨行请用「按文本替换」）。",
        ));
    }
    // 换行符已在前面剔除，跨行选区在字符序列里会「看起来连续」。
    // 这里用行内垂直中心连续性把跨行拆穿，避免把两行挤成一行。
    // 注意：空格等字符的框是退化的（高 ~1-2pt、中心与字母差好几 pt），必须跳过，
    // 否则任何含空格的选区都会被误判成「跨行」（真机 F19 首测即栽在这）。
    let mut prev_center: Option<f32> = None;
    let mut prev_h: f32 = 0.0;
    for i in first..=last {
        let (l, b, r, t) = boxes[i];
        let h = (t - b).abs();
        if h < 1.0 || (r - l) <= 0.01 {
            continue; // 退化字符框（空格等），不参与行内中心连续性
        }
        let cy = (b + t) / 2.0;
        if let Some(p) = prev_center {
            let tol = 0.6 * h.max(prev_h).max(2.0);
            if (cy - p).abs() > tol {
                return Err(AppError::internal(
                    "框选跨了多行：按位置替换只支持同一行内的连续文字，请缩到一行再试（跨行请用「按文本替换」）。",
                ));
            }
        }
        prev_center = Some(cy);
        prev_h = h;
    }
    if old_text.trim().is_empty() {
        return Err(AppError::internal("框选里只有空白字符，没有可替换的文字。"));
    }
    // 上下标在 PDF 里是小字号 + 基线偏移的独立文本对象（"H₂O" 的 2 是另一个对象）。
    // 若放任跨字号选区通过，替换文字会整体落到首个对象上并沿用它的大字号，
    // 上标会被压成正文大小、基线也被拉平——排版当场毁掉。这里直接拒绝，
    // 让用户只框同一字号的文字（单独框上标本身是允许的）。
    // 用宽松框高度而不是紧贴框：紧贴框是墨迹，同一行里 "H" 与 "o" 就能差 30%，
    // 按墨迹比会误伤正常文字。
    {
        let mut h_lo = f32::MAX;
        let mut h_hi = 0.0f32;
        for i in first..=last {
            let (_, b, _, t) = lboxes[i];
            let h = (t - b).abs();
            if h < 1.0 {
                continue; // 退化框（空格等），不参与
            }
            h_lo = h_lo.min(h);
            h_hi = h_hi.max(h);
        }
        if h_hi > 0.0 && h_lo < f32::MAX && h_hi / h_lo > 1.5 {
            return Err(AppError::internal(format!(
                "选区里有大小不同的字（最大 {:.1}pt / 最小 {:.1}pt，多半是上下标或脚注）。整段替换会把小字压成正文大小、破坏排版，已拒绝且未写出文件。请只框选同一字号的文字；要改上标就单独框那个上标。",
                h_hi, h_lo
            )));
        }
    }
    if let Some(exp) = expected {
        let (a, b) = (compact(exp), compact(&old_text));
        if !a.is_empty() && a != b {
            return Err(AppError::internal(format!(
                "框选文字与预览不一致（预览「{}」/ 引擎「{}」）。页面可能在预览后被修改过，请重新加载后再试。",
                exp.trim(),
                old_text.trim()
            )));
        }
    }

    // 2) 字符层顺序必须与内容流文本对象顺序一致，否则下标会映射到错误的对象
    let (runs, run_chars, owner) = collect_runs(&page);
    if runs.is_empty() {
        return Err(AppError::internal(format!(
            "第 {} 页的文字不在页面级文本对象中（可能在表单 XObject 内），无法按位置替换；请改用「按文本替换」。",
            page_no
        )));
    }
    if run_chars.len() != page_chars.len() || run_chars != page_chars {
        return Err(AppError::internal(format!(
            "本页文字层顺序与内容流不一致（文字层 {} 字符 / 内容流 {} 字符），按位置替换可能改错对象，已拒绝且未写出文件。请改用「按文本替换」。",
            page_chars.len(),
            run_chars.len()
        )));
    }
    let before_count = {
        let page_str: String = page_chars.iter().collect();
        count_str(&page_str, &old_text)
    };

    // 3) 只改这一段：拆到 run 上，首个 run 得到替换文字，后续 run 清空
    let mut per_run: Vec<Vec<(usize, usize, Option<String>)>> = vec![Vec::new(); runs.len()];
    for (ri, s, e, is_first) in split_match(&owner, &runs, first, last + 1) {
        per_run[ri].push((s, e, if is_first { Some(replace.to_string()) } else { None }));
    }

    let mut to_remove: Vec<usize> = Vec::new();
    let mut modified = 0usize;
    for (ri, run) in runs.iter().enumerate() {
        if per_run[ri].is_empty() {
            continue;
        }
        let edits: Vec<(usize, usize, Option<&str>)> = per_run[ri]
            .iter()
            .map(|(a, b, r)| (*a, *b, r.as_deref()))
            .collect();
        let new_text = apply_edits(&run.text, &edits);
        if new_text.is_empty() {
            to_remove.push(run.object_index);
            continue;
        }
        {
            let mut obj = page
                .objects()
                .get(run.object_index)
                .map_err(pdf_error)?;
            let Some(text_obj) = obj.as_text_object_mut() else {
                continue;
            };
            // 非 ASCII 才需要关心字形覆盖。分三档判断：
            //   · 内嵌字体 → 能写（真缺字形由写后读回校验兜住）
            //   · 内置标准字体 + 字符都在 Latin-1（± ° µ × ÷ 等）→ 能写
            //     （WinAnsiEncoding 覆盖 U+00A0..U+00FF，之前一刀切拒绝是过严）
            //   · Symbol / ZapfDingbats → 私有编码，不能按 Latin-1 写
            let non_ascii: Vec<char> = new_text.chars().filter(|c| !c.is_ascii()).collect();
            if !non_ascii.is_empty() {
                let font = text_obj.font();
                let name = font.name();
                let lower = name.to_ascii_lowercase();
                let symbol_encoded = lower.contains("symbol") || lower.contains("dingbat");
                let latin1_only = non_ascii
                    .iter()
                    .all(|c| ('\u{a0}'..='\u{ff}').contains(c));
                // 判断依据是「能不能真的写出这个字」，不是字体的名义分类：
                // PDFium 对未嵌入的 Helvetica 也是按 WinAnsi 编码写字节的，
                // 而 is_built_in() 依赖字体名字匹配，对内容流里的字体经常返回 false。
                // 所以：嵌入字体一律放行（缺字形由读回校验兜住）；未嵌入的只放行
                // Latin-1 范围，且排除 Symbol/Dingbats（它们不是 WinAnsi 编码）。
                // 拿不准的情况交给写后读回校验，宁可报错也不写坏。
                let embedded = font.is_embedded().unwrap_or(false);
                let writable = embedded || (latin1_only && !symbol_encoded);
                if !writable {
                    let reason = if symbol_encoded {
                        "该处用的是 Symbol/Dingbats 编码的特殊字体，不是常规文字编码".to_string()
                    } else {
                        "该处字体未嵌入，且目标字符超出 Latin-1 范围".to_string()
                    };
                    return Err(AppError::internal(format!(
                        "第 {} 页无法写入「{}」：{}。已中止且未写出任何文件。ASCII 与 Latin-1 范围的符号（± ° µ × ÷ 等）可直接改；中文或 Latin-1 之外的数学符号，需要该 PDF 已内嵌包含这些字形的字体，否则请改用「按文本替换」或换掉整段字体。",
                        page_no, new_text, reason
                    )));
                }
            }
            if text_obj.set_text(&new_text).is_err() {
                return Err(AppError::internal(format!(
                    "第 {} 页该处文本的字体不支持新文字（缺字形或字体未嵌入），已中止且未写出任何文件。",
                    page_no
                )));
            }
            modified += 1;
        }
    }
    to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for idx in to_remove {
        let _ = page.objects_mut().remove_object_at_index(idx);
    }
    page.regenerate_content().map_err(pdf_error)?;

    ctx.tick(2, 2, "写出并校验".to_string())?;
    let bytes = doc.save_to_bytes().map_err(pdf_error)?;
    std::fs::write(out_tmp, bytes).map_err(AppError::from)?;

    // 4) 写后读回：新文字必须真的进了文本层，且原文出现次数恰好减少 1
    if let Err(e) = verify_rect_edit(pdfium, out_tmp, page_no, &old_text, replace, before_count) {
        let _ = std::fs::remove_file(out_tmp);
        return Err(e);
    }
    Ok(RectEditOutcome {
        page: page_no,
        old_text,
        objects_modified: modified,
        selected_chars: picked.len(),
    })
}

fn verify_rect_edit(
    pdfium: &Pdfium,
    out_tmp: &Path,
    page_no: u32,
    old_text: &str,
    replace: &str,
    before_count: usize,
) -> AppResult<()> {
    let doc = pdfium
        .load_pdf_from_file(out_tmp, None)
        .map_err(pdf_error)?;
    let page = doc
        .pages()
        .get((page_no - 1) as PdfPageIndex)
        .map_err(pdf_error)?;
    let (_, chars, _) = collect_runs(&page);
    let hay: String = chars.iter().collect();
    if !replace.trim().is_empty() && !compact(&hay).contains(&compact(replace)) {
        return Err(AppError::internal(format!(
            "替换结果校验失败：输出中读不到「{}」。多半是原字体缺字形（PDFium 会静默写空格），已删除输出且未改动原文件。",
            replace
        )));
    }
    let after = count_str(&hay, old_text);
    // 替换文字自身包含原文时（如 "a" → "aa"），次数不会减少，不做减法断言。
    let want = if replace.contains(old_text) {
        before_count
    } else {
        before_count.saturating_sub(1)
    };
    if after != want {
        return Err(AppError::internal(format!(
            "替换结果校验失败：原文出现次数 {} → {}（期望 {}）。已删除输出且未改动原文件，请改用「按文本替换」。",
            before_count, after, want
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// F11：批注（文本高亮 / 下划线 / 删除线 / 波浪线 / 便签）
// ---------------------------------------------------------------------------

pub struct AnnotOutcome {
    pub matches: usize,
    pub quads: usize,
    pub pages_touched: usize,
}

pub(crate) struct Quad {
    pub(crate) page_no: u32,
    pub(crate) points: [f32; 8],
}

pub(crate) fn pdf_quads(
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    find: &str,
    match_case: bool,
    pages: Option<&str>,
    cap: usize,
    out_tmp: &Path,
) -> AppResult<(Vec<Quad>, usize, u32)> {
    with_unrotated(path, out_tmp, |effective| {
    let doc = pdfium
        .load_pdf_from_file(effective, password)
        .map_err(pdf_error)?;
    let total = doc.pages().len() as u32;
    let targets = parse_pages(pages, total)?;
    let mut out: Vec<Quad> = Vec::new();
    let mut matches = 0usize;
    for &page_no in &targets {
        let page = doc
            .pages()
            .get((page_no - 1) as PdfPageIndex)
            .map_err(pdf_error)?;
        let text = page.text().map_err(pdf_error)?;
        let opts = PdfSearchOptions::new().match_case(match_case);
        let search = text.search(find, &opts).map_err(pdf_error)?;
        while let Some(segs) = search.find_next() {
            matches += 1;
            for i in 0..segs.len() {
                if let Ok(seg) = segs.get(i) {
                    let q = seg.bounds().to_quad_points();
                    out.push(Quad {
                        page_no,
                        points: [
                            q.x1().value,
                            q.y1().value,
                            q.x2().value,
                            q.y2().value,
                            q.x3().value,
                            q.y3().value,
                            q.x4().value,
                            q.y4().value,
                        ],
                    });
                    if out.len() >= cap {
                        return Ok((out, matches, total));
                    }
                }
            }
        }
    }
        Ok((out, matches, total))
    })
}

fn push_annot(doc: &mut Document, page_id: ObjectId, annot_id: ObjectId) -> AppResult<()> {
    let page = doc
        .get_object_mut(page_id)
        .map_err(lopdf_error)?
        .as_dict_mut()
        .map_err(lopdf_error)?;
    if !page.has(b"Annots") {
        page.set("Annots", Vec::<Object>::new());
    }
    let arr = page
        .get_mut(b"Annots")
        .map_err(lopdf_error)?
        .as_array_mut()
        .map_err(lopdf_error)?;
    arr.push(Object::Reference(annot_id));
    Ok(())
}

/// F11 主流程：PDFium 定位 → lopdf 写标准注解字典 → 另存新文件。
#[allow(clippy::too_many_arguments)]
pub fn annotate(
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    find: &str,
    kind: &str,
    color: &str,
    author: &str,
    note: &str,
    match_case: bool,
    pages: Option<&str>,
    out_tmp: &Path,
) -> AppResult<AnnotOutcome> {
    if find.trim().is_empty() {
        return Err(AppError::internal("批注查找内容不能为空"));
    }
    let subtype = match kind {
        "highlight" => "Highlight",
        "underline" => "Underline",
        "strikeout" => "StrikeOut",
        "squiggly" => "Squiggly",
        "note" => "Text",
        _ => return Err(AppError::internal(format!("未知批注类型：{}", kind))),
    };
    let rgb = parse_color(color)?;
    let (quads, matches, _total) =
        pdf_quads(pdfium, path, password, find, match_case, pages, 20000, out_tmp)?;
    if quads.is_empty() {
        return Err(AppError::internal(format!(
            "没有找到「{}」对应的可选中文本，无法标注（扫描件请先走 F5 OCR）。",
            find
        )));
    }

    let mut doc = pdf::load_doc(path, password)?;
    let page_map = doc.get_pages();
    let mut by_page: std::collections::BTreeMap<u32, Vec<[f32; 8]>> = Default::default();
    for q in &quads {
        by_page.entry(q.page_no).or_default().push(q.points);
    }
    let stamp = now_pdf_date();
    let mut written = 0usize;
    let mut pages_touched = 0usize;
    for (page_no, points) in by_page {
        let Some(&page_id) = page_map.get(&page_no) else {
            continue;
        };
        pages_touched += 1;
        // 文本标记类注解允许一个 /QuadPoints 数组承载多个四边形。
        let quad_arr: Vec<Object> = points
            .iter()
            .flat_map(|p| p.iter().map(|&v| Object::Real(v)))
            .collect();
        let min_x = points.iter().map(|p| p[0].min(p[6])).fold(f32::MAX, f32::min);
        let min_y = points.iter().map(|p| p[1].min(p[3])).fold(f32::MAX, f32::min);
        let max_x = points.iter().map(|p| p[2].max(p[4])).fold(f32::MIN, f32::max);
        let max_y = points.iter().map(|p| p[5].max(p[7])).fold(f32::MIN, f32::max);

        let mut d = dictionary! {
            "Type" => "Annot",
            "Subtype" => subtype,
            "Rect" => vec![
                Object::Real(min_x), Object::Real(min_y),
                Object::Real(max_x), Object::Real(max_y),
            ],
            "C" => vec![
                Object::Real(rgb[0] as f32), Object::Real(rgb[1] as f32), Object::Real(rgb[2] as f32),
            ],
            "F" => 4i64,
            "P" => Object::Reference(page_id),
            "M" => Object::string_literal(stamp.clone()),
            "NM" => Object::string_literal(format!("payoncepdf-{}-{}", page_no, written)),
        };
        if !author.trim().is_empty() {
            d.set("T", Object::string_literal(author.to_string()));
        }
        if !note.trim().is_empty() {
            d.set("Contents", Object::string_literal(note.to_string()));
        }
        if subtype == "Text" {
            d.set("Name", "Note");
            d.set("Open", false);
            // 便签用小方框，不需要 QuadPoints。
        } else {
            d.set("QuadPoints", quad_arr);
            d.set("CA", 0.4f32);
        }
        let annot_id = doc.add_object(Object::Dictionary(d));
        push_annot(&mut doc, page_id, annot_id)?;
        written += 1;
    }
    write_doc(&mut doc, out_tmp)?;
    Ok(AnnotOutcome {
        matches,
        quads: quads.len(),
        pages_touched,
    })
}

// ---------------------------------------------------------------------------
// F12：页面管理
// ---------------------------------------------------------------------------

/// 重建为扁平页树：/Pages 的 Kids 直接持有全部页面引用，每个页面 /Parent 指回根节点。
/// 这样可确定性地处理删除 / 抽取 / 插入，不依赖源文档的嵌套页树结构。
fn rebuild_page_tree(doc: &mut Document, keep: &[ObjectId]) -> AppResult<()> {
    let pages_id = doc
        .catalog()
        .map_err(lopdf_error)?
        .get(b"Pages")
        .map_err(lopdf_error)?
        .as_reference()
        .map_err(lopdf_error)?;
    let kids: Vec<Object> = keep.iter().map(|&id| Object::Reference(id)).collect();
    {
        let root = doc
            .get_object_mut(pages_id)
            .map_err(lopdf_error)?
            .as_dict_mut()
            .map_err(lopdf_error)?;
        root.set("Kids", kids);
        root.set("Count", keep.len() as i64);
    }
    for &pid in keep {
        if let Ok(page) = doc
            .get_object_mut(pid)
            .map_err(lopdf_error)?
            .as_dict_mut()
        {
            page.set("Parent", Object::Reference(pages_id));
        }
    }
    Ok(())
}

fn page_media_box(doc: &Document, page_id: ObjectId) -> [f32; 4] {
    let fallback = [0.0f32, 0.0, 595.276, 841.89];
    let Ok(page) = doc.get_object(page_id).and_then(|o| o.as_dict()) else {
        return fallback;
    };
    let Ok(arr) = page.get(b"MediaBox").and_then(Object::as_array) else {
        return fallback;
    };
    if arr.len() < 4 {
        return fallback;
    }
    let mut r = fallback;
    for (i, o) in arr.iter().take(4).enumerate() {
        r[i] = match o {
            Object::Integer(v) => *v as f32,
            Object::Real(v) => *v,
            _ => fallback[i],
        };
    }
    r
}

fn blank_page(doc: &mut Document, parent: ObjectId, media_box: [f32; 4]) -> AppResult<ObjectId> {
    let content_id = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let mb: Vec<Object> = media_box.iter().map(|&v| Object::Real(v)).collect();
    let id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(parent),
        "MediaBox" => mb,
        "Contents" => Object::Reference(content_id),
        "Resources" => Dictionary::new(),
    });
    Ok(id)
}

/// F12 主流程：`mode` = rotate | delete | extract | insert | crop。
#[allow(clippy::too_many_arguments)]
pub fn page_ops(
    app: Option<&tauri::AppHandle>,
    path: &Path,
    password: Option<&str>,
    mode: &str,
    pages_spec: &str,
    degrees: i64,
    insert_at: u32,
    insert_count: u32,
    margins: [f32; 4],
    out_tmp: &Path,
    tmp_dir: &Path,
) -> AppResult<String> {
    // 抽取复用已验证的 qpdf 切页，避免自己重建页面资源引用。
    if mode == "extract" {
        let app = app.ok_or_else(|| {
            AppError::internal("抽取页面需要 qpdf sidecar，当前上下文不可用")
        })?;
        let tokens = range_tokens(pages_spec)?;
        let tmp_slice = tmp_dir.join("extract.pdf");
        qpdf::slice(app, path, password, &tokens, &tmp_slice, tmp_dir)?;
        let bytes = std::fs::read(&tmp_slice).map_err(AppError::from)?;
        std::fs::write(out_tmp, bytes).map_err(AppError::from)?;
        return Ok(format!("抽取 {} 个页码范围", tokens.len()));
    }

    let mut doc = pdf::load_doc(path, password)?;
    let page_map = doc.get_pages();
    let total = page_map.len() as u32;
    if total == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let ordered: Vec<ObjectId> = page_map.values().copied().collect();
    let targets = parse_pages(Some(pages_spec), total)?;

    match mode {
        "rotate" => {
            if degrees % 90 != 0 {
                return Err(AppError::internal("旋转角度必须是 90 的倍数"));
            }
            for &p in &targets {
                let pid = ordered[(p - 1) as usize];
                let old = doc
                    .get_object(pid)
                    .ok()
                    .and_then(|o| o.as_dict().ok())
                    .and_then(|d| d.get(b"Rotate").ok())
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(0);
                let next = (old + degrees).rem_euclid(360);
                if let Ok(d) = doc
                    .get_object_mut(pid)
                    .map_err(lopdf_error)?
                    .as_dict_mut()
                {
                    d.set("Rotate", next);
                }
            }
            write_doc(&mut doc, out_tmp)?;
            Ok(format!("旋转 {} 页 {}°", targets.len(), degrees))
        }
        "delete" => {
            let drop: std::collections::HashSet<u32> = targets.iter().copied().collect();
            if drop.len() >= total as usize {
                return Err(AppError::internal("不能删除全部页面"));
            }
            let keep: Vec<ObjectId> = ordered
                .iter()
                .enumerate()
                .filter(|(i, _)| !drop.contains(&((*i + 1) as u32)))
                .map(|(_, id)| *id)
                .collect();
            rebuild_page_tree(&mut doc, &keep)?;
            write_doc(&mut doc, out_tmp)?;
            Ok(format!("删除 {} 页，剩余 {} 页", drop.len(), keep.len()))
        }
        "insert" => {
            let at = insert_at.clamp(1, total + 1) as usize;
            let count = insert_count.clamp(1, 200) as usize;
            let parent = doc
                .catalog()
                .map_err(lopdf_error)?
                .get(b"Pages")
                .map_err(lopdf_error)?
                .as_reference()
                .map_err(lopdf_error)?;
            let mb = page_media_box(&doc, ordered[0]);
            let mut keep = ordered.clone();
            for _ in 0..count {
                let id = blank_page(&mut doc, parent, mb)?;
                keep.insert(at - 1, id);
            }
            rebuild_page_tree(&mut doc, &keep)?;
            write_doc(&mut doc, out_tmp)?;
            Ok(format!("在第 {} 页前插入 {} 张空白页", at, count))
        }
        "crop" => {
            for &p in &targets {
                let pid = ordered[(p - 1) as usize];
                let mb = page_media_box(&doc, pid);
                let box4 = [
                    mb[0] + margins[0],
                    mb[1] + margins[3],
                    mb[2] - margins[2],
                    mb[3] - margins[1],
                ];
                if box4[2] <= box4[0] || box4[3] <= box4[1] {
                    return Err(AppError::internal("裁剪边距过大，页面尺寸将变为负数"));
                }
                let arr: Vec<Object> = box4.iter().map(|&v| Object::Real(v)).collect();
                if let Ok(d) = doc
                    .get_object_mut(pid)
                    .map_err(lopdf_error)?
                    .as_dict_mut()
                {
                    d.set("CropBox", arr);
                }
            }
            write_doc(&mut doc, out_tmp)?;
            Ok(format!("裁剪 {} 页", targets.len()))
        }
        _ => Err(AppError::internal(format!("未知页面操作：{}", mode))),
    }
}

// ---------------------------------------------------------------------------
// F13：水印 / 页眉页脚 / 页码 / 元数据 / 加密
// ---------------------------------------------------------------------------

/// 往页面资源里注册一个 FOnce 字体与透明度 ExtGState，返回内容流前缀所需资源名。
fn ensure_text_resources(doc: &mut Document, page_id: ObjectId, opacity: f32) -> AppResult<()> {
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let gs_id = doc.add_object(dictionary! {
        "Type" => "ExtGState",
        "CA" => Object::Real(opacity),
        "ca" => Object::Real(opacity),
    });
    let res = doc
        .get_or_create_resources(page_id)
        .map_err(lopdf_error)?
        .as_dict_mut()
        .map_err(lopdf_error)?;
    if !res.has(b"Font") {
        res.set("Font", Dictionary::new());
    }
    if let Ok(fonts) = res.get_mut(b"Font").and_then(Object::as_dict_mut) {
        fonts.set("FOnce", Object::Reference(font_id));
    }
    if let Ok(gs) = res.get_mut(b"ExtGState").and_then(Object::as_dict_mut) {
        gs.set("GSOnce", Object::Reference(gs_id));
    } else {
        let mut g = Dictionary::new();
        g.set("GSOnce", Object::Reference(gs_id));
        res.set("ExtGState", g);
    }
    Ok(())
}

fn text_show(size: f32, x: f32, y: f32, literal: &str, gray: f32) -> String {
    format!(
        "q\n/GSOnce gs\n{} {} {} rg\nBT\n/FOnce {} Tf\n1 0 0 1 {} {} Tm\n({}) Tj\nET\nQ\n",
        gray, gray, gray, size, x, y, literal
    )
}

fn set_metadata(doc: &mut Document, title: &str, author: &str, subject: &str, keywords: &str) -> AppResult<()> {
    let mut info = Dictionary::new();
    for (key, value) in [
        ("Title", title),
        ("Author", author),
        ("Subject", subject),
        ("Keywords", keywords),
    ] {
        if !value.trim().is_empty() {
            info.set(key, Object::string_literal(value.to_string()));
        }
    }
    info.set("Producer", Object::string_literal("PayOnce PDF (local)".to_string()));
    info.set("ModDate", Object::string_literal(now_pdf_date()));
    let id = doc.add_object(Object::Dictionary(info));
    doc.trailer.set("Info", Object::Reference(id));
    Ok(())
}

/// F13 主流程（不含加密，加密由命令层用 qpdf 在临时结果上再跑一次）。
#[allow(clippy::too_many_arguments)]
pub fn decorate(
    path: &Path,
    password: Option<&str>,
    watermark: &str,
    opacity: f32,
    angle: f32,
    watermark_size: f32,
    header: &str,
    footer: &str,
    page_numbers: bool,
    number_position: &str,
    title: &str,
    author: &str,
    subject: &str,
    keywords: &str,
    out_tmp: &Path,
) -> AppResult<usize> {
    // 提前校验：任何不支持的字符都必须在写文件前失败。
    for s in [watermark, header, footer] {
        if !s.trim().is_empty() {
            winansi_literal(s)?;
        }
    }
    let mut doc = pdf::load_doc(path, password)?;
    let page_map = doc.get_pages();
    let count = page_map.len();
    if count == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let rad = angle.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    for (i, (_p, pid)) in page_map.iter().enumerate() {
        let mb = page_media_box(&doc, *pid);
        let w = mb[2] - mb[0];
        let h = mb[3] - mb[1];
        let mut content = String::new();
        if !watermark.trim().is_empty() {
            let lit = winansi_literal(watermark)?;
            let cx = mb[0] + w / 2.0;
            let cy = mb[1] + h / 2.0;
            // 以 Helvetica 近似字宽居中（winansi 每字符 ~0.55em）
            let approx = lit.chars().count() as f32 * watermark_size * 0.55;
            content.push_str(&format!(
                "q\n/GSOnce gs\nBT\n/FOnce {} Tf\n{} {} {} {} {} {} Tm\n({}) Tj\nET\nQ\n",
                watermark_size, cos, sin, -sin, cos, cx - approx / 2.0, cy, lit
            ));
        }
        if !header.trim().is_empty() {
            content.push_str(&text_show(
                9.0,
                mb[0] + 36.0,
                mb[3] - 24.0,
                &winansi_literal(header)?,
                0.35,
            ));
        }
        if !footer.trim().is_empty() {
            content.push_str(&text_show(
                9.0,
                mb[0] + 36.0,
                mb[1] + 20.0,
                &winansi_literal(footer)?,
                0.35,
            ));
        }
        if page_numbers {
            let label = format!("{} / {}", i + 1, count);
            let (x, y) = match number_position {
                "headerRight" => (mb[2] - 64.0, mb[3] - 24.0),
                "footerRight" => (mb[2] - 64.0, mb[1] + 20.0),
                _ => (mb[0] + w / 2.0 - 18.0, mb[1] + 20.0),
            };
            content.push_str(&text_show(9.0, x, y, &label, 0.35));
        }
        if !content.is_empty() {
            ensure_text_resources(&mut doc, *pid, opacity.clamp(0.05, 1.0))?;
            doc.add_page_contents(*pid, content.into_bytes())
                .map_err(lopdf_error)?;
        }
    }
    set_metadata(&mut doc, title, author, subject, keywords)?;
    write_doc(&mut doc, out_tmp)?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// F14：导出 TXT / HTML / DOCX / XLSX（文字层，不还原版式）
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// 极简 ZIP（store，无压缩）：OOXML 允许 stored 条目，Word/Excel 可正常打开。
/// 不引入 zip crate，避免新增依赖与许可证审查。
fn zip_store(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;
    let (dos_time, dos_date) = (0u16, 0x5821u16); // 1980-01-01，固定以获得可复现输出
    for (name, data) in entries {
        let nb = name.as_bytes();
        let crc = crc32(data);
        let len = data.len() as u32;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 文件名
        out.extend_from_slice(&0u16.to_le_bytes()); // store
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(nb);
        out.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&dos_time.to_le_bytes());
        central.extend_from_slice(&dos_date.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(nb);

        offset += 30 + nb.len() as u32 + len;
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

const CT_DOCX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const RELS_ROOT_DOCX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const CT_XLSX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#;
const RELS_ROOT_XLSX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="PayOnce PDF" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#;

fn docx_bytes(pages: &[String]) -> Vec<u8> {
    let mut body = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
    );
    for (i, page) in pages.iter().enumerate() {
        if i > 0 {
            body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
        }
        for line in page.lines() {
            body.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            body.push_str(&xml_escape(line));
            body.push_str("</w:t></w:r></w:p>");
        }
        if page.is_empty() {
            body.push_str("<w:p><w:r><w:t xml:space=\"preserve\">[本页无可提取文字，可能为扫描件]</w:t></w:r></w:p>");
        }
    }
    body.push_str("<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr></w:body></w:document>");
    zip_store(&[
        ("[Content_Types].xml".into(), CT_DOCX.as_bytes().to_vec()),
        ("_rels/.rels".into(), RELS_ROOT_DOCX.as_bytes().to_vec()),
        ("word/document.xml".into(), body.into_bytes()),
    ])
}

fn xlsx_bytes(pages: &[String]) -> Vec<u8> {
    let mut rows = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    let mut r = 1usize;
    for (pi, page) in pages.iter().enumerate() {
        for line in page.lines() {
            rows.push_str(&format!(
                "<row r=\"{}\"><c r=\"A{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c></row>",
                r,
                r,
                xml_escape(line)
            ));
            r += 1;
        }
        if page.is_empty() {
            rows.push_str(&format!(
                "<row r=\"{}\"><c r=\"A{}\" t=\"inlineStr\"><is><t>[第 {} 页无可提取文字]</t></is></c></row>",
                r, r, pi + 1
            ));
            r += 1;
        }
    }
    rows.push_str("</sheetData></worksheet>");
    zip_store(&[
        ("[Content_Types].xml".into(), CT_XLSX.as_bytes().to_vec()),
        ("_rels/.rels".into(), RELS_ROOT_XLSX.as_bytes().to_vec()),
        ("xl/workbook.xml".into(), WORKBOOK.as_bytes().to_vec()),
        ("xl/_rels/workbook.xml.rels".into(), WORKBOOK_RELS.as_bytes().to_vec()),
        ("xl/worksheets/sheet1.xml".into(), rows.into_bytes()),
    ])
}

/// F14 主流程：逐页提取文字层 → 输出指定格式。
pub fn export_text(
    ctx: &JobCtx<'_>,
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    format: &str,
    pages: Option<&str>,
    out_tmp: &Path,
) -> AppResult<(usize, usize)> {
    let fmt = match format {
        "txt" | "html" | "docx" | "xlsx" => format,
        _ => return Err(AppError::internal(format!("不支持的导出格式：{}", format))),
    };
    with_unrotated(path, out_tmp, |effective| {
    let doc = pdfium
        .load_pdf_from_file(effective, password)
        .map_err(pdf_error)?;
    let total = doc.pages().len() as u32;
    let targets = parse_pages(pages, total)?;
    let mut page_texts: Vec<String> = Vec::new();
    for (k, &p) in targets.iter().enumerate() {
        ctx.tick(k + 1, targets.len(), format!("提取第 {} 页文字", p))?;
        let page = doc
            .pages()
            .get((p - 1) as PdfPageIndex)
            .map_err(pdf_error)?;
        let text = page.text().map_err(pdf_error)?;
        page_texts.push(text.all());
    }
    let chars: usize = page_texts.iter().map(|s| s.chars().count()).sum();
    let bytes: Vec<u8> = match fmt {
        "txt" => page_texts
            .iter()
            .enumerate()
            .map(|(i, s)| format!("===== 第 {} 页 =====\n{}\n", i + 1, s))
            .collect::<String>()
            .into_bytes(),
        "html" => {
            let mut h = String::from("<!doctype html><html lang=\"zh\"><head><meta charset=\"utf-8\"><title>PayOnce PDF 本地导出</title></head><body>\n");
            for (i, s) in page_texts.iter().enumerate() {
                h.push_str(&format!("<section data-page=\"{}\">\n", i + 1));
                for line in s.lines() {
                    h.push_str(&format!("<p>{}</p>\n", xml_escape(line)));
                }
                h.push_str("</section>\n");
            }
            h.push_str("</body></html>\n");
            h.into_bytes()
        }
        "docx" => docx_bytes(&page_texts),
        "xlsx" => xlsx_bytes(&page_texts),
        _ => unreachable!(),
    };
        std::fs::write(out_tmp, bytes).map_err(AppError::from)?;
        Ok((targets.len(), chars))
    })
}

// ---------------------------------------------------------------------------
// 单元测试：只测纯逻辑，不依赖 Tauri / PDFium / 磁盘
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn pages_parser() {
        assert_eq!(parse_pages(None, 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_pages(Some("all"), 2).unwrap(), vec![1, 2]);
        assert_eq!(parse_pages(Some("1-3,5,5,8"), 10).unwrap(), vec![1, 2, 3, 5, 8]);
        assert!(parse_pages(Some("0"), 5).is_err());
        assert!(parse_pages(Some("9"), 5).is_err());
        assert!(parse_pages(Some("3-1"), 5).is_err());
        assert!(parse_pages(Some("abc"), 5).is_err());
    }

    #[test]
    fn range_tokenizer() {
        assert_eq!(range_tokens("1-5, 8").unwrap(), vec!["1-5", "8"]);
        assert!(range_tokens("1;drop").is_err());
    }

    #[test]
    fn color_parser() {
        let c = parse_color("#FFD54F").unwrap();
        assert!((c[0] - 1.0).abs() < 1e-9);
        assert!(parse_color("red").is_err());
    }

    #[test]
    fn winansi_guard() {
        assert_eq!(winansi_literal("Draft (v2)").unwrap(), "Draft \\(v2\\)");
        assert!(winansi_literal("机密").is_err());
    }

    #[test]
    fn finder_case_and_word() {
        let h: Vec<char> = "Cat cat scatter cat".chars().collect();
        let n: Vec<char> = "cat".chars().collect();
        // "Cat cat scatter cat"：cat 在 0 / 4 / 9(scatter 内) / 16 共 4 处；
        // 区分大小写排除 0；全词匹配排除 9。
        assert_eq!(find_all(&h, &n, false, false).len(), 4);
        assert_eq!(find_all(&h, &n, true, false).len(), 3);
        assert_eq!(find_all(&h, &n, false, true).len(), 3);
    }

    #[test]
    fn cross_object_split() {
        let runs = vec![
            TextRun { object_index: 0, text: "Hello ".into() },
            TextRun { object_index: 1, text: "World".into() },
        ];
        let (chars, owner) = {
            let mut chars = Vec::new();
            let mut owner = Vec::new();
            for (ri, r) in runs.iter().enumerate() {
                for (bi, c) in r.text.char_indices() {
                    owner.push((ri, bi));
                    chars.push(c);
                }
            }
            (chars, owner)
        };
        assert_eq!(chars.iter().collect::<String>(), "Hello World");
        let groups = split_match(&owner, &runs, 4, 9);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].3, true);
        assert_eq!(groups[1].3, false);
        let edits: Vec<(usize, usize, Option<&str>)> = vec![(0, 5, Some("Hi"))];
        assert_eq!(apply_edits("Hello ", &edits), "Hi ");
    }

    #[test]
    fn zip_is_structurally_valid() {
        let data = zip_store(&[("a.txt".into(), b"hello".to_vec())]);
        assert_eq!(&data[0..4], &0x0403_4b50u32.to_le_bytes());
        assert_eq!(&data[data.len() - 22..data.len() - 18], &0x0605_4b50u32.to_le_bytes());
        assert_eq!(crc32(b"hello"), 0x3610_a686);
    }

    /// 产出真实 .docx / .xlsx 供外部 Python zipfile 校验（避免只自证 ZIP 头）。
    #[test]
    fn ooxml_samples_are_written() {
        let dir = std::env::temp_dir().join("payoncepdf_ooxml_check");
        std::fs::create_dir_all(&dir).unwrap();
        let pages = vec!["第一段\n第二段 & <标签>".to_string(), String::new()];
        std::fs::write(dir.join("sample.docx"), docx_bytes(&pages)).unwrap();
        std::fs::write(dir.join("sample.xlsx"), xlsx_bytes(&pages)).unwrap();
        assert!(std::fs::metadata(dir.join("sample.docx")).unwrap().len() > 500);
        assert!(std::fs::metadata(dir.join("sample.xlsx")).unwrap().len() > 500);
    }

    #[test]
    fn xml_escape_basic() {
        assert_eq!(xml_escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
    }

    /// 生成一个最小合法 2 页 PDF（不依赖任何外部文件/PDFium）。
    fn tiny_pdf_bytes(pages: usize) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut ids = Vec::new();
        for _ in 0..pages {
            let content_id = doc.add_object(Object::Stream(Stream::new(
                Dictionary::new(),
                b"q Q\n".to_vec(),
            )));
            let pid = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => vec![
                    Object::Integer(0), Object::Integer(0),
                    Object::Integer(595), Object::Integer(842),
                ],
                "Contents" => Object::Reference(content_id),
                "Resources" => Object::Dictionary(Dictionary::new()),
            });
            ids.push(pid);
        }
        let kids: Vec<Object> = ids.iter().map(|&id| Object::Reference(id)).collect();
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Count" => pages as i64,
                "Kids" => kids,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));
        pdf::save_to_bytes(&mut doc).unwrap()
    }

    fn temp_case(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("payoncepdf_extras_{}_{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.join("in.pdf"), dir.join("out.pdf"))
    }

    /// 真实文件级：水印 + 页码 + 元数据写入后，重新载入 PDF 校验内容流与 /Info。
    #[test]
    fn decorate_writes_real_pdf() {
        let (input, output) = temp_case("decorate");
        std::fs::write(&input, tiny_pdf_bytes(2)).unwrap();
        let count = decorate(
            &input,
            None,
            "DRAFT",
            0.3,
            45.0,
            48.0,
            "",
            "",
            true,
            "footerCenter",
            "测试标题",
            "Author",
            "Subject",
            "keywords",
            &output,
        )
        .unwrap();
        assert_eq!(count, 2);
        let doc = Document::load(&output).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let pid = *doc.get_pages().get(&1).unwrap();
        let content = String::from_utf8_lossy(&doc.get_page_content(pid).unwrap()).to_string();
        assert!(content.contains("(DRAFT)"), "水印未写入内容流: {}", content);
        assert!(content.contains("(1 / 2)"), "页码未写入内容流: {}", content);
        let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let info = doc.get_object(info_id).unwrap().as_dict().unwrap();
        let title = info.get(b"Title").unwrap().as_str().unwrap();
        assert_eq!(String::from_utf8_lossy(title), "测试标题");
        // 中文水印必须被明确拒绝，而不是静默写成乱码。
        assert!(decorate(
            &input, None, "机密", 0.3, 45.0, 48.0, "", "", false,
            "footerCenter", "", "", "", "", &output,
        )
        .is_err());
    }

    /// 真实文件级：旋转 / 删除 / 插入 / 裁剪后页数与页面字典正确。
    #[test]
    fn page_ops_writes_real_pdf() {
        let (input, output) = temp_case("pages");
        std::fs::write(&input, tiny_pdf_bytes(3)).unwrap();

        // 旋转第 1 页 90°
        page_ops(None, &input, None, "rotate", "1", 90, 1, 1, [0.0; 4], &output, input.parent().unwrap())
            .unwrap();
        let doc = Document::load(&output).unwrap();
        let pid = *doc.get_pages().get(&1).unwrap();
        assert_eq!(
            doc.get_object(pid).unwrap().as_dict().unwrap().get(b"Rotate").unwrap().as_i64().unwrap(),
            90
        );

        // 删除第 2 页 → 2 页
        page_ops(None, &input, None, "delete", "2", 0, 1, 1, [0.0; 4], &output, input.parent().unwrap())
            .unwrap();
        assert_eq!(Document::load(&output).unwrap().get_pages().len(), 2);

        // 插入 2 张空白页 → 5 页
        page_ops(None, &input, None, "insert", "all", 0, 2, 2, [0.0; 4], &output, input.parent().unwrap())
            .unwrap();
        assert_eq!(Document::load(&output).unwrap().get_pages().len(), 5);

        // 裁剪第 1 页 → /CropBox 存在且尺寸仍为正
        page_ops(None, &input, None, "crop", "1", 0, 1, 1, [10.0, 10.0, 10.0, 10.0], &output, input.parent().unwrap())
            .unwrap();
        let doc = Document::load(&output).unwrap();
        let pid = *doc.get_pages().get(&1).unwrap();
        let arr = doc.get_object(pid).unwrap().as_dict().unwrap().get(b"CropBox").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 4);
    }

    // -----------------------------------------------------------------------
    // 引擎级端到端：真实 PDF 文件 + 真实 PDFium DLL + qpdf --check。
    // 与「cargo check 编译通过」的区别：这里真的写文件、真的重新打开读回文字。
    // -----------------------------------------------------------------------

    /// PDFium 的 bindings 是进程级 OnceCell，整个测试进程只能 bind 一次。
    /// 与 `engine::render::tests` 共用同一个实例，避免两个测试模块并行时
    /// 第二次 `bind_to_library` 抛 PdfiumLibraryBindingsAlreadyInitialized。
    fn test_pdfium() -> &'static Pdfium {
        crate::engine::render::tests::test_pdfium()
    }

    fn ascii_pdf_bytes(text: &str) -> Vec<u8> {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        });
        let content = format!("BT /F1 24 Tf 72 700 Td ({}) Tj ET\n", escaped);
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));
        let mut res = Dictionary::new();
        res.set(
            "Font",
            Object::Dictionary(dictionary! { "F1" => Object::Reference(font_id) }),
        );
        let pid = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(595), Object::Integer(842),
            ],
            "Contents" => Object::Reference(content_id),
            "Resources" => Object::Dictionary(res),
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Count" => 1i64,
                "Kids" => vec![Object::Reference(pid)],
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));
        pdf::save_to_bytes(&mut doc).unwrap()
    }

    /// 用系统简体中文 TTF 生成一份真正嵌入 CJK 字形的 PDF（不是字体回退的假中文）。
    fn cjk_pdf(pdfium: &Pdfium, path: &Path, text: &str) {
        let font_file = PathBuf::from("C:/Windows/Fonts/simhei.ttf");
        let mut doc = pdfium.create_new_pdf().unwrap();
        let font = doc
            .fonts_mut()
            .load_true_type_from_file(&font_file, true)
            .unwrap();
        let mut page = doc
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::a4())
            .unwrap();
        page.objects_mut()
            .create_text_object(
                PdfPoints::new(72.0),
                PdfPoints::new(700.0),
                text,
                font,
                PdfPoints::new(24.0),
            )
            .unwrap();
        doc.save_to_file(path).unwrap();
    }

    fn page_text(pdfium: &Pdfium, path: &Path) -> String {
        let doc = pdfium.load_pdf_from_file(path, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let text = page.text().unwrap().all();
        text
    }

    /// 用独立 qpdf 进程做结构校验（不经过 AppHandle）。qpdf 缺失时跳过而非伪装通过。
    fn qpdf_check(path: &Path) -> bool {
        let exe = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.sidecar-cache/qpdf/qpdf-11.9.1-mingw64/bin/qpdf.exe");
        if !exe.exists() {
            eprintln!("qpdf.exe 缺失，跳过结构校验（不视为通过）");
            return false;
        }
        match std::process::Command::new(&exe)
            .arg("--check")
            .arg(path)
            .output()
        {
            Ok(o) => {
                if !o.status.success() {
                    eprintln!("qpdf --check 失败：{}", String::from_utf8_lossy(&o.stdout));
                }
                o.status.success()
            }
            Err(e) => {
                eprintln!("qpdf 无法启动：{}", e);
                false
            }
        }
    }

    fn offline_ctx(cancel: &AtomicBool) -> JobCtx<'_> {
        JobCtx {
            app: None,
            job_id: "e2e",
            cancel,
            emit: false,
        }
    }

    /// F10：ASCII → ASCII 替换后重新读回文本，且输出通过 qpdf 结构校验。
    #[test]
    fn e2e_edit_ascii_real_pdf() {
        let pdfium = test_pdfium();
        let (input, output) = temp_case("edit_ascii");
        std::fs::write(&input, ascii_pdf_bytes("Hello World")).unwrap();
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let out = replace_text(
            &ctx, pdfium, &input, None, "World", "PayOnce PDF", true, false, None, &output,
        )
        .unwrap();
        assert_eq!(out.replacements, 1);
        let text = page_text(pdfium, &output);
        assert!(text.contains("PayOnce PDF"), "替换后读回文本 = {:?}", text);
        assert!(!text.contains("World"));
        assert_eq!(std::fs::read(&input).unwrap(), ascii_pdf_bytes("Hello World"));
        assert!(qpdf_check(&output), "改字输出未通过 qpdf --check");
    }

    /// F10：嵌入 CJK 字体的真实中文 PDF → 改数字 / 改汉字，验证「支持中文」不是空话。
    #[test]
    fn e2e_edit_cjk_real_pdf() {
        if !PathBuf::from("C:/Windows/Fonts/simhei.ttf").exists() {
            eprintln!("系统无 simhei.ttf，跳过中文验证");
            return;
        }
        let pdfium = test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_e2e_cjk_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("cjk.pdf");
        cjk_pdf(pdfium, &input, "合同编号 2026 年版");
        let before = page_text(pdfium, &input);
        assert!(before.contains("合同"), "生成的中文字体 PDF 读回 = {:?}", before);

        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let digits = dir.join("digits.pdf");
        replace_text(&ctx, pdfium, &input, None, "2026", "2027", true, false, None, &digits)
            .unwrap();
        assert!(page_text(pdfium, &digits).contains("2027"));

        let chinese = dir.join("chinese.pdf");
        replace_text(&ctx, pdfium, &input, None, "合同", "协议", true, false, None, &chinese)
            .expect("嵌入完整 CJK 字体的文档应能替换汉字");
        let after = page_text(pdfium, &chinese);
        assert!(after.contains("协议"), "中文替换后读回 = {:?}", after);
        assert!(qpdf_check(&chinese), "中文改字输出未通过 qpdf --check");
    }

    /// F10 安全闸门：原字体无中文字形时必须报错且不写半成品。
    #[test]
    fn e2e_edit_missing_glyph_fails_closed() {
        let pdfium = test_pdfium();
        let (input, output) = temp_case("edit_glyph");
        std::fs::write(&input, ascii_pdf_bytes("Hello World")).unwrap();
        let _ = std::fs::remove_file(&output);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text(
            &ctx, pdfium, &input, None, "World", "世界", true, false, None, &output,
        );
        assert!(r.is_err(), "Helvetica 不含中文字形，必须明确失败");
        assert!(!output.exists(), "失败时不得写出乱码 PDF");
    }

    /// F11：写出标准 /Highlight 注解 + /QuadPoints，并重新载入校验。
    #[test]
    fn e2e_annotate_real_pdf() {
        let pdfium = test_pdfium();
        let (input, output) = temp_case("annot_e2e");
        std::fs::write(&input, ascii_pdf_bytes("Hello World")).unwrap();
        let outcome = annotate(
            pdfium, &input, None, "World", "highlight", "#FFD54F", "Tester", "核对",
            true, None, &output,
        )
        .unwrap();
        assert!(outcome.matches >= 1 && outcome.quads >= 1);
        let doc = Document::load(&output).unwrap();
        let pid = *doc.get_pages().get(&1).unwrap();
        let annots = doc
            .get_object(pid)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        let annot = doc
            .get_object(annots[0].as_reference().unwrap())
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(annot.get(b"Subtype").unwrap().as_name().unwrap()),
            "Highlight"
        );
        assert!(annot.get(b"QuadPoints").is_ok());
        assert!(qpdf_check(&output), "批注输出未通过 qpdf --check");
    }

    /// F16：中文 TXT → PDF，重新打开验证中文真的进了文本层（不是方框）。
    #[test]
    fn e2e_text_to_pdf_with_cjk() {
        if crate::engine::more::system_cjk_font().is_err() {
            eprintln!("系统无 CJK 字体，跳过");
            return;
        }
        let pdfium = test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_e2e_txt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("note.txt");
        let out = dir.join("note.pdf");
        std::fs::write(&src, "合同编号 2027\n第一章 交付验收\nHello World").unwrap();
        let pages = crate::engine::more::text_to_pdf(pdfium, &src, "txt", &out).unwrap();
        assert_eq!(pages, 1);
        let text = compact(&page_text(pdfium, &out));
        assert!(text.contains("合同编号"), "中文文本层 = {:?}", text);
        assert!(text.contains("Hello"));
        assert!(qpdf_check(&out), "文本转 PDF 输出未通过 qpdf --check");
    }

    /// F16：中文水印用嵌入 CJK 字体写入，原文字层不受损。
    #[test]
    fn e2e_cjk_watermark_real_pdf() {
        if crate::engine::more::system_cjk_font().is_err() {
            eprintln!("系统无 CJK 字体，跳过");
            return;
        }
        let pdfium = test_pdfium();
        let (input, output) = temp_case("cjk_wm");
        std::fs::write(&input, ascii_pdf_bytes("Hello World")).unwrap();
        let n = crate::engine::more::cjk_watermark(
            pdfium, &input, None, "机密文件", 36.0, &output,
        )
        .unwrap();
        assert_eq!(n, 1);
        let text = compact(&page_text(pdfium, &output));
        assert!(text.contains("机密文件"), "中文水印未写入：{:?}", text);
        assert!(text.contains("Hello"));
        assert!(qpdf_check(&output), "中文水印输出未通过 qpdf --check");
    }

    /// F15 脱敏：必须真删除文字对象（复制粘贴也取不到）并留下黑框内容流。
    #[test]
    fn e2e_redact_really_removes_text() {
        let pdfium = test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_e2e_redact_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.pdf");
        let out = dir.join("redacted.pdf");
        std::fs::write(&input, ascii_pdf_bytes("Account 1234 5678 secret")).unwrap();
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let terms = vec!["1234".to_string(), "secret".to_string(), "NOMATCH".to_string()];
        let o = crate::engine::objects::redact(
            &ctx, pdfium, &input, None, &terms, true, None, &out, &dir,
        )
        .unwrap();
        assert_eq!(o.terms_removed, 2, "未命中的词应跳过而不是伪装成功");
        assert!(o.boxes >= 2);
        let text = page_text(pdfium, &out);
        assert!(!text.contains("1234"), "脱敏后文字层仍含敏感词：{:?}", text);
        assert!(!text.contains("secret"));
        let doc = Document::load(&out).unwrap();
        let pid = *doc.get_pages().get(&1).unwrap();
        let content = String::from_utf8_lossy(&doc.get_page_content(pid).unwrap()).to_string();
        assert!(content.contains(" re f"), "未写入黑框内容流");
        assert!(qpdf_check(&out), "脱敏输出未通过 qpdf --check");
    }

    /// F11 兼容回归：旋转过的页面仍能定位文字并写出注解（去旋转副本）。
    #[test]
    fn e2e_annotate_rotated_page() {
        let pdfium = test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_e2e_rot_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.pdf");
        let rotated = dir.join("rotated.pdf");
        let out = dir.join("annotated.pdf");
        std::fs::write(&a, ascii_pdf_bytes("Hello World")).unwrap();
        page_ops(None, &a, None, "rotate", "1", 90, 1, 1, [0.0; 4], &rotated, &dir).unwrap();
        let o = annotate(
            pdfium, &rotated, None, "World", "highlight", "#FFD54F", "", "", true, None, &out,
        )
        .unwrap();
        assert!(o.matches >= 1 && o.quads >= 1);
        assert!(qpdf_check(&out));
    }

    /// F14：四种格式都从真实 PDF 文字层导出，DOCX/XLSX 必须是合法 ZIP。
    #[test]
    fn e2e_export_real_pdf_all_formats() {
        let pdfium = test_pdfium();
        let (input, _) = temp_case("export_e2e");
        std::fs::write(&input, ascii_pdf_bytes("Hello World")).unwrap();
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        for fmt in ["txt", "html", "docx", "xlsx"] {
            let out = input.with_file_name(format!("out.{}", fmt));
            let (pages, chars) =
                export_text(&ctx, pdfium, &input, None, fmt, None, &out).unwrap();
            assert_eq!(pages, 1);
            assert!(chars >= 11, "{} 导出字符数异常：{}", fmt, chars);
            let bytes = std::fs::read(&out).unwrap();
            assert!(bytes.len() > 30, "{} 输出过小", fmt);
            match fmt {
                "txt" | "html" => assert!(String::from_utf8_lossy(&bytes).contains("Hello")),
                _ => assert_eq!(&bytes[0..2], b"PK", "{} 不是 ZIP", fmt),
            }
        }
    }

    /// 组合链路：改字 → 高亮 → 旋转 → 导出，验证每一步产物都能被下一步正常读取。
    #[test]
    fn e2e_pipeline_edit_annotate_pages_export() {
        let pdfium = test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_e2e_pipe_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.pdf");
        let b = dir.join("b.pdf");
        let c = dir.join("c.pdf");
        std::fs::write(&a, ascii_pdf_bytes("Draft 2026 contract")).unwrap();
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        replace_text(&ctx, pdfium, &a, None, "2026", "2027", true, false, None, &b).unwrap();
        annotate(pdfium, &b, None, "contract", "underline", "#33AA66", "", "", true, None, &c)
            .unwrap();
        // 旋转页会触发 PDFium 文本提取为空的兼容问题；导出必须仍能拿到文字。
        let rotated = dir.join("rotated.pdf");
        page_ops(
            None, &c, None, "rotate", "1", 90, 1, 1, [0.0; 4], &rotated, &dir,
        )
        .unwrap();
        let txt = dir.join("final.txt");
        export_text(&ctx, pdfium, &rotated, None, "txt", None, &txt).unwrap();
        let final_text = std::fs::read_to_string(&txt).unwrap();
        assert!(final_text.contains("2027"), "链路最终文本 = {:?}", final_text);
        assert!(qpdf_check(&rotated), "链路最终 PDF 未通过 qpdf --check");
    }

    // -----------------------------------------------------------------------
    // F19：按矩形精确替换（同页重复文字只改框选那一处）
    // -----------------------------------------------------------------------

    /// 两条独立文本对象、都含 "Draft" 的一页 PDF：用来验证 F19 不会像
    /// 「按文本替换」那样把两处一起改掉。
    fn duplicate_pdf_bytes() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        });
        let content = "BT /F1 24 Tf 72 700 Td (Draft one) Tj ET\n\
                       BT /F1 24 Tf 72 620 Td (Draft two) Tj ET\n";
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.as_bytes().to_vec(),
        )));
        let mut res = Dictionary::new();
        res.set(
            "Font",
            Object::Dictionary(dictionary! { "F1" => Object::Reference(font_id) }),
        );
        let pid = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(595), Object::Integer(842),
            ],
            "Contents" => Object::Reference(content_id),
            "Resources" => Object::Dictionary(res),
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Count" => 1i64,
                "Kids" => vec![Object::Reference(pid)],
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));
        pdf::save_to_bytes(&mut doc).unwrap()
    }

    /// 用真实 PDFium 字符框取「第 occurrence 次出现的 word」的外接矩形（模拟前端框选）。
    fn rect_around_word(
        pdfium: &Pdfium,
        path: &Path,
        word: &str,
        occurrence: usize,
    ) -> (f32, f32, f32, f32) {
        let doc = pdfium.load_pdf_from_file(path, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let text = page.text().unwrap();
        let mut chars: Vec<char> = Vec::new();
        let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
        for c in text.chars().iter() {
            let b = c
                .tight_bounds()
                .or_else(|_| c.loose_bounds())
                .unwrap_or_else(|_| PdfRect::zero());
            chars.push(c.unicode_char().unwrap_or('?'));
            boxes.push((
                b.left().value,
                b.bottom().value,
                b.right().value,
                b.top().value,
            ));
        }
        let needle: Vec<char> = word.chars().collect();
        let mut seen = 0usize;
        let mut start = None;
        for i in 0..chars.len().saturating_sub(needle.len() - 1) {
            if chars[i..i + needle.len()] == needle[..] {
                if seen == occurrence {
                    start = Some(i);
                    break;
                }
                seen += 1;
            }
        }
        let s = start.unwrap_or_else(|| panic!("第 {} 次出现的 {:?} 不存在", occurrence, word));
        let slice = &boxes[s..s + needle.len()];
        let mut rect = slice[0];
        for b in &slice[1..] {
            rect.0 = rect.0.min(b.0);
            rect.1 = rect.1.min(b.1);
            rect.2 = rect.2.max(b.2);
            rect.3 = rect.3.max(b.3);
        }
        rect
    }

    /// F19 核心：同页两处 "Draft"，只改框选的第一处。
    #[test]
    fn e2e_rect_edit_only_touches_selected_occurrence() {
        let pdfium = test_pdfium();
        let (input, output) = temp_case("rect_edit");
        std::fs::write(&input, duplicate_pdf_bytes()).unwrap();
        let before = page_text(pdfium, &input);
        assert_eq!(count_str(&before, "Draft"), 2, "夹具应含两处 Draft：{:?}", before);

        let rect = rect_around_word(pdfium, &input, "Draft", 0);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let out = replace_text_at_rect(
            &ctx, pdfium, &input, None, 1, rect, "Final", Some("Draft"), &output,
        )
        .expect("按位置替换应成功");

        assert_eq!(out.page, 1);
        assert_eq!(out.old_text, "Draft");
        assert_eq!(out.selected_chars, 5);
        let after = page_text(pdfium, &output);
        assert_eq!(count_str(&after, "Draft"), 1, "只应改掉一处：{:?}", after);
        assert!(after.contains("Final one"), "第一处应变成 Final one：{:?}", after);
        assert!(after.contains("Draft two"), "第二处必须保持原样：{:?}", after);
        assert_eq!(std::fs::read(&input).unwrap(), duplicate_pdf_bytes(), "原文件不得被修改");
        assert!(qpdf_check(&output), "按位置替换输出未通过 qpdf --check");
    }

    /// F19 安全闸门：框空白 / 与预览文字不一致 / 跨行，都必须报错且不写半成品。
    #[test]
    fn rect_edit_fails_closed() {
        let pdfium = test_pdfium();
        let (input, output) = temp_case("rect_edit_gate");
        std::fs::write(&input, duplicate_pdf_bytes()).unwrap();
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let _ = std::fs::remove_file(&output);

        // 1) 框在页面空白处
        let blank = (300.0, 300.0, 400.0, 340.0);
        let e = replace_text_at_rect(&ctx, pdfium, &input, None, 1, blank, "X", None, &output)
            .err()
            .expect("空白框选必须报错");
        assert!(e.to_string().contains("没有可选文字"), "{}", e);
        assert!(!output.exists(), "失败时不得留下输出文件");

        // 2) 预览文字与引擎不一致（页面在预览后被改过）
        let rect = rect_around_word(pdfium, &input, "Draft", 0);
        let e = replace_text_at_rect(
            &ctx, pdfium, &input, None, 1, rect, "X", Some("Drafx"), &output,
        )
        .err()
        .expect("预览不一致必须报错");
        assert!(e.to_string().contains("不一致"), "{}", e);
        assert!(!output.exists());

        // 3) 一次框住两行 → 跨行必须拒绝
        let both = (60.0, 610.0, 300.0, 730.0);
        let e = replace_text_at_rect(&ctx, pdfium, &input, None, 1, both, "X", None, &output)
            .err()
            .expect("跨行框选必须报错");
        assert!(e.to_string().contains("跨") || e.to_string().contains("多行"), "{}", e);
        assert!(!output.exists());
    }

    /// 真实中文样本（内嵌 SimHei 子集 + Identity-H + ToUnicode）：
    /// 验证 F19 能否按坐标把中文改掉，且不改动同页其它中文。
    #[test]
    fn e2e_rect_edit_real_cjk_pdf() {
        let pdfium = test_pdfium();
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/text_cjk_real.pdf"
        ));
        if !path.exists() {
            eprintln!("跳过：缺 text_cjk_real.pdf（先跑 python scripts/make-cjk-fixture.py）");
            return;
        }
        let (input, output) = temp_case("rect_edit_cjk");
        std::fs::copy(&path, &input).unwrap();
        let before = page_text(pdfium, &input);
        eprintln!("改前 = {:?}", before);
        assert!(before.contains("付款期限为 30 日"), "夹具应含待改句：{:?}", before);

        // 用真实字符框构造「30 日」的矩形（等价于用户在界面上拖选这三个字符）
        let doc = pdfium.load_pdf_from_file(&input, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let (chars, boxes) = {
            let t = page.text().unwrap();
            let mut chars: Vec<char> = Vec::new();
            let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
            for c in t.chars().iter() {
                let ch = c.unicode_char().unwrap_or('\u{FFFD}');
                if ch == '\r' || ch == '\n' {
                    continue;
                }
                let b = c
                    .tight_bounds()
                    .or_else(|_| c.loose_bounds())
                    .unwrap_or_else(|_| PdfRect::zero());
                chars.push(ch);
                boxes.push((
                    b.left().value,
                    b.bottom().value,
                    b.right().value,
                    b.top().value,
                ));
            }
            (chars, boxes)
        };
        drop(page);
        drop(doc);
        let page_str: String = chars.iter().collect();
        let needle = "30 日";
        assert!(page_str.contains(needle), "文字层应含 30 日：{:?}", page_str);
        // 注意：必须按字符下标定位（中文一个字符 3 字节，str::find 返回字节偏移）
        let needle_chars: Vec<char> = needle.chars().collect();
        let s0 = chars
            .windows(needle_chars.len())
            .position(|w| w == &needle_chars[..])
            .expect("字符序列里应能找到 30 日");
        let e0 = s0 + needle_chars.len();
        let mut rect = boxes[s0];
        for i in s0 + 1..e0 {
            let b = boxes[i];
            rect.0 = rect.0.min(b.0);
            rect.1 = rect.1.min(b.1);
            rect.2 = rect.2.max(b.2);
            rect.3 = rect.3.max(b.3);
        }
        let drag = (rect.0 - 1.0, rect.1 - 1.0, rect.2 + 1.0, rect.3 + 1.0);

        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 1, drag, "45 日", Some(needle), &output);
        match r {
            Ok(o) => {
                eprintln!("F19 中文替换成功：old={:?} chars={}", o.old_text, o.selected_chars);
                let after = page_text(pdfium, &output);
                eprintln!("改后 = {:?}", after);
                assert!(after.contains("付款期限为 45 日"), "应改成 45 日：{:?}", after);
                assert!(!after.contains("30 日"), "原文 30 日应已不在：{:?}", after);
                // 同页其它中文必须原样保留
                assert!(after.contains("中文测试"), "其它行不得被改：{:?}", after);
                assert!(after.contains("合同编号：2026 年第 018 号"), "其它行不得被改：{:?}", after);
                assert_eq!(std::fs::read(&input).unwrap(), std::fs::read(&path).unwrap());
                // 把改后页面渲染出来，供人工目视确认字形真的换成了 45 日
                let edited = crate::engine::render::PdfDoc::open(pdfium, &output, None)
                    .expect("改后文件应能重新打开");
                let (png, _, _) = edited.render_page_png(1, 150).expect("改后页面应能渲染");
                let shot = PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../tests/out/cjk-real-edited.png"
                ));
                std::fs::write(&shot, &png).unwrap();
                eprintln!("改后渲染：{}（{} 字节）", shot.display(), png.len());
            }
            Err(e) => {
                eprintln!("F19 中文替换被拒绝：{}", e);
                panic!("真实中文 PDF 的按位置替换应当可用，实际被拒绝：{}", e);
            }
        }
    }

    /// 取一页的字符、紧贴框、宽松框。
    ///
    /// 宽松框（loose_bounds）由字体的 ascent/descent 决定，同一字号下所有字符一致，
    /// 因此可以当「字号代理」用；紧贴框（tight_bounds）是真实墨迹，字母有高低差，
    /// 拿它比字号会误判（"H" 与 "o" 高度就能差 30%）。
    #[allow(clippy::type_complexity)]
    fn page_char_boxes(
        pdfium: &Pdfium,
        path: &Path,
        page_1based: u32,
    ) -> (String, Vec<(f32, f32, f32, f32)>, Vec<(f32, f32, f32, f32)>) {
        let doc = pdfium.load_pdf_from_file(path, None).unwrap();
        let page = doc.pages().get((page_1based - 1) as PdfPageIndex).unwrap();
        let t = page.text().unwrap();
        let mut chars = String::new();
        let mut tight = Vec::new();
        let mut loose = Vec::new();
        for c in t.chars().iter() {
            let ch = c.unicode_char().unwrap_or('\u{FFFD}');
            if ch == '\r' || ch == '\n' {
                continue;
            }
            let tb = c
                .tight_bounds()
                .or_else(|_| c.loose_bounds())
                .unwrap_or_else(|_| PdfRect::zero());
            let lb = c
                .loose_bounds()
                .or_else(|_| c.tight_bounds())
                .unwrap_or_else(|_| PdfRect::zero());
            chars.push(ch);
            tight.push((
                tb.left().value,
                tb.bottom().value,
                tb.right().value,
                tb.top().value,
            ));
            loose.push((
                lb.left().value,
                lb.bottom().value,
                lb.right().value,
                lb.top().value,
            ));
        }
        (chars, tight, loose)
    }

    /// 诊断：英文 / 科学符号 / 上下标 / 分式样本的文字层能不能被 PDFium 正确还原。
    /// 这是回答「这些能不能改」的前提——文字层读不对，后面一律免谈。
    #[test]
    fn debug_symbol_fixture_layout() {
        let pdfium = test_pdfium();
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/text_symbols.pdf"
        ));
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf（先跑 python scripts/make-symbol-fixture.py）");
            return;
        }
        for page_no in 1..=5u32 {
            let (chars, tight, loose) = page_char_boxes(pdfium, &path, page_no);
            eprintln!("--- 第 {} 页 ---", page_no);
            eprintln!("文字层 = {:?}", chars);
            let mut hs: Vec<f32> = loose.iter().map(|b| (b.3 - b.1).abs()).collect();
            hs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if let (Some(&lo), Some(&hi)) = (hs.first(), hs.last()) {
                eprintln!("宽松框高 min={:.2} max={:.2}", lo, hi);
            }
            for (i, ch) in chars.chars().enumerate() {
                let tb = tight[i];
                let lb = loose[i];
                eprintln!(
                    "  [{}] {:?} tight=({:.1},{:.1},{:.1},{:.1}) loose_h={:.2}",
                    i,
                    ch,
                    tb.0,
                    tb.1,
                    tb.2,
                    tb.3,
                    (lb.3 - lb.1).abs()
                );
            }
        }
    }

    /// 在字符序列里找到第 nth 次出现的 needle，返回把它整个框住的拖选矩形。
    fn rect_for(
        hay: &[char],
        boxes: &[(f32, f32, f32, f32)],
        needle: &str,
        nth: usize,
    ) -> (f32, f32, f32, f32) {
        let nc: Vec<char> = needle.chars().collect();
        let (s, _) = hay
            .windows(nc.len())
            .enumerate()
            .filter(|(_, w)| *w == &nc[..])
            .nth(nth)
            .unwrap_or_else(|| panic!("文字层里找不到第 {} 个「{}」", nth, needle));
        let mut r = boxes[s];
        for i in s + 1..s + nc.len() {
            let b = boxes[i];
            r.0 = r.0.min(b.0);
            r.1 = r.1.min(b.1);
            r.2 = r.2.max(b.2);
            r.3 = r.3.max(b.3);
        }
        (r.0 - 1.0, r.1 - 1.0, r.2 + 1.0, r.3 + 1.0)
    }

    fn symbol_fixture() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/text_symbols.pdf"
        ))
    }

    /// 英文正文：内置 Helvetica + WinAnsi，改数字。
    #[test]
    fn e2e_rect_edit_english() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_en");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 1);
        let chars: Vec<char> = chars_s.chars().collect();
        let rect = rect_for(&chars, &tight, "30", 1); // "Payment due in 30 days"
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 1, rect, "45", Some("30"), &output);
        match r {
            Ok(o) => {
                eprintln!("英文改字成功：old={:?}", o.old_text);
                let after = page_text_at(pdfium, &output, 1);
                eprintln!("改后 = {:?}", after);
                assert!(after.contains("Payment due in 45 days"), "应改成 45：{:?}", after);
                assert!(after.contains("Invoice No. 30"), "另一处 30 不得被改：{:?}", after);
            }
            Err(e) => panic!("英文按位置替换应当可用，实际被拒绝：{}", e),
        }
    }

    /// Latin-1 符号：± ° µ × ÷ 都在 WinAnsi 里，内置 Helvetica 也能写。
    #[test]
    fn e2e_rect_edit_latin1_symbols() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_latin1");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 2);
        let chars: Vec<char> = chars_s.chars().collect();
        // 先改一个 Latin-1 符号：× → ÷
        let rect = rect_for(&chars, &tight, "\u{d7}", 0);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 2, rect, "\u{f7}", Some("\u{d7}"), &output);
        match r {
            Ok(_) => {
                let after = page_text_at(pdfium, &output, 2);
                eprintln!("Latin-1 符号改后 = {:?}", after);
                assert!(after.contains("3 \u{f7} 4"), "× 应改成 ÷：{:?}", after);
                // 文字层对不等于画面对：把改后页面渲染出来，确认 ÷ 真的有字形
                let edited = crate::engine::render::PdfDoc::open(pdfium, &output, None)
                    .expect("改后文件应能重新打开");
                let (png, _, _) = edited.render_page_png(2, 150).expect("第 2 页应能渲染");
                let shot = PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../tests/out/symbol-latin1-edited.png"
                ));
                std::fs::write(&shot, &png).unwrap();
                eprintln!("改后渲染：{}（{} 字节）", shot.display(), png.len());
            }
            Err(e) => panic!("内置 WinAnsi 字体的 Latin-1 符号应当可改，实际被拒绝：{}", e),
        }
    }

    /// 数学符号：内嵌 Arial 子集，数字替换应当成功；改成子集里没有的符号必须被拦。
    #[test]
    fn e2e_rect_edit_math_symbols() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_math");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 3);
        let chars: Vec<char> = chars_s.chars().collect();
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);

        let rect = rect_for(&chars, &tight, "3.14", 0);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 3, rect, "2.72", Some("3.14"), &output);
        match r {
            Ok(_) => {
                let after = page_text_at(pdfium, &output, 3);
                eprintln!("数学样本改后 = {:?}", after);
                assert!(after.contains("3 \u{2248} 2.72"), "数字应改成功：{:?}", after);
                assert!(after.contains("\u{2264} 5") && after.contains("\u{2265} 2"),
                        "同页其它符号必须原样：{:?}", after);
                // 目视证据：渲染改后页面，确认 ≈ ≤ ≥ ∑ ∫ √ π Ω 都在，没变成方块
                let edited = crate::engine::render::PdfDoc::open(pdfium, &output, None)
                    .expect("改后文件应能重新打开");
                let (png, _, _) = edited.render_page_png(3, 150).expect("第 3 页应能渲染");
                let shot = PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../tests/out/symbol-math-edited.png"
                ));
                std::fs::write(&shot, &png).unwrap();
                eprintln!("改后渲染：{}（{} 字节）", shot.display(), png.len());
            }
            Err(e) => panic!("内嵌字体数字替换应当可用，实际被拒绝：{}", e),
        }
    }

    /// 数学符号改成一个字体里没有的字符：必须拒绝、不产出输出文件、原文件字节不变。
    #[test]
    fn e2e_rect_edit_missing_glyph_is_rejected() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_missing_glyph");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 3);
        let chars: Vec<char> = chars_s.chars().collect();
        let rect = rect_for(&chars, &tight, "\u{2248}", 0);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        if output.exists() {
            std::fs::remove_file(&output).unwrap();
        }
        // ≠ 不在子集里：字体的 cmap 没有它，PDFium 只能写空格
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 3, rect, "\u{2260}", Some("\u{2248}"), &output);
        match r {
            Ok(_) => panic!("字体缺字形时必须报错，不能静默写坏"),
            Err(e) => {
                eprintln!("缺字形被拒（正确）：{}", e);
                assert!(!output.exists(), "拒绝时不得留下输出文件");
                assert_eq!(std::fs::read(&input).unwrap(), std::fs::read(&path).unwrap(),
                           "拒绝时原文件必须一字未动");
            }
        }
    }

    fn page_text_at(pdfium: &Pdfium, path: &Path, page_1based: u32) -> String {
        let doc = pdfium.load_pdf_from_file(path, None).unwrap();
        let page = doc.pages().get((page_1based - 1) as PdfPageIndex).unwrap();
        let text = page.text().unwrap();
        let all = text.all();
        drop(text);
        drop(page);
        all
    }

    /// 上下标：拖选含上标的 "H2O" 必须被拒（否则会把上标压成正文大小，毁排版）。
    #[test]
    fn e2e_rect_edit_superscript_span_rejected() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_supsub");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 4);
        let chars: Vec<char> = chars_s.chars().collect();
        let rect = rect_for(&chars, &tight, "H2O", 0);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 4, rect, "CO2", Some("H2O"), &output);
        match r {
            Ok(_) => panic!("跨字号的选区（含上标）必须拒绝，否则上标会被压成正文"),
            Err(e) => {
                eprintln!("跨字号被拒：{}", e);
                assert_eq!(std::fs::read(&input).unwrap(), std::fs::read(&path).unwrap(),
                           "拒绝时原文件必须一字未动");
            }
        }
    }

    /// 上下标：只框选上标那一个字符（同字号），应当允许替换。
    #[test]
    fn e2e_rect_edit_superscript_alone() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_sup_alone");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 4);
        let chars: Vec<char> = chars_s.chars().collect();
        // "H2O" 里的那个 2
        let h = chars.windows(3).position(|w| w == &['H', '2', 'O'][..]).unwrap();
        let b = tight[h + 1];
        let rect = (b.0 - 0.5, b.1 - 0.5, b.2 + 0.5, b.3 + 0.5);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 4, rect, "3", Some("2"), &output);
        match r {
            Ok(_) => {
                let after = page_text_at(pdfium, &output, 4);
                eprintln!("上标单独改后 = {:?}", after);
                assert!(after.contains("H3O"), "上标应改成 3：{:?}", after);
            }
            Err(e) => panic!("只框选上标一个字符应当可用，实际被拒绝：{}", e),
        }
    }

    /// 分式：拖选分子+分母（中间夹着一条画出来的横线）必须被拒，不能把两个对象挤成一行。
    #[test]
    fn e2e_rect_edit_fraction_span_rejected() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, output) = temp_case("rect_edit_fraction");
        std::fs::copy(&path, &input).unwrap();
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 5);
        let chars: Vec<char> = chars_s.chars().collect();
        eprintln!("第 5 页文字层 = {:?}", chars_s);
        // 分子 1 与分母 2 在字符序列里相邻
        let i = chars.windows(2).position(|w| w == &['1', '2'][..]).expect("应能找到分子的 1 与分母的 2");
        let (a, b) = (tight[i], tight[i + 1]);
        let rect = (a.0.min(b.0) - 1.0, a.1.min(b.1) - 1.0, a.2.max(b.2) + 1.0, a.3.max(b.3) + 1.0);
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 5, rect, "3", Some("12"), &output);
        match r {
            Ok(_) => panic!("跨行的分式选区必须拒绝"),
            Err(e) => eprintln!("分式跨行被拒：{}", e),
        }
    }

    /// 连续改两处：第二轮必须以第一轮的结果为输入，改动才会累积。
    /// 这正是「只能改一次」的回归测试——旧前端第二次仍从原始文件出发，
    /// 第一处改动会被丢掉。
    #[test]
    fn e2e_rect_edit_chained_two_rounds() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, _) = temp_case("rect_edit_chain");
        std::fs::copy(&path, &input).unwrap();
        let dir = input.parent().unwrap().to_path_buf();
        let round1 = dir.join("round1.pdf");
        let round2 = dir.join("round2.pdf");
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);

        // 第一轮：Invoice No. 30 → 60（第 1 个 30）
        let (chars_s, _, tight) = page_char_boxes(pdfium, &input, 1);
        let chars: Vec<char> = chars_s.chars().collect();
        let r1 = rect_for(&chars, &tight, "30", 0);
        replace_text_at_rect(&ctx, pdfium, &input, None, 1, r1, "60", Some("30"), &round1)
            .expect("第一轮应成功");
        let after1 = page_text_at(pdfium, &round1, 1);
        assert!(after1.contains("Invoice No. 60"), "第一轮应改掉 Invoice 那处：{after1}");

        // 第二轮：以第一轮结果为输入，改 Payment due in 30 days
        let (chars2_s, _, tight2) = page_char_boxes(pdfium, &round1, 1);
        let chars2: Vec<char> = chars2_s.chars().collect();
        assert!(
            chars2_s.contains("Payment due in 30 days"),
            "第二轮输入应仍含待改句：{chars2_s}"
        );
        let r2 = rect_for(&chars2, &tight2, "30", 0);
        replace_text_at_rect(&ctx, pdfium, &round1, None, 1, r2, "45", Some("30"), &round2)
            .expect("第二轮应成功");

        let after2 = page_text_at(pdfium, &round2, 1);
        eprintln!("两轮之后 = {:?}", after2);
        assert!(after2.contains("Invoice No. 60"), "第一处改动必须保留：{after2}");
        assert!(after2.contains("Payment due in 45 days"), "第二处应改成功：{after2}");
        assert_eq!(
            std::fs::read(&input).unwrap(),
            std::fs::read(&path).unwrap(),
            "两轮都不许动原始文件"
        );
        let _ = std::fs::remove_file(&round1);
        let _ = std::fs::remove_file(&round2);
    }

    /// 原地更新：输入和输出是同一个 `*_edited.pdf`。
    /// 这是连续编辑真正落盘的那一步——Windows 上要在文件被 PDFium 读过之后
    /// 用 rename 覆盖它，失败就会变成 E1007「文件被占用」。必须实测。
    #[test]
    fn e2e_rect_edit_in_place_updates_same_file() {
        let pdfium = test_pdfium();
        let path = symbol_fixture();
        if !path.exists() {
            eprintln!("跳过：缺 text_symbols.pdf");
            return;
        }
        let (input, _) = temp_case("rect_edit_inplace");
        std::fs::copy(&path, &input).unwrap();
        let dir = input.parent().unwrap().to_path_buf();
        let edited = dir.join("doc_edited.pdf");
        std::fs::copy(&path, &edited).unwrap();

        let (chars_s, _, tight) = page_char_boxes(pdfium, &edited, 1);
        let chars: Vec<char> = chars_s.chars().collect();
        let rect = rect_for(&chars, &tight, "30", 0);
        let out_tmp = dir.join("tmp_inplace.pdf");
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);

        replace_text_at_rect(&ctx, pdfium, &edited, None, 1, rect, "60", Some("30"), &out_tmp)
            .expect("引擎应能读 doc_edited.pdf");
        crate::commands::extras::commit_one_chained(
            &edited,
            &edited.to_string_lossy(),
            &out_tmp,
        )
        .expect("原地更新同一份 _edited 文件应当成功（否则连续编辑落不了盘）");

        let after = page_text_at(pdfium, &edited, 1);
        eprintln!("原地更新后 = {:?}", after);
        assert!(after.contains("Invoice No. 60"), "同一文件应被更新：{after}");
        assert!(!out_tmp.exists(), "临时文件应已提交，不留残影");
    }

    /// 真机回归（2026-11-21 用户首测 F19 报「没改成新字」）：
    /// fixture 第 1 行含空格，空格字符框退化导致旧闸门误判「跨行」。
    /// 本测试按前端语义聚行，验证含空格行 + 紧拖选（没套住空格中心）都能成功。
    #[test]
    fn e2e_rect_edit_fixture_line_with_spaces() {
        let pdfium = test_pdfium();
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/text_cjk_special.pdf"
        ));
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let (input, output) = temp_case("rect_edit_fixture");
        std::fs::copy(&path, &input).unwrap();

        // 与前端完全相同的字符来源与剔除规则
        let doc = pdfium.load_pdf_from_file(&input, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let (chars, boxes) = {
            let t = page.text().unwrap();
            let mut chars: Vec<char> = Vec::new();
            let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
            for c in t.chars().iter() {
                let ch = c.unicode_char().unwrap_or('\u{FFFD}');
                if ch == '\r' || ch == '\n' {
                    continue;
                }
                let b = c
                    .tight_bounds()
                    .or_else(|_| c.loose_bounds())
                    .unwrap_or_else(|_| PdfRect::zero());
                chars.push(ch);
                boxes.push((
                    b.left().value,
                    b.bottom().value,
                    b.right().value,
                    b.top().value,
                ));
            }
            (chars, boxes)
        };
        drop(page);
        drop(doc);
        let page_str: String = chars.iter().collect();
        let line1 = "PayOnce PDF - CJK and special symbols fixture";
        assert!(page_str.contains(line1), "夹具应含第 1 行：{:?}", page_str);

        // 第 1 行在字符序列里的下标范围
        let s0 = page_str.find(line1).unwrap();
        let e0 = s0 + line1.len();
        let slice: Vec<(usize, char, (f32, f32, f32, f32))> = (s0..e0)
            .map(|i| (i, chars[i], boxes[i]))
            .collect();
        let mut rect = slice[0].2;
        for &(_, _, b) in &slice[1..] {
            rect.0 = rect.0.min(b.0);
            rect.1 = rect.1.min(b.1);
            rect.2 = rect.2.max(b.2);
            rect.3 = rect.3.max(b.3);
        }

        // 场景 A：正常拖选（矩形略大于文字，和真人一样）
        let drag = (rect.0 - 2.0, rect.1 - 2.0, rect.2 + 2.0, rect.3 + 2.0);
        let out_a = output.with_extension("a.pdf");
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 1, drag, "ZZZ", Some(line1), &out_a);
        let ok_a = match r {
            Ok(o) => {
                println!("场景 A（正常拖选）OK old={:?} chars={}", o.old_text, o.selected_chars);
                assert_eq!(o.old_text, line1);
                let after = page_text(pdfium, &out_a);
                assert!(after.contains("ZZZ"), "第 1 行应改为 ZZZ：{:?}", after);
                assert!(!after.contains(line1), "原句应已不在：{:?}", after);
                // 第 2、3 行必须原样保留
                assert!(after.contains("© ® ± × ÷ § ¶ €"), "特殊符号行应保留：{:?}", after);
                true
            }
            Err(e) => {
                println!("场景 A ERR {}", e);
                false
            }
        };
        assert!(ok_a, "含空格的正常拖选必须成功（真机首测的失败场景）");

        // 场景 B：紧拖选——垂直方向只套住字母中部，套不住空格的退化中心
        let tight = (rect.0 - 1.0, rect.3 - 6.0, rect.2 + 1.0, rect.3 - 1.0);
        let out_b = output.with_extension("b.pdf");
        let r = replace_text_at_rect(&ctx, pdfium, &input, None, 1, tight, "QQQ", Some(line1), &out_b);
        match r {
            Ok(o) => {
                println!("场景 B（紧拖选）OK old={:?} chars={}", o.old_text, o.selected_chars);
                let after = page_text(pdfium, &out_b);
                assert!(after.contains("QQQ"), "紧拖选也应改字：{:?}", after);
            }
            Err(e) => println!("场景 B ERR（可接受，但须是明确原因） {}", e),
        }

        // 原文件不得被修改
        assert_eq!(
            std::fs::read(&input).unwrap(),
            std::fs::read(&path).unwrap(),
            "原文件字节不变"
        );
    }

    /// 诊断输出：对 fixture 的每一行打印 F19 各闸门结果（--nocapture 查看）。
    #[test]
    fn debug_rect_edit_real_fixture_per_line() {
        let pdfium = test_pdfium();
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/text_cjk_special.pdf"
        ));
        let cancel = AtomicBool::new(false);
        let ctx = offline_ctx(&cancel);
        let (input, output) = temp_case("rect_edit_fixture_diag");
        std::fs::copy(&path, &input).unwrap();

        let doc = pdfium.load_pdf_from_file(&input, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let (chars, boxes) = {
            let t = page.text().unwrap();
            let mut chars: Vec<char> = Vec::new();
            let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
            for c in t.chars().iter() {
                let ch = c.unicode_char().unwrap_or('\u{FFFD}');
                if ch == '\r' || ch == '\n' {
                    continue;
                }
                let b = c
                    .tight_bounds()
                    .or_else(|_| c.loose_bounds())
                    .unwrap_or_else(|_| PdfRect::zero());
                chars.push(ch);
                boxes.push((
                    b.left().value,
                    b.bottom().value,
                    b.right().value,
                    b.top().value,
                ));
            }
            (chars, boxes)
        };
        drop(page);
        drop(doc);
        println!("total chars (no-newline) = {}", chars.len());

        // 前端语义聚行：运行均值 cy + 组内按字符索引排序
        let mut items: Vec<(usize, f32)> = (0..chars.len())
            .map(|i| (i, (boxes[i].1 + boxes[i].3) / 2.0))
            .collect();
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));
        let mut lines: Vec<(f32, Vec<usize>)> = Vec::new();
        for &(i, cy) in &items {
            match lines.last_mut() {
                Some(l) if (cy - l.0).abs() <= 2.0 => {
                    let n = l.1.len();
                    l.0 = (l.0 * n as f32 + cy) / (n + 1) as f32;
                    l.1.push(i);
                }
                _ => lines.push((cy, vec![i])),
            }
        }

        for (li, (_, members)) in lines.iter().enumerate() {
            let members: Vec<usize> = {
                let mut m = members.clone();
                m.sort_by_key(|&i| i);
                m
            };
            let line_text: String = members.iter().map(|&i| chars[i]).collect();
            let mut rect = boxes[members[0]];
            for &i in &members[1..] {
                let b = boxes[i];
                rect.0 = rect.0.min(b.0);
                rect.1 = rect.1.min(b.1);
                rect.2 = rect.2.max(b.2);
                rect.3 = rect.3.max(b.3);
            }
            let drag = (rect.0 - 2.0, rect.1 - 2.0, rect.2 + 2.0, rect.3 + 2.0);
            println!(
                "--- line {} text={:?} drag=({:.1},{:.1},{:.1},{:.1})",
                li, line_text, drag.0, drag.1, drag.2, drag.3
            );
            let out = output.with_extension(format!("line{}.pdf", li));
            match replace_text_at_rect(&ctx, pdfium, &input, None, 1, drag, "ZZZ", Some(&line_text), &out) {
                Ok(o) => println!("    OK old={:?} chars={}", o.old_text, o.selected_chars),
                Err(e) => println!("    ERR {}", e),
            }
        }
    }

    /// F19 坐标口径：字符框覆盖比例与选区命中的纯函数。
    #[test]
    fn rect_pick_matches_frontend_rule() {
        // 两个字符框：A 完全在选区内，B 只被覆盖一半，C 在选区外。
        let boxes = vec![
            (10.0, 100.0, 20.0, 112.0),
            (20.0, 100.0, 30.0, 112.0),
            (40.0, 100.0, 50.0, 112.0),
        ];
        let sel = (8.0, 98.0, 25.0, 114.0);
        let picked = pick_chars_by_rect(&boxes, sel);
        assert_eq!(picked, vec![0, 1], "B 被覆盖 50% ≥ 40%，应命中；C 在框外");
        assert!((char_cover_ratio(boxes[0], sel) - 1.0).abs() < 1e-6);
        assert!((char_cover_ratio(boxes[1], sel) - 0.5).abs() < 1e-6);
        assert_eq!(char_cover_ratio(boxes[2], sel), 0.0);
        // 零面积字符框不参与覆盖率计算，但中心点仍可命中。
        assert_eq!(char_cover_ratio((10.0, 100.0, 10.0, 100.0), sel), 0.0);
        assert_eq!(pick_chars_by_rect(&[(10.0, 100.0, 10.0, 100.0)], sel), vec![0]);
    }
}
