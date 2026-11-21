//! F15：文档对象层——创建可填写表单、密文脱敏（redaction）、书签管理。
//!
//! 对标矩阵 §1.1 的三项 ❌。只用已在包内的 lopdf(MIT) / PDFium(BSD-3)，
//! 不引云端、不引新依赖。所有产物写到临时路径，由命令层原子提交。
//!
//! 诚实边界：
//! - 表单：创建文本域（单行/多行/必填），`/NeedAppearances true` 让阅读器生成外观；
//!   不做复选框/单选/下拉/计算脚本，也不做数字签名域。
//! - 脱敏：**先真删除文字对象**再盖黑框，不是只画一层黑矩形（那样复制仍能取到原文）。
//!   只处理有文字层的可选中文本；扫描件需先 F5 OCR；框位置来自文字命中矩形。
//! - 书签：F15 书签管理支持多级（spec 行首 `>` 表示子级，set/add/rename/delete
//!   全程用 parent 栈还原层级，不会把已有层级压平）；F1 合并的 `merge_bookmarks`
//!   同样保留源文件原有层级。重建 /Outlines 后旧对象成为孤儿，不影响渲染；
//!   页面增删后书签不自动跟随页号；不支持拖放换父级（层级由 spec 文本决定）。

use crate::engine::{extras, pdf, JobCtx};
use crate::error::{AppError, AppResult};
use lopdf::{dictionary, Bookmark, Dictionary, Document, Object, ObjectId};
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn lopdf_error(e: lopdf::Error) -> AppError {
    AppError::parse(format!("PDF 结构写入失败：{}", e))
}

/// PDF 文本字符串：ASCII 直接字面量，非 ASCII 用 UTF-16BE + BOM（中文标题/字段名）。
pub fn pdf_text_string(s: &str) -> Object {
    if s.is_ascii() {
        return Object::string_literal(s.to_string());
    }
    let mut b = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        b.extend_from_slice(&u.to_be_bytes());
    }
    Object::string_literal(b)
}

fn write_doc(doc: &mut Document, out_tmp: &Path) -> AppResult<()> {
    std::fs::write(out_tmp, pdf::save_to_bytes(doc)?).map_err(AppError::from)
}

fn append_annot(doc: &mut Document, page_id: ObjectId, annot_id: ObjectId) -> AppResult<()> {
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

// ---------------------------------------------------------------------------
// 表单创建
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub name: String,
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub default: String,
    pub multiline: bool,
    pub required: bool,
}

pub struct FormOutcome {
    pub fields: usize,
    pub pages: usize,
}

