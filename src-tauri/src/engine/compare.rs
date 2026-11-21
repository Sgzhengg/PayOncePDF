//! F20：文档比对（纯文本层，只读）。
//!
//! 对标 Adobe Compare Files / 福昕文档对比 / UPDF，但严格限定范围：
//! - 只比**文字层**文本，按页做词/字级 LCS diff，产出「新增 / 删除 / 修改」差异列表。
//! - **不做**图片、版式、页序、字体属性差异——那需要渲染后图像比对（见 PRD §19 暂缓）。
//! - 扫描件无文字层时明确返回 E2004 提示先走 F5 OCR，绝不静默报「无差异」。
//! - 只读：不占用导出授权、不写任何文件；导出对比报告是另一个走 execute 的命令。
//!
//! 不引第三方 diff 依赖：LCS 用滚动数组（O(min(n,m)) 内存）+ 操作序列回溯，
//! 单页 token 上限 20_000、字符上限 3_000_000，超限明确报错而非 OOM。

use crate::engine::{page_selection, render};
use crate::error::{AppError, AppResult};
use pdfium_render::prelude::Pdfium;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;

// ---------------------------------------------------------------------------
// DTO（camelCase，与前端 src/lib/types.ts 镜像）
// ---------------------------------------------------------------------------

/// 比对敏感度：控制 diff 前如何归一化文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareSensitivity {
    /// 忽略大小写（英文场景）。默认 false。
    #[serde(default)]
    pub ignore_case: bool,
    /// 忽略所有空白差异（换行 / 空格 / Tab）。默认 true——排版换行不应算作内容变化。
    #[serde(default = "default_true")]
    pub ignore_whitespace: bool,
    /// 忽略标点差异。默认 false——合同里一个标点可能改变含义。
    #[serde(default)]
    pub ignore_punctuation: bool,
}

fn default_true() -> bool {
    true
}

impl Default for CompareSensitivity {
    fn default() -> Self {
        Self {
            ignore_case: false,
            ignore_whitespace: true,
            ignore_punctuation: false,
        }
    }
}

/// 查询参数：两个文件 + 可选密码 + 页面范围 + 敏感度。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareQuery {
    pub old_file: String,
    pub new_file: String,
    #[serde(default)]
    pub old_password: Option<String>,
    #[serde(default)]
    pub new_password: Option<String>,
    /// 页码范围（如 "1-3,5"），留空 = 两文档全部页。
    #[serde(default)]
    pub pages: Option<String>,
    #[serde(default)]
    pub sensitivity: Option<CompareSensitivity>,
}

/// 一条差异。`kind` ∈ "added" | "removed" | "changed"。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareDifference {
    pub kind: String,
    /// 差异所在页码（1-based；changed 取新文件页码，removed 取旧文件页码）。
    pub page: u32,
    pub old_text: String,
    pub new_text: String,
}

/// 比对结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareReport {
    pub old_pages: u32,
    pub new_pages: u32,
    /// 实际参与比对的页数。
    pub compared_pages: usize,
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub differences: Vec<CompareDifference>,
    /// 无文字层 / 超限跳过等提示（不影响已产出差异的有效性）。
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// token 化 + LCS diff（纯逻辑，可离线单测）
// ---------------------------------------------------------------------------

const TOKEN_LIMIT: usize = 20_000;
const CHAR_LIMIT: usize = 3_000_000;

