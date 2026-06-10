//! 节点账号两步鉴权（组件二）
//!
//! 与自建 Auth Server（组件一）配合：用 账号密码 + 设备指纹 换取 64 位设备 Token，
//! Token 写入本地 `node-auth.json`，连接时由 `enhance` 注入到 hysteria2 节点的
//! `password` 字段。Token 有效期 7 天，支持基于本地凭据的静默续期。

use crate::utils::dirs;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Duration, Utc};
use clash_verge_logging::{Type, logging};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 本地凭据/Token 存储文件名
const NODE_AUTH_FILE: &str = "node-auth.json";
/// 距离过期小于该阈值（天）时触发静默续期
const RENEW_BEFORE_DAYS: i64 = 2;

/// 本地持久化的鉴权状态（含密码，仅存于本机用于静默续期）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeAuthState {
    /// Auth Server 基址，例如 `https://auth.example.com`
    pub server: String,
    pub username: String,
    /// 用于静默续期的密码（仅本地存储，不回传前端）
    #[serde(default)]
    pub password: String,
    /// 64 位设备 Token
    pub token: String,
    /// 服务端返回的过期时间（ISO-8601）
    #[serde(default)]
    pub expires_at: String,
    pub device_fp: String,
    pub platform: String,
    #[serde(default)]
    pub max_devices: Option<u32>,
    #[serde(default)]
    pub active_devices: Option<u32>,
}

/// 回传前端的状态（不含密码与完整 Token）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeAuthStatus {
    pub logged_in: bool,
    pub server: String,
    pub username: String,
    pub expires_at: String,
    pub device_fp: String,
    pub platform: String,
    pub max_devices: Option<u32>,
    pub active_devices: Option<u32>,
    /// Token 是否已过期
    pub expired: bool,
}

/// Auth Server `POST /login` 的请求体。
#[derive(Debug, Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
    device_fp: &'a str,
    platform: &'a str,
}

/// Auth Server `POST /login` 的响应体。
#[derive(Debug, Deserialize)]
struct LoginResponse {
    token: String,
    #[serde(default)]
    expires_at: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    max_devices: Option<u32>,
    #[serde(default)]
    active_devices: Option<u32>,
}

/// `node-auth.json` 的绝对路径。
fn state_path() -> Result<PathBuf> {
    Ok(dirs::app_home_dir()?.join(NODE_AUTH_FILE))
}

/// 读取本地鉴权状态；不存在或解析失败时返回 `None`。
pub fn load_state() -> Option<NodeAuthState> {
    let path = state_path().ok()?;
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<NodeAuthState>(&text) {
            Ok(state) => Some(state),
            Err(err) => {
                logging!(warn, Type::Config, "解析 node-auth.json 失败: {err}");
                None
            }
        },
        Err(err) => {
            logging!(warn, Type::Config, "读取 node-auth.json 失败: {err}");
            None
        }
    }
}

/// 写入本地鉴权状态。
fn save_state(state: &NodeAuthState) -> Result<()> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let text = serde_json::to_string_pretty(state).context("序列化 node-auth 状态失败")?;
    std::fs::write(&path, text).with_context(|| format!("写入 {} 失败", path.display()))?;
    Ok(())
}

/// 删除本地鉴权状态（登出）。
pub fn clear_state() -> Result<()> {
    let path = state_path()?;
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("删除 {} 失败", path.display()))?;
    }
    Ok(())
}

/// 当前平台标识。
pub const fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// 读取稳定的设备指纹。
///
/// - Windows: 注册表 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`
/// - macOS: `ioreg` 中的 `IOPlatformUUID`
/// - Linux: `/etc/machine-id`（回退 `/var/lib/dbus/machine-id`）
///
/// 任意分支失败时回退为主机名，确保始终返回一个非空指纹。
pub fn device_fingerprint() -> String {
    let raw = read_machine_id().unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        let host = gethostname::gethostname().to_string_lossy().to_string();
        return format!("{}-host-{}", platform_name(), host);
    }
    format!("{}-{}", platform_name(), raw)
}

#[cfg(target_os = "windows")]
fn read_machine_id() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let crypto = hklm
        .open_subkey_with_flags("SOFTWARE\\Microsoft\\Cryptography", KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    let guid: String = crypto.get_value("MachineGuid").ok()?;
    Some(guid)
}