/// 在指定页创建 AcroForm 文本域。字段合并进 Widget 注解（单层结构，兼容性最好）。
pub fn create_form(
    path: &Path,
    password: Option<&str>,
    fields: &[FormField],
    out_tmp: &Path,
) -> AppResult<FormOutcome> {
    if fields.is_empty() {
        return Err(AppError::internal("至少添加一个表单字段"));
    }
    let mut doc = pdf::load_doc(path, password)?;
    let page_map = doc.get_pages();
    let total = page_map.len() as u32;
    if total == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let mut seen = std::collections::HashSet::new();
    for f in fields {
        if f.name.trim().is_empty() {
            return Err(AppError::internal("表单字段名不能为空"));
        }
        if !seen.insert(f.name.as_str()) {
            return Err(AppError::internal(format!("表单字段名重复：{}", f.name)));
        }
        if f.page == 0 || f.page > total {
            return Err(AppError::internal(format!(
                "字段「{}」的页码 {} 超出 1..{}",
                f.name, f.page, total
            )));
        }
        if f.width <= 0.0 || f.height <= 0.0 {
            return Err(AppError::internal(format!("字段「{}」尺寸必须为正", f.name)));
        }
    }

    // /DR 里放一个标准 Helvetica，供阅读器生成外观时使用。
    let helv = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let mut dr = Dictionary::new();
    let mut fonts = Dictionary::new();
    fonts.set("Helv", Object::Reference(helv));
    dr.set("Font", Object::Dictionary(fonts));

    let mut field_refs: Vec<Object> = Vec::new();
    let mut touched: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for f in fields {
        let pid = page_map[&f.page];
        let mut flags = 0i64;
        if f.multiline {
            flags |= 1 << 12; // Multiline
        }
        if f.required {
            flags |= 1 << 1; // Required
        }
        let w = dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "FT" => "Tx",
            "T" => pdf_text_string(&f.name),
            "TU" => pdf_text_string(&f.name),
            "Rect" => vec![
                Object::Real(f.x), Object::Real(f.y),
                Object::Real(f.x + f.width), Object::Real(f.y + f.height),
            ],
            "P" => Object::Reference(pid),
            "F" => 4i64,
            "DA" => Object::string_literal("/Helv 12 Tf 0 g".to_string()),
            "V" => pdf_text_string(&f.default),
            "Ff" => flags,
        };
        let id = doc.add_object(Object::Dictionary(w));
        append_annot(&mut doc, pid, id)?;
        field_refs.push(Object::Reference(id));
        touched.insert(f.page);
    }

    // 若原文档已有 AcroForm，保留既有字段，不覆盖用户已有表单。
    // 先在只读块里把旧引用复制出来，避免持有 doc 的不可变借用再取可变借用。
    let mut all_fields: Vec<Object> = Vec::new();
    {
        if let Ok(catalog) = doc.catalog() {
            if let Ok(existing) = catalog.get(b"AcroForm") {
                if let Ok(old_id) = existing.as_reference() {
                    if let Ok(old) = doc.get_object(old_id) {
                        if let Ok(arr) = old
                            .as_dict()
                            .and_then(|d| d.get(b"Fields"))
                            .and_then(|o| o.as_array())
                        {
                            all_fields.extend(arr.iter().cloned());
                        }
                    }
                }
            }
        }
    }
    all_fields.extend(field_refs);
    let acro = dictionary! {
        "Fields" => all_fields,
        "NeedAppearances" => true,
        "DA" => Object::string_literal("/Helv 0 Tf 0 g".to_string()),
        "DR" => Object::Dictionary(dr),
    };
    let acro_id = doc.add_object(Object::Dictionary(acro));
    doc.catalog_mut()
        .map_err(lopdf_error)?
        .set("AcroForm", Object::Reference(acro_id));
    write_doc(&mut doc, out_tmp)?;
    Ok(FormOutcome {
        fields: fields.len(),
        pages: touched.len(),
    })
}

// ---------------------------------------------------------------------------
// 密文脱敏：真删除文字 + 黑框
// ---------------------------------------------------------------------------

pub struct RedactOutcome {
    pub terms_removed: usize,
    pub matches: usize,
    pub boxes: usize,
    pub pages: usize,
}