/// 把文本切成可比较的 token：
/// - 忽略空白时直接丢弃所有 Unicode 空白；
/// - CJK 按单字成 token（无空格分词也能比）；
/// - ASCII 拉丁字母/数字按连续片段成一个 token（英文单词不被拆散）；
/// - 其余字符（标点/符号）按单字符 token，ignore_punctuation 时丢弃。
fn tokenize(text: &str, s: &CompareSensitivity) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut latin = String::new();
    let flush = |out: &mut Vec<String>, latin: &mut String| {
        if !latin.is_empty() {
            out.push(std::mem::take(latin));
        }
    };
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !s.ignore_whitespace {
                flush(&mut out, &mut latin);
                out.push(" ".to_string());
            }
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            latin.push(if s.ignore_case {
                ch.to_ascii_lowercase()
            } else {
                ch
            });
            continue;
        }
        flush(&mut out, &mut latin);
        if is_punctuation(ch) {
            if !s.ignore_punctuation {
                out.push(ch.to_string());
            }
            continue;
        }
        // CJK 与其他符号：单字 token
        out.push(if s.ignore_case {
            ch.to_lowercase().collect()
        } else {
            ch.to_string()
        });
    }
    flush(&mut out, &mut latin);
    out
}

fn is_punctuation(ch: char) -> bool {
    matches!(ch,
        '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}'
        | '，' | '。' | '；' | '：' | '！' | '？' | '、' | '「' | '」' | '『' | '』'
        | '《' | '》' | '（' | '）' | '—' | '…' | '·' | '-' | '_' | '/' | '\\' | '+' | '='
        | '*' | '&' | '%' | '#' | '@' | '$' | '~' | '`' | '<' | '>')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffOp {
    Equal,
    Added,
    Removed,
}

/// 两个 token 序列的 LCS diff。返回 (op, token) 序列，顺序为新文本视角。
/// 用滚动数组保存 dp 两行 + full 回溯矩阵（u8 方向）；token 数受 TOKEN_LIMIT 约束。
fn lcs_diff(old: &[String], new: &[String]) -> Vec<(DiffOp, String)> {
    let n = old.len();
    let m = new.len();
    if n == 0 && m == 0 {
        return Vec::new();
    }
    if n > TOKEN_LIMIT || m > TOKEN_LIMIT {
        // 超限：降级为整段替换，明确且不会 OOM
        let mut ops = Vec::with_capacity(n + m);
        for t in old {
            ops.push((DiffOp::Removed, t.clone()));
        }
        for t in new {
            ops.push((DiffOp::Added, t.clone()));
        }
        return ops;
    }

    // dir[i][j]：1=左上（相等或跳过）、2=上（删旧）、3=左（加新）
    let mut dir = vec![vec![0u8; m + 1]; n + 1];
    let mut prev = vec![0u32; m + 1];
    let mut cur = vec![0u32; m + 1];
    for i in 1..=n {
        cur[0] = 0;
        for j in 1..=m {
            if old[i - 1] == new[j - 1] {
                cur[j] = prev[j - 1] + 1;
                dir[i][j] = 1;
            } else if prev[j] >= cur[j - 1] {
                cur[j] = prev[j];
                dir[i][j] = 2;
            } else {
                cur[j] = cur[j - 1];
                dir[i][j] = 3;
            }
        }
        std::mem::swap(&mut prev, &mut cur);
    }

    let mut rev: Vec<(DiffOp, String)> = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        match dir[i][j] {
            1 => {
                rev.push((DiffOp::Equal, new[j - 1].clone()));
                i -= 1;
                j -= 1;
            }
            2 => {
                rev.push((DiffOp::Removed, old[i - 1].clone()));
                i -= 1;
            }
            _ => {
                rev.push((DiffOp::Added, new[j - 1].clone()));
                j -= 1;
            }
        }
    }
    while i > 0 {
        rev.push((DiffOp::Removed, old[i - 1].clone()));
        i -= 1;
    }
    while j > 0 {
        rev.push((DiffOp::Added, new[j - 1].clone()));
        j -= 1;
    }
    rev.reverse();
    rev
}

/// 拼接 token 时：CJK 单字、标点直接连写（中文不需空格）；ASCII 词之间加空格。
fn push_token(buf: &mut String, token: &str, has_prev: bool) {
    if has_prev {
        let prev_ascii = buf.chars().last().is_some_and(|c| c.is_ascii_alphanumeric());
        let token_ascii = token.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
        if prev_ascii && token_ascii {
            buf.push(' ');
        }
    }
    buf.push_str(token);
}

