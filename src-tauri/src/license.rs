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
/// 独立的「本机已启用过试用」标记。删掉 / 写坏 trial.json 也不能重新领 7 天。
const TRIAL_MARKER_FILE: &str = "trial.started";
const TRIAL_MARKER_CONTENT: &[u8] = b"PayOnce PDF trial enabled on this machine\n";
const ACTIVATION_FILE: &str = "activation.json";
/// 编译期内嵌的 Ed25519 公钥（base64 编码的原始 32 字节）。
/// 随二进制分发，删掉外部公钥文件也不会让授权退回「信任本地标记」的兜底路径。
const EMBEDDED_PUBLIC_KEY_B64: &str = include_str!("../keys/public.key");
const ACTIVATE_URL_ENV: &str = "PAYONCEPDF_ACTIVATE_URL";
const DEFAULT_ACTIVATE_URL: &str = "https://payoncepdf.com/api/activate";
const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TrialFile {
    started_at: i64,
    digest: String,
}

/// 离线激活缓存。**不含任何自证字段**：是否有效一律由 Ed25519 签名判定。
/// 旧版缓存里的 `verified` 会被 serde 当作未知字段忽略。
#[derive(Serialize, Deserialize)]
struct ActivationFile {
    key: String,
    payload: String,
    signature: String,
    fingerprint: String,
    activated_at: i64,
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

/// trial.json 的判定结果。抽成纯函数便于单测锁住「删文件不能续期」这条边界。
#[derive(Debug, PartialEq)]
enum TrialDecision {
    /// 真正首次启动（无记录、无启用标记）：允许创建 7 天试用。
    Fresh,
    Valid(TrialFile),
    /// 记录存在但摘要不匹配：保留内容，按过期处理，不改写文件。
    Tampered(TrialFile),
    /// 已启用过试用，但记录缺失 / 读取失败 / JSON 损坏：一律过期，绝不重建。
    Expired,
}

fn decide_trial(raw: Option<&str>, marker_existed: bool, fp: &str) -> TrialDecision {
    match raw {
        Some(text) => match serde_json::from_str::<TrialFile>(text) {
            Ok(t) if t.digest == trial_digest(t.started_at, fp) => TrialDecision::Valid(t),
            Ok(t) => TrialDecision::Tampered(t),
            Err(_) => TrialDecision::Expired,
        },
        None if marker_existed => TrialDecision::Expired,
        None => TrialDecision::Fresh,
    }
}

/// 读取 trial.json；不存在 / 无法解析且本机已有启用标记时按**过期**处理，不再重建。
/// 文件存在但摘要不匹配 → 同样保守拒绝导出。两种情况都**不删除、不改写用户文件**。
fn load_or_create_trial(app: &AppHandle) -> AppResult<(TrialFile, bool)> {
    let dir = app_data_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(AppError::from)?;
    let path = dir.join(TRIAL_FILE);
    let marker = dir.join(TRIAL_MARKER_FILE);
    let fp = fingerprint();

    // 标记幂等写入：老用户升级时（已有 trial.json、无标记）补写一次，
    // 之后删 trial.json 就会命中 Expired 分支。
    let marker_existed = marker.is_file();
    if !marker_existed {
        let _ = std::fs::write(&marker, TRIAL_MARKER_CONTENT);
    }

    let raw = std::fs::read_to_string(&path).ok();
    match decide_trial(raw.as_deref(), marker_existed, &fp) {
        TrialDecision::Fresh => {
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
        TrialDecision::Valid(t) => Ok((t, true)),
        TrialDecision::Tampered(t) => {
            crate::logging::record_hint(
                "trial",
                "E2004",
                "trial.json 摘要不匹配（疑似被篡改），按过期处理",
            );
            Ok((t, false))
        }
        TrialDecision::Expired => {
            crate::logging::record_hint(
                "trial",
                "E2005",
                "trial.json 缺失或损坏，但本机已有试用启用标记，按过期处理（不重建）",
            );
            Ok((
                TrialFile {
                    started_at: 0,
                    digest: String::new(),
                },
                false,
            ))
        }
    }
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

fn decode_public_key(text: &str) -> AppResult<[u8; 32]> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|e| AppError::internal(format!("Ed25519 公钥不是合法 base64：{}", e)))?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| AppError::internal("Ed25519 公钥必须是原始 32 字节"))
}

