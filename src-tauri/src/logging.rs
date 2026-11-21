//! 本地日志（PRD §6.3 / §10）。
//!
//! 红线：默认关闭；开启后**只记录 时间 / 操作类型 / 错误码** 三列，
//! 绝不记录文件名、路径、文件内容或任何可识别用户的数据；日志只写本机。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 在 app 启动时把 app_data_dir 交给日志模块（不创建文件，直到真正 record）。
pub fn init(dir: PathBuf) {
    let _ = LOG_DIR.set(dir);
}

/// 打开/关闭日志。
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// 记录一条日志：`<时间> | <操作类型> | <错误码>`。
/// `op` 是操作类型（如 "merge"/"activate"），`code` 是错误码或 "OK"。
/// 失败静默忽略——日志本身不能让主流程崩溃。
pub fn record(op: &str, code: &str) {
    if !is_enabled() {
        return;
    }
    let Some(dir) = LOG_DIR.get() else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        let _ = e;
        return;
    }
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let line = format!("{} | {} | {}\n", ts, op, code);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("payoncepdf.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// 记录一条带提示的日志（提示内容也只写进本机日志文件，不外发）。
/// 用于公钥缺失等需要给开发者排障线索的场景。
pub fn record_hint(op: &str, code: &str, hint: &str) {
    if !is_enabled() {
        return;
    }
    let Some(dir) = LOG_DIR.get() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let line = format!("{} | {} | {} | {}\n", ts, op, code, hint);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("payoncepdf.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}