/// 先在 PDFium 里把命中文字从文本对象中删除，再用 lopdf 在原坐标盖黑框。
/// 两份产物都在临时目录，失败时不触碰输入与最终输出。
#[allow(clippy::too_many_arguments)]
pub fn redact(
    ctx: &JobCtx<'_>,
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    terms: &[String],
    match_case: bool,
    pages: Option<&str>,
    out_tmp: &Path,
    tmp_dir: &Path,
) -> AppResult<RedactOutcome> {
    let cleaned: Vec<&String> = terms.iter().filter(|t| !t.trim().is_empty()).collect();
    if cleaned.is_empty() {
        return Err(AppError::internal("请至少填写一个要脱敏的词"));
    }

    // 1) 取每个词在原文档里的命中矩形（用未删除的原文定位）。
    let mut by_page: BTreeMap<u32, Vec<[f32; 8]>> = BTreeMap::new();
    let mut present: Vec<String> = Vec::new();
    let mut matches = 0usize;
    for (i, term) in cleaned.iter().enumerate() {
        ctx.tick(i + 1, cleaned.len(), format!("定位「{}」", term))?;
        let (quads, n, _total) = extras::pdf_quads(
            pdfium, path, password, term, match_case, pages, 20000, out_tmp,
        )?;
        if quads.is_empty() {
            continue; // 未命中的词跳过，不假装脱敏成功
        }
        present.push((*term).clone());
        matches += n;
        for q in quads {
            by_page.entry(q.page_no).or_default().push(q.points);
        }
    }
    if present.is_empty() {
        return Err(AppError::internal(
            "没有找到任何要脱敏的文字。请确认文档含可选中文本（扫描件请先 OCR）、大小写与页码范围。",
        ));
    }

    // 2) 逐个词真删除文字对象（每次以上一次产物为输入）。
    let mut current = path.to_path_buf();
    for (i, term) in present.iter().enumerate() {
        let next = tmp_dir.join(format!("redact_step_{}.pdf", i));
        // 只有第一次读的是原始（可能加密的）文件；中间产物是 PDFium 新写的，未加密。
        let pw = if i == 0 { password } else { None };
        extras::replace_text(
            ctx, pdfium, &current, pw, term, "", match_case, false, pages, &next,
        )?;
        current = next;
    }

    // 3) 在删除后的文档上盖黑框，覆盖原文字所在矩形。
    let mut doc = pdf::load_doc(&current, None)?;
    let page_map = doc.get_pages();
    let mut boxes = 0usize;
    let mut pages_touched = 0usize;
    for (page_no, quads) in &by_page {
        let Some(&pid) = page_map.get(page_no) else {
            continue;
        };
        let mut content = String::from("q\n0 0 0 rg\n");
        for p in quads {
            let x0 = [p[0], p[2], p[4], p[6]].iter().cloned().fold(f32::MAX, f32::min) - 1.0;
            let y0 = [p[1], p[3], p[5], p[7]].iter().cloned().fold(f32::MAX, f32::min) - 1.0;
            let x1 = [p[0], p[2], p[4], p[6]].iter().cloned().fold(f32::MIN, f32::max) + 1.0;
            let y1 = [p[1], p[3], p[5], p[7]].iter().cloned().fold(f32::MIN, f32::max) + 1.0;
            content.push_str(&format!("{} {} {} {} re f\n", x0, y0, x1 - x0, y1 - y0));
            boxes += 1;
        }
        content.push_str("Q\n");
        doc.add_page_contents(pid, content.into_bytes())
            .map_err(lopdf_error)?;
        pages_touched += 1;
    }
    write_doc(&mut doc, out_tmp)?;
    Ok(RedactOutcome {
        terms_removed: present.len(),
        matches,
        boxes,
        pages: pages_touched,
    })
}

// ---------------------------------------------------------------------------
// 书签（顶层）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkEntry {
    pub level: u32,
    pub title: String,
    pub page: u32,
}

pub fn list_bookmarks(path: &Path, password: Option<&str>) -> AppResult<Vec<BookmarkEntry>> {
    let doc = pdf::load_doc(path, password)?;
    match doc.get_toc() {
        Ok(toc) => Ok(toc
            .toc
            .into_iter()
            .map(|t| BookmarkEntry {
                level: t.level as u32,
                title: t.title,
                page: t.page as u32,
            })
            .collect()),
        Err(_) => Ok(Vec::new()), // 没有 /Outlines 是正常状态，不是错误
    }
}

/// 每行 `[>...]页码 标题`，例如 `3 第三章 交付验收`；标题允许含空格。
/// 行首 `>` 数量表示层级：无 `>` = 一级，`>` = 二级，`>>` = 三级，依此类推。
/// 纯 `页码 标题`（旧格式）仍是一级，完全向后兼容。
/// 层级不得跳跃（第一行必须一级，子级的父级必须已存在）；跳跃即报错，避免挂到错误父节点。
pub fn parse_bookmark_spec(spec: &str, total: u32) -> AppResult<Vec<BookmarkEntry>> {
    let mut out = Vec::new();
    let mut prev_level = 0u32;
    for (i, line) in spec.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let level_raw = line.bytes().take_while(|&b| b == b'>').count() as u32 + 1;
        let rest = line.trim_start_matches('>').trim_start();
        let mut parts = rest.splitn(2, char::is_whitespace);
        let page: u32 = parts
            .next()
            .unwrap_or("")
            .trim()
            .parse()
            .map_err(|_| AppError::internal(format!("书签第 {} 行格式应为「[>]页码 标题」", i + 1)))?;
        let title = parts.next().unwrap_or("").trim().to_string();
        if title.is_empty() {
            return Err(AppError::internal(format!("书签第 {} 行缺少标题", i + 1)));
        }
        if page == 0 || page > total {
            return Err(AppError::internal(format!(
                "书签第 {} 行页码 {} 超出 1..{}",
                i + 1, page, total
            )));
        }
        if level_raw > prev_level + 1 {
            return Err(AppError::internal(format!(
                "书签第 {} 行层级跳跃（第 {} 级没有父书签，上一行是第 {} 级）",
                i + 1, level_raw, prev_level
            )));
        }
        prev_level = level_raw;
        out.push(BookmarkEntry {
            level: level_raw,
            title,
            page,
        });
    }
    Ok(out)
}

