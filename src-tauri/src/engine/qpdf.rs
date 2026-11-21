//! qpdf CLI 封装（PRD §9.1；工程红线：qpdf Apache-2.0）。
//!
//! 负责 F1 合并、F2 拆分的按页切片、已知密码解密、页数校验。
//! 说明：`qpdf --empty --pages ... --` 会重建一个全新的 PDF 文档对象树，
//! **无法把各输入文件的 /Outlines(书签) 合并进结果**（qpdf 只会丢弃源 outline）。
//! 该限制会在合并命令返回的 message 中明确告知用户（见 jobs.rs）。

use crate::engine::sidecar;
use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// 已知限制说明，供上层拼进 JobResult.message。
pub const OUTLINE_LIMITATION: &str =
    "注意：qpdf 合并会重建文档树，无法保留/合并各源文件的书签（outline）；文字层与页面内容完整保留。";

fn map_qpdf_error(stderr: &str) -> AppError {
    let s = stderr.to_ascii_lowercase();
    if s.contains("invalid password") || s.contains("incorrect password") {
        AppError::bad_password("密码不正确")
    } else if s.contains("password") && (s.contains("required") || s.contains("encrypted")) {
        AppError::need_password("该文件有密码，请输入")
    } else if s.contains("damaged")
        || s.contains("syntax error")
        || s.contains("unexpected")
        || s.contains("not a pdf")
        || s.contains("recovery")
    {
        AppError::parse("文件无法读取，已跳过（qpdf 无法解析该 PDF）")
    } else {
        AppError::internal(format!("qpdf 执行失败：{}", stderr.trim()))
    }
}

fn run(app: &AppHandle, args: &[String]) -> AppResult<String> {
    let bin = sidecar::qpdf_path(app)?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(args);
    sidecar::apply_child_env(app, &mut cmd);
    let out = cmd
        .output()
        .map_err(|e| AppError::internal(format!("无法启动 qpdf：{}", e)))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        return Err(map_qpdf_error(&stderr));
    }
    Ok(stdout)
}

/// 用 `qpdf --show-npages` 读取页数（支持已知密码）。
pub fn page_count(app: &AppHandle, file: &Path, password: Option<&str>) -> AppResult<u32> {
    let mut args: Vec<String> = Vec::new();
    if let Some(p) = password {
        args.push(format!("--password={}", p));
    }
    args.push("--show-npages".to_string());
    args.push(file.to_string_lossy().to_string());
    let stdout = run(app, &args)?;
    stdout
        .trim()
        .parse::<u32>()
        .map_err(|_| AppError::parse(format!("无法解析 {} 的页数", file.display())))
}

/// 用已知密码解密（移除加密层）到临时文件；仅用于读取，不做任何破解。
pub fn decrypt(
    app: &AppHandle,
    input: &Path,
    password: &str,
    output: &Path,
) -> AppResult<()> {
    let args: Vec<String> = vec![
        format!("--password={}", password),
        "--decrypt".to_string(),
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
    ];
    run(app, &args)?;
    Ok(())
}

/// 把逗号分隔的页码范围切成可作为 `--pages` 参数的 token 列表。
/// 例如 "1-5,8,11-13" -> ["1-5", "8", "11-13"]。
pub fn parse_range_tokens(s: &str) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for raw in s.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        if !token
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
        {
            return Err(AppError::internal(format!("非法页码范围：{}", token)));
        }
        // 去掉内部空格后作为单个范围 token。
        out.push(token.chars().filter(|c| !c.is_whitespace()).collect());
    }
    if out.is_empty() {
        return Err(AppError::internal("页码范围为空"));
    }
    Ok(out)
}

/// 合并多个 PDF 为单个输出文件。
/// - 有已知密码时先把每个输入解密到 `tmp_dir`，再合并（qpdf 的 `--pages` 不便逐文件带密码）。
/// - 输出写到调用方给的临时路径，由上层再做原子提交。
pub fn merge(
    app: &AppHandle,
    inputs: &[PathBuf],
    password: Option<&str>,
    output: &Path,
    tmp_dir: &Path,
) -> AppResult<()> {
    let effective: Vec<PathBuf> = if let Some(pw) = password {
        let mut v = Vec::with_capacity(inputs.len());
        for (i, f) in inputs.iter().enumerate() {
            let tmp = tmp_dir.join(format!("merge_dec_{}.pdf", i));
            decrypt(app, f, pw, &tmp)?;
            v.push(tmp);
        }
        v
    } else {
        inputs.to_vec()
    };

    let mut args: Vec<String> = vec!["--empty".to_string(), "--pages".to_string()];
    for f in &effective {
        args.push(f.to_string_lossy().to_string());
    }
    args.push("--".to_string());
    args.push(output.to_string_lossy().to_string());
    run(app, &args)?;
    Ok(())
}

/// F13：AES-256 加密 + 权限位（仅设置用户已知密码，不做任何破解）。
/// `allow_print` / `allow_copy` / `allow_modify` 对应 qpdf 的 --print / --extract / --modify。
/// 输出写入调用方给的临时路径，由上层原子提交。
#[allow(clippy::too_many_arguments)]
pub fn encrypt(
    app: &AppHandle,
    input: &Path,
    user_password: &str,
    owner_password: &str,
    allow_print: bool,
    allow_copy: bool,
    allow_modify: bool,
    output: &Path,
) -> AppResult<()> {
    let owner = if owner_password.is_empty() {
        user_password
    } else {
        owner_password
    };
    // qpdf 要求 --encrypt 的权限选项全部出现在 `--` 之前。
    let args: Vec<String> = vec![
        "--encrypt".to_string(),
        user_password.to_string(),
        owner.to_string(),
        "256".to_string(),
        format!("--print={}", if allow_print { "full" } else { "none" }),
        format!("--extract={}", if allow_copy { "y" } else { "n" }),
        format!("--modify={}", if allow_modify { "all" } else { "none" }),
        "--".to_string(),
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
    ];
    run(app, &args)?;
    Ok(())
}

/// 从单个 PDF 抽取给定页范围写到输出（F2 每 N 页 / 指定范围切片）。
/// `ranges` 是若干 `--pages` 的页码 token（同一次 `--pages` 允许多个范围）。
pub fn slice(
    app: &AppHandle,
    input: &Path,
    password: Option<&str>,
    ranges: &[String],
    output: &Path,
    tmp_dir: &Path,
) -> AppResult<()> {
    let effective = if let Some(pw) = password {
        let tmp = tmp_dir.join("slice_dec.pdf");
        decrypt(app, input, pw, &tmp)?;
        tmp
    } else {
        input.to_path_buf()
    };

    let mut args: Vec<String> = vec![
        "--empty".to_string(),
        "--pages".to_string(),
        effective.to_string_lossy().to_string(),
    ];
    for r in ranges {
        args.push(r.clone());
    }
    args.push("--".to_string());
    args.push(output.to_string_lossy().to_string());
    run(app, &args)?;
    Ok(())
}
