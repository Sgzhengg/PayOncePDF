//! F4 渲染：PDFium 动态加载 → PNG/JPG 位图（PRD §7 F4、§9.1；许可 BSD-3）。
//!
//! 运行时从 `resource_dir` 找 `pdfium.dll` / `libpdfium.dylib` / `libpdfium.so`，
//! 找不到返回 E2002。动态库进程内只加载一次（`Pdfium::new` 内部是全局 OnceCell），
//! 用 `OnceLock` + `Mutex` 保护初始化，避免并发重复 bind 报错。

use crate::engine::{geometry, sidecar};
use crate::error::{AppError, AppResult};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageFormat, RgbImage};
use pdfium_render::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

/// F18 画布框选修订联动：一页的字符级文字布局。
///
/// 坐标一律是 **PDF point、页面左下原点**（与 `FPDFText_GetCharBox` / lopdf 的
/// `/Rect` / F11 `QuadPoints` 一致），前端按「图片显示尺寸 → 页面 pt」比例换算。
/// 纯内存查询：不写临时文件、不联网、不上传文件内容或文件名。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextCharBox {
    /// PDFium text page 的全局字符索引（0-based），前端按此顺序拼接选中文字。
    pub index: u32,
    pub ch: String,
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTextLayout {
    pub page: u32,
    pub pages: u32,
    pub page_width_pt: f32,
    pub page_height_pt: f32,
    pub chars: Vec<TextCharBox>,
    /// 按 `chars` 顺序拼接的整页文本，供前端判断选中文字是否在本页重复出现。
    pub text: String,
    /// 超过字符上限时置 true 且 `chars` / `text` 为空：前端必须禁用框选，不得猜测坐标。
    pub truncated: bool,
    /// 扫描件、纯图片页或旋转页为 false：前端必须禁用框选。
    pub has_text_layer: bool,
    /// 页面 /Rotate（0 / 90 / 180 / 270）。非 0 时字符 bbox 是内容流坐标，
    /// 与旋转后的渲染图坐标系不一致，直接求交会错位，因此一律禁用框选。
    pub rotation: u32,
    /// 不可框选的明确原因；可框选时为 null（不序列化）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported_reason: Option<String>,
}

/// 单页最多回传 6 万个字符，避免超大页面把 IPC 与前端内存撑爆。
pub const MAX_LAYOUT_CHARS: usize = 60_000;

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());

/// 加载（进程内一次）PDFium，返回静态引用。缺库 → E2002。
pub(crate) fn load_pdfium(app: &AppHandle) -> AppResult<&'static Pdfium> {
    if let Some(p) = PDFIUM.get() {
        return Ok(p);
    }
    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = PDFIUM.get() {
        return Ok(p);
    }
    let lib: PathBuf = sidecar::pdfium_path(app)?; // 找不到 → E2002
    let bindings = Pdfium::bind_to_library(&lib)
        .map_err(|e| AppError::pdfium_missing(format!("PDFium 动态库加载失败：{}", e)))?;
    let pdfium = Pdfium::new(bindings);
    let _ = PDFIUM.set(pdfium);
    Ok(PDFIUM.get().expect("PDFIUM 已设置"))
}

fn map_load_error(e: PdfiumError, password: Option<&str>) -> AppError {
    let s = e.to_string().to_ascii_lowercase();
    if s.contains("password") {
        if password.is_some() {
            AppError::bad_password("密码不正确")
        } else {
            AppError::need_password("该文件有密码，请输入")
        }
    } else {
        AppError::parse(format!("文件无法读取（PDFium 无法解析）：{}", e))
    }
}

/// 一次打开的 PDF 文档句柄（借用已加载的 PDFium），可重复渲染多页。
pub struct PdfDoc<'a> {
    doc: PdfDocument<'a>,
}

impl<'a> PdfDoc<'a> {
    pub fn open(pdfium: &'a Pdfium, path: &Path, password: Option<&str>) -> AppResult<Self> {
        let doc = pdfium
            .load_pdf_from_file(path, password)
            .map_err(|e| map_load_error(e, password))?;
        Ok(Self { doc })
    }

    pub fn page_count(&self) -> u32 {
        self.doc.pages().len().max(0) as u32
    }