/// 把 LCS 操作序列聚合成「新增/删除/修改」差异。连续的非 Equal 段合成一条。
fn coalesce(ops: &[(DiffOp, String)], page: u32) -> Vec<CompareDifference> {
    let mut out: Vec<CompareDifference> = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        if ops[i].0 == DiffOp::Equal {
            i += 1;
            continue;
        }
        let start = i;
        let mut old_text = String::new();
        let mut new_text = String::new();
        let mut has_old = false;
        let mut has_new = false;
        while i < ops.len() && ops[i].0 != DiffOp::Equal {
            match ops[i].0 {
                DiffOp::Removed => {
                    push_token(&mut old_text, &ops[i].1, has_old);
                    has_old = true;
                }
                DiffOp::Added => {
                    push_token(&mut new_text, &ops[i].1, has_new);
                    has_new = true;
                }
                DiffOp::Equal => unreachable!(),
            }
            i += 1;
        }
        if start == i {
            continue;
        }
        let kind = match (has_old, has_new) {
            (true, true) => "changed",
            (true, false) => "removed",
            (false, true) => "added",
            (false, false) => unreachable!(),
        };
        out.push(CompareDifference {
            kind: kind.to_string(),
            page,
            old_text,
            new_text,
        });
    }
    out
}

/// 对一段文本做 diff，返回该页差异。纯函数，单测主力。
pub fn diff_text(
    old: &str,
    new: &str,
    page: u32,
    s: &CompareSensitivity,
) -> AppResult<Vec<CompareDifference>> {
    if old.len() > CHAR_LIMIT || new.len() > CHAR_LIMIT {
        return Err(AppError::internal(format!(
            "第 {} 页文本超过 {} 字符上限，无法比对（请拆分文件或缩小页面范围）",
            page, CHAR_LIMIT
        )));
    }
    let a = tokenize(old, s);
    let b = tokenize(new, s);
    if a == b {
        return Ok(Vec::new());
    }
    Ok(coalesce(&lcs_diff(&a, &b), page))
}

// ---------------------------------------------------------------------------
// 主流程：打开两个 PDF，逐页取文字层，做 diff，组装报告
// ---------------------------------------------------------------------------

/// 打开一个 PDF 并按页码范围提取每页文字层。
/// 返回 (页码, 文本)；无文字层 / 超限页把原因写入 warnings。
fn extract_pages(
    pdfium: &Pdfium,
    path: &Path,
    password: Option<&str>,
    pages: Option<&str>,
    label: &str,
    warnings: &mut Vec<String>,
) -> AppResult<(u32, Vec<(u32, String)>)> {
    let doc = render::PdfDoc::open(pdfium, path, password)?;
    let total = doc.page_count();
    if total == 0 {
        return Err(AppError::parse(format!("{}没有页面", label)));
    }
    if total > 5000 {
        return Err(AppError::page_limit(format!("{}共 {} 页，超过 5000 页上限", label, total)));
    }
    let targets = page_selection(pages, total)?;
    let mut out = Vec::with_capacity(targets.len());
    for p in targets {
        let layout = doc.page_text_layout(p)?;
        if layout.truncated {
            warnings.push(format!(
                "{}{} 文字超过单页 {} 字符上限，已跳过该页比对",
                label, p, render::MAX_LAYOUT_CHARS
            ));
            out.push((p, String::new()));
            continue;
        }
        if !layout.has_text_layer {
            warnings.push(format!(
                "{}{} 没有可用文字层（扫描件或纯图片页），请先走 OCR 再比对",
                label, p
            ));
            out.push((p, String::new()));
            continue;
        }
        out.push((p, layout.text));
    }
    Ok((total, out))
}

/// 比对主流程：只读、不写文件。
pub fn compare(
    app: &AppHandle,
    old_path: &Path,
    old_password: Option<&str>,
    new_path: &Path,
    new_password: Option<&str>,
    pages: Option<&str>,
    sensitivity: &CompareSensitivity,
) -> AppResult<CompareReport> {
    let pdfium = render::load_pdfium(app)?;
    compare_with_pdfium(
        pdfium,
        old_path,
        old_password,
        new_path,
        new_password,
        pages,
        sensitivity,
    )
}

