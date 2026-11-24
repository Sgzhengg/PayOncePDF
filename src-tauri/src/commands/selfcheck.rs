//! F21 应用内功能自检（参考 Grabber `run_selftest` 的分层做法）。
//!
//! 目的：真机 GUI 测试成本高，把「不依赖人眼 / 系统弹窗 / 硬件」的部分收进应用内，
//! 点一次即可跑完，并把结构化报告写到下载目录，便于回传定位；剩下真正需要真机的
//! 项目在 `NEEDS_REAL_DEVICE` 里显式列出，绝不假装通过。
//!
//! 三层：
//! - `env.*`   环境依赖（报告目录可写、sidecar 就绪、授权状态、版本同源）
//! - `logic.*` 纯逻辑（页码 / 书签层级 / diff / 旋转坐标 / 输出保护 / 错误码）
//! - `engine.*`真实引擎端到端（内置样本 PDF → qpdf / lopdf / PDFium → 校验产物）
//!
//! 诚实边界：自检不占导出授权、不联网、不碰用户文件；只在系统临时目录里造样本，
//! 结束后清理。报告只写版本 / 组件路径 / 用例结果，不含任何用户文件名或路径。

use crate::commands::EmptyPayload;
use crate::engine::{
    self, compare, extras, geometry, more, objects, ocr, pdf, qpdf, render, sidecar,
};
use crate::error::{AppError, AppResult};
use crate::{license, state};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tauri::{AppHandle, Manager};

pub const SCHEMA: &str = "payonce.selftest/1";

/// 自检覆盖不到、必须留在真机人工验证的项目（与 `docs/real-device-test-plan.md` 对应）。
/// 这份清单随报告一起返回，用户可以直接照着跑剩下那部分，不再重复已被自检覆盖的功能。
pub const NEEDS_REAL_DEVICE: &[&str] = &[
    "安装包全新安装 / 覆盖安装 / 卸载（SmartScreen、Gatekeeper、公证、快捷方式）",
    "干净机器缺 WebView2 时的启动与安装器行为",
    "原生文件选择 / 保存对话框与从资源管理器拖放",
    "高 DPI / 多显示器 / 系统缩放下的画布框选坐标与布局",
    "真实鼠标拖选、连续改多处、改完仍可继续框选（F18/F19 交互链路）",
    "试用到期弹缴费窗口、输入真实激活码、断网激活（需改系统时间或真实订单）",
    "系统通知、托盘 / 菜单栏、打印对话框、mailto 唤起邮件客户端",
    "断网抓包验证零上传、日志内容审计（需外部工具）",
    "扫描件 OCR 识别准确率、中文语言包实际效果（自检只验证引擎能跑通）",
    "5000 页 / 500MB 大文件性能、内存峰值、长任务进度与取消",
    "产物在 Adobe Reader / PDFgear / Word / Excel 中的兼容性",
    "macOS 实机（Apple 签名、字体、权限、通用二进制）",
];

/// 一条用例。字段与 Grabber 的 `SelftestCase` 兼容（case / ok / ms / detail），
/// 另加 group / feature / skipped / optional，用于分组展示与「跳过 ≠ 通过」的区分。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfCheckCase {
    pub case: String,
    pub group: String,
    /// 对应用户可见的功能编号，报告据此汇总「已自检覆盖的功能」。
    pub feature: String,
    pub ok: bool,
    pub skipped: bool,
    pub optional: bool,
    pub ms: u64,
    pub detail: String,
}

enum Outcome {
    Pass(String),
    Fail(String),
    Skip(String),
}

fn pass(detail: impl Into<String>) -> Outcome {
    Outcome::Pass(detail.into())
}

fn fail(detail: impl Into<String>) -> Outcome {
    Outcome::Fail(detail.into())
}

fn skip(detail: impl Into<String>) -> Outcome {
    Outcome::Skip(detail.into())
}

/// 断言失败时给出明确 detail，而不是只写 false。
fn verdict(ok: bool, detail: impl Into<String>) -> Outcome {
    let d = detail.into();
    if ok {
        pass(d)
    } else {
        fail(format!("结果不符合预期：{}", d))
    }
}

fn short(s: &str) -> String {
    s.chars().take(240).collect()
}

fn err_detail(e: &AppError) -> String {
    short(&format!("{} {}", e.code, e.message))
}