/// 外部文件优先（便于运维轮换）；缺失 / 损坏时回退到编译期内嵌公钥。
/// 因此发行包永远有可用信任根：删掉 keys/public.key 不会改变验签行为。
fn read_public_key(app: &AppHandle) -> AppResult<[u8; 32]> {
    for p in candidate_key_paths(app) {
        if !p.is_file() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Ok(key) = decode_public_key(&text) {
                return Ok(key);
            }
        }
    }
    decode_public_key(EMBEDDED_PUBLIC_KEY_B64)
}

/// 供 F21 自检 / CI 断言复用：返回 (是否可用, 来源说明)。
pub fn public_key_diagnostic(app: &AppHandle) -> (bool, String) {
    for p in candidate_key_paths(app) {
        if p.is_file() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if decode_public_key(&text).is_ok() {
                    return (true, format!("external: {}", p.display()));
                }
            }
        }
    }
    match decode_public_key(EMBEDDED_PUBLIC_KEY_B64) {
        Ok(_) => (true, "embedded in binary".to_string()),
        Err(e) => (false, format!("内嵌公钥无效：{}", e)),
    }
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

/// 激活令牌的必填契约：payload 必须绑定具体 license key、本机指纹与签发时间。
/// 缺任一字段或不一致 → 拒绝（不再接受「字段存在才校验」的宽松行为）。
fn payload_matches(payload: &[u8], key: &str, fp: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let k = match v.get("key").and_then(|x| x.as_str()) {
        Some(k) => k,
        None => return false,
    };
    let pf = match v.get("fingerprint").and_then(|x| x.as_str()) {
        Some(p) => p,
        None => return false,
    };
    let issued = match v.get("issuedAt").and_then(|x| x.as_str()) {
        Some(i) => i,
        None => return false,
    };
    // issuedAt 只校验存在与非空，不按本机时钟判定（避免时钟漂移误锁已付费用户）。
    if k != key || pf != fp || issued.is_empty() {
        return false;
    }
    // expiresAt 为 null 表示买断永久；若给出时间且已过期则拒绝。
    if let Some(exp) = v.get("expiresAt") {
        if !exp.is_null() {
            let s = match exp.as_str() {
                Some(s) => s,
                None => return false,
            };
            match chrono::DateTime::parse_from_rfc3339(s) {
                Ok(dt) if dt.timestamp_millis() < now_ms() => return false,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    }
    true
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
    // 3) payload 必须完整绑定 key / 指纹 / 签发时间。
    if !payload_matches(&payload_bytes, &act.key, &act.fingerprint) {
        return false;
    }
    // 4) 只认签名。公钥不可用一律拒绝，绝不使用缓存里的「已验签」自证字段。
    verify_signature(app, &payload_bytes, &act.signature).is_ok()
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

    // 公钥必须可用（随包资源或编译期内嵌），否则立即拒绝，不发无意义的网络请求。
    read_public_key(app)?;

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
    // 验签通过后仍要强校验字段契约：防止服务端漏字段的令牌被缓存成「看起来已激活」。
    if !payload_matches(&payload_bytes, &key, &fp) {
        return Err(AppError::internal(
            "激活令牌字段不完整或与当前密钥 / 设备不匹配，请联系支持",
        ));
    }

    let act = ActivationFile {
        key: key.clone(),
        payload: parsed.payload,
        signature: parsed.signature,
        fingerprint: fp,
        activated_at: now_ms(),
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

#[cfg(test)]
mod trial_decision_tests {
    use super::{decide_trial, trial_digest, TrialDecision, TrialFile};

    const FP: &str = "unit-test-fingerprint";
    const START: i64 = 1_700_000_000_000;

    fn valid_json() -> String {
        serde_json::to_string(&TrialFile {
            started_at: START,
            digest: trial_digest(START, FP),
        })
        .unwrap()
    }

    #[test]
    fn first_launch_without_marker_is_fresh() {
        assert_eq!(decide_trial(None, false, FP), TrialDecision::Fresh);
    }

    /// P1-2 回归锁：删掉 trial.json 不能重新领 7 天。
    #[test]
    fn deleted_trial_with_marker_is_expired_not_reset() {
        assert_eq!(decide_trial(None, true, FP), TrialDecision::Expired);
    }

    #[test]
    fn corrupted_json_is_expired_not_reset() {
        assert_eq!(
            decide_trial(Some("{not json"), true, FP),
            TrialDecision::Expired
        );
        assert_eq!(decide_trial(Some(""), false, FP), TrialDecision::Expired);
    }

    #[test]
    fn valid_trial_is_accepted() {
        match decide_trial(Some(&valid_json()), true, FP) {
            TrialDecision::Valid(t) => assert_eq!(t.started_at, START),
            other => panic!("expected Valid, got {:?}", other),
        }
    }

    #[test]
    fn tampered_digest_and_other_machine_are_not_fresh() {
        let mut t: TrialFile = serde_json::from_str(&valid_json()).unwrap();
        t.digest = "deadbeef".to_string();
        let json = serde_json::to_string(&t).unwrap();
        assert!(matches!(
            decide_trial(Some(&json), true, FP),
            TrialDecision::Tampered(_)
        ));
        assert!(matches!(
            decide_trial(Some(&valid_json()), true, "other-machine"),
            TrialDecision::Tampered(_)
        ));
    }
}

#[cfg(test)]
mod payload_contract_tests {
    use super::payload_matches;

    const KEY: &str = "PAYONCE-ABCD-EFGH-IJKL";
    const FP: &str = "fp-123";

    fn full() -> serde_json::Value {
        serde_json::json!({
            "key": KEY,
            "fingerprint": FP,
            "issuedAt": "2026-11-24T00:00:00Z",
            "expiresAt": null,
            "nonce": "abc123"
        })
    }
    fn bytes(v: &serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(v).unwrap()
    }

    #[test]
    fn accepts_full_payload() {
        assert!(payload_matches(&bytes(&full()), KEY, FP));
    }

    /// P1-3 回归锁：旧 mock 那种只有 keyHash 的 payload 必须被拒。
    #[test]
    fn rejects_legacy_payload_without_key() {
        let mut v = full();
        v.as_object_mut().unwrap().remove("key");
        assert!(!payload_matches(&bytes(&v), KEY, FP));
    }

    #[test]
    fn rejects_missing_fingerprint_or_issued_at() {
        let mut v = full();
        v.as_object_mut().unwrap().remove("fingerprint");
        assert!(!payload_matches(&bytes(&v), KEY, FP));
        let mut v = full();
        v.as_object_mut().unwrap().remove("issuedAt");
        assert!(!payload_matches(&bytes(&v), KEY, FP));
    }

    #[test]
    fn rejects_mismatched_key_and_fingerprint() {
        assert!(!payload_matches(&bytes(&full()), "PAYONCE-XXXX-XXXX-XXXX", FP));
        assert!(!payload_matches(&bytes(&full()), KEY, "other-fp"));
    }

    #[test]
    fn rejects_expired_and_malformed_payload() {
        let mut v = full();
        v["expiresAt"] = serde_json::json!("2020-01-01T00:00:00Z");
        assert!(!payload_matches(&bytes(&v), KEY, FP));
        assert!(!payload_matches(b"not json", KEY, FP));
    }
}
