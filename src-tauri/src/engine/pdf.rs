//! lopdf 结构操作（PRD §9.1；工程红线：lopdf / image 均合规）。
//!
//! - F2 bookmarks 模式：读 /Outlines 顶层标题作为切点。
//! - F3 压缩：遍历图像 XObject（DCTDecode / FlateDecode），用 `image` 重编码
//!   并按 JPEG 质量二分搜索逼近 targetBytes；**只改图像 XObject 流，绝不碰
//!   文字层（内容流/字体）与书签/outlines 对象**。
//! - F4 图片→PDF：用 lopdf 生成 PDF（A4/letter/auto、边距、按文件名顺序）。
//! - F6 签名/表单：签名图作为 XObject 叠加到指定页矩形，表单写 /V；
//!   flatten 时把值烘焙进页面 appearance 并移除 /AcroForm。

use crate::engine::JobCtx;
use crate::error::{AppError, AppResult};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageFormat, RgbImage};
use lopdf::{
    dictionary, Dictionary, Document, Object, ObjectId, Stream,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn le(e: lopdf::Error, ctx: &str) -> AppError {
    AppError::internal(format!("{}：{}", ctx, e))
}

/// 载入 PDF；加密文件用已知密码解密（仅读取，不破解）。
pub(crate) fn load_doc(path: &Path, password: Option<&str>) -> AppResult<Document> {
    let mut doc = Document::load(path)
        .map_err(|e| AppError::parse(format!("文件无法读取/解析：{}", e)))?;
    if doc.is_encrypted() {
        match password {
            Some(pw) => doc
                .decrypt(pw)
                .map_err(|_| AppError::bad_password("密码不正确"))?,
            None => return Err(AppError::need_password("该文件有密码，请输入")),
        }
    }
    Ok(doc)
}

/// 页数（sidecar-free，供拆分/校验使用）。
pub fn page_count(path: &Path, password: Option<&str>) -> AppResult<u32> {
    Ok(load_doc(path, password)?.get_pages().len() as u32)
}

// ---------------------------------------------------------------------------
// F2：按书签拆分——读取 /Outlines 顶层标题与起始页
// ---------------------------------------------------------------------------

/// 返回 `(标题, 起始页 1-based)` 的顶层书签切点；读不到则明确报错。
pub fn outline_split_points(path: &Path, password: Option<&str>) -> AppResult<Vec<(String, u32)>> {
    let doc = load_doc(path, password)?;
    let toc = doc
        .get_toc()
        .map_err(|e| AppError::parse(format!("无法读取书签 /Outlines：{}", e)))?;

    let mut points: Vec<(String, u32)> = toc
        .toc
        .iter()
        .filter(|t| t.level <= 1) // 顶层
        .map(|t| (t.title.clone(), t.page as u32))
        .collect();
    points.sort_by_key(|(_, p)| *p);
    points.dedup_by_key(|(_, p)| *p);
    if points.is_empty() {
        return Err(AppError::parse(
            "未找到顶层书签（/Outlines），无法按书签拆分",
        ));
    }
    if points[0].1 > 1 {
        points.insert(0, ("(封面)".to_string(), 1));
    }
    Ok(points)
}

// ---------------------------------------------------------------------------
// F3：压缩到指定大小
// ---------------------------------------------------------------------------

pub struct CompressOutcome {
    pub path: PathBuf,
    pub actual_bytes: u64,
    pub quality_label: String,
    pub reached: bool,
    pub image_count: usize,
}

struct ImageEntry {
    id: ObjectId,
    img: DynamicImage,
}

/// 把图像 XObject 的流内容解码成 DynamicImage；不支持的类型返回 None（保持原样）。
fn decode_stream_image(s: &Stream) -> Option<DynamicImage> {
    let filters: Vec<Vec<u8>> = s
        .filters()
        .unwrap_or_default()
        .iter()
        .map(|f| f.to_vec())
        .collect();
    let has_dct = filters.iter().any(|f| f == b"DCTDecode");
    let has_jpx = filters.iter().any(|f| f == b"JPXDecode");
    if has_jpx {
        return None; // JPEG2000 需要额外解码器，跳过
    }
    if has_dct && filters.len() == 1 {
        // DCTDecode：流内容本身就是完整 JPEG 字节。
        return image::load_from_memory_with_format(&s.content, ImageFormat::Jpeg).ok();
    }

    // FlateDecode / 无滤镜 / ASCII85 / LZW：解压后是原始像素。
    let raw = s.get_plain_content().ok()?;
    let w = s.dict.get(b"Width").and_then(Object::as_i64).ok()? as u32;
    let h = s.dict.get(b"Height").and_then(Object::as_i64).ok()? as u32;
    let bpc = s
        .dict
        .get(b"BitsPerComponent")
        .and_then(Object::as_i64)
        .ok()
        .unwrap_or(8);
    if bpc != 8 || w == 0 || h == 0 {
        return None;
    }
    let cs = s.dict.get(b"ColorSpace").and_then(Object::as_name).ok();
    match cs {
        Some(b"DeviceGray") | None => {
            RgbImage::from_raw(w, h, expand_gray_to_rgb(&raw, w, h)).map(DynamicImage::ImageRgb8)
        }
        Some(b"DeviceRGB") => RgbImage::from_raw(w, h, raw).map(DynamicImage::ImageRgb8),
        _ => None, // DeviceCMYK / Indexed / ICCBased：不处理
    }
}

fn expand_gray_to_rgb(raw: &[u8], w: u32, h: u32) -> Vec<u8> {
    let need = (w as usize) * (h as usize) * 3;
    let mut out = Vec::with_capacity(need);
    for i in 0..(w as usize) * (h as usize) {
        let g = *raw.get(i).unwrap_or(&255);
        out.extend_from_slice(&[g, g, g]);
    }
    out
}

fn collect_images(doc: &Document) -> Vec<ImageEntry> {
    let mut out = Vec::new();
    for (&id, obj) in &doc.objects {
        let Object::Stream(s) = obj else { continue };
        let is_image = s
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .ok()
            .map(|n| n == b"Image")
            .unwrap_or(false);
        if !is_image {
            continue;
        }
        if let Some(img) = decode_stream_image(s) {
            out.push(ImageEntry { id, img });
        }
    }
    out
}

fn scaled_dims(w: u32, h: u32, scale: f32) -> (u32, u32) {
    let nw = (w as f32 * scale).round().max(1.0) as u32;
    let nh = (h as f32 * scale).round().max(1.0) as u32;
    (nw, nh)
}

/// 用给定质量/缩放重编码所有图像 XObject 为 JPEG(DCTDecode)，并更新其字典。
/// 只改图像流本身——文字层内容流、字体、书签/outlines 对象完全不动。
fn apply_encoding(
    doc: &mut Document,
    entries: &[ImageEntry],
    quality: u8,
    scale: f32,
) -> AppResult<()> {
    for e in entries {
        let (nw, nh) = scaled_dims(e.img.width(), e.img.height(), scale);
        let resized = if nw == e.img.width() && nh == e.img.height() {
            e.img.clone()
        } else {
            e.img.thumbnail(nw, nh)
        };
        let rgb = resized.to_rgb8();
        let mut jpeg: Vec<u8> = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality)
            .encode(rgb.as_raw(), nw, nh, ExtendedColorType::Rgb8)
            .map_err(|e| AppError::internal(format!("JPEG 重编码失败：{}", e)))?;

        let Some(obj) = doc.objects.get_mut(&e.id) else {
            continue;
        };
        let Object::Stream(s) = obj else { continue };
        // 移除旧的滤镜/解码参数/透明蒙版/反相，写入新的 DCTDecode 流。
        s.set_plain_content(jpeg);
        s.dict.remove(b"Decode");
        s.dict.remove(b"SMask");
        s.dict.remove(b"Mask");
        s.dict.set("Filter", "DCTDecode");
        s.dict.set("ColorSpace", "DeviceRGB");
        s.dict.set("BitsPerComponent", 8i64);
        s.dict.set("Width", nw as i64);
        s.dict.set("Height", nh as i64);
    }
    Ok(())
}