#[allow(clippy::too_many_arguments)]
fn record(
    cases: &mut Vec<SelfCheckCase>,
    name: &str,
    group: &str,
    feature: &str,
    optional: bool,
    t0: Instant,
    outcome: Outcome,
) {
    let (ok, skipped, detail) = match outcome {
        Outcome::Pass(d) => (true, false, d),
        Outcome::Fail(d) => (false, false, d),
        Outcome::Skip(d) => (true, true, d),
    };
    cases.push(SelfCheckCase {
        case: name.to_string(),
        group: group.to_string(),
        feature: feature.to_string(),
        ok,
        skipped,
        optional,
        ms: t0.elapsed().as_millis() as u64,
        detail: short(&detail),
    });
}

/// 用 lopdf 造一份 N 页、内置 Helvetica 的最小 PDF（含文字层）。
/// 不依赖 fixtures、不联网、不读用户文件；自检的所有输入都由它生成。
fn sample_pdf_bytes(pages: u32, text: &str) -> AppResult<Vec<u8>> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let escaped = text
        .replace('\\', r"\\")
        .replace('(', r"\(")
        .replace(')', r"\)");
    let content = format!("BT /F1 18 Tf 72 700 Td ({}) Tj ET\n", escaped);
    let content_id = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        content.into_bytes(),
    )));

    let mut kids: Vec<Object> = Vec::new();
    for _ in 0..pages {
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
        kids.push(Object::Reference(pid));
    }
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
    pdf::save_to_bytes(&mut doc)
}

