//! Sidecar 定位与三平台自检（PRD §9.1 / §17，CONTRACT §3 `get_app_info`）。
//!
//! 随包的 qpdf / tesseract 是 `externalBin`，运行时会被放到 exe 同目录（带 target-triple 后缀），
//! pdfium 与 tessdata 是 `bundle.resources`。这里做多路径探测，兼容：
//! - qpdf.exe / qpdf / qpdf-<triple> / qpdf-<triple>.exe
//! - pdfium.dll(win) / libpdfium.dylib(mac) / libpdfium.so(linux)
//! - tessdata/ 目录（含 *.traineddata）

use crate::commands::SidecarInfo;
use crate::error::{AppError, AppResult};
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub fn arch() -> String {
    std::env::consts::ARCH.to_string()
}

pub fn platform() -> String {
    std::env::consts::OS.to_string()
}

/// 探测用的候选目录集合。
fn search_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(rd) = app.path().resource_dir() {
        v.push(rd.clone());
        v.push(rd.join("bin"));
        v.push(rd.join("resources"));
        v.push(rd.join("resources").join("bin"));
    }
    if let Some(ed) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        v.push(ed.clone());
        v.push(ed.join("bin"));
    }
    // 开发态回退（cargo run 时 CARGO_MANIFEST_DIR 可用）。
    if let Ok(md) = std::env::var("CARGO_MANIFEST_DIR") {
        let base = PathBuf::from(md);
        v.push(base.join("bin"));
        v.push(base.join("resources"));
        v.push(base.join("keys"));
    }
    v
}

/// 可执行文件样式：无扩展名或 .exe（排除 .dll/.dylib/.so 等）。
fn is_exe_like(name: &str) -> bool {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    ext.is_empty() || ext.eq_ignore_ascii_case("exe")
}

/// 在目录内找以 `prefix` 开头的可执行文件（兼容 externalBin 的 triple 后缀）。
fn find_bin_in(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let low = name.to_ascii_lowercase();
            (low == prefix || low.starts_with(&format!("{}.", prefix)) || low.starts_with(&format!("{}-", prefix)))
                && is_exe_like(name)
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
        let with_exe = dir.join(format!("{}.exe", name));
        if with_exe.is_file() {
            return Some(with_exe);
        }
    }
    None
}

fn find_binary(app: &AppHandle, prefix: &str) -> Option<PathBuf> {
    for dir in search_dirs(app) {
        if let Some(p) = find_bin_in(&dir, prefix) {
            return Some(p);
        }
    }
    find_on_path(prefix)
}

/// PDFium 动态库文件名（按平台）。
pub fn pdfium_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

fn find_pdfium(app: &AppHandle) -> Option<PathBuf> {
    let name = pdfium_filename();
    for dir in search_dirs(app) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

// ---- F16：可选 OCR 语言包（30+ 语言，按需安装）----
//
// 这是全产品**唯一**的运行时联网点，且满足三条硬约束：
// 1. 只在用户主动点击「安装语言包」时发生；
// 2. 请求 URL 只含语言代码，不含任何文件、文件名、路径或用户数据；
// 3. 下载的是公开的静态字典，不调用任何云端处理/AI 接口。
// PDFgear 的 AI 走 Azure OpenAI 并上传文件内容，与本机制性质完全不同。

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguageInfo {
    pub code: String,
    pub label: String,
    pub installed: bool,
}

/// tessdata_fast 常用语言清单（与官方仓库命名一致）。
const AVAILABLE_LANGS: &[(&str, &str)] = &[
    ("eng", "英语"), ("chi_sim", "简体中文"), ("chi_tra", "繁体中文"),
    ("jpn", "日语"), ("kor", "韩语"), ("fra", "法语"), ("deu", "德语"),
    ("spa", "西班牙语"), ("rus", "俄语"), ("ita", "意大利语"),
    ("por", "葡萄牙语"), ("nld", "荷兰语"), ("ara", "阿拉伯语"),
    ("hin", "印地语"), ("vie", "越南语"), ("tha", "泰语"),
    ("ind", "印尼语"), ("tur", "土耳其语"), ("pol", "波兰语"),
    ("ukr", "乌克兰语"), ("ces", "捷克语"), ("ron", "罗马尼亚语"),
    ("hun", "匈牙利语"), ("ell", "希腊语"), ("heb", "希伯来语"),
    ("fas", "波斯语"), ("swe", "瑞典语"), ("dan", "丹麦语"),
    ("nor", "挪威语"), ("fin", "芬兰语"), ("bul", "保加利亚语"),
    ("hrv", "克罗地亚语"), ("slk", "斯洛伐克语"), ("cat", "加泰罗尼亚语"),
    ("est", "爱沙尼亚语"), ("lav", "拉脱维亚语"), ("lit", "立陶宛语"),
    ("tam", "泰米尔语"), ("tel", "泰卢固语"), ("ben", "孟加拉语"),
    ("urd", "乌尔都语"), ("afr", "南非荷兰语"),
];

/// 用户级 tessdata 目录：随包目录通常只读，额外语言包放这里。
pub fn user_tessdata_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("无法定位应用数据目录：{}", e)))?
        .join("tessdata"))
}

