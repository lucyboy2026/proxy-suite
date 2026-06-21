//! Telegram Bot 通知与授权回调辅助。未配置时仅记录日志。

use crate::config::TelegramConfig;
use anyhow::{Context, Result};
use serde_json::json;

fn api(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{token}/{method}")
}

/// 给管理员发送纯文本消息。
pub async fn send_admin(cfg: &TelegramConfig, text: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .post(api(&cfg.bot_token, "sendMessage"))
        .json(&json!({
            "chat_id": cfg.admin_chat_id,
            "text": text,
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        }))
        .send()
        .await
        .context("调用 Telegram sendMessage 失败")?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram 返回错误: {body}");
    }
    Ok(())
}

/// 给管理员发送带「同意 / 拒绝」内联按钮的注册审批消息。
/// 回调数据格式：`approve:<user_id>` / `reject:<user_id>`。
pub async fn send_approval_request(
    cfg: &TelegramConfig,
    user_id: i64,
    email: &str,
    device_fp: &str,
    platform: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let text = format!(
        "🆕 <b>新设备注册申请</b>\n邮箱: <code>{email}</code>\n平台: {platform}\n设备指纹: <code>{device_fp}</code>\n\n点击下方按钮授权（默认设备数/期限取服务端默认值），或在 Web 后台精细授权。"
    );
    let resp = client
        .post(api(&cfg.bot_token, "sendMessage"))
        .json(&json!({
            "chat_id": cfg.admin_chat_id,
            "text": text,
            "parse_mode": "HTML",
            "reply_markup": {
                "inline_keyboard": [[
                    {"text": "✅ 同意", "callback_data": format!("approve:{user_id}")},
                    {"text": "❌ 拒绝", "callback_data": format!("reject:{user_id}")}
                ]]
            }
        }))
        .send()
        .await
        .context("调用 Telegram sendMessage 失败")?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram 返回错误: {body}");
    }
    Ok(())
}

/// 应答 callback_query（消除按钮转圈）。
pub async fn answer_callback(cfg: &TelegramConfig, callback_id: &str, text: &str) -> Result<()> {
    let client = reqwest::Client::new();
    client
        .post(api(&cfg.bot_token, "answerCallbackQuery"))
        .json(&json!({ "callback_query_id": callback_id, "text": text }))
        .send()
        .await
        .context("调用 Telegram answerCallbackQuery 失败")?;
    Ok(())
}
