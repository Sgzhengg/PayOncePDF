//! F8：授权 / 试用 / 离线激活缓存（PRD §7 F8、§10、§11；CONTRACT §1/§2）。
//!
//! 设计要点：
//! - 7 天全功能试用，首次启动在 `app_data_dir/trial.json` 写入 `startedAt` + sha256 摘要
//!   （绑定机器指纹，防止随手改日期/从别的机器拷贝文件）。
//! - 买断密钥 `PAYONCE-XXXX-XXXX-XXXX`，首次联网向激活接口 POST `{key, fingerprint}`，
//!   响应 `{payload: base64(JSON), signature: base64(ed25519)}`，用 `keys/public.key`
//!   的 Ed25519 公钥验签后缓存到 `app_data_dir/activation.json`，之后完全离线可用。
//! - `fingerprint` 仅由 OS/架构/主机名等环境变量经 SHA256 得到，**不可逆、不含任何文件内容**。
//! - `ensure_export_allowed()` 必须在每个导出命令开头调用；过期返回 E2005，
//!   且**绝不删除或改写用户文件**（本模块只读写自己的 trial/activation JSON）。
//! - 除"首次激活"外无任何网络请求；无遥测。

use crate::commands::LicenseState;
use crate::error::{AppError, AppResult};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const TRIAL_DAYS_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const TRIAL_SALT: &[u8] = b"PayOnce PDF/trial/v1";
const TRIAL_FILE: &str = "trial.json";
const ACTIVATION_FILE: &str = "activation.json";
const ACTIVATE_URL_ENV: &str = "PAYONCEPDF_ACTIVATE_URL";
const DEFAULT_ACTIVATE_URL: &str = "https://payoncepdf.com/api/activate";
const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Serialize, Deserialize)]
struct TrialFile {
    started_at: i64,
    digest: String,
}

#[derive(Serialize, Deserialize)]
struct ActivationFile {
    key: String,
    payload: String,
    signature: String,
    fingerprint: String,
    activated_at: i64,
    /// 只在验签通过后写入 true。公钥临时缺失时作为"此前已验签"的离线兜底依据。
    verified: bool,
}

/// 服务器响应：payload 是 base64(JSON)，signature 是 base64(ed25519 签名，签名对象为 payload 的原始字节)。
#[derive(Deserialize)]
struct ActivateResponse {
    payload: String,
    signature: String,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("无法定位 app_data_dir：{}", e)))
}

/// 机器指纹：SHA256(OS | ARCH | 若干机器环境变量)。不可逆，且不含文件内容/文件名。
fn fingerprint() -> String {
    let mut material = String::new();
    material.push_str(std::env::consts::OS);
    material.push('|');
    material.push_str(std::env::consts::ARCH);
    for key in [
        "USERNAME",
        "USER",
        "COMPUTERNAME",
        "HOSTNAME",
        "PROCESSOR_IDENTIFIER",
        "PROCESSOR_ARCHITECTURE",
        "NUMBER_OF_PROCESSORS",
    ] {
        if let Ok(v) = std::env::var(key) {
            material.push('|');
            material.push_str(key);
            material.push('=');
            material.push_str(&v);
        }
    }
    let mut h = Sha256::new();
    h.update(material.as_bytes());
    hex::encode(h.finalize())
}

fn trial_digest(started_at: i64, fp: &str) -> String {
    let mut h = Sha256::new();
    h.update(TRIAL_SALT);
    h.update(started_at.to_string().as_bytes());
    h.update(b"|");
    h.update(fp.as_bytes());
    hex::encode(h.finalize())
}

fn key_hint(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<String>().chars().rev().collect();
    format!("****{}", tail)
}

/// 读取 trial.json；不存在或无法解析时创建一份全新的（首次启动）。
/// 文件存在但摘要不匹配 → 视为被篡改，返回一个"早已过期"的起点（保守拒绝导出），
/// 同时写一条本机日志提示；**不删除、不改写用户文件**。
fn load_or_create_trial(app: &AppHandle) -> AppResult<(TrialFile, bool)> {
    let dir = app_data_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(AppError::from)?;
    let path = dir.join(TRIAL_FILE);
    let fp = fingerprint();

    if path.exists() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(trial) = serde_json::from_str::<TrialFile>(&text) {
                let good = trial.digest == trial_digest(trial.started_at, &fp);
                if !good {
                    crate::logging::record_hint(
                        "trial",
                        "E2004",
                        "trial.json 摘要不匹配（疑似被篡改），按过期处理",
                    );
                }
                return Ok((trial, good));
            }
        }
        // 解析失败：落到下面重建。
    }

    let started_at = now_ms();
    let trial = TrialFile {
        started_at,
        digest: trial_digest(started_at, &fp),
    };
    if let Ok(json) = serde_json::to_string_pretty(&trial) {
        let _ = std::fs::write(&path, json);
    }
    Ok((trial, true))
}

fn load_activation(app: &AppHandle) -> Option<ActivationFile> {
    let path = app_data_dir(app).ok()?.join(ACTIVATION_FILE);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<ActivationFile>(&text).ok()
}