/// 逐条跑用例，返回结构化报告；同时写一份 `selftest-<时间戳>.json` 到下载目录。
pub fn run_self_check_blocking(app: &AppHandle) -> serde_json::Value {
    let started = Instant::now();
    let ts = chrono::Utc::now().timestamp_millis();
    let ui_version = app.package_info().version.to_string();
    let mut cases: Vec<SelfCheckCase> = Vec::new();

    // 所有中间文件只在系统临时目录，结束清理；绝不写进用户选择的输出目录。
    let tmp = std::env::temp_dir().join(format!(
        "payoncepdf-selftest-{}-{}",
        std::process::id(),
        ts
    ));
    let _ = std::fs::create_dir_all(&tmp);

    // 报告目录与 Grabber 一致：下载目录；不可写时退到 AppData，再不行退到 temp。
    let mut report_dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| std::env::temp_dir());
    {
        let t0 = Instant::now();
        let probe = report_dir.join("payoncepdf-selftest-probe.tmp");
        let writable = std::fs::create_dir_all(&report_dir)
            .and_then(|_| std::fs::write(&probe, b"ok"))
            .is_ok();
        let _ = std::fs::remove_file(&probe);
        record(
            &mut cases,
            "env.report_dir_writable",
            "环境依赖",
            "报告输出",
            false,
            t0,
            verdict(writable, report_dir.to_string_lossy().to_string()),
        );
        if !writable {
            report_dir = std::env::temp_dir();
        }
    }

    // ---- A. 环境依赖 -------------------------------------------------------
    let sidecars = sidecar::self_check(app);
    let info_of = |name: &str| sidecars.iter().find(|s| s.name.as_str() == name);
    let ready = |name: &str| info_of(name).map(|s| s.ready).unwrap_or(false);
    let path_of = |name: &str| {
        info_of(name)
            .map(|s| s.path.clone())
            .unwrap_or_default()
    };

    for (name, feature, optional) in [
        ("qpdf", "F1/F2/F13", false),
        ("pdfium", "F4/F10/F17/F18", false),
        ("tesseract", "F5 OCR", true),
        ("tessdata", "F5 OCR", true),
    ] {
        let t0 = Instant::now();
        let r = ready(name);
        let where_ = path_of(name);
        let outcome = if r {
            pass(format!("就绪 · {}", where_))
        } else if optional {
            skip(format!("可选组件缺失（{}）；OCR 链路需在真机验证", name))
        } else {
            fail(format!("核心组件缺失：{}（{}）", name, where_))
        };
        record(
            &mut cases,
            &format!("env.{}_ready", name),
            "环境依赖",
            feature,
            optional,
            t0,
            outcome,
        );
    }

    {
        let t0 = Instant::now();
        let outcome = match license::get_license_state(app) {
            Ok(st) => pass(format!(
                "status={} days_left={} can_export={}",
                st.status, st.trial_days_left, st.can_export
            )),
            Err(e) => fail(err_detail(&e)),
        };
        record(
            &mut cases,
            "env.license_state",
            "环境依赖",
            "F8 试用与授权",
            false,
            t0,
            outcome,
        );
    }
    {
        // P0-1 / P1-1 回归：发行包必须有可用的 Ed25519 信任根，
        // 不能退化到「信任 activation.json 自证字段」的兜底路径。
        let t0 = Instant::now();
        let (ok, source) = license::public_key_diagnostic(app);
        record(
            &mut cases,
            "env.license_public_key",
            "环境依赖",
            "F8 激活信任根",
            false,
            t0,
            verdict(ok, format!("公钥可用 · 来源={}", source)),
        );
    }
    {
        // Grabber 的教训：界面版本与引擎版本必须同源，否则报告版本号会骗人。
        let t0 = Instant::now();
        let cargo_version = env!("CARGO_PKG_VERSION");
        record(
            &mut cases,
            "env.version_consistency",
            "环境依赖",
            "版本同源",
            false,
            t0,
            verdict(
                ui_version == cargo_version,
                format!("界面={} 引擎={}", ui_version, cargo_version),
            ),
        );
    }

    // ---- B. 纯逻辑（不需要引擎 / 不需要文件） -------------------------------
    {
        let t0 = Instant::now();
        let ok = engine::page_selection(Some("1-2,4"), 5)
            .map(|v| v == vec![1, 2, 4])
            .unwrap_or(false)
            && engine::page_selection(Some("9"), 5).is_err();
        record(
            &mut cases,
            "logic.page_selection",
            "纯逻辑",
            "F2 页码范围",
            false,
            t0,
            verdict(ok, "\"1-2,4\" → [1,2,4]；越界 \"9\" 被拒"),
        );
    }
    {
        let t0 = Instant::now();
        let ok = objects::parse_bookmark_spec("1 封面\n>2 目录\n>>3 细则", 3)
            .map(|v| v.iter().map(|e| e.level).collect::<Vec<u32>>() == vec![1, 2, 3])
            .unwrap_or(false)
            && objects::parse_bookmark_spec(">>3 跳级", 3).is_err();
        record(
            &mut cases,
            "logic.bookmark_levels",
            "纯逻辑",
            "F15 多级书签",
            false,
            t0,
            verdict(ok, "三级层级正确；层级跳跃被拒"),
        );
    }
    {
        let t0 = Instant::now();
        let s = compare::CompareSensitivity::default();
        let diff = compare::diff_text("Invoice No. 30", "Invoice No. 45", 1, &s)
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        let same = compare::diff_text("Same text", "Same text", 1, &s)
            .map(|v| v.is_empty())
            .unwrap_or(false);
        record(
            &mut cases,
            "logic.diff_text",
            "纯逻辑",
            "F20 文档比对",
            false,
            t0,
            verdict(diff && same, "有改动报差异；同文本报无差异"),
        );
    }
    {
        let t0 = Instant::now();
        let mut ok = true;
        for deg in [0u32, 90, 180, 270] {
            let rot = geometry::PageRotation::from_degrees(deg);
            let (dw, dh) = geometry::display_size(rot, 595.0, 842.0);
            let r = (50.0f32, 60.0, 200.0, 120.0);
            let d = geometry::content_to_display(rot, r, 595.0, 842.0);
            let back = geometry::display_to_content(rot, d, dw, dh);
            ok &= (back.0 - r.0).abs() < 0.01
                && (back.1 - r.1).abs() < 0.01
                && (back.2 - r.2).abs() < 0.01
                && (back.3 - r.3).abs() < 0.01;
        }
        record(
            &mut cases,
            "logic.geometry_rotation",
            "纯逻辑",
            "F18/F19 旋转坐标",
            false,
            t0,
            verdict(ok, "0/90/180/270 度内容↔显示坐标往返一致"),
        );
    }
    {
        let t0 = Instant::now();
        let ascii = extras::winansi_literal("Net 30 (USD) - 45");
        let cjk = extras::winansi_literal("中文水印");
        let ok = ascii.is_ok() && cjk.is_err();
        record(
            &mut cases,
            "logic.winansi_literal",
            "纯逻辑",
            "F13 水印边界",
            false,
            t0,
            verdict(ok, "拉丁文本通过；中文明确报错，不写乱码"),
        );
    }
    {
        let t0 = Instant::now();
        let ok = extras::parse_color("#5EEAD4")
            .map(|v| (v[0] - 94.0 / 255.0).abs() < 0.01 && (v[1] - 234.0 / 255.0).abs() < 0.01)
            .unwrap_or(false)
            && extras::parse_color("#123").is_err()
            && qpdf::parse_range_tokens("1-3, 5")
                .map(|v| v == vec!["1-3".to_string(), "5".to_string()])
                .unwrap_or(false);
        record(
            &mut cases,
            "logic.params_parsing",
            "纯逻辑",
            "F11/F2 参数解析",
            false,
            t0,
            verdict(ok, "颜色、页码 token 合法值通过 / 非法值被拒"),
        );
    }
    {
        let t0 = Instant::now();
        let existing = tmp.join("guard.pdf");
        let _ = std::fs::write(&existing, b"%PDF-1.4\n");
        let renamed = state::resolve_output_path(&existing);
        let guard = state::assert_not_overwriting_inputs(&existing, &[existing.as_path()]);
        let ok = renamed != existing
            && renamed.file_name().map(|n| n.to_string_lossy() == "guard_1.pdf").unwrap_or(false)
            && guard.is_err();
        record(
            &mut cases,
            "logic.output_guard",
            "纯逻辑",
            "原文件保护",
            false,
            t0,
            verdict(ok, "同名输出自动换名；原地覆盖被拒"),
        );
    }
    {
        let t0 = Instant::now();
        let ok = AppError::parse("x").code == "E1001"
            && AppError::sidecar_missing("x").code == "E2001"
            && AppError::output_busy("x").code == "E1007";
        record(
            &mut cases,
            "logic.error_codes",
            "纯逻辑",
            "错误码契约",
            false,
            t0,
            verdict(ok, "E1001 / E2001 / E1007 语义稳定"),
        );
    }

    // ---- C. 真实引擎端到端 -------------------------------------------------
    let sample = tmp.join("sample30.pdf");
    let sample_alt = tmp.join("sample45.pdf");
    {
        let t0 = Instant::now();
        let gen = (|| -> AppResult<()> {
            std::fs::write(&sample, sample_pdf_bytes(3, "Invoice No. 30 / Payment due in 30 days")?)
                .map_err(AppError::from)?;
            std::fs::write(&sample_alt, sample_pdf_bytes(3, "Invoice No. 45 / Payment due in 30 days")?)
                .map_err(AppError::from)?;
            Ok(())
        })();
        let pages = pdf::page_count(&sample, None).unwrap_or(0);
        record(
            &mut cases,
            "engine.sample_pdf",
            "引擎端到端",
            "自检样本",
            false,
            t0,
            match gen {
                Ok(()) => verdict(pages == 3, format!("内置样本生成成功，{} 页", pages)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }

    let cancel = AtomicBool::new(false);
    let ctx = engine::JobCtx {
        app: Some(app),
        job_id: "selftest",
        cancel: &cancel,
        emit: false,
    };

    {
        let t0 = Instant::now();
        let outcome = match qpdf::page_count(app, &sample, None) {
            Ok(n) => verdict(n == 3, format!("qpdf 读回 {} 页", n)),
            Err(e) => fail(err_detail(&e)),
        };
        record(&mut cases, "engine.qpdf_page_count", "引擎端到端", "F1/F2 页数校验", false, t0, outcome);
    }
    {
        let t0 = Instant::now();
        let out = tmp.join("merged.pdf");
        let r = qpdf::merge(app, &[sample.clone(), sample.clone()], None, &out, &tmp)
            .and_then(|_| qpdf::page_count(app, &out, None));
        record(
            &mut cases,
            "engine.qpdf_merge",
            "引擎端到端",
            "F1 合并",
            false,
            t0,
            match r {
                Ok(n) => verdict(n == 6, format!("两份 3 页样本合并为 {} 页", n)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let out = tmp.join("sliced.pdf");
        let r = qpdf::slice(app, &sample, None, &["1-2".to_string()], &out, &tmp)
            .and_then(|_| qpdf::page_count(app, &out, None));
        record(
            &mut cases,
            "engine.qpdf_slice",
            "引擎端到端",
            "F2 拆分",
            false,
            t0,
            match r {
                Ok(n) => verdict(n == 2, format!("抽取 1-2 页得到 {} 页", n)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let enc = tmp.join("enc.pdf");
        let dec = tmp.join("dec.pdf");
        let r = (|| -> AppResult<(u32, u32)> {
            qpdf::encrypt(app, &sample, "selfcheck-pw", "", true, true, true, &enc)?;
            let n1 = qpdf::page_count(app, &enc, Some("selfcheck-pw"))?;
            qpdf::decrypt(app, &enc, "selfcheck-pw", &dec)?;
            Ok((n1, pdf::page_count(&dec, None)?))
        })();
        record(
            &mut cases,
            "engine.qpdf_encrypt_decrypt",
            "引擎端到端",
            "F13 AES-256 加密",
            false,
            t0,
            match r {
                Ok((a, b)) => verdict(a == 3 && b == 3, format!("加密后按密码读 {} 页，解密后 {} 页", a, b)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let r = (|| -> AppResult<String> {
            let doc = render::open(app, &sample, None)?;
            let layout = doc.page_text_layout(1)?;
            Ok(format!(
                "has_text_layer={} chars={} text={:?}",
                layout.has_text_layer,
                layout.chars.len(),
                short(&layout.text)
            ))
        })();
        record(
            &mut cases,
            "engine.pdfium_text_layer",
            "引擎端到端",
            "F10/F17/F18 文字层",
            false,
            t0,
            match r {
                Ok(d) if d.contains("has_text_layer=true") && d.contains("Invoice No. 30") => pass(d),
                Ok(d) => fail(format!("文字层内容不符：{}", d)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let r = (|| -> AppResult<String> {
            let doc = render::open(app, &sample, None)?;
            let (png, w, h) = doc.render_page_png(1, 72)?;
            Ok(format!("PNG {} 字节 · {}x{}", png.len(), w, h))
        })();
        record(
            &mut cases,
            "engine.pdfium_render",
            "引擎端到端",
            "F4/F17 页面渲染",
            false,
            t0,
            match r {
                Ok(d) if !d.contains("PNG 0 字节") => pass(d),
                Ok(d) => fail(format!("渲染结果为空：{}", d)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let out = tmp.join("edited45.pdf");
        let r = (|| -> AppResult<(usize, String)> {
            let pf = render::load_pdfium(app)?;
            let e = extras::replace_text(&ctx, pf, &sample, None, "30", "45", true, false, None, &out)?;
            let doc = render::open(app, &out, None)?;
            Ok((e.replacements, doc.page_text_layout(1)?.text))
        })();
        record(
            &mut cases,
            "engine.replace_text",
            "引擎端到端",
            "F10 就地改字",
            false,
            t0,
            match r {
                Ok((n, text)) if n >= 1 && text.contains("45") => {
                    pass(format!("替换 {} 处，结果文字层含 45", n))
                }
                Ok((n, text)) => fail(format!("替换 {} 处但结果文字层未读到 45：{:?}", n, short(&text))),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let out = tmp.join("deleted.pdf");
        let r = extras::page_ops(Some(app), &sample, None, "delete", "3", 0, 0, 0, [0.0; 4], &out, &tmp)
            .and_then(|_| pdf::page_count(&out, None));
        record(
            &mut cases,
            "engine.page_ops_delete",
            "引擎端到端",
            "F12 页面管理",
            false,
            t0,
            match r {
                Ok(n) => verdict(n == 2, format!("删第 3 页后剩 {} 页", n)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let out = tmp.join("bookmarked.pdf");
        let r = (|| -> AppResult<Vec<u32>> {
            objects::manage_bookmarks(&sample, None, "set", "1 封面\n>2 目录\n>>3 细则", "", "", &out)?;
            Ok(objects::list_bookmarks(&out, None)?.iter().map(|e| e.level).collect())
        })();
        record(
            &mut cases,
            "engine.bookmarks_roundtrip",
            "引擎端到端",
            "F15 多级书签",
            false,
            t0,
            match r {
                Ok(v) => verdict(matches!(v.as_slice(), [1, 2, 3]), format!("写回层级 {:?}", v)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let r = compare::compare(
            app,
            &sample,
            None,
            &sample_alt,
            None,
            None,
            &compare::CompareSensitivity::default(),
        );
        record(
            &mut cases,
            "engine.compare",
            "引擎端到端",
            "F20 文档比对",
            false,
            t0,
            match r {
                Ok(rep) => verdict(
                    rep.differences.len() > 0 && rep.changed > 0,
                    format!("{} 条差异 / {} 处改动", rep.differences.len(), rep.changed),
                ),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let png: PathBuf = tmp.join("selftest.png");
        let out = tmp.join("from_image.pdf");
        let r = (|| -> AppResult<u32> {
            let mut img = image::RgbImage::new(96, 64);
            for p in img.pixels_mut() {
                *p = image::Rgb([230u8, 240u8, 250u8]);
            }
            img.save_with_format(&png, image::ImageFormat::Png)
                .map_err(|e| AppError::internal(format!("生成测试图片失败：{}", e)))?;
            pdf::images_to_pdf(&ctx, &[png.clone()], "a4", 24.0, &out)?;
            pdf::page_count(&out, None)
        })();
        record(
            &mut cases,
            "engine.images_to_pdf",
            "引擎端到端",
            "F4 图片转 PDF",
            false,
            t0,
            match r {
                Ok(n) => verdict(n == 1, format!("1 张 PNG → {} 页 PDF", n)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let txt = tmp.join("selftest.txt");
        let out = tmp.join("from_text.pdf");
        let r = (|| -> AppResult<u32> {
            std::fs::write(&txt, "PayOnce PDF self check\nLine 2\n").map_err(AppError::from)?;
            let pf = render::load_pdfium(app)?;
            let _ = more::text_to_pdf(pf, &txt, "txt", &out)?;
            pdf::page_count(&out, None)
        })();
        record(
            &mut cases,
            "engine.text_to_pdf",
            "引擎端到端",
            "F16 文本转 PDF",
            false,
            t0,
            match r {
                Ok(n) if n >= 1 => pass(format!("TXT → {} 页 PDF", n)),
                Ok(n) => fail(format!("生成了 {} 页（预期至少 1 页）", n)),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let img_pdf = tmp.join("from_image.pdf");
        let r = pdf::compress(&ctx, &img_pdf, None, 5_000_000, "balanced", &tmp);
        record(
            &mut cases,
            "engine.compress",
            "引擎端到端",
            "F3 压缩",
            false,
            t0,
            match r {
                Ok(c) => verdict(
                    c.path.exists() && c.actual_bytes > 0,
                    format!("{} 字节 · {} 张图 · {}", c.actual_bytes, c.image_count, c.quality_label),
                ),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        let t0 = Instant::now();
        let src = tmp.join("commit_src.tmp");
        let dst = tmp.join("commit_dst.tmp");
        let r = (|| -> AppResult<bool> {
            std::fs::write(&src, b"selftest").map_err(AppError::from)?;
            state::atomic_commit(&src, &dst)?;
            Ok(std::fs::read(&dst).map(|v| v == b"selftest").unwrap_or(false))
        })();
        record(
            &mut cases,
            "engine.atomic_commit",
            "引擎端到端",
            "原子提交",
            false,
            t0,
            match r {
                Ok(true) => pass("临时文件原子提交后内容一致"),
                Ok(false) => fail("提交后内容不一致"),
                Err(e) => fail(err_detail(&e)),
            },
        );
    }
    {
        // OCR 是唯一可能需要外部语言包的链路：组件不齐时跳过（不算失败），
        // 并在报告的「仍需真机」清单里保留扫描件准确率验证。
        let t0 = Instant::now();
        let outcome = if !(ready("tesseract") && ready("tessdata")) {
            skip("tesseract / tessdata 未就绪，OCR 引擎链路本次跳过")
        } else if let Err(e) = sidecar::check_language(app, "eng") {
            // tessdata_ready 只保证目录里有任意 .traineddata，具体语言包可能仍缺。
            // 这是环境缺件而非引擎缺陷，标跳过并留在真机清单，不制造假 FAIL。
            skip(format!(
                "OCR 语言包 eng 不可用（{}）；扫描件 OCR 需真机验证",
                err_detail(&e)
            ))
        } else {
            let ocr_dir = tmp.join("ocr");
            let _ = std::fs::create_dir_all(&ocr_dir);
            match ocr::run(&ctx, &sample, None, "txt", &[String::from("eng")], None, &ocr_dir) {
                Ok(p) if p.exists() => pass(format!("OCR 输出已生成：{}", p.display())),
                Ok(_) => fail("OCR 返回了不存在的输出路径"),
                Err(e) => fail(err_detail(&e)),
            }
        };
        record(
            &mut cases,
            "ocr.optional_engine",
            "可选（缺组件则跳过）",
            "F5 OCR",
            true,
            t0,
            outcome,
        );
    }

    // ---- 汇总与报告 --------------------------------------------------------
    let total = cases.len();
    let passed = cases.iter().filter(|c| c.ok && !c.skipped).count();
    let failed = cases.iter().filter(|c| !c.ok).count();
    let skipped = cases.iter().filter(|c| c.skipped).count();
    let mut features: Vec<String> = Vec::new();
    for c in &cases {
        if !c.feature.is_empty() && !features.contains(&c.feature) {
            features.push(c.feature.clone());
        }
    }

    let mut report = serde_json::json!({
        "schema": SCHEMA,
        "started_at": ts,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "env": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "app": ui_version,
            "qpdf": path_of("qpdf"),
            "pdfium": path_of("pdfium"),
            "tesseract": path_of("tesseract"),
            "report_dir": report_dir.to_string_lossy().to_string(),
        },
        "summary": { "total": total, "passed": passed, "failed": failed, "skipped": skipped },
        "coverage": {
            "self_checked": features,
            "needs_real_device": NEEDS_REAL_DEVICE,
        },
        "cases": cases,
    });

    let path = report_dir.join(format!("selftest-{}.json", ts));
    match std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap_or_default()) {
        Ok(()) => report["path"] = serde_json::json!(path.to_string_lossy().to_string()),
        Err(e) => report["write_error"] = serde_json::json!(e.to_string()),
    }
    // PDFium 在 Windows 上可能仍持有文件句柄，清理失败不影响报告本身。
    let _ = std::fs::remove_dir_all(&tmp);
    report
}

/// 前端「运行自检」调用；与 Grabber 一样放到阻塞线程池，避免卡住 UI 线程。
#[tauri::command]
pub async fn run_self_check(payload: EmptyPayload, app: AppHandle) -> Result<serde_json::Value, String> {
    let _ = payload;
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || run_self_check_blocking(&app2))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_pdf_has_requested_pages() {
        let dir = std::env::temp_dir().join(format!("payoncepdf_selftest_unit_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("sample.pdf");
        let bytes = sample_pdf_bytes(3, "Invoice No. 30").expect("应能生成样本 PDF");
        std::fs::write(&p, bytes).expect("应能写出样本");
        assert_eq!(pdf::page_count(&p, None).unwrap(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn case_recording_distinguishes_pass_fail_skip() {
        let mut cases = Vec::new();
        let t = Instant::now();
        record(&mut cases, "a", "g", "f", false, t, pass("ok"));
        record(&mut cases, "b", "g", "f", false, t, fail("bad"));
        record(&mut cases, "c", "g", "f", true, t, skip("absent"));
        assert_eq!(cases.len(), 3);
        assert_eq!(
            cases.iter().map(|c| (c.ok, c.skipped)).collect::<Vec<_>>(),
            vec![(true, false), (false, false), (true, true)]
        );
        assert_eq!(cases.iter().filter(|c| c.ok && !c.skipped).count(), 1);
        assert_eq!(cases.iter().filter(|c| !c.ok).count(), 1);
        assert_eq!(cases.iter().filter(|c| c.skipped).count(), 1);
    }

    #[test]
    fn real_device_gap_list_is_explicit() {
        assert!(NEEDS_REAL_DEVICE.len() >= 10);
        assert!(NEEDS_REAL_DEVICE.iter().any(|s| s.contains("安装")));
        assert!(NEEDS_REAL_DEVICE.iter().any(|s| s.contains("拖放")));
        assert!(NEEDS_REAL_DEVICE.iter().any(|s| s.contains("macOS")));
    }
}