/// `action` = set | add | rename | delete。
/// `spec` 用于 set/add；`target`/`new_title` 用于 rename/delete。
#[allow(clippy::too_many_arguments)]
pub fn manage_bookmarks(
    path: &Path,
    password: Option<&str>,
    action: &str,
    spec: &str,
    target: &str,
    new_title: &str,
    out_tmp: &Path,
) -> AppResult<(usize, String)> {
    let mut doc = pdf::load_doc(path, password)?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;
    if total == 0 {
        return Err(AppError::parse("文档没有页面"));
    }
    let mut entries = list_bookmarks(path, password)?;
    let before = entries.len();

    match action {
        "set" => entries = parse_bookmark_spec(spec, total)?,
        "add" => entries.extend(parse_bookmark_spec(spec, total)?),
        "rename" => {
            if target.trim().is_empty() || new_title.trim().is_empty() {
                return Err(AppError::internal("重命名需要填写原标题与新标题"));
            }
            let mut hit = 0;
            for e in &mut entries {
                if e.title == target.trim() {
                    e.title = new_title.trim().to_string();
                    hit += 1;
                }
            }
            if hit == 0 {
                return Err(AppError::internal(format!("没有找到书签「{}」", target.trim())));
            }
        }
        "delete" => {
            if target.trim().is_empty() {
                return Err(AppError::internal("删除需要填写书签标题"));
            }
            let n = entries.len();
            entries.retain(|e| e.title != target.trim());
            if entries.len() == n {
                return Err(AppError::internal(format!("没有找到书签「{}」", target.trim())));
            }
        }
        other => {
            return Err(AppError::internal(format!(
                "未知书签操作：{}（可用 set/add/rename/delete）",
                other
            )))
        }
    }

    // 重建 /Outlines：lopdf 的 add_bookmark + build_outline 会生成完整链式字典。
    // 用 parent 栈把每条书签挂到它上一级的最后一条上——这是 list_bookmarks 读得到
    // 层级、merge_bookmarks 也保得住层级的关键；此前这里传 None，导致任何一次
    // set/add/rename/delete 都把合并保住的层级重新压平成一整列顶层书签。
    let mut added = 0usize;
    let mut stack: Vec<(u32, u32)> = Vec::new(); // (level, bookmark id) —— lopdf add_bookmark 的 parent 用 u32
    for e in &entries {
        let Some(&pid) = pages.get(&e.page) else {
            continue;
        };
        while stack.last().is_some_and(|(l, _)| *l >= e.level) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, id)| *id);
        let id = doc.add_bookmark(
            Bookmark::new(e.title.clone(), [0.0, 0.0, 0.0], 0, pid),
            parent,
        );
        stack.push((e.level, id));
        added += 1;
    }
    if let Some(outline_id) = doc.build_outline() {
        doc.get_object_mut(outline_id)
            .map_err(lopdf_error)?
            .as_dict_mut()
            .map_err(lopdf_error)?
            .set("Type", "Outlines");
        doc.catalog_mut()
            .map_err(lopdf_error)?
            .set("Outlines", Object::Reference(outline_id));
    } else if !entries.is_empty() {
        return Err(AppError::internal("书签写入失败：未能构建 /Outlines"));
    }
    write_doc(&mut doc, out_tmp)?;
    let deepest = entries.iter().map(|e| e.level).max().unwrap_or(1);
    Ok((
        added,
        format!(
            "书签 {} → {} 条，重建 /Outlines（最深 {} 级）",
            before, added, deepest
        ),
    ))
}