    /// 按 DPI 把指定页（1-based）渲染并写入图片文件。
    /// `format` = "png" | "jpg"；`quality` 仅对 jpg 生效（1-100）。
    pub fn render_page(
        &self,
        page_1based: u32,
        dpi: u32,
        out: &Path,
        format: &str,
        quality: u8,
    ) -> AppResult<()> {
        let index = (page_1based.saturating_sub(1)) as PdfPageIndex;
        let page = self
            .doc
            .pages()
            .get(index)
            .map_err(|e| AppError::parse(format!("无法读取第 {} 页：{}", page_1based, e)))?;

        // PDF 1pt = 1/72 inch，按 DPI 换算像素尺寸。
        let w_px = (page.width().value * dpi as f32 / 72.0).round().max(1.0) as Pixels;
        let h_px = (page.height().value * dpi as f32 / 72.0).round().max(1.0) as Pixels;

        let bitmap = page
            .render(w_px, h_px, None)
            .map_err(|e| AppError::internal(format!("渲染第 {} 页失败：{}", page_1based, e)))?;
        let img: DynamicImage = bitmap
            .as_image()
            .map_err(|e| AppError::internal(format!("位图转换失败：{}", e)))?;

        if format.eq_ignore_ascii_case("jpg") || format.eq_ignore_ascii_case("jpeg") {
            let rgb: RgbImage = img.to_rgb8();
            let mut buf: Vec<u8> = Vec::new();
            let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|e| AppError::internal(format!("JPEG 编码失败：{}", e)))?;
            std::fs::write(out, buf).map_err(AppError::from)?;
        } else {
            img.save_with_format(out, ImageFormat::Png)
                .map_err(|e| AppError::internal(format!("PNG 写入失败：{}", e)))?;
        }
        Ok(())
    }

    /// 单页渲染为 PNG 字节（F17 内置阅读视图用）。
    /// 纯内存：不写临时文件、不联网、不上传；DPI 夹在 48–200 防超大页面撑爆内存。
    pub fn render_page_png(
        &self,
        page_1based: u32,
        dpi: u32,
    ) -> AppResult<(Vec<u8>, u32, u32)> {
        let dpi = dpi.clamp(48, 200);
        let index = (page_1based.saturating_sub(1)) as PdfPageIndex;
        let page = self
            .doc
            .pages()
            .get(index)
            .map_err(|e| AppError::parse(format!("无法读取第 {} 页：{}", page_1based, e)))?;

        let w_px = (page.width().value * dpi as f32 / 72.0).round().max(1.0) as Pixels;
        let h_px = (page.height().value * dpi as f32 / 72.0).round().max(1.0) as Pixels;
        let bitmap = page
            .render(w_px, h_px, None)
            .map_err(|e| AppError::internal(format!("渲染第 {} 页失败：{}", page_1based, e)))?;
        let img: DynamicImage = bitmap
            .as_image()
            .map_err(|e| AppError::internal(format!("位图转换失败：{}", e)))?;

        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| AppError::internal(format!("PNG 编码失败：{}", e)))?;
        Ok((buf.into_inner(), w_px as u32, h_px as u32))
    }

    /// 页面尺寸（PDF point，1pt = 1/72 inch），供阅读视图显示纸张规格。
    pub fn page_size_pt(&self, page_1based: u32) -> AppResult<(f32, f32)> {
        let index = (page_1based.saturating_sub(1)) as PdfPageIndex;
        let page = self
            .doc
            .pages()
            .get(index)
            .map_err(|e| AppError::parse(format!("无法读取第 {} 页：{}", page_1based, e)))?;
        Ok((page.width().value, page.height().value))
    }

    /// F18：读取单页字符级文字布局（PDFium `FPDFText_GetCharBox`）。
    ///
    /// 旋转页（`/Rotate != 0`）支持框选：PDFium 的字符框是内容流坐标，渲染图是
    /// 旋转后的显示坐标；这里用 `geometry::content_to_display()` 把字符框变换到
    /// 显示坐标系再回传，前端「屏幕→显示 pt」的求交口径不变。纯内存查询，
    /// 不生成临时「去旋转」副本，保持 F17/F18 不落盘的隐私口径。
    pub fn page_text_layout(&self, page_1based: u32) -> AppResult<PageTextLayout> {
        let pages = self.page_count();
        if pages == 0 {
            return Err(AppError::parse("该 PDF 没有可读取的页面"));
        }
        let page_no = page_1based.clamp(1, pages);
        let page = self
            .doc
            .pages()
            .get((page_no - 1) as PdfPageIndex)
            .map_err(|e| AppError::parse(format!("无法读取第 {} 页：{}", page_no, e)))?;
        let width = page.width().value;
        let height = page.height().value;
        let rotation = match page.rotation() {
            Ok(PdfPageRenderRotation::Degrees90) => 90,
            Ok(PdfPageRenderRotation::Degrees180) => 180,
            Ok(PdfPageRenderRotation::Degrees270) => 270,
            _ => 0,
        };
        let empty = |truncated: bool, reason: &str| PageTextLayout {
            page: page_no,
            pages,
            page_width_pt: width,
            page_height_pt: height,
            chars: Vec::new(),
            text: String::new(),
            truncated,
            has_text_layer: false,
            rotation,
            unsupported_reason: Some(reason.to_string()),
        };
        let rot = geometry::PageRotation::from_degrees(rotation);
        let text = page
            .text()
            .map_err(|e| AppError::parse(format!("第 {} 页文字层读取失败：{}", page_no, e)))?;
        let count = text.len() as usize;
        if count == 0 {
            return Ok(empty(
                false,
                "本页没有可选中文字（扫描件或纯图片页）；如需处理请先走 OCR",
            ));
        }
        if count > MAX_LAYOUT_CHARS {
            return Ok(empty(
                true,
                "本页文字超过 6 万字符，为保证响应速度已禁用框选；可在右侧手动填写查找文字",
            ));
        }

        // 字符框是内容流坐标；旋转页要用内容流尺寸把它变换到显示坐标系。
        // width/height 是显示尺寸（实测 90/270 时已交换），故先反推内容流尺寸。
        let (content_w, content_h) = geometry::display_size(rot, width, height);
        let collection = text.chars();
        let mut chars: Vec<TextCharBox> = Vec::with_capacity(count);
        let mut full = String::with_capacity(count * 2);
        for c in collection.iter() {
            let bounds = c
                .tight_bounds()
                .or_else(|_| c.loose_bounds())
                .unwrap_or_else(|_| PdfRect::zero());
            let ch = c.unicode_char().unwrap_or('\u{FFFD}');
            full.push(ch);
            let (left, bottom, right, top) = geometry::content_to_display(
                rot,
                (
                    bounds.left().value,
                    bounds.bottom().value,
                    bounds.right().value,
                    bounds.top().value,
                ),
                content_w,
                content_h,
            );
            chars.push(TextCharBox {
                index: c.index() as u32,
                ch: ch.to_string(),
                left,
                bottom,
                right,
                top,
            });
        }
        let has_text_layer = !chars.is_empty();
        Ok(PageTextLayout {
            page: page_no,
            pages,
            page_width_pt: width,
            page_height_pt: height,
            chars,
            text: full,
            truncated: false,
            has_text_layer,
            rotation,
            unsupported_reason: None,
        })
    }
}

