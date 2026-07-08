//! 客户端 API：注册 / 登录 / 拉取订阅 / hysteria2 鉴权回调。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::{gen_token, hash_password, verify_password};
use crate::clash;
use crate::error::{AppError, AppResult};
use crate::models::{
    consume_verification_token, count_user_devices, ensure_subscription_key, find_device, find_device_by_token,
    find_user_by_email, find_user_by_id, find_user_by_subscription_key, latest_active_token, list_user_devices,
    parse_dt, upsert_verification_token,
};
use crate::notify;
use crate::online::client_ip;
use crate::state::AppState;
use std::time::Duration as StdDuration;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub device_fp: String,
    #[serde(default)]
    pub platform: String,
}

impl RegisterRequest {
    fn email(&self) -> Option<&str> {
        self.email
            .as_deref()
            .or(self.username.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// POST /register
pub async fn register(State(state): State<AppState>, Json(req): Json<RegisterRequest>) -> AppResult<impl IntoResponse> {
    let email = req
        .email()
        .ok_or_else(|| AppError::bad_request("缺少邮箱"))?
        .to_lowercase();
    if !email.contains('@') {
        return Err(AppError::bad_request("邮箱格式不正确"));
    }
    if req.password.len() < 6 {
        return Err(AppError::bad_request("密码至少 6 位"));
    }
    if req.device_fp.trim().is_empty() {
        return Err(AppError::bad_request("缺少设备指纹"));
    }
    let platform = if req.platform.is_empty() {
        "unknown"
    } else {
        &req.platform
    };

    let pool = &state.pool;

    if let Some(user) = find_user_by_email(pool, &email).await? {
        upsert_device(pool, user.id, &req.device_fp, platform).await?;
        // 邮箱尚未验证：重发验证邮件，不打扰管理员。
        if !user.is_email_verified() && state.cfg.smtp.is_some() {
            let token = upsert_verification_token(pool, user.id, VERIFY_TTL_HOURS).await?;
            notify::send_verification_email(&state.cfg, pool, &email, &token).await;
            return Ok((
                StatusCode::ACCEPTED,
                Json(json!({
                    "status": "pending_verification",
                    "message": "该邮箱尚未验证，已重新发送验证邮件，请查收并点击邮件中的链接。"
                })),
            ));
        }
        // 已存在：根据状态返回提示，并把新设备指纹登记 + 通知管理员。
        notify::on_new_registration(&state.cfg, pool, user.id, &email, &req.device_fp, platform).await;
        let msg = match user.status.as_str() {
            "active" => "账号已存在，请直接登录；如需增加设备，已通知管理员审核。",
            "suspended" => "账号已被停用，请联系管理员。",
            _ => "账号已在审核中，请等待管理员授权。",
        };
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({ "status": user.status, "message": msg })),
        ));
    }

    let password_hash = hash_password(&req.password)?;
    let now = Utc::now().to_rfc3339();
    // 配置了 SMTP 时要求先验证邮箱；未配置则退化为免验证直接进入审核。
    let require_verify = state.cfg.smtp.is_some();
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users(email, password_hash, status, max_devices, email_verified, created_at)
         VALUES(?, ?, 'pending', ?, ?, ?) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(state.cfg.default_max_devices as i64)
    .bind(if require_verify { 0i64 } else { 1i64 })
    .bind(&now)
    .fetch_one(pool)
    .await?;

    upsert_device(pool, user_id, &req.device_fp, platform).await?;

    if require_verify {
        let token = upsert_verification_token(pool, user_id, VERIFY_TTL_HOURS).await?;
        notify::send_verification_email(&state.cfg, pool, &email, &token).await;
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "pending_verification",
                "message": "注册成功，验证邮件已发送，请在 24 小时内点击邮件中的链接完成验证。"
            })),
        ));
    }

    notify::on_new_registration(&state.cfg, pool, user_id, &email, &req.device_fp, platform).await;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "pending",
            "message": "注册成功，已通知管理员审核。授权后将邮件通知你。"
        })),
    ))
}

/// 邮箱验证令牌有效期（小时）
const VERIFY_TTL_HOURS: i64 = 24;

#[derive(Debug, Deserialize)]
pub struct VerifyQuery {
    pub token: Option<String>,
}

