//! 高层通知：注册申请 / 授权完成，自动走 Telegram + 邮件，并记录事件。

use crate::config::Config;
use crate::models::User;
use crate::{db, email, telegram};
use sqlx::SqlitePool;

/// 新注册/新设备申请：通知管理员（Telegram 审批按钮 + 邮件）。
pub async fn on_new_registration(
    cfg: &Config,
    pool: &SqlitePool,
    user_id: i64,
    email_addr: &str,
    device_fp: &str,
    platform: &str,
) {
    db::log_event(
        pool,
        "register",
        Some(email_addr),
        &format!("platform={platform} device_fp={device_fp}"),
    )
    .await
    .ok();

    let mut delivered = false;

    if let Some(tg) = &cfg.telegram {
        match telegram::send_approval_request(tg, user_id, email_addr, device_fp, platform).await {
            Ok(()) => delivered = true,
            Err(e) => tracing::warn!("Telegram 通知失败: {e:#}"),
        }
    }

    if let Some(smtp) = &cfg.smtp {
        let subject = format!("[Clash Verge] 新设备注册申请: {email_addr}");
        let body = format!(
            "收到新的设备注册/绑定申请：\n\n邮箱: {email_addr}\n平台: {platform}\n设备指纹: {device_fp}\n\n请到后台审批: {}/admin/users\n",
            cfg.public_base_url
        );
        match email::send(smtp, &smtp.admin_to, &subject, &body).await {
            Ok(()) => delivered = true,
            Err(e) => tracing::warn!("管理员邮件通知失败: {e:#}"),
        }
    }

    if !delivered {
        tracing::warn!("未配置任何通知渠道（Telegram/SMTP），注册申请仅记录在数据库：{email_addr}");
    }
}

/// 授权完成：邮件通知用户其期限与可绑定设备数。
pub async fn on_user_authorized(cfg: &Config, pool: &SqlitePool, user: &User) {
    db::log_event(
        pool,
        "authorize",
        Some(&user.email),
        &format!(
            "max_devices={} expires_at={}",
            user.max_devices,
            user.expires_at.as_deref().unwrap_or("-")
        ),
    )
    .await
    .ok();

    if let Some(smtp) = &cfg.smtp {
        let subject = "[Clash Verge] 你的账号已开通".to_string();
        let body = format!(
            "你的账号已通过审核并开通：\n\n邮箱: {}\n可绑定设备数: {}\n有效期至: {}\n\n请在客户端使用该邮箱与密码登录，登录后将自动拉取订阅。\n服务器地址: {}\n",
            user.email,
            user.max_devices,
            user.expires_at.as_deref().unwrap_or("长期"),
            cfg.public_base_url
        );
        if let Err(e) = email::send(smtp, &user.email, &subject, &body).await {
            tracing::warn!("用户开通邮件失败: {e:#}");
        }
    }

    if let Some(tg) = &cfg.telegram {
        let text = format!(
            "✅ 已开通用户 <code>{}</code>\n设备数: {} | 到期: {}",
            user.email,
            user.max_devices,
            user.expires_at.as_deref().unwrap_or("长期")
        );
        if let Err(e) = telegram::send_admin(tg, &text).await {
            tracing::warn!("Telegram 开通通知失败: {e:#}");
        }
    }
}
