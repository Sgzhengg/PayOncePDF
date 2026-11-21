//! 任务状态：取消注册表、进度/完成事件发射、临时目录与原子提交。
//!
//! 参见 CONTRACT §1：长任务用 `payoncepdf://progress` / `payoncepdf://finished`；
//! 取消用 `jobId -> Arc<AtomicBool>`；临时文件写 `temp_dir()/payoncepdf-<jobId>/`，结束清理。

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// 固定事件名。
pub const PROGRESS_EVENT: &str = "payoncepdf://progress";
pub const FINISHED_EVENT: &str = "payoncepdf://finished";

/// 挂载在 Tauri `manage()` 上的全局状态。
pub struct AppState {
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cancels: Mutex::new(HashMap::new()),
        }
    }

    /// 任务开始时注册，返回该任务的取消标志。
    pub fn register(&self, job_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut map = self.cancels.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(job_id.to_string(), flag.clone());
        flag
    }

    /// 触发取消；返回是否命中一个正在进行的任务。
    pub fn cancel(&self, job_id: &str) -> bool {
        let map = self.cancels.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(flag) = map.get(job_id) {
            flag.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn is_cancelled(&self, job_id: &str) -> bool {
        let map = self.cancels.lock().unwrap_or_else(|e| e.into_inner());
        map.get(job_id)
            .map(|f| f.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// 任务结束后清理注册表项。
    pub fn finish(&self, job_id: &str) {
        let mut map = self.cancels.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(job_id);
    }
}

/// 每个文件/每页处理后调用：命中取消则返回 E2003。
pub fn check_cancel(flag: &AtomicBool) -> AppResult<()> {
    if flag.load(Ordering::SeqCst) {
        Err(AppError::cancelled("已取消，原文件未改动"))
    } else {
        Ok(())
    }
}

/// 进度事件 payload（与前端 `ProgressEvent` 对齐，camelCase）。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub job_id: String,
    pub done: usize,
    pub total: usize,
    pub message: String,
}

pub fn emit_progress(
    app: &AppHandle,
    job_id: &str,
    done: usize,
    total: usize,
    message: impl Into<String>,
) {
    let evt = ProgressEvent {
        job_id: job_id.to_string(),
        done,
        total,
        message: message.into(),
    };
    // 事件推送失败不应中断任务。
    let _ = app.emit(PROGRESS_EVENT, evt);
}

/// 结束事件：`payoncepdf://finished` 推送 `JobResult`（泛型避免与 commands 层循环依赖）。
pub fn emit_finished<T>(app: &AppHandle, payload: T)
where
    T: Serialize + Clone,
{
    let _ = app.emit(FINISHED_EVENT, payload);
}

/// 临时目录：`std::env::temp_dir()/payoncepdf-<jobId>/`。
/// 对 jobId 做白名单过滤，避免路径穿越。
pub fn temp_dir_for(job_id: &str) -> PathBuf {
    let safe: String = job_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let safe = if safe.is_empty() { "job".to_string() } else { safe };
    std::env::temp_dir().join(format!("payoncepdf-{}", safe))
}

pub fn create_temp_dir(job_id: &str) -> std::io::Result<PathBuf> {
    let dir = temp_dir_for(job_id);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn cleanup_temp_dir(job_id: &str) {
    let _ = std::fs::remove_dir_all(temp_dir_for(job_id));
}

/// 遇到已存在输出文件时"换名"：在扩展名前追加 `_1`、`_2` ... 直到不冲突。
/// 这样既不覆盖用户已有文件，也不会因为一次偶发重名直接失败。
pub fn resolve_output_path(requested: &Path) -> PathBuf {
    if !requested.exists() {
        return requested.to_path_buf();
    }
    let stem = requested
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string();
    let ext = requested
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    let parent = requested.parent().unwrap_or_else(|| Path::new(""));
    for i in 1..10000u32 {
        let candidate = parent.join(format!("{}_{}{}", stem, i, ext));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 极端情况下返回一个带时间戳的名字。
    let ts = chrono::Utc::now().timestamp_millis();
    parent.join(format!("{}_{}{}", stem, ts, ext))
}

/// 把临时文件原子替换（rename）为目标路径；跨磁盘时退化为 copy + remove。
/// 目标被其它进程占用 / 权限不足时返回 E1007。
pub fn atomic_commit(tmp: &Path, final_path: &Path) -> AppResult<()> {
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    match std::fs::rename(tmp, final_path) {
        Ok(()) => Ok(()),
        Err(_rename_err) => {
            // 跨卷 rename 会失败，用 copy 兜底；copy 因占用失败则映射为 E1007。
            std::fs::copy(tmp, final_path).map_err(|e| {
                AppError::output_busy(format!("目标文件可能被其它程序占用：{}", e))
            })?;
            let _ = std::fs::remove_file(tmp);
            Ok(())
        }
    }
}

/// 确保 `output` 不等于任何输入文件（绝不原地覆盖输入）。
pub fn assert_not_overwriting_inputs(output: &Path, inputs: &[&Path]) -> AppResult<()> {
    let out = std::fs::canonicalize(output).unwrap_or_else(|_| output.to_path_buf());
    for input in inputs {
        if let Ok(canon) = std::fs::canonicalize(input) {
            if canon == out {
                return Err(AppError::internal(
                    "输出路径与输入文件相同，拒绝原地覆盖用户文件",
                ));
            }
        }
    }
    Ok(())
}