/// GET /verify?token=... —— 邮箱验证链接（邮件里点开，浏览器访问）。
pub async fn verify_email(
    State(state): State<AppState>,
    Query(q): Query<VerifyQuery>,
) -> AppResult<axum::response::Html<String>> {
    let token = q
        .token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("缺少验证令牌"))?;

    let pool = &state.pool;
    let user_id = consume_verification_token(pool, token)
        .await?
        .ok_or_else(|| AppError::bad_request("验证链接无效或已过期，请在客户端重新注册以获取新邮件"))?;
    let user = find_user_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::bad_request("用户不存在"))?;

    // 邮箱验证通过后才通知管理员审核，避免假邮箱刷屏。
    let (fp, platform) = list_user_devices(pool, user_id)
        .await
        .unwrap_or_default()
        .last()
        .map(|d| (d.device_fp.clone(), d.platform.clone().unwrap_or_default()))
        .unwrap_or_default();
    notify::on_new_registration(&state.cfg, pool, user.id, &user.email, &fp, &platform).await;

    Ok(axum::response::Html(format!(
        "<!doctype html><html lang=\"zh\"><head><meta charset=\"utf-8\"><title>邮箱验证成功</title></head>\
         <body style=\"font-family:sans-serif;max-width:32em;margin:4em auto;text-align:center\">\
         <h2>✅ 邮箱验证成功</h2>\
         <p>{} 已完成验证。</p>\
         <p>管理员审核开通后会邮件通知你，届时在客户端登录即可。</p>\
         </body></html>",
        user.email
    )))
}

async fn upsert_device(pool: &sqlx::SqlitePool, user_id: i64, device_fp: &str, platform: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO devices(user_id, device_fp, platform, created_at, last_seen_at)
         VALUES(?, ?, ?, ?, ?)
         ON CONFLICT(user_id, device_fp)
         DO UPDATE SET platform = excluded.platform, last_seen_at = excluded.last_seen_at",
    )
    .bind(user_id)
    .bind(device_fp)
    .bind(platform)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub device_fp: String,
    #[serde(default)]
    pub platform: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    /// Token 过期时间（驱动客户端静默续期，<= 7 天）
    pub expires_at: String,
    pub username: String,
    pub max_devices: u32,
    pub active_devices: u32,
    /// 账号到期时间（用于展示「使用期限」），长期为空
    pub account_expires_at: Option<String>,
    /// 固定订阅链接（客户端登录后据此自动拉取 Clash 订阅）
    pub subscription_url: String,
}