fn bundle_tessdata(app: &AppHandle) -> Option<PathBuf> {
    for dir in search_dirs(app) {
        let td = dir.join("tessdata");
        if td.is_dir() && tessdata_ready(&td) {
            return Some(td);
        }
    }
    None
}

fn find_tessdata(app: &AppHandle) -> Option<PathBuf> {
    // 用户目录优先：安装过额外语言包后，它必须成为唯一生效的 tessdata。
    if let Ok(user) = user_tessdata_dir(app) {
        if tessdata_ready(&user) {
            return Some(user);
        }
    }
    bundle_tessdata(app)
}

fn installed_languages(app: &AppHandle) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(b) = bundle_tessdata(app) {
        dirs.push(b);
    }
    if let Ok(u) = user_tessdata_dir(app) {
        dirs.push(u);
    }
    for d in dirs {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.filter_map(|x| x.ok()) {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("traineddata") {
                    if let Some(code) = p.file_stem().and_then(|s| s.to_str()) {
                        if !out.iter().any(|x| x == code) {
                            out.push(code.to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn list_languages(app: &AppHandle) -> Vec<OcrLanguageInfo> {
    let installed = installed_languages(app);
    AVAILABLE_LANGS
        .iter()
        .map(|(code, label)| OcrLanguageInfo {
            code: (*code).to_string(),
            label: (*label).to_string(),
            installed: installed.iter().any(|x| x == code),
        })
        .collect()
}

/// 下载一个 tessdata_fast 语言包到用户目录。白名单校验，先临时文件再原子替换。
pub fn install_language(app: &AppHandle, lang: &str) -> AppResult<String> {
    let code = lang.trim().to_ascii_lowercase();
    if !code
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        || code.len() > 20
        || !AVAILABLE_LANGS.iter().any(|(c, _)| *c == code.as_str())
    {
        return Err(AppError::internal(format!(
            "不支持的语言代码「{}」；请从语言列表中选择",
            lang
        )));
    }
    let user = user_tessdata_dir(app)?;
    std::fs::create_dir_all(&user).map_err(AppError::from)?;

    // 把随包自带语言一并复制过来，保证用户目录成为完整可用的 tessdata。
    if let Some(bundle) = bundle_tessdata(app) {
        if let Ok(rd) = std::fs::read_dir(&bundle) {
            for e in rd.filter_map(|x| x.ok()) {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("traineddata") {
                    let dest = user.join(p.file_name().unwrap_or_default());
                    if !dest.exists() {
                        let _ = std::fs::copy(&p, &dest);
                    }
                }
            }
        }
    }

    let url = format!(
        "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/{}.traineddata",
        code
    );
    let response = ureq::get(&url)
        .set("User-Agent", "PayOnce/1.0 (local OCR language pack installer)")
        .call()
        .map_err(|e| {
            AppError::internal(format!(
                "语言包下载失败（{}）。检查网络后重试；语言包是可选的，现有 OCR 功能不受影响。",
                e
            ))
        })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(AppError::from)?;
    if let Err(reason) = validate_traineddata(&bytes) {
        return Err(AppError::ocr_lang(format!(
            "下载内容不是有效的 OCR 语言包（{}）；已丢弃，未改动现有语言包。",
            reason
        )));
    }
    let tmp = user.join(format!("{}.tmp", code));
    let dest = user.join(format!("{}.traineddata", code));
    std::fs::write(&tmp, &bytes).map_err(AppError::from)?;
    if dest.exists() {
        let _ = std::fs::remove_file(&dest); // Windows 的 rename 不允许覆盖
    }
    std::fs::rename(&tmp, &dest).map_err(AppError::from)?;
    Ok(format!(
        "已安装 {} 语言包（{} KB）到用户目录；识别全程仍在本机完成，文件内容不上传。",
        code,
        bytes.len() / 1024
    ))
}

/// tessdata_fast 语言包的下载内容校验。
///
/// 之前只查 `len >= 50_000`：一个足够大的 HTML 错误页 / CDN 占位文件 / 截断下载
/// 都会写进用户目录，随后 OCR 静默失败。traineddata 是 protobuf 编码，文件头
/// 以 `0x18`（field 3 / varint）开头，正文含大量 0xFF 填充；这里做结构校验，
/// 不信任文件名和 HTTP 状态。校验失败一律不落盘。
fn validate_traineddata(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() < 50_000 {
        return Err("文件过小（< 50 KB）");
    }
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("文件异常大（> 32 MB），疑似下载错误");
    }
    // tessdata_fast 的 protobuf 头：首字节 0x18（field 3, wire type 0）。
    if bytes.first() != Some(&0x18) {
        return Err("文件头不是 traineddata 的 protobuf 标记");
    }
    // 真实 traineddata 含大量 0xFF/0x00 填充；HTML/JSON 错误页不会有。
    if !bytes.iter().any(|b| *b == 0xFF) {
        return Err("缺少 traineddata 的二进制填充特征（疑似文本错误页）");
    }
    let head = &bytes[..bytes.len().min(16)];
    if head.starts_with(b"<") || head.starts_with(b"{") {
        return Err("内容以 HTML/JSON 开头，疑似服务端错误页");
    }
    Ok(())
}

fn tessdata_ready(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .any(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x == "traineddata")
                        .unwrap_or(false)
                })
        })
        .unwrap_or(false)
}

// ---- 供引擎使用的严格定位（缺失即返回明确错误码）----

pub fn qpdf_path(app: &AppHandle) -> AppResult<PathBuf> {
    find_binary(app, "qpdf").ok_or_else(|| {
        AppError::sidecar_missing(
            "未找到 qpdf 组件（sidecar 缺失或架构不匹配），请重新安装或运行自检",
        )
    })
}

pub fn tesseract_path(app: &AppHandle) -> AppResult<PathBuf> {
    find_binary(app, "tesseract").ok_or_else(|| {
        AppError::sidecar_missing(
            "未找到 tesseract 组件（sidecar 缺失或架构不匹配），请重新安装或运行自检",
        )
    })
}

pub fn tessdata_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = find_tessdata(app).ok_or_else(|| {
        AppError::ocr_lang("未找到 tessdata 语言包目录，请安装语言包")
    })?;
    if !tessdata_ready(&dir) {
        return Err(AppError::ocr_lang(
            "tessdata 目录内没有 .traineddata 语言包，请安装中文/英文语言包",
        ));
    }
    Ok(dir)
}

