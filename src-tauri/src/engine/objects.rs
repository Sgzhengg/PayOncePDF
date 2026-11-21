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
//! - 书签：只做顶层书签（不建嵌套层级）；重建 /Outlines 后旧的 outline 对象成为孤儿，
//!   不影响渲染；页面增删后书签不自动跟随页号。

use crate::engine::{extras, pdf, JobCtx};
use crate::error::{AppError, AppResult};
use lopdf::{dictionary, Bookmark, Dictionary, Document, Object, ObjectId};
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

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

/// 每行 `页码 标题`，例如 `3 第三章 交付验收`；标题允许含空格。
pub fn parse_bookmark_spec(spec: &str, total: u32) -> AppResult<Vec<BookmarkEntry>> {
    let mut out = Vec::new();
    for (i, line) in spec.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let page: u32 = parts
            .next()
            .unwrap_or("")
            .trim()
            .parse()
            .map_err(|_| AppError::internal(format!("书签第 {} 行格式应为「页码 标题」", i + 1)))?;
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
        out.push(BookmarkEntry { level: 1, title, page });
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
    let mut added = 0usize;
    for e in &entries {
        let Some(&pid) = pages.get(&e.page) else {
            continue;
        };
        doc.add_bookmark(
            Bookmark::new(e.title.clone(), [0.0, 0.0, 0.0], 0, pid),
            None,
        );
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
    Ok((
        added,
        format!("书签 {} → {} 条，重建顶层 /Outlines", before, added),
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
    fn bookmark_spec_validates_page_range() {
        assert_eq!(parse_bookmark_spec("1 A\n3 B", 3).unwrap().len(), 2);
        assert!(parse_bookmark_spec("4 C", 3).is_err());
        assert!(parse_bookmark_spec("abc D", 3).is_err());
        assert!(parse_bookmark_spec("1 ", 3).is_err());
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