/// POST /login
pub async fn login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> AppResult<Json<LoginResponse>> {
    let email = req
        .email
        .as_deref()
        .or(req.username.as_deref())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("缺少邮箱"))?;
    if req.device_fp.trim().is_empty() {
        return Err(AppError::bad_request("缺少设备指纹"));
    }
    let platform = if req.platform.is_empty() {
        "unknown"
    } else {
        &req.platform
    };

    let pool = &state.pool;
    let user = find_user_by_email(pool, &email)
        .await?
        .ok_or_else(|| AppError::unauthorized("用户名或密码错误"))?;

    if !verify_password(&req.password, &user.password_hash) {
        return Err(AppError::unauthorized("用户名或密码错误"));
    }
    if !user.is_email_verified() {
        return Err(AppError::forbidden("邮箱尚未验证，请先点击验证邮件中的链接（可重新注册以重发邮件）"));
    }
    match user.status.as_str() {
        "active" => {}
        "suspended" => return Err(AppError::forbidden("账号已被停用，请联系管理员")),
        _ => return Err(AppError::forbidden("账号待管理员审核授权")),
    }
    if user.is_expired() {
        return Err(AppError::forbidden("账号已过期，请联系管理员续期"));
    }

    // 设备绑定
    let now = Utc::now();
    let existing = find_device(pool, user.id, &req.device_fp).await?;
    if let Some(d) = &existing {
        if d.revoked != 0 {
            return Err(AppError::forbidden("该设备已被禁用，请联系管理员"));
        }
    } else {
        let count = count_user_devices(pool, user.id).await?;
        if count >= user.max_devices {
            return Err(AppError::forbidden(format!(
                "已达可绑定设备上限（{}台），请在其它设备登出或联系管理员",
                user.max_devices
            )));
        }
    }

    // Token 过期时间 = min(now + token_ttl, 账号到期)
    let token_cap = now + Duration::days(state.cfg.token_ttl_days);
    let token_exp = match user.expires_at.as_deref().and_then(parse_dt) {
        Some(acc) if acc < token_cap => acc,
        _ => token_cap,
    };
    let token = gen_token();
    let token_exp_str = token_exp.to_rfc3339();
    let now_str = now.to_rfc3339();

    if existing.is_some() {
        sqlx::query(
            "UPDATE devices SET token = ?, token_expires_at = ?, platform = ?, last_seen_at = ?
             WHERE user_id = ? AND device_fp = ?",
        )
        .bind(&token)
        .bind(&token_exp_str)
        .bind(platform)
        .bind(&now_str)
        .bind(user.id)
        .bind(&req.device_fp)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO devices(user_id, device_fp, platform, token, token_expires_at, created_at, last_seen_at)
             VALUES(?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user.id)
        .bind(&req.device_fp)
        .bind(platform)
        .bind(&token)
        .bind(&token_exp_str)
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await?;
    }

    let active_devices = count_user_devices(pool, user.id).await? as u32;
    let sub_key = ensure_subscription_key(pool, user.id).await?;
    // 订阅链接携带本设备指纹，供 `/sub/:key` 校验设备绑定
    let subscription_url = format!(
        "{}/sub/{}?fp={}",
        state.cfg.public_base_url.trim_end_matches('/'),
        sub_key,
        urlencode(&req.device_fp)
    );

    Ok(Json(LoginResponse {
        token,
        expires_at: token_exp_str,
        username: user.email.clone(),
        max_devices: user.max_devices as u32,
        active_devices,
        account_expires_at: user.expires_at.clone(),
        subscription_url,
    }))
}

/// 最小 percent-encode：保留字母数字与 `-_.~`，其余字节转 `%XX`。
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Debug, Deserialize)]
pub struct ConfigQuery {
    pub token: Option<String>,
}

/// GET /config?token=...  （或 Authorization: Bearer <token>）
/// 返回该用户的 Clash YAML（hysteria2 password 注入设备 token）。
pub async fn get_config(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<ConfigQuery>,
) -> AppResult<impl IntoResponse> {
    let token = q
        .token
        .clone()
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        })
        .ok_or_else(|| AppError::unauthorized("缺少 token"))?;

    let pool = &state.pool;
    let device = find_device_by_token(pool, &token)
        .await?
        .ok_or_else(|| AppError::unauthorized("token 无效"))?;

    if let Some(exp) = device.token_expires_at.as_deref().and_then(parse_dt) {
        if Utc::now() >= exp {
            return Err(AppError::unauthorized("token 已过期，请重新登录"));
        }
    }
    let user = find_user_by_id(pool, device.user_id)
        .await?
        .ok_or_else(|| AppError::unauthorized("用户不存在"))?;
    if !user.is_active() || user.is_expired() {
        return Err(AppError::forbidden("账号不可用"));
    }

    let template = clash::get_template(pool).await?;
    let yaml = clash::render(&template, &token);

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/x-yaml; charset=utf-8")],
        yaml,
    ))
}

/// GET /sub/:key —— 长期固定的订阅链接。
///
/// 与 `/config?token=` 不同，这里用「不随 7 天 Token 轮换而失效」的 `subscription_key`
/// 寻址，服务端自动注入该用户当前最近活跃设备的 Token。客户端导入一次即可长期自动更新；
/// 实际连接时客户端 `enhance` 还会用本机最新 Token 覆盖 password，故订阅内的 Token 仅作占位。
#[derive(Debug, Deserialize)]
pub struct SubQuery {
    /// 请求方设备指纹；开启 `sub_require_fp` 时必须是该账号已绑定的设备。
    pub fp: Option<String>,
}