#[cfg(target_os = "macos")]
fn read_machine_id() -> Option<String> {
    let output = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains("IOPlatformUUID")
            && let Some(idx) = line.find('=')
        {
            return Some(line[idx + 1..].trim().trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn read_machine_id() -> Option<String> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(id) = std::fs::read_to_string(path) {
            let id = id.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// 规范化服务器地址：去掉结尾 `/`，缺省补 `https://`。
fn normalize_server(server: &str) -> String {
    let s = server.trim().trim_end_matches('/');
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

/// 调用 Auth Server `/login` 换取 Token 并持久化。
pub async fn login(server: &str, username: &str, password: &str) -> Result<NodeAuthStatus> {
    let server = normalize_server(server);
    let device_fp = device_fingerprint();
    let platform = platform_name();

    let url = format!("{server}/login");
    let body = LoginRequest {
        username,
        password,
        device_fp: &device_fp,
        platform,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("构建 HTTP 客户端失败")?;

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("请求 {url} 失败"))?;

    let status = resp.status();
    if !status.is_success() {
        let msg = resp.text().await.unwrap_or_default();
        anyhow::bail!("登录失败（HTTP {}）：{}", status.as_u16(), msg.trim());
    }

    let parsed: LoginResponse = resp.json().await.context("解析登录响应失败")?;
    if parsed.token.is_empty() {
        anyhow::bail!("登录响应缺少 token");
    }

    let state = NodeAuthState {
        server,
        username: if parsed.username.is_empty() {
            username.to_string()
        } else {
            parsed.username
        },
        password: password.to_string(),
        token: parsed.token,
        expires_at: parsed.expires_at,
        device_fp,
        platform: platform.to_string(),
        max_devices: parsed.max_devices,
        active_devices: parsed.active_devices,
    };
    save_state(&state)?;
    logging!(info, Type::Config, "节点账号登录成功: {}", state.username);
    Ok(to_status(&state))
}

/// 返回当前登录状态（供前端展示）。
pub fn status() -> NodeAuthStatus {
    match load_state() {
        Some(state) => to_status(&state),
        None => NodeAuthStatus::default(),
    }
}

/// 登出并清除本地凭据。
pub fn logout() -> Result<()> {
    clear_state()?;
    logging!(info, Type::Config, "节点账号已登出");
    Ok(())
}

/// 若 Token 即将过期且本地存有凭据，则静默重新登录续期。
/// 返回是否实际续期。
pub async fn renew_if_needed() -> Result<bool> {
    let Some(state) = load_state() else {
        return Ok(false);
    };
    if state.password.is_empty() {
        return Ok(false);
    }
    if !needs_renew(&state.expires_at) {
        return Ok(false);
    }
    logging!(info, Type::Config, "节点 Token 临近过期，尝试静默续期");
    login(&state.server, &state.username, &state.password).await?;
    Ok(true)
}

/// 供 `enhance` 同步读取当前 Token（无有效凭据时返回 `None`）。
pub fn current_token() -> Option<String> {
    let state = load_state()?;
    if state.token.is_empty() || is_expired(&state.expires_at) {
        return None;
    }
    Some(state.token)
}

fn to_status(state: &NodeAuthState) -> NodeAuthStatus {
    NodeAuthStatus {
        logged_in: !state.token.is_empty(),
        server: state.server.clone(),
        username: state.username.clone(),
        expires_at: state.expires_at.clone(),
        device_fp: state.device_fp.clone(),
        platform: state.platform.clone(),
        max_devices: state.max_devices,
        active_devices: state.active_devices,
        expired: is_expired(&state.expires_at),
    }
}

fn parse_expiry(expires_at: &str) -> Option<DateTime<Utc>> {
    if expires_at.is_empty() {
        return None;
    }
    // 服务端可能返回带或不带时区的 ISO 时间，逐一尝试。
    if let Ok(dt) = DateTime::parse_from_rfc3339(expires_at) {
        return Some(dt.with_timezone(&Utc));
    }
    chrono::NaiveDateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

fn is_expired(expires_at: &str) -> bool {
    match parse_expiry(expires_at) {
        Some(exp) => Utc::now() >= exp,
        // 无法解析过期时间时按未过期处理，避免误删可用 Token。
        None => false,
    }
}

fn needs_renew(expires_at: &str) -> bool {
    match parse_expiry(expires_at) {
        Some(exp) => Utc::now() + Duration::days(RENEW_BEFORE_DAYS) >= exp,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_server_adds_scheme_and_trims() {
        assert_eq!(normalize_server("auth.example.com/"), "https://auth.example.com");
        assert_eq!(normalize_server("http://1.2.3.4:9000"), "http://1.2.3.4:9000");
        assert_eq!(normalize_server(" https://a.com/ "), "https://a.com");
    }

    #[test]
    fn device_fingerprint_is_non_empty_and_prefixed() {
        let fp = device_fingerprint();
        assert!(!fp.is_empty());
        assert!(fp.starts_with(platform_name()));
    }

    #[test]
    fn expiry_helpers_handle_past_and_future() {
        let past = (Utc::now() - Duration::days(1)).to_rfc3339();
        let future = (Utc::now() + Duration::days(7)).to_rfc3339();
        let soon = (Utc::now() + Duration::days(1)).to_rfc3339();
        assert!(is_expired(&past));
        assert!(!is_expired(&future));
        assert!(needs_renew(&soon));
        assert!(!needs_renew(&future));
        // 无法解析视为未过期/不续期
        assert!(!is_expired(""));
        assert!(!needs_renew("garbage"));
    }
}
