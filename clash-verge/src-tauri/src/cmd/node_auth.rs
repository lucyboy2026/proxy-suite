//! 节点账号两步鉴权相关命令（组件二）

use super::{CmdResult, StringifyErr as _};
use crate::feat::node_auth::{self, NodeAuthRegisterResult, NodeAuthStatus};

/// 读取当前设备指纹（用于前端展示）。
#[tauri::command]
pub fn node_auth_get_device_fp() -> String {
    node_auth::device_fingerprint()
}

/// 返回当前节点账号登录状态。
#[tauri::command]
pub fn node_auth_get_status() -> NodeAuthStatus {
    node_auth::status()
}

/// 账号密码 + 设备指纹注册新账号（创建后待管理员授权）。
#[tauri::command]
pub async fn node_auth_register(
    server: String,
    username: String,
    password: String,
) -> CmdResult<NodeAuthRegisterResult> {
    node_auth::register(&server, &username, &password).await.stringify_err()
}

/// 账号密码 + 设备指纹登录，成功后持久化 Token。
#[tauri::command]
pub async fn node_auth_login(server: String, username: String, password: String) -> CmdResult<NodeAuthStatus> {
    node_auth::login(&server, &username, &password).await.stringify_err()
}

/// 登出并清除本地凭据。
#[tauri::command]
pub fn node_auth_logout() -> CmdResult<()> {
    node_auth::logout().stringify_err()
}

/// 主动触发一次「临近过期则续期」。
#[tauri::command]
pub async fn node_auth_renew() -> CmdResult<bool> {
    node_auth::renew_if_needed().await.stringify_err()
}