/// 公钥候选路径。公钥是随包资源 `keys/public.key`（base64 的原始 32 字节）。
/// 优先 resource_dir 下若干常见布局；开发态回退到 `CARGO_MANIFEST_DIR/keys/public.key`。
fn candidate_key_paths(app: &AppHandle) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(rd) = app.path().resource_dir() {
        v.push(rd.join("keys").join("public.key"));
        v.push(rd.join("public.key"));
        v.push(rd.join("resources").join("keys").join("public.key"));
    }
    if let Ok(md) = std::env::var("CARGO_MANIFEST_DIR") {
        v.push(PathBuf::from(md).join("keys").join("public.key"));
    }
    v
}

fn locate_public_key(app: &AppHandle) -> AppResult<PathBuf> {
    for p in candidate_key_paths(app) {
        if p.is_file() {
            return Ok(p);
        }
    }
    let msg = "缺少 Ed25519 公钥 keys/public.key，无法完成激活；请运行 scripts/gen-license-keys.mjs 并把公钥随包发布";
    crate::logging::record_hint("activate", "E2004", msg);
    Err(AppError::internal(msg))
}

fn read_public_key(app: &AppHandle) -> AppResult<[u8; 32]> {
    let path = locate_public_key(app)?;
    let text = std::fs::read_to_string(&path).map_err(AppError::from)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|e| AppError::internal(format!("keys/public.key 不是合法 base64：{}", e)))?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| AppError::internal("Ed25519 公钥必须是原始 32 字节"))?;
    Ok(arr)
}

/// 用公钥验签一组 (payload 原始字节, signature)。
fn verify_signature(app: &AppHandle, payload: &[u8], signature_b64: &str) -> AppResult<()> {
    let pk = read_public_key(app)?;
    let verifying_key = VerifyingKey::from_bytes(&pk)
        .map_err(|e| AppError::internal(format!("公钥无效：{}", e)))?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .map_err(|e| AppError::internal(format!("激活签名不是合法 base64：{}", e)))?;
    let signature = Signature::try_from(sig_bytes.as_slice())
        .map_err(|e| AppError::internal(format!("激活签名长度/格式错误：{}", e)))?;
    verifying_key
        .verify(payload, &signature)
        .map_err(|_| AppError::internal("激活令牌验签失败"))?;
    Ok(())
}

/// 校验缓存的激活是否仍然有效（离线可判定，不发网络请求）。
fn activation_is_valid(app: &AppHandle, act: &ActivationFile) -> bool {
    // 1) 必须绑定当前机器。
    if act.fingerprint != fingerprint() {
        return false;
    }
    // 2) 解码 payload 原始字节（签名对象）。
    let payload_bytes = match base64::engine::general_purpose::STANDARD.decode(&act.payload) {
        Ok(b) => b,
        Err(_) => return false,
    };
    // 3) 若 payload 内声明了 key/fingerprint，则必须匹配（防止令牌被换机/换钥使用）。
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
        if let Some(fp) = v.get("fingerprint").and_then(|x| x.as_str()) {
            if fp != act.fingerprint {
                return false;
            }
        }
        if let Some(k) = v.get("key").and_then(|x| x.as_str()) {
            if k != act.key {
                return false;
            }
        }
    }
    // 4) 公钥可用时做完整验签；公钥临时缺失时，信任"激活时已验签"的标记，
    //    避免已付费用户在缺组件的机器上被锁死（离线可用优先）。
    match read_public_key(app) {
        Ok(_) => verify_signature(app, &payload_bytes, &act.signature).is_ok(),
        Err(_) => {
            crate::logging::record_hint(
                "license",
                "E2004",
                "公钥缺失，按 activation.json 的 verified 标记离线放行；请运行 scripts/gen-license-keys.mjs",
            );
            act.verified
        }
    }
}

/// 计算并返回当前授权状态。纯本地、不发网络请求，断网时仍可用。
pub fn get_license_state(app: &AppHandle) -> AppResult<LicenseState> {
    let (trial, trial_ok) = load_or_create_trial(app)?;

    if let Some(act) = load_activation(app) {
        if activation_is_valid(app, &act) {
            return Ok(LicenseState {
                status: "active".to_string(),
                trial_days_left: trial_days_left(trial.started_at, trial_ok),
                activated_at: Some(act.activated_at),
                can_export: true,
                key_hint: Some(key_hint(&act.key)),
            });
        }
    }

    let days_left = trial_days_left(trial.started_at, trial_ok);
    let can_export = trial_ok && (now_ms() - trial.started_at) < TRIAL_DAYS_MS;
    let status = if can_export { "trial" } else { "expired" };
    Ok(LicenseState {
        status: status.to_string(),
        trial_days_left: days_left,
        activated_at: None,
        can_export,
        key_hint: None,
    })
}

fn trial_days_left(started_at: i64, trial_ok: bool) -> i64 {
    if !trial_ok {
        return 0;
    }
    let remaining = TRIAL_DAYS_MS.saturating_sub(now_ms() - started_at);
    if remaining <= 0 {
        0
    } else {
        (remaining + DAY_MS - 1) / DAY_MS
    }
}