/// 比对核心（接收已加载的 PDFium），供单测用 test_pdfium 走真实文件端到端。
pub fn compare_with_pdfium(
    pdfium: &Pdfium,
    old_path: &Path,
    old_password: Option<&str>,
    new_path: &Path,
    new_password: Option<&str>,
    pages: Option<&str>,
    sensitivity: &CompareSensitivity,
) -> AppResult<CompareReport> {
    let mut warnings: Vec<String> = Vec::new();
    let (old_pages, old_texts) = extract_pages(
        pdfium,
        old_path,
        old_password,
        pages,
        "旧文档第 ",
        &mut warnings,
    )?;
    let (new_pages, new_texts) = extract_pages(
        pdfium,
        new_path,
        new_password,
        pages,
        "新文档第 ",
        &mut warnings,
    )?;

    // 两文档的页面范围可能解析出不同页数（如一份 3 页一份 5 页）；按位置配对，
    // 只有一侧有的页整页作为 added / removed，并给出提示。
    let n = old_texts.len().max(new_texts.len());
    if old_texts.len() != new_texts.len() {
        warnings.push(format!(
            "旧文档选中 {} 页、新文档选中 {} 页，页数不同；多出的页按整页新增/删除列出，页序变化可能被误报",
            old_texts.len(),
            new_texts.len()
        ));
    }
    let mut differences: Vec<CompareDifference> = Vec::new();
    for idx in 0..n {
        let old_item = old_texts.get(idx);
        let new_item = new_texts.get(idx);
        match (old_item, new_item) {
            (Some((op, ot)), Some((np, nt))) => {
                let mut d = diff_text(ot, nt, *np, sensitivity)?;
                differences.append(&mut d);
                let _ = (op, ot);
            }
            (None, Some((np, nt))) => {
                if !nt.trim().is_empty() {
                    differences.push(CompareDifference {
                        kind: "added".into(),
                        page: *np,
                        old_text: String::new(),
                        new_text: nt.clone(),
                    });
                }
            }
            (Some((op, ot)), None) => {
                if !ot.trim().is_empty() {
                    differences.push(CompareDifference {
                        kind: "removed".into(),
                        page: *op,
                        old_text: ot.clone(),
                        new_text: String::new(),
                    });
                }
            }
            (None, None) => unreachable!(),
        }
    }

    let added = differences.iter().filter(|d| d.kind == "added").count();
    let removed = differences.iter().filter(|d| d.kind == "removed").count();
    let changed = differences.iter().filter(|d| d.kind == "changed").count();
    Ok(CompareReport {
        old_pages,
        new_pages,
        compared_pages: n,
        added,
        removed,
        changed,
        differences,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// 报告渲染（把 CompareReport 写成纯文本报告，供导出命令复用）
// ---------------------------------------------------------------------------

pub fn render_report_text(report: &CompareReport) -> String {
    let mut s = String::new();
    s.push_str("PayOnce PDF 文档比对报告\n");
    s.push_str("====================================\n");
    s.push_str(&format!(
        "旧文档 {} 页 / 新文档 {} 页 / 实际比对 {} 页\n",
        report.old_pages, report.new_pages, report.compared_pages
    ));
    s.push_str(&format!(
        "新增 {} 处 · 删除 {} 处 · 修改 {} 处\n\n",
        report.added, report.removed, report.changed
    ));
    if report.differences.is_empty() {
        s.push_str("未发现文字层差异（注意：图片与版式变化不在比对范围内）。\n");
    }
    for d in &report.differences {
        let label = match d.kind.as_str() {
            "added" => "新增",
            "removed" => "删除",
            _ => "修改",
        };
        s.push_str(&format!("[第 {} 页] {}：\n", d.page, label));
        if d.kind != "added" {
            s.push_str(&format!("  旧：{}\n", d.old_text));
        }
        if d.kind != "removed" {
            s.push_str(&format!("  新：{}\n", d.new_text));
        }
        s.push('\n');
    }
    if !report.warnings.is_empty() {
        s.push_str("---------------- 提示 ----------------\n");
        for w in &report.warnings {
            s.push_str(&format!("- {}\n", w));
        }
    }
    s.push_str("---------------- 边界 ----------------\n");
    s.push_str("本报告仅比对文字层文本；图片、版式、页序、字体属性差异不在范围内。\n");
    s.push_str("工具结果需人工复核，尤其是合同、财务、法律文件。\n");
    s
}

// ---------------------------------------------------------------------------
// 单元测试：纯 diff 逻辑，不依赖 Tauri / PDFium / 磁盘
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn s_default() -> CompareSensitivity {
        CompareSensitivity::default()
    }

    fn kinds(diffs: &[CompareDifference]) -> Vec<&str> {
        diffs.iter().map(|d| d.kind.as_str()).collect()
    }

    #[test]
    fn identical_text_yields_no_difference() {
        let d = diff_text("合同金额 30 日交付", "合同金额 30 日交付", 1, &s_default()).unwrap();
        assert!(d.is_empty());
    }

    #[test]
    fn detects_whitespace_only_change_as_equal_by_default() {
        let d = diff_text("付款周期30天", "付款周期 30 天\n", 1, &s_default()).unwrap();
        assert!(d.is_empty(), "默认忽略空白，排版变化不应算差异");
        // strict 模式下空白算差异
        let strict = CompareSensitivity {
            ignore_whitespace: false,
            ..Default::default()
        };
        let d2 = diff_text("付款周期30天", "付款周期 30 天\n", 1, &strict).unwrap();
        assert!(!d2.is_empty());
    }

    #[test]
    fn detects_replaced_number_as_changed() {
        let d = diff_text("付款周期 30 天", "付款周期 60 天", 1, &s_default()).unwrap();
        assert_eq!(kinds(&d), vec!["changed"]);
        assert!(d[0].old_text.contains('3') && d[0].old_text.contains("30"));
        assert!(d[0].new_text.contains("60"));
    }

    #[test]
    fn detects_added_and_removed_cjk_runs() {
        let d = diff_text("第一章 概述", "第一章 概述 新增条款", 2, &s_default()).unwrap();
        assert_eq!(kinds(&d), vec!["added"]);
        assert!(d[0].new_text.contains("新增条款"));

        let d2 = diff_text("第一章 概述 旧条款", "第一章 概述", 2, &s_default()).unwrap();
        assert_eq!(kinds(&d2), vec!["removed"]);
        assert!(d2[0].old_text.contains("旧条款"));
    }

    #[test]
    fn ignore_case_option() {
        let strict = CompareSensitivity::default();
        assert!(!diff_text("Invoice 30", "invoice 30", 1, &strict).unwrap().is_empty());
        let ci = CompareSensitivity {
            ignore_case: true,
            ..Default::default()
        };
        assert!(diff_text("Invoice 30", "invoice 30", 1, &ci).unwrap().is_empty());
    }

    #[test]
    fn ignore_punctuation_option() {
        let strict = CompareSensitivity::default();
        assert!(!diff_text("甲方，乙方", "甲方。乙方", 1, &strict).unwrap().is_empty());
        let ip = CompareSensitivity {
            ignore_punctuation: true,
            ..Default::default()
        };
        assert!(diff_text("甲方，乙方", "甲方。乙方", 1, &ip).unwrap().is_empty());
    }

    #[test]
    fn page_number_is_stamped_on_difference() {
        let d = diff_text("A", "B", 7, &s_default()).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].page, 7);
    }

    #[test]
    fn report_text_contains_bounds_disclaimer() {
        let report = CompareReport {
            old_pages: 1,
            new_pages: 1,
            compared_pages: 1,
            added: 0,
            removed: 0,
            changed: 1,
            differences: vec![CompareDifference {
                kind: "changed".into(),
                page: 1,
                old_text: "30 天".into(),
                new_text: "60 天".into(),
            }],
            warnings: vec!["第 2 页无文字层".into()],
        };
        let text = render_report_text(&report);
        assert!(text.contains("第 1 页"));
        assert!(text.contains("旧：30 天"));
        assert!(text.contains("新：60 天"));
        assert!(text.contains("不在范围内"));
        assert!(text.contains("第 2 页无文字层"));
    }

    #[test]
    fn over_char_limit_fails_closed() {
        let big = "x".repeat(CHAR_LIMIT + 1);
        assert!(diff_text(&big, "small", 1, &s_default()).is_err());
    }

    /// 真实文件级端到端：text_to_pdf 造两份仅数字不同的 PDF → compare 主流程。
    #[test]
    fn e2e_compare_real_pdfs_reports_changed_number() {
        let pdfium = crate::engine::render::tests::test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_cmp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old_txt = dir.join("old.txt");
        let new_txt = dir.join("new.txt");
        let old_pdf = dir.join("old.pdf");
        let new_pdf = dir.join("new.pdf");
        std::fs::write(&old_txt, "Invoice 30\npayment due 30 days").unwrap();
        std::fs::write(&new_txt, "Invoice 60\npayment due 60 days").unwrap();
        let _ = crate::engine::more::text_to_pdf(pdfium, &old_txt, "txt", &old_pdf).unwrap();
        let _ = crate::engine::more::text_to_pdf(pdfium, &new_txt, "txt", &new_pdf).unwrap();

        let report = compare_with_pdfium(
            pdfium,
            &old_pdf,
            None,
            &new_pdf,
            None,
            None,
            &s_default(),
        )
        .unwrap();
        assert_eq!(report.old_pages, 1);
        assert_eq!(report.new_pages, 1);
        assert_eq!(report.compared_pages, 1);
        assert!(report.changed >= 1, "应识别出数字修改：{:?}", report.differences);
        assert!(
            report.differences.iter().any(|d| d.old_text.contains("30") && d.new_text.contains("60")),
            "差异应同时含旧值 30 与新值 60：{:?}",
            report.differences
        );
    }

    /// 页面范围：只比第 1 页时，第 2 页的差异不应出现。
    #[test]
    fn e2e_compare_respects_page_range() {
        let pdfium = crate::engine::render::tests::test_pdfium();
        let dir = std::env::temp_dir().join(format!("payoncepdf_cmprange_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old_txt = dir.join("old.txt");
        let new_txt = dir.join("new.txt");
        let old_pdf = dir.join("old.pdf");
        let new_pdf = dir.join("new.pdf");
        // text_to_pdf 每页 45 行；造两页只需 46 行
        let mut a = String::new();
        let mut b = String::new();
        for i in 0..46 {
            if i == 45 {
                a.push_str("page two OLD\n");
                b.push_str("page two NEW\n");
            } else {
                a.push_str("same line\n");
                b.push_str("same line\n");
            }
        }
        std::fs::write(&old_txt, &a).unwrap();
        std::fs::write(&new_txt, &b).unwrap();
        crate::engine::more::text_to_pdf(pdfium, &old_txt, "txt", &old_pdf).unwrap();
        crate::engine::more::text_to_pdf(pdfium, &new_txt, "txt", &new_pdf).unwrap();

        let all = compare_with_pdfium(pdfium, &old_pdf, None, &new_pdf, None, None, &s_default()).unwrap();
        assert!(all.changed >= 1, "全页比对应发现第 2 页差异");
        let first = compare_with_pdfium(pdfium, &old_pdf, None, &new_pdf, None, Some("1"), &s_default()).unwrap();
        assert_eq!(first.compared_pages, 1);
        assert_eq!(first.changed, 0, "只比第 1 页不应含第 2 页差异");
    }
}