pub(crate) fn save_to_bytes(doc: &mut Document) -> AppResult<Vec<u8>> {
    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(AppError::from)?;
    Ok(buf)
}

fn quality_label(quality: &str, q: u8, scale: f32) -> String {
    let tier = if q >= 80 {
        "高画质"
    } else if q >= 50 {
        "均衡"
    } else {
        "小体积"
    };
    format!(
        "{}（JPEG 质量 {}，缩放 {:.0}%），输入档位 {}",
        tier,
        q,
        scale * 100.0,
        quality
    )
}

/// F3 主流程：图像重编码 + 质量二分搜索逼近目标大小。
/// 达不到目标时仍产出最小可达结果，并在 outcome 中说明实际大小与画质档位。
pub fn compress(
    ctx: &JobCtx<'_>,
    path: &Path,
    password: Option<&str>,
    target_bytes: u64,
    quality: &str,
    tmp_dir: &Path,
) -> AppResult<CompressOutcome> {
    let mut doc = load_doc(path, password)?;
    if doc.get_pages().len() > 5000 {
        return Err(AppError::page_limit("单文件超过 5000 页，已拒绝"));
    }
    let entries = collect_images(&doc);
    let out_path = tmp_dir.join("compressed.pdf");

    if entries.is_empty() {
        let bytes = save_to_bytes(&mut doc)?;
        std::fs::write(&out_path, &bytes).map_err(AppError::from)?;
        return Ok(CompressOutcome {
            path: out_path,
            actual_bytes: bytes.len() as u64,
            quality_label: "未找到可重编码的图像，原样保留（文字层与书签不变）".to_string(),
            reached: (bytes.len() as u64) <= target_bytes,
            image_count: 0,
        });
    }

    let (q_floor, scales): (u8, Vec<f32>) = match quality {
        "high" => (55, vec![1.0, 0.9]),
        "balanced" => (35, vec![1.0, 0.85, 0.7]),
        "small" => (20, vec![0.8, 0.65, 0.5]),
        _ => (15, vec![1.0, 0.85, 0.7, 0.55, 0.4]), // auto
    };
    let q_ceil: u8 = 95;

    // best = (pdf 字节, 采用的质量, 缩放, 大小)
    let mut best: Option<(Vec<u8>, u8, f32, u64)> = None;

    for (si, &scale) in scales.iter().enumerate() {
        ctx.tick(
            si + 1,
            scales.len(),
            format!("压缩画质搜索（缩放 {:.0}%）", scale * 100.0),
        )?;

        // 二分搜索：找到能 <= target 的最高 JPEG 质量。
        let mut lo = q_floor;
        let mut hi = q_ceil;
        let mut round_best: Option<(Vec<u8>, u8, u64)> = None;
        let mut steps = 0;
        while lo <= hi && steps < 20 {
            steps += 1;
            let mid = ((lo as u16 + hi as u16) / 2) as u8;
            apply_encoding(&mut doc, &entries, mid, scale)?;
            let bytes = save_to_bytes(&mut doc)?;
            let size = bytes.len() as u64;
            if size <= target_bytes {
                round_best = Some((bytes, mid, size));
                lo = mid.saturating_add(1);
            } else {
                if mid == 0 {
                    break;
                }
                hi = mid - 1;
            }
        }

        if let Some((bytes, q, size)) = round_best {
            // 当前（最保真）缩放已达目标：采用并停止。
            best = Some((bytes, q, scale, size));
            break;
        }

        // 本缩放仍达不到目标：记录该缩放下最小质量的结果作为兜底。
        apply_encoding(&mut doc, &entries, q_floor, scale)?;
        let bytes = save_to_bytes(&mut doc)?;
        let size = bytes.len() as u64;
        if best.is_none() || size < best.as_ref().unwrap().3 {
            best = Some((bytes, q_floor, scale, size));
        }
    }

    let (bytes, q, scale, size) = best.expect("至少产生一轮压缩结果");
    std::fs::write(&out_path, &bytes).map_err(AppError::from)?;
    let reached = size <= target_bytes;
    Ok(CompressOutcome {
        path: out_path,
        actual_bytes: size,
        quality_label: quality_label(quality, q, scale),
        reached,
        image_count: entries.len(),
    })
}