/// qpdf 合并会重建文档树、丢弃各源文件的 /Outlines。本函数在其产物上重建书签：
/// 依次读取每个源文件的顶层/嵌套书签，按该文件贡献的页数给目标页加偏移，
/// 再写进合并结果的 /Outlines。层级也一并保留（用 parent 栈还原父子关系）。
///
/// - `sources` 顺序必须与 qpdf 合并时的输入顺序完全一致，否则页码会错。
/// - `merged` 只读；`out_tmp` 必须与之不同，避免读写同一路径。
/// - 源文件无书签不算错误；某条书签目标页越界则跳过并计数，绝不写坏文件。
pub fn merge_bookmarks(
    sources: &[PathBuf],
    password: Option<&str>,
    merged: &Path,
    out_tmp: &Path,
) -> AppResult<(usize, String)> {
    if merged == out_tmp {
        return Err(AppError::internal("书签合并：输入输出路径不能相同"));
    }
    let mut doc = pdf::load_doc(merged, None)?;
    let merged_pages = doc.get_pages();
    let total = merged_pages.len() as u32;

    let mut offset: u32 = 0; // 当前源文件第一页在结果中的页号（1-based）
    let mut added = 0usize;
    let mut dropped = 0usize;
    let mut stack: Vec<(usize, u32)> = Vec::new(); // (level, bookmark id)

    for src in sources {
        let src_doc = pdf::load_doc(src, password)?;
        let n = src_doc.get_pages().len() as u32;
        if let Ok(toc) = src_doc.get_toc() {
            for t in toc.toc {
                let target = offset.saturating_add(t.page as u32);
                if target == 0 || target > total {
                    dropped += 1;
                    continue;
                }
                let Some(&pid) = merged_pages.get(&target) else {
                    dropped += 1;
                    continue;
                };
                while stack.last().is_some_and(|(l, _)| *l >= t.level) {
                    stack.pop();
                }
                let parent = stack.last().map(|(_, id)| *id);
                let id = doc.add_bookmark(Bookmark::new(t.title, [0.0, 0.0, 0.0], 0, pid), parent);
                stack.push((t.level, id));
                added += 1;
            }
        }
        offset = offset.saturating_add(n);
    }

    if added > 0 {
        let outline_id = doc
            .build_outline()
            .ok_or_else(|| AppError::internal("合并书签失败：未能构建 /Outlines"))?;
        doc.get_object_mut(outline_id)
            .map_err(lopdf_error)?
            .as_dict_mut()
            .map_err(lopdf_error)?
            .set("Type", "Outlines");
        doc.catalog_mut()
            .map_err(lopdf_error)?
            .set("Outlines", Object::Reference(outline_id));
    }
    write_doc(&mut doc, out_tmp)?;
    Ok((
        added,
        format!(
            "从 {} 个源文件合并 {} 条书签（保留层级{}）",
            sources.len(),
            added,
            if dropped > 0 {
                format!("，跳过 {} 条越界", dropped)
            } else {
                String::new()
            }
        ),
    ))
}