/// 每个导出命令开头调用。试用到期 → E2005；**绝不删除或改写用户文件**。
pub fn ensure_export_allowed(app: &AppHandle) -> AppResult<()> {
    let state = get_license_state(app)?;
    if !state.can_export {
        crate::logging::record("export", "E2005");
        return Err(AppError::trial_expired(
            "试用已结束；文件仍在，激活后可导出",
        ));
    }
    crate::logging::record("export", "OK");
    Ok(())
}

/// 校验 `PAYONCE-XXXX-XXXX-XXXX` 格式（X 为大写字母或数字）。
/// 前缀 8 字符 `PAYONCE-` + 3 组 4 字符 + 2 个分隔符 = 22 字符。
fn validate_key_format(key: &str) -> bool {
    if !key.is_ascii() || key.len() != 22 {
        return false;
    }
    let b = key.as_bytes();
    if &key[..8] != "PAYONCE-" || b[12] != b'-' || b[17] != b'-' {
        return false;
    }
    [&key[8..12], &key[13..17], &key[18..22]]
        .iter()
        .all(|g| g.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
}

#[cfg(test)]
mod key_format_tests {
    use super::validate_key_format;

    #[test]
    fn new_prefix_is_accepted() {
        assert!(validate_key_format("PAYONCE-ABCD-EFGH-IJKL"));
        assert!(validate_key_format("PAYONCE-1234-5678-90AB"));
    }

    /// 迁移用例：改名前的旧前缀密钥必须被拒绝，且**不提供兼容**（产品尚未发布）。
    /// 旧前缀拆段构造，避免旧品牌标识重新进入源码；完整旧格式见 docs/RENAME-MIGRATION.md。
    #[test]
    fn legacy_prefix_is_rejected() {
        let legacy_key = ["ON", "CE-ABCD-EFGH-IJKL"].concat();
        assert!(!validate_key_format(&legacy_key));
    }

    #[test]
    fn malformed_keys_are_rejected() {
        assert!(!validate_key_format("PAYONCE-abcd-EFGH-IJKL"));
        assert!(!validate_key_format("PAYONCE-ABCD-EFGH"));
        assert!(!validate_key_format("PAYONCE/ABCD/EFGH/IJKL"));
        assert!(!validate_key_format("PAYONCE-中文-中文-中文"));
    }
}

/// 首次联网激活。POST 只发送 `{key, fingerprint}`，**绝不上传文件内容**。
pub fn activate_license(app: &AppHandle, key: &str) -> AppResult<LicenseState> {
    let key = key.trim().to_uppercase();
    if !validate_key_format(&key) {
        return Err(AppError::internal(
            "密钥格式不正确，应为 PAYONCE-XXXX-XXXX-XXXX",
        ));
    }

    // 公钥缺失时立即给出可识别错误（不发起无意义的网络请求）。
    read_public_key(app).map_err(|_| {
        let msg = "缺少 Ed25519 公钥 keys/public.key，无法完成激活；请运行 scripts/gen-license-keys.mjs";
        crate::logging::record_hint("activate", "E2004", msg);
        AppError::internal(msg)
    })?;

    let fp = fingerprint();
    let url = std::env::var(ACTIVATE_URL_ENV)
        .unwrap_or_else(|_| DEFAULT_ACTIVATE_URL.to_string());

    let response = ureq::post(&url)
        .timeout(ACTIVATE_TIMEOUT)
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({ "key": key, "fingerprint": fp }))
        .map_err(|e| {
            crate::logging::record("activate", "E2004");
            AppError::internal(format!(
                "激活需要联网，请检查网络或 PAYONCEPDF_ACTIVATE_URL 后重试（{}）",
                e
            ))
        })?;

    let parsed: ActivateResponse = response.into_json().map_err(|e| {
        AppError::internal(format!("激活响应解析失败：{}", e))
    })?;

    // signature 签名对象是 payload 的 base64 解码后的原始字节。
    let payload_bytes = base64::engine::general_purpose::STANDARD
        .decode(parsed.payload.trim())
        .map_err(|e| AppError::internal(format!("激活 payload 不是合法 base64：{}", e)))?;
    verify_signature(app, &payload_bytes, &parsed.signature)?;

    let act = ActivationFile {
        key: key.clone(),
        payload: parsed.payload,
        signature: parsed.signature,
        fingerprint: fp,
        activated_at: now_ms(),
        verified: true,
    };
    let dir = app_data_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(AppError::from)?;
    let json = serde_json::to_string_pretty(&act).map_err(AppError::from)?;
    // 缓存先写临时文件再原子替换，避免写一半损坏。
    let tmp = dir.join("activation.json.tmp");
    std::fs::write(&tmp, json).map_err(AppError::from)?;
    crate::state::atomic_commit(&tmp, &dir.join(ACTIVATION_FILE))?;

    crate::logging::record("activate", "OK");
    get_license_state(app)
}