// ---------------------------------------------------------------------------
// F4：图片 → PDF
// ---------------------------------------------------------------------------

fn composite_white(img: DynamicImage) -> RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let raw = rgba.into_raw();
    let mut buf = Vec::with_capacity((w as usize) * (h as usize) * 3);
    for px in raw.chunks_exact(4) {
        let a = px[3] as f32 / 255.0;
        let blend = |c: u8| (c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        buf.push(blend(px[0]));
        buf.push(blend(px[1]));
        buf.push(blend(px[2]));
    }
    RgbImage::from_raw(w, h, buf).unwrap_or_else(|| RgbImage::new(w, h))
}

fn build_image_xobject(rgb: &RgbImage) -> AppResult<Stream> {
    let (w, h) = (rgb.width(), rgb.height());
    let mut jpeg: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
        .map_err(|e| AppError::internal(format!("图片 JPEG 编码失败：{}", e)))?;
    let dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => w as i64,
        "Height" => h as i64,
        "ColorSpace" => "DeviceRGB",
        "BitsPerComponent" => 8i64,
        "Filter" => "DCTDecode",
    };
    Ok(Stream::new(dict, jpeg))
}

fn reals(vals: &[f32]) -> Vec<Object> {
    vals.iter().map(|&v| Object::Real(v)).collect()
}

