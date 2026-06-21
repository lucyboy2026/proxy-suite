//! Telegram Webhook：处理管理员点击「同意 / 拒绝」内联按钮。
//!
//! 部署时需调用一次 `setWebhook` 指向 `<PUBLIC_BASE_URL>/tg/webhook`。

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Duration, Utc};
use serde::Deserialize;

use crate::models::find_user_by_id;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct Update {
    #[serde(default)]
    callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    id: String,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    from: Option<TgUser>,
}

#[derive(Debug, Deserialize)]
pub struct TgUser {
    id: i64,
}

/// POST /tg/webhook
pub async fn webhook(State(state): State<AppState>, Json(update): Json<Update>) -> impl IntoResponse {
    let Some(cfg) = state.cfg.telegram.clone() else {
        return Json(serde_json::json!({"ok": true}));
    };
    let Some(cb) = update.callback_query else {
        return Json(serde_json::json!({"ok": true}));
    };

    // 只接受来自管理员 chat 的操作
    let from_admin = cb
        .from
        .as_ref()
        .map(|u| u.id.to_string() == cfg.admin_chat_id)
        .unwrap_or(false);
    let data = cb.data.unwrap_or_default();

    let mut reply = "已处理".to_string();
    if from_admin {
        if let Some(id_str) = data.strip_prefix("approve:") {
            if let Ok(id) = id_str.parse::<i64>() {
                let expires_at = (Utc::now() + Duration::days(state.cfg.default_valid_days)).to_rfc3339();
                let now = Utc::now().to_rfc3339();
                sqlx::query(
                    "UPDATE users SET status='active', max_devices=?, expires_at=?, authorized_at=? WHERE id=?",
                )
                .bind(state.cfg.default_max_devices as i64)
                .bind(&expires_at)
                .bind(&now)
                .bind(id)
                .execute(&state.pool)
                .await
                .ok();
                if let Ok(Some(user)) = find_user_by_id(&state.pool, id).await {
                    crate::notify::on_user_authorized(&state.cfg, &state.pool, &user).await;
                }
                reply = "✅ 已授权".to_string();
            }
        } else if let Some(id_str) = data.strip_prefix("reject:") {
            if let Ok(id) = id_str.parse::<i64>() {
                sqlx::query("UPDATE users SET status='suspended' WHERE id=?")
                    .bind(id)
                    .execute(&state.pool)
                    .await
                    .ok();
                reply = "❌ 已拒绝".to_string();
            }
        }
    } else {
        reply = "无权限".to_string();
    }

    crate::telegram::answer_callback(&cfg, &cb.id, &reply).await.ok();
    Json(serde_json::json!({"ok": true}))
}
