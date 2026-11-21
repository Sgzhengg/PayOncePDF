//! 应用类命令：自检 / 授权 / 日志开关 / 打开输出文件夹 / 取消（CONTRACT §3）。
//! 这些命令不是导出任务，不调用 `ensure_export_allowed()`。

use crate::commands::{
    ActivateRequest, AppInfo, CancelRequest, EmptyPayload, LicenseState, OpenFolderRequest,
    SetLoggingRequest,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::{license, logging};
use tauri::{AppHandle, Manager};

/// F9 / §8 / §17：版本、平台、架构、sidecar 自检结果。恒 offlineCapable=true、
/// telemetry=false、preview=false（无遥测、非浏览器预览）。
#[tauri::command]
pub fn get_app_info(payload: EmptyPayload, app: AppHandle) -> Result<AppInfo, AppError> {
    let _ = payload;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: crate::engine::sidecar::platform(),
        arch: crate::engine::sidecar::arch(),
        sidecars: crate::engine::sidecar::self_check(&app),
        offline_capable: true,
        telemetry: false,
        preview: false,
    })
}

/// F8：查询授权状态。纯本地、不发网络请求，断网时仍可用。
#[tauri::command]
pub fn get_license_state(payload: EmptyPayload, app: AppHandle) -> Result<LicenseState, AppError> {
    let _ = payload;
    license::get_license_state(&app)
}

/// F8：首次联网激活（仅发送 {key, fingerprint}，绝不上传文件内容）。
#[tauri::command]
pub fn activate_license(
    payload: ActivateRequest,
    app: AppHandle,
) -> Result<LicenseState, AppError> {
    license::activate_license(&app, &payload.key)
}

/// 取消任务：置位对应 AtomicBool，任务在下一个检查点返回 E2003。
#[tauri::command]
pub fn cancel_job(payload: CancelRequest, app: AppHandle) -> Result<(), AppError> {
    let st = app.state::<AppState>();
    st.cancel(&payload.job_id);
    logging::record("cancel", "OK");
    Ok(())
}

/// 开关本地日志（默认关闭；开启后仅记录 时间/操作类型/错误码）。
#[tauri::command]
pub fn set_logging(payload: SetLoggingRequest, app: AppHandle) -> Result<(), AppError> {
    let _ = &app;
    logging::set_enabled(payload.enabled);
    Ok(())
}

/// 打开输出文件夹。Windows `explorer`、macOS `open`、Linux 忽略。
/// 用 std::process::Command，避免插件 API 风险。
#[tauri::command]
pub fn open_output_folder(
    payload: OpenFolderRequest,
    app: AppHandle,
) -> Result<(), AppError> {
    let _ = &app;
    open_folder(&payload.path)
}

fn open_folder(path: &str) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(AppError::from)?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(AppError::from)?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux：按约定忽略，不报错。
        let _ = path;
    }
    Ok(())
}