/// 图片转 PDF：按文件名顺序、页面大小（auto/a4/letter）与边距排版。
/// 结果写入 `out_tmp`（临时路径），由上层原子提交。
pub fn images_to_pdf(
    ctx: &JobCtx<'_>,
    files: &[PathBuf],
    page_size: &str,
    margin: f64,
    out_tmp: &Path,
) -> AppResult<()> {
    let mut sorted = files.to_vec();
    sorted.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids: Vec<ObjectId> = Vec::new();

    let n = sorted.len();
    for (i, f) in sorted.iter().enumerate() {
        ctx.tick(i + 1, n, format!("排版图片 {}/{}", i + 1, n))?;
        let img = image::open(f)
            .map_err(|e| AppError::parse(format!("图片无法读取 {}：{}", f.display(), e)))?;
        let rgb = composite_white(img);
        let (iw, ih) = (rgb.width() as f32, rgb.height() as f32);

        let (page_w, page_h, scale) = match page_size {
            "a4" => {
                let (pw, ph) = (595.276, 841.89);
                (pw, ph, page_scale(iw, ih, pw, ph, margin as f32))
            }
            "letter" => {
                let (pw, ph) = (612.0, 792.0);
                (pw, ph, page_scale(iw, ih, pw, ph, margin as f32))
            }
            _ => (iw + 2.0 * margin as f32, ih + 2.0 * margin as f32, 1.0),
        };
        let draw_w = iw * scale;
        let draw_h = ih * scale;
        let avail_w = page_w - 2.0 * margin as f32;
        let avail_h = page_h - 2.0 * margin as f32;
        let x = margin as f32 + (avail_w - draw_w) / 2.0;
        let y = margin as f32 + (avail_h - draw_h) / 2.0;

        let img_stream = build_image_xobject(&rgb)?;
        let img_id = doc.add_object(img_stream);
        let img_name = format!("Im{}", img_id.0);

        let content = format!(
            "q\n{} 0 0 {} {} {} cm\n/{} Do\nQ\n",
            draw_w, draw_h, x, y, img_name
        );
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => reals(&[0.0, 0.0, page_w, page_h]),
        });
        doc.add_xobject(page_id, img_name.as_bytes(), img_id)
            .map_err(|e| le(e, "注册图片 XObject 失败"))?;
        page_ids.push(page_id);
    }

    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => page_ids.len() as i64,
            "Kids" => kids,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let bytes = save_to_bytes(&mut doc)?;
    std::fs::write(out_tmp, bytes).map_err(AppError::from)?;
    Ok(())
}