/// 便捷入口：加载 PDFium 并打开文档。
pub fn open(app: &AppHandle, pdf: &Path, password: Option<&str>) -> AppResult<PdfDoc<'static>> {
    let pdfium = load_pdfium(app)?;
    PdfDoc::open(pdfium, pdf, password)
}

/// 仅查询页数（供 OCR / 拆分做页数校验）。
pub fn page_count(app: &AppHandle, pdf: &Path, password: Option<&str>) -> AppResult<u32> {
    Ok(open(app, pdf, password)?.page_count())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lopdf::{dictionary, Dictionary, Document, Object, Stream};
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static TEST_PDFIUM: OnceLock<Pdfium> = OnceLock::new();

    /// PDFium 的 bindings 是进程级 OnceCell，整个测试进程只能 bind 一次；
    /// `engine::extras` 的端到端测试也委托到这个唯一实例。
    pub(crate) fn test_pdfium() -> &'static Pdfium {
        TEST_PDFIUM.get_or_init(|| {
            let candidates = [
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium.dll"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../.sidecar-cache/pdfium/bin/pdfium.dll"),
            ];
            let lib = candidates
                .iter()
                .find(|p| p.exists())
                .expect("找不到 pdfium.dll：请先运行 node scripts/provision-sidecars.mjs");
            let bindings = Pdfium::bind_to_library(lib)
                .expect("PDFium 动态库加载失败（依赖 DLL 需与 pdfium.dll 同目录）");
            Pdfium::new(bindings)
        })
    }

    /// 真实中文样本（scripts/make-cjk-fixture.py 生成，内嵌 SimHei 子集 + ToUnicode）：
    /// 文字层必须还原成汉字，渲染必须出图。用于补上「中文 PDF 从未验证」的缺口。
    #[test]
    fn cjk_real_fixture_has_text_layer_and_renders() {
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/text_cjk_real.pdf"
        ));
        if !path.exists() {
            eprintln!("跳过：缺 text_cjk_real.pdf（先跑 python scripts/make-cjk-fixture.py）");
            return;
        }
        let pdfium = test_pdfium();
        let doc = PdfDoc::open(pdfium, &path, None).expect("真实中文 PDF 应能打开");

        let layout = doc.page_text_layout(1).expect("应返回字符布局");
        assert!(layout.has_text_layer, "内嵌字体的中文 PDF 必须有文字层");
        assert_eq!(layout.rotation, 0);
        let text = layout.text.clone();
        eprintln!("文字层 = {:?}", text);
        for needle in ["中文测试", "就地改错字", "合同编号", "2026", "付款期限", "逾期按日计息"] {
            assert!(text.contains(needle), "文字层缺 {:?}：{:?}", needle, text);
        }
        assert!(!text.contains('\u{FFFD}'), "文字层不应出现替换符：{:?}", text);
        // 坐标必须是有限正数（前端据此画高亮）
        assert!(layout.page_width_pt > 100.0 && layout.page_height_pt > 100.0);
        assert!(!layout.chars.is_empty());
        for c in &layout.chars {
            assert!(c.right >= c.left && c.top >= c.bottom, "字符框退化：{:?}", c);
        }

        let (png, w, h) = doc.render_page_png(1, 150).expect("应能渲染成 PNG");
        assert!(png.len() > 5_000, "渲染结果不应是空白图：{} 字节", png.len());
        assert!(w > 800 && h > 1000, "150 DPI 下 A4 应约 1240x1754：{}x{}", w, h);
        let out = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/out/cjk-real-render.png"
        ));
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::write(&out, &png).unwrap();
        eprintln!("已写出 {}（{} 字节，{}x{}）", out.display(), png.len(), w, h);
    }

    /// 用内置 Helvetica 造一份最小 PDF；rotate != 0 时给页面加 /Rotate。
    fn text_pdf_bytes(text: &str, rotate: i64) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let content = if text.is_empty() {
            "q Q\n".to_string()
        } else {
            let escaped = text
                .replace('\\', r"\\")
                .replace('(', r"\(")
                .replace(')', r"\)");
            format!("BT /F1 18 Tf 72 700 Td ({}) Tj ET\n", escaped)
        };
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));
        let mut res = Dictionary::new();
        res.set(
            "Font",
            Object::Dictionary(dictionary! { "F1" => Object::Reference(font_id) }),
        );
        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(595), Object::Integer(842),
            ],
            "Contents" => Object::Reference(content_id),
            "Resources" => Object::Dictionary(res),
        };
        if rotate != 0 {
            page.set("Rotate", rotate);
        }
        let pid = doc.add_object(page);
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
        crate::engine::pdf::save_to_bytes(&mut doc).unwrap()
    }

    /// 每个用例用独立文件名：Rust 单测并行执行，共用路径会在 Windows 上互相拒绝访问。
    fn layout_of(bytes: &[u8], page: u32, tag: &str) -> PageTextLayout {
        let dir = std::env::temp_dir().join(format!("payoncepdf_layout_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("in_{tag}.pdf"));
        std::fs::write(&path, bytes).unwrap();
        let pdfium = test_pdfium();
        let doc = PdfDoc::open(pdfium, &path, None).unwrap();
        let out = doc.page_text_layout(page).unwrap();
        // PDFium 可能仍持有文件句柄；清理失败不影响断言，忽略即可。
        drop(doc);
        let _ = std::fs::remove_file(&path);
        out
    }

    #[test]
    fn layout_returns_real_char_boxes_and_text() {
        let out = layout_of(&text_pdf_bytes("Hello World", 0), 1, "ascii");
        assert_eq!(out.page, 1);
        assert_eq!(out.pages, 1);
        assert!(!out.truncated);
        assert!(out.has_text_layer);
        assert!(out.text.contains("Hello World"), "text={:?}", out.text);
        assert!(out.chars.len() >= 11, "chars={}", out.chars.len());
        assert_eq!(out.chars[0].index, 0);
        assert_eq!(out.chars[0].ch, "H");
        // 包围盒必须落在页面内，且真实字形有正宽度/高度（而不是全 0 的假坐标）。
        assert!(out.chars.iter().any(|c| c.right > c.left && c.top > c.bottom));
        assert!(out
            .chars
            .iter()
            .all(|c| c.left >= -1.0 && c.right <= out.page_width_pt + 1.0
                && c.bottom >= -1.0 && c.top <= out.page_height_pt + 1.0));
        // 拼接顺序与 text 一致（前端依赖这个顺序做选中文字与重复计数）。
        assert_eq!(
            out.chars.iter().map(|c| c.ch.as_str()).collect::<String>(),
            out.text
        );
    }

    #[test]
    fn blank_page_has_no_text_layer() {
        let blank = layout_of(&text_pdf_bytes("", 0), 1, "blank");
        assert!(!blank.has_text_layer);
        assert!(blank.chars.is_empty());
        assert!(!blank.truncated);
    }

    /// 2026-11-21：旋转页框选支持。字符框必须落在显示尺寸内（595×842 内容流
    /// 旋转 90° 后是 842×595），且文本顺序不变，前端才能正确求交与拼接。
    #[test]
    fn rotated_page_returns_display_space_char_boxes() {
        for deg in [0i64, 90, 180, 270] {
            let out = layout_of(&text_pdf_bytes("Hello World", deg), 1, &format!("r{deg}"));
            assert_eq!(out.rotation, deg as u32);
            assert!(out.has_text_layer, "/Rotate {} 应可框选", deg);
            assert_eq!(out.text.replace(' ', ""), "HelloWorld");
            assert!(out.unsupported_reason.is_none());
            // 90/270 显示尺寸交换，字符框必须落在新坐标系内。
            let (dw, dh) = if deg % 180 == 0 { (595.0, 842.0) } else { (842.0, 595.0) };
            assert_eq!(out.page_width_pt, dw);
            assert_eq!(out.page_height_pt, dh);
            for c in &out.chars {
                assert!(c.left >= -0.5 && c.right <= dw + 0.5, "deg={} {:?}", deg, c);
                assert!(c.bottom >= -0.5 && c.top <= dh + 0.5, "deg={} {:?}", deg, c);
            }
        }
    }

    #[test]
    fn layout_clamps_out_of_range_page() {
        let out = layout_of(&text_pdf_bytes("Hello", 0), 99, "clamp");
        assert_eq!(out.page, 1);
        assert!(out.has_text_layer);
    }

    /// F18 前端「显示像素 → PDF pt」换算的前提：渲染图尺寸必须严格等于
    /// `pageWidthPt / pageHeightPt × dpi / 72`，且字符 bbox 换算后落在图片内。
    /// 若这条不成立，框选矩形与文字框会整体错位。
    #[test]
    fn rendered_pixels_match_layout_coordinate_space() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_layout_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("in_geometry.pdf");
        std::fs::write(&path, text_pdf_bytes("Hello World", 0)).unwrap();
        let pdfium = test_pdfium();
        let doc = PdfDoc::open(pdfium, &path, None).unwrap();
        let layout = doc.page_text_layout(1).unwrap();
        for dpi in [48u32, 96, 200] {
            let (_, w, h) = doc.render_page_png(1, dpi).unwrap();
            let expected_w = (layout.page_width_pt * dpi as f32 / 72.0).round() as u32;
            let expected_h = (layout.page_height_pt * dpi as f32 / 72.0).round() as u32;
            assert_eq!(w, expected_w, "dpi={dpi} 宽度与换算基准不一致");
            assert_eq!(h, expected_h, "dpi={dpi} 高度与换算基准不一致");
        }
        let (_, w, h) = doc.render_page_png(1, 96).unwrap();
        for c in layout.chars.iter().filter(|c| c.right > c.left) {
            let px = (c.right * 96.0 / 72.0) as u32;
            let py_top = ((layout.page_height_pt - c.top) * 96.0 / 72.0) as u32;
            assert!(px <= w && py_top <= h, "字符 bbox 超出渲染图");
        }
        drop(doc);
        let _ = std::fs::remove_file(&path);
    }
}