/// 检查某个语言包是否存在（如 "eng" / "chi_sim"）。
pub fn check_language(app: &AppHandle, lang: &str) -> AppResult<()> {
    let dir = tessdata_dir(app)?;
    if !dir.join(format!("{}.traineddata", lang)).exists() {
        return Err(AppError::ocr_lang(format!(
            "缺少 OCR 语言包 {}.traineddata，请安装后重试",
            lang
        )));
    }
    Ok(())
}

pub fn pdfium_path(app: &AppHandle) -> AppResult<PathBuf> {
    find_pdfium(app).ok_or_else(|| {
        AppError::pdfium_missing(format!(
            "未找到 {}（PDFium 渲染引擎未加载），请重新安装",
            pdfium_filename()
        ))
    })
}

/// 为子进程补齐动态库搜索路径。
///
/// qpdf / tesseract 的 Windows 构建依赖同目录 DLL；打包时这些 DLL 作为
/// `bundle.resources` 落在 resource_dir，而不是 externalBin 的可执行文件目录。
/// Windows 会按 "子进程 exe 目录 → 系统目录 → PATH" 搜索 DLL，macOS 则看
/// DYLD_LIBRARY_PATH，因此这里把 resource_dir / exe 目录前置到子进程环境。
/// 开发态（`cargo tauri dev`）下 DLL 就在 `src-tauri/bin/`，同样能命中。
pub fn apply_child_env(app: &AppHandle, cmd: &mut std::process::Command) {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = app.path().resource_dir() {
        dirs.push(rd.clone());
        dirs.push(rd.join("resources"));
    }
    if let Some(ed) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        dirs.push(ed.clone());
        dirs.push(ed.join("bin"));
    }
    if let Ok(md) = std::env::var("CARGO_MANIFEST_DIR") {
        dirs.push(PathBuf::from(md).join("bin"));
    }
    let dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| d.is_dir()).collect();

    if let Some(old) = std::env::var_os("PATH") {
        let mut paths = dirs.clone();
        paths.extend(std::env::split_paths(&old));
        if let Ok(joined) = std::env::join_paths(paths) {
            cmd.env("PATH", joined);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if !dirs.is_empty() {
            if let Ok(joined) = std::env::join_paths(&dirs) {
                let mut value = joined;
                if let Some(old) = std::env::var_os("DYLD_LIBRARY_PATH") {
                    value.push(":");
                    value.push(old);
                }
                cmd.env("DYLD_LIBRARY_PATH", value);
            }
        }
    }
}