fn page_scale(iw: f32, ih: f32, pw: f32, ph: f32, margin: f32) -> f32 {
    let avail_w = (pw - 2.0 * margin).max(1.0);
    let avail_h = (ph - 2.0 * margin).max(1.0);
    (avail_w / iw).min(avail_h / ih).min(1.0)
}

// ---------------------------------------------------------------------------
// F6：签名图片叠加 + 表单填写 / 扁平化
// ---------------------------------------------------------------------------

fn escape_pdf_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_ascii() && !c.is_control() => out.push(c),
            // 限制：使用标准 Helvetica/WinAnsi，不嵌入字体，非拉丁字符（如中文）
            // 无法在烘焙外观流中正确显示，这里降级为 '?'（见下方说明）。
            _ => out.push('?'),
        }
    }
    out
}

fn get_rect(d: &Dictionary) -> Option<[f32; 4]> {
    let arr = d.get(b"Rect").ok()?.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let mut r = [0f32; 4];
    for i in 0..4 {
        r[i] = match &arr[i] {
            Object::Integer(v) => *v as f32,
            Object::Real(v) => *v,
            _ => return None,
        };
    }
    Some(r)
}

fn find_page_for_annot(doc: &Document, aid: ObjectId) -> Option<ObjectId> {
    for (_num, pid) in doc.get_pages() {
        if let Ok(page) = doc.get_object(pid).and_then(Object::as_dict) {
            if let Ok(arr) = page.get(b"Annots").and_then(Object::as_array) {
                if arr.iter().any(|o| o.as_reference().ok() == Some(aid)) {
                    return Some(pid);
                }
            }
        }
    }
    None
}

/// 把字段值以标准 Helvetica 文本烘焙进指定页的 appearance（content stream）。
/// 限制：仅文本字段、仅基础拉丁字符、不嵌入字体、不支持多行/复选/单选外观。
fn bake_text_appearance(doc: &mut Document, page_id: ObjectId, rect: [f32; 4], value: &str) {
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    if let Ok(fonts) = doc
        .get_or_create_resources(page_id)
        .and_then(Object::as_dict_mut)
    {
        if !fonts.has(b"Font") {
            fonts.set("Font", Dictionary::new());
        }
        if let Ok(fdict) = fonts.get_mut(b"Font").and_then(Object::as_dict_mut) {
            fdict.set("FOnce", Object::Reference(font_id));
        }
    }
    let x = rect[0] + 2.0;
    let y = rect[1] + 2.0;
    let size = ((rect[3] - rect[1]) * 0.6).clamp(6.0, 12.0);
    let content = format!(
        "q\nBT\n/FOnce {} Tf\n1 0 0 1 {} {} Tm\n({}) Tj\nET\nQ\n",
        size,
        x,
        y,
        escape_pdf_string(value)
    );
    let _ = doc.add_page_contents(page_id, content.into_bytes());
}