// ---------------------------------------------------------------------------
// 真实文件级测试（不需要 AppHandle / PDFium 的表单与书签部分）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_pdf(pages: usize) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut ids = Vec::new();
        for _ in 0..pages {
            let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
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

    #[test]
    fn form_fields_are_real_acroform_widgets() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_form_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.pdf");
        let output = dir.join("out.pdf");
        std::fs::write(&input, tiny_pdf(2)).unwrap();
        let fields = vec![
            FormField {
                name: "buyer".into(), page: 1, x: 72.0, y: 700.0,
                width: 200.0, height: 24.0, default: "ACME".into(),
                multiline: false, required: true,
            },
            FormField {
                name: "备注".into(), page: 2, x: 72.0, y: 650.0,
                width: 300.0, height: 80.0, default: "机密".into(),
                multiline: true, required: false,
            },
        ];
        let out = create_form(&input, None, &fields, &output).unwrap();
        assert_eq!(out.fields, 2);
        assert_eq!(out.pages, 2);
        let doc = Document::load(&output).unwrap();
        let acro_id = doc
            .catalog()
            .unwrap()
            .get(b"AcroForm")
            .unwrap()
            .as_reference()
            .unwrap();
        let acro = doc.get_object(acro_id).unwrap().as_dict().unwrap();
        assert_eq!(acro.get(b"Fields").unwrap().as_array().unwrap().len(), 2);
        assert!(acro.get(b"NeedAppearances").is_ok());
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
        let widget = doc
            .get_object(annots[0].as_reference().unwrap())
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(widget.get(b"Subtype").unwrap().as_name().unwrap()),
            "Widget"
        );
        assert_eq!(
            String::from_utf8_lossy(widget.get(b"FT").unwrap().as_name().unwrap()),
            "Tx"
        );
    }

    #[test]
    fn bookmark_set_add_rename_delete_roundtrip() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_bm_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.pdf");
        let output = dir.join("out.pdf");
        std::fs::write(&input, tiny_pdf(3)).unwrap();

        let (n, _) = manage_bookmarks(
            &input, None, "set", "1 封面\n2 第二章 交付\n3 附录", "", "", &output,
        )
        .unwrap();
        assert_eq!(n, 3);
        let list = list_bookmarks(&output, None).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].title, "第二章 交付");
        assert_eq!(list[1].page, 2);

        let next = dir.join("next.pdf");
        manage_bookmarks(&output, None, "rename", "", "第二章 交付", "第二章 验收", &next).unwrap();
        let list = list_bookmarks(&next, None).unwrap();
        assert!(list.iter().any(|e| e.title == "第二章 验收"));
        assert!(!list.iter().any(|e| e.title == "第二章 交付"));

        let last = dir.join("last.pdf");
        manage_bookmarks(&next, None, "delete", "", "附录", "", &last).unwrap();
        assert_eq!(list_bookmarks(&last, None).unwrap().len(), 2);
    }

    #[test]
    fn manage_bookmarks_keeps_nested_levels_across_operations() {
        // 先造一个带真实层级的源 PDF（顶层 + 子级 + 孙级 + 第二个顶层）
        let dir = std::env::temp_dir().join(format!("payoncepdf_bmlvl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.pdf");
        let set_out = dir.join("set.pdf");
        let rename_out = dir.join("rename.pdf");
        std::fs::write(
            &src,
            pdf_with_bookmarks(
                5,
                &[
                    (1, "封面", None),
                    (2, "第一章", Some(0)),
                    (3, "1.1 背景", Some(1)),
                    (4, "附录", None),
                ],
            ),
        )
        .unwrap();
        assert_eq!(
            list_bookmarks(&src, None).unwrap().iter().map(|e| e.level).collect::<Vec<_>>(),
            vec![1, 2, 3, 1]
        );

        // set 用带层级的 spec 整体替换，层级必须写回而不是压成一级
        let (n, msg) = manage_bookmarks(
            &src,
            None,
            "set",
            "1 封面\n2 第一章\n> 3 1.1 目标\n>> 4 1.1.1 范围\n5 附录",
            "",
            "",
            &set_out,
        )
        .unwrap();
        assert_eq!(n, 5, "{msg}");
        let levels = list_bookmarks(&set_out, None).unwrap().iter().map(|e| e.level).collect::<Vec<_>>();
        assert_eq!(levels, vec![1, 1, 2, 3, 1], "set 后的层级应保持，不能压平");

        // 再改一次（rename 子级）：上轮修的「改一次就丢层级」不能复发
        manage_bookmarks(
            &set_out,
            None,
            "rename",
            "",
            "1.1 目标",
            "1.1 范围与目标",
            &rename_out,
        )
        .unwrap();
        let after = list_bookmarks(&rename_out, None).unwrap();
        assert_eq!(
            after.iter().map(|e| e.level).collect::<Vec<_>>(),
            vec![1, 1, 2, 3, 1],
            "rename 后层级必须原样保住"
        );
        assert_eq!(after[2].title, "1.1 范围与目标");

        // 合并产物（本就带层级）经 set 再写，层级不塌
        let merged_like = dir.join("merged_like.pdf");
        manage_bookmarks(&src, None, "set", "1 封面\n> 2 第一章\n> 3 附录", "", "", &merged_like).unwrap();
        manage_bookmarks(&merged_like, None, "delete", "", "附录", "", &rename_out).unwrap();
        let trimmed = list_bookmarks(&rename_out, None).unwrap();
        assert_eq!(trimmed.iter().map(|e| e.level).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn bookmark_spec_validates_page_range() {
        assert_eq!(parse_bookmark_spec("1 A\n3 B", 3).unwrap().len(), 2);
        assert!(parse_bookmark_spec("4 C", 3).is_err());
        assert!(parse_bookmark_spec("abc D", 3).is_err());
        assert!(parse_bookmark_spec("1 ", 3).is_err());
    }

    #[test]
    fn bookmark_spec_parses_and_validates_levels() {
        // 旧格式（纯「页码 标题」）仍是一级，完全向后兼容
        let flat = parse_bookmark_spec("1 A\n3 B", 3).unwrap();
        assert_eq!(flat.iter().map(|e| e.level).collect::<Vec<_>>(), vec![1, 1]);
        // 行首 > 数量 = 层级，标题里的空格与 > 不受影响
        let nested = parse_bookmark_spec("1 总览\n> 2 章节\n>> 3 3.1 > 重点\n> 4 第二章", 4).unwrap();
        assert_eq!(
            nested.iter().map(|e| e.level).collect::<Vec<_>>(),
            vec![1, 2, 3, 2]
        );
        assert_eq!(nested[2].title, "3.1 > 重点");
        assert_eq!(nested[3].title, "第二章");
        // 层级跳跃拒绝：第一行就是子级、或跳过一级直接到孙级
        assert!(parse_bookmark_spec("> 1 孤儿", 3).is_err());
        assert!(parse_bookmark_spec("1 A\n>> 2 跳级", 3).is_err());
        // 相邻同级 / 正常降级都允许
        assert!(parse_bookmark_spec("1 A\n2 B\n> 3 C\n>> 4 D\n> 5 E\n6 F", 6).is_ok());
    }

    /// 用 lopdf 直接造带层级的书签文件（manage_bookmarks 只能造顶层，不够用）。
    fn pdf_with_bookmarks(pages: usize, specs: &[(usize, &str, Option<usize>)]) -> Vec<u8> {
        let mut doc = Document::load_mem(&tiny_pdf(pages)).unwrap();
        let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let mut ids: Vec<Option<u32>> = vec![None; specs.len()];
        for (i, (page, title, parent)) in specs.iter().enumerate() {
            let pid = page_ids[*page - 1];
            let parent_id = parent.and_then(|p| ids[p]);
            let id = doc.add_bookmark(
                Bookmark::new((*title).to_string(), [0.0, 0.0, 0.0], 0, pid),
                parent_id,
            );
            ids[i] = Some(id);
        }
        if let Some(outline_id) = doc.build_outline() {
            doc.get_object_mut(outline_id)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Type", "Outlines");
            doc.catalog_mut().unwrap().set("Outlines", Object::Reference(outline_id));
        }
        pdf::save_to_bytes(&mut doc).unwrap()
    }

    #[test]
    fn merge_bookmarks_offsets_pages_and_keeps_cjk_titles() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_mbm_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.pdf");
        let b = dir.join("b.pdf");
        let merged = dir.join("merged.pdf");
        let out = dir.join("out.pdf");
        std::fs::write(&a, pdf_with_bookmarks(2, &[(1, "A封面", None), (2, "A第二章", None)])).unwrap();
        std::fs::write(&b, pdf_with_bookmarks(2, &[(1, "B第一章", None), (2, "B附录", None)])).unwrap();
        // 模拟 qpdf 产物：4 页、无 outline
        std::fs::write(&merged, tiny_pdf(4)).unwrap();

        let (n, msg) = merge_bookmarks(&[a.clone(), b.clone()], None, &merged, &out).unwrap();
        assert_eq!(n, 4, "{msg}");
        let list = list_bookmarks(&out, None).unwrap();
        assert_eq!(list.len(), 4);
        assert_eq!((list[0].title.as_str(), list[0].page), ("A封面", 1));
        assert_eq!((list[1].title.as_str(), list[1].page), ("A第二章", 2));
        assert_eq!((list[2].title.as_str(), list[2].page), ("B第一章", 3));
        assert_eq!((list[3].title.as_str(), list[3].page), ("B附录", 4));
    }

    #[test]
    fn merge_bookmarks_preserves_nested_levels() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_mbm_lvl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.pdf");
        let merged = dir.join("merged.pdf");
        let out = dir.join("out.pdf");
        std::fs::write(
            &a,
            pdf_with_bookmarks(
                3,
                &[(1, "总览", None), (2, "章节一", Some(0)), (3, "小节", Some(1))],
            ),
        )
        .unwrap();
        std::fs::write(&merged, tiny_pdf(3)).unwrap();
        merge_bookmarks(&[a], None, &merged, &out).unwrap();
        let list = list_bookmarks(&out, None).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().map(|e| e.level).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(list[2].title, "小节");
    }

    #[test]
    fn merge_bookmarks_skips_no_outline_and_out_of_range() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_mbm_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.pdf"); // 2 页，无书签
        let b = dir.join("b.pdf"); // 2 页，1 条书签指向本文件第 1 页
        let merged = dir.join("merged.pdf"); // 故意只有 2 页：b 的目标页 3 越界
        let out = dir.join("out.pdf");
        std::fs::write(&a, tiny_pdf(2)).unwrap();
        std::fs::write(&b, pdf_with_bookmarks(2, &[(1, "越界条目", None)])).unwrap();
        std::fs::write(&merged, tiny_pdf(2)).unwrap();
        let (n, msg) = merge_bookmarks(&[a, b], None, &merged, &out).unwrap();
        assert_eq!(n, 0, "{msg}");
        assert!(msg.contains("跳过 1 条越界"));
        assert!(list_bookmarks(&out, None).unwrap().is_empty());
    }

    /// 真实链路：造带书签 PDF → qpdf 合并（书签丢失）→ merge_bookmarks → 读回验证。
    /// 用独立 qpdf 进程，不经过 AppHandle；sidecar 缺失时跳过（不视为通过）。
    #[test]
    fn e2e_merge_bookmarks_after_real_qpdf() {
        let exe = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("bin/qpdf-x86_64-pc-windows-msvc.exe");
        if !exe.exists() {
            eprintln!("qpdf sidecar 缺失，跳过真实链路验证（不视为通过）");
            return;
        }
        let dir = std::env::temp_dir().join(format!("payoncepdf_mbm_e2e_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.pdf");
        let b = dir.join("b.pdf");
        let merged = dir.join("qpdf_merged.pdf");
        let out = dir.join("bookmarked.pdf");
        std::fs::write(&a, pdf_with_bookmarks(2, &[(1, "甲-封面", None), (2, "甲-正文", None)])).unwrap();
        std::fs::write(&b, pdf_with_bookmarks(2, &[(1, "乙-目录", None), (2, "乙-附录", None)])).unwrap();

        let status = std::process::Command::new(&exe)
            .args(["--empty", "--pages"])
            .arg(&a)
            .arg(&b)
            .arg("--")
            .arg(&merged)
            .status()
            .unwrap();
        assert!(status.success(), "qpdf 合并失败");
        // 前置事实：qpdf 产物确实没有书签
        assert!(list_bookmarks(&merged, None).unwrap().is_empty());

        let (n, msg) = merge_bookmarks(&[a, b], None, &merged, &out).unwrap();
        assert_eq!(n, 4, "{msg}");
        let list = list_bookmarks(&out, None).unwrap();
        assert_eq!(
            list.iter().map(|e| (e.title.as_str(), e.page)).collect::<Vec<_>>(),
            vec![("甲-封面", 1), ("甲-正文", 2), ("乙-目录", 3), ("乙-附录", 4)]
        );

        // 产物能通过 qpdf 结构校验（书签 /Outlines 写入没破坏文档树）
        let check = std::process::Command::new(&exe)
            .arg("--check")
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "qpdf --check 失败：{}",
            String::from_utf8_lossy(&check.stdout)
        );
    }

    #[test]
    fn merge_bookmarks_refuses_same_input_output_path() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_mbm_same_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("p.pdf");
        std::fs::write(&p, tiny_pdf(1)).unwrap();
        assert!(merge_bookmarks(&[], None, &p, &p).is_err());
    }

    #[test]
    fn pdf_text_string_uses_utf16_for_cjk() {
        let o = pdf_text_string("机密");
        let bytes = match o {
            Object::String(b, _) => b,
            _ => panic!("expected string"),
        };
        assert_eq!(&bytes[0..2], &[0xFE, 0xFF]);
        assert!(bytes.len() > 4);
    }
}