pub async fn get_subscription(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<SubQuery>,
) -> AppResult<impl IntoResponse> {
    let pool = &state.pool;
    let user = find_user_by_subscription_key(pool, key.trim())
        .await?
        .ok_or_else(|| AppError::unauthorized("订阅链接无效"))?;
    if !user.is_active() {
        return Err(AppError::forbidden("账号待授权或已停用"));
    }
    if user.is_expired() {
        return Err(AppError::forbidden("账号已过期，请联系管理员续期"));
    }

    // 设备绑定校验：订阅只能在该账号已登录过的设备上拉取，链接外传无效。
    let fp = q.fp.as_deref().map(str::trim).unwrap_or_default();
    let bound_device = if fp.is_empty() {
        if state.cfg.sub_require_fp {
            return Err(AppError::unauthorized("订阅链接缺少设备标识，请在客户端登录后自动导入"));
        }
        None
    } else {
        let device = find_device(pool, user.id, fp)
            .await?
            .filter(|d| d.revoked == 0)
            .ok_or_else(|| AppError::forbidden("该设备未绑定此账号，请先在客户端登录"))?;
        Some(device)
    };

    // 优先注入请求设备自己的有效 Token，否则退回最近活跃设备的 Token；
    // 尚无可用 Token 时留空，连接时由客户端 enhance 注入。
    let device_token = bound_device.and_then(|d| {
        let valid = match d.token_expires_at.as_deref().and_then(parse_dt) {
            Some(exp) => Utc::now() < exp,
            None => true,
        };
        d.token.filter(|t| !t.is_empty() && valid)
    });
    let token = match device_token {
        Some(t) => t,
        None => latest_active_token(pool, user.id).await?.unwrap_or_default(),
    };
    let template = clash::get_template(pool).await?;
    let yaml = clash::render(&template, &token);

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/x-yaml; charset=utf-8")],
        yaml,
    ))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // addr/tx 为 hysteria 协议字段，保留以兼容请求体，暂不使用
pub struct HysteriaAuthRequest {
    #[serde(default)]
    pub addr: String,
    /// hysteria2 (v2) 用 `auth` 字段携带凭据（即我们注入的 token）
    #[serde(default)]
    pub auth: Option<String>,
    /// hysteria v1 用 `payload`
    #[serde(default)]
    pub payload: Option<String>,
    #[serde(default)]
    pub tx: Option<u64>,
}

/// POST /auth  —— hysteria2 HTTP 鉴权后端回调。始终返回 200。
pub async fn hysteria_auth(State(state): State<AppState>, Json(req): Json<HysteriaAuthRequest>) -> impl IntoResponse {
    let token = req.auth.or(req.payload).unwrap_or_default();
    let ok_resp = |ok: bool, id: &str, msg: &str| Json(json!({ "ok": ok, "id": id, "msg": msg }));

    if token.is_empty() {
        return ok_resp(false, "", "缺少凭据");
    }
    let pool = &state.pool;
    let device = match find_device_by_token(pool, &token).await {
        Ok(Some(d)) => d,
        Ok(None) => return ok_resp(false, "", "token 无效"),
        Err(e) => {
            tracing::error!("hysteria_auth 查询失败: {e:#}");
            return ok_resp(false, "", "服务端错误");
        }
    };
    if let Some(exp) = device.token_expires_at.as_deref().and_then(parse_dt) {
        if Utc::now() >= exp {
            return ok_resp(false, "", "token 已过期");
        }
    }
    let user = match find_user_by_id(pool, device.user_id).await {
        Ok(Some(u)) => u,
        _ => return ok_resp(false, "", "用户不存在"),
    };
    if !user.is_active() || user.is_expired() {
        return ok_resp(false, "", "账号不可用");
    }
    // 同时在线 IP 限制：防止同一 Token 被转发后多地同时使用。
    // 限的是「同时在线的不同来源 IP 数」，动态 IP 切换不受影响。
    let limit = state.cfg.max_online_ips as usize;
    let ip = client_ip(&req.addr);
    if limit > 0 && !ip.is_empty() {
        let ttl = StdDuration::from_secs(state.cfg.online_ip_ttl_secs);
        if !state.online.admit(&token, ip, limit, ttl) {
            tracing::warn!("token 同时在线 IP 超限: user={} ip={}", user.email, ip);
            return ok_resp(false, "", "同时在线设备过多，请稍后再试或联系管理员");
        }
    }
    // 标识用「邮箱#设备指纹」，便于服务端按用户限速/统计。
    let id = format!("{}#{}", user.email, device.device_fp);
    ok_resp(true, &id, "")
}