/// 写 /V 并（可选）扁平化：烘焙 appearance + 移除 /AcroForm + 去掉可编辑 widget。
fn fill_and_flatten(
    doc: &mut Document,
    fields: &HashMap<String, String>,
    flatten: bool,
) -> AppResult<()> {
    let widget_ids: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(&id, o)| {
            let d = o.as_dict().ok()?;
            let is_widget = d
                .get(b"Subtype")
                .and_then(Object::as_name)
                .ok()
                .map(|n| n == b"Widget")
                .unwrap_or(false);
            is_widget.then_some(id)
        })
        .collect();

    let mut bakes: Vec<(ObjectId, [f32; 4], String)> = Vec::new();
    for id in &widget_ids {
        let (name, rect) = {
            let d = doc.objects.get(id).and_then(|o| o.as_dict().ok());
            let Some(d) = d else { continue };
            let name = d
                .get(b"T")
                .ok()
                .and_then(|v| v.as_str().ok())
                .map(|b| String::from_utf8_lossy(b).to_string());
            (name, get_rect(d))
        };
        let Some(name) = name else { continue };
        let Some(value) = fields.get(&name) else { continue };

        if let Some(d) = doc.objects.get_mut(id).and_then(|o| o.as_dict_mut().ok()) {
            d.set("V", Object::string_literal(value.clone()));
        }
        if flatten {
            if let (Some(page_id), Some(rect)) = (find_page_for_annot(doc, *id), rect) {
                bakes.push((page_id, rect, value.clone()));
            }
        }
    }

    if flatten {
        for (page_id, rect, value) in bakes {
            bake_text_appearance(doc, page_id, rect, &value);
        }
        // 移除 /AcroForm：阅读器不再把它当作可填写表单。
        if let Ok(cat) = doc.catalog_mut() {
            cat.remove(b"AcroForm");
        }
        // 从页面 /Annots 去掉 widget 注解，使字段不可再编辑；
        // 值已经烘焙进上面的 appearance 内容流。
        let widget_set: HashSet<ObjectId> = widget_ids.into_iter().collect();
        let page_ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
        for pid in page_ids {
            if let Ok(page) = doc.get_object_mut(pid).and_then(|o| o.as_dict_mut()) {
                if let Ok(annots) = page.get_mut(b"Annots").and_then(Object::as_array_mut) {
                    annots.retain(|o| match o.as_reference() {
                        Ok(rid) => !widget_set.contains(&rid),
                        Err(_) => true,
                    });
                }
            }
        }
    }
    Ok(())
}

/// F6 主流程：签名图片叠加到指定页矩形 + 表单填写/扁平化。
#[allow(clippy::too_many_arguments)]
pub fn sign(
    path: &Path,
    password: Option<&str>,
    signature_path: &Path,
    page: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    fields: &HashMap<String, String>,
    flatten_fields: bool,
    out_tmp: &Path,
) -> AppResult<()> {
    let mut doc = load_doc(path, password)?;
    if doc.get_pages().len() > 5000 {
        return Err(AppError::page_limit("单文件超过 5000 页，已拒绝"));
    }
    let page_id = *doc
        .get_pages()
        .get(&page)
        .ok_or_else(|| AppError::internal(format!("页码 {} 不存在", page)))?;

    let img = image::open(signature_path)
        .map_err(|e| AppError::parse(format!("签名图片无法读取：{}", e)))?;
    let rgb = composite_white(img);
    let img_stream = build_image_xobject(&rgb)?;
    let img_id = doc.add_object(img_stream);
    let img_name = format!("Im{}", img_id.0);
    doc.add_xobject(page_id, img_name.as_bytes(), img_id)
        .map_err(|e| le(e, "注册签名 XObject 失败"))?;

    // 增量追加内容流（不解码/改写原有内容流 → 文字层与外观完整保留）：
    // q w 0 0 h x y cm /Im Do Q
    let content = format!(
        "q\n{} 0 0 {} {} {} cm\n/{} Do\nQ\n",
        width, height, x, y, img_name
    );
    doc.add_page_contents(page_id, content.into_bytes())
        .map_err(|e| le(e, "追加签名内容流失败"))?;

    if !fields.is_empty() || flatten_fields {
        fill_and_flatten(&mut doc, fields, flatten_fields)?;
    }

    let bytes = save_to_bytes(&mut doc)?;
    std::fs::write(out_tmp, bytes).map_err(AppError::from)?;
    Ok(())
}