/// `get_app_info` 用的自检结果（qpdf / tesseract / tessdata / pdfium）。
pub fn self_check(app: &AppHandle) -> Vec<SidecarInfo> {
    let a = arch();
    let qpdf = find_binary(app, "qpdf");
    let tesseract = find_binary(app, "tesseract");
    let pdfium = find_pdfium(app);
    let tessdata = find_tessdata(app);

    let info = |name: &str, ready: bool, path: String| SidecarInfo {
        name: name.to_string(),
        ready,
        path,
        arch: a.clone(),
    };

    vec![
        info(
            "qpdf",
            qpdf.is_some(),
            qpdf.as_ref().map(display).unwrap_or_default(),
        ),
        info(
            "tesseract",
            tesseract.is_some(),
            tesseract.as_ref().map(display).unwrap_or_default(),
        ),
        info(
            "tessdata",
            tessdata.as_ref().map(|d| tessdata_ready(d)).unwrap_or(false),
            tessdata.as_ref().map(display).unwrap_or_default(),
        ),
        info(
            "pdfium",
            pdfium.is_some(),
            pdfium.as_ref().map(display).unwrap_or_default(),
        ),
    ]
}

fn display(p: &PathBuf) -> String {
    p.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traineddata_validator_accepts_real_bundled_packs() {
        for code in ["eng", "chi_sim"] {
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources/tessdata")
                .join(format!("{}.traineddata", code));
            let Ok(bytes) = std::fs::read(&p) else {
                eprintln!("随包语言包 {} 不存在，跳过（不视为通过）", code);
                continue;
            };
            assert!(validate_traineddata(&bytes).is_ok(), "真实语言包 {} 被误拒", code);
        }
    }

    #[test]
    fn traineddata_validator_rejects_html_and_truncated() {
        let mut html = vec![b'<'; 60_000];
        html.extend_from_slice(b"<!DOCTYPE html><html>404 Not Found</html>");
        assert!(validate_traineddata(&html).is_err());

        let mut wrong_head = vec![0u8; 60_000];
        wrong_head[0] = 0x3C; // '<'
        wrong_head[100] = 0xFF;
        assert!(validate_traineddata(&wrong_head).is_err());

        let mut no_fill = vec![0x18u8; 60_000];
        no_fill[100] = b'a'; // 没有 0xFF 填充
        assert!(validate_traineddata(&no_fill).is_err());

        let tiny = vec![0x18u8; 1_000];
        assert!(validate_traineddata(&tiny).is_err());

        let mut huge = vec![0x18u8; 33 * 1024 * 1024];
        huge[100] = 0xFF;
        assert!(validate_traineddata(&huge).is_err());
    }
}
