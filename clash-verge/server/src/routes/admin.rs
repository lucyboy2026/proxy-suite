//! Web 后台管理平台（服务端渲染 HTML）。

use axum::extract::{Form, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use chrono::{Duration, Utc};
use serde::Deserialize;

use crate::clash;
use crate::models::{find_user_by_id, list_user_devices, list_users};
use crate::state::AppState;

const COOKIE: &str = "sid";

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| {
        let kv = kv.trim();
        let (k, v) = kv.split_once('=')?;
        if k == name {
            Some(v.to_string())
        } else {
            None
        }
    })
}

fn is_authed(state: &AppState, headers: &HeaderMap) -> bool {
    cookie_value(headers, COOKIE)
        .map(|sid| state.sessions.validate(&sid))
        .unwrap_or(false)
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root{{--bg:#0f172a;--card:#1e293b;--fg:#e2e8f0;--muted:#94a3b8;--accent:#3b82f6;--ok:#22c55e;--warn:#f59e0b;--bad:#ef4444;}}
*{{box-sizing:border-box}}
body{{margin:0;font-family:system-ui,-apple-system,Segoe UI,Roboto,Arial,"PingFang SC","Microsoft YaHei";background:var(--bg);color:var(--fg);}}
header{{padding:16px 24px;background:var(--card);display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid #334155}}
header h1{{font-size:18px;margin:0}}
main{{padding:24px;max-width:1100px;margin:0 auto}}
.cards{{display:flex;gap:16px;flex-wrap:wrap;margin-bottom:24px}}
.card{{background:var(--card);border:1px solid #334155;border-radius:12px;padding:16px 20px;min-width:140px}}
.card .n{{font-size:28px;font-weight:700}}
.card .l{{color:var(--muted);font-size:13px;margin-top:4px}}
table{{width:100%;border-collapse:collapse;background:var(--card);border-radius:12px;overflow:hidden}}
th,td{{padding:10px 12px;text-align:left;border-bottom:1px solid #334155;font-size:14px;vertical-align:middle}}
th{{color:var(--muted);font-weight:600}}
.badge{{padding:2px 8px;border-radius:999px;font-size:12px}}
.b-active{{background:rgba(34,197,94,.15);color:var(--ok)}}
.b-pending{{background:rgba(245,158,11,.15);color:var(--warn)}}
.b-suspended{{background:rgba(239,68,68,.15);color:var(--bad)}}
form.inline{{display:inline}}
input,textarea,button{{font-family:inherit;font-size:13px;border-radius:8px;border:1px solid #475569;background:#0b1220;color:var(--fg);padding:6px 8px}}
input[type=number]{{width:64px}}
button{{cursor:pointer;background:var(--accent);border-color:var(--accent);color:#fff;padding:6px 12px}}
button.ghost{{background:transparent;border-color:#475569;color:var(--fg)}}
button.danger{{background:var(--bad);border-color:var(--bad)}}
a{{color:var(--accent);text-decoration:none}}
.row-actions{{display:flex;gap:6px;flex-wrap:wrap;align-items:center}}
textarea{{width:100%;min-height:420px;font-family:ui-monospace,Menlo,Consolas,monospace}}
.muted{{color:var(--muted);font-size:12px}}
</style></head>
<body>{body}</body></html>"#
    ))
}

fn nav() -> String {
    r#"<header><h1>Clash Verge · 设备授权后台</h1>
<nav class="row-actions">
<a href="/admin">概览/用户</a>
<a href="/admin/template">订阅模板</a>
<form class="inline" method="post" action="/admin/logout"><button class="ghost">登出</button></form>
</nav></header>"#
        .to_string()
}

// ---------------- 登录 ----------------

pub async fn login_page() -> Html<String> {
    let body = r#"<main style="max-width:360px">
<h2>管理员登录</h2>
<form method="post" action="/admin/login">
<p><input name="username" placeholder="用户名" style="width:100%" required></p>
<p><input name="password" type="password" placeholder="密码" style="width:100%" required></p>
<p><button type="submit" style="width:100%">登录</button></p>
</form></main>"#;
    page("登录", body)
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

pub async fn login_submit(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    if form.username == state.cfg.admin_username && form.password == state.cfg.admin_password {
        let sid = state.sessions.create();
        let cookie = format!("{COOKIE}={sid}; HttpOnly; Path=/; SameSite=Lax; Max-Age=604800");
        ([(header::SET_COOKIE, cookie)], Redirect::to("/admin")).into_response()
    } else {
        let body = r#"<main style="max-width:360px"><h2>管理员登录</h2>
<p style="color:#ef4444">用户名或密码错误</p>
<form method="post" action="/admin/login">
<p><input name="username" placeholder="用户名" style="width:100%" required></p>
<p><input name="password" type="password" placeholder="密码" style="width:100%" required></p>
<p><button type="submit" style="width:100%">登录</button></p>
</form></main>"#;
        page("登录", body).into_response()
    }
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(sid) = cookie_value(&headers, COOKIE) {
        state.sessions.destroy(&sid);
    }
    let cookie = format!("{COOKIE}=; HttpOnly; Path=/; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], Redirect::to("/admin/login")).into_response()
}

// ---------------- 概览 / 用户列表 ----------------

pub async fn dashboard(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    match render_dashboard(&state).await {
        Ok(html) => html.into_response(),
        Err(e) => {
            tracing::error!("dashboard 渲染失败: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, "内部错误").into_response()
        }
    }
}

async fn render_dashboard(state: &AppState) -> anyhow::Result<Html<String>> {
    let users = list_users(&state.pool).await?;
    let total = users.len();
    let active = users.iter().filter(|u| u.status == "active" && !u.is_expired()).count();
    let pending = users.iter().filter(|u| u.status == "pending").count();
    let expired = users.iter().filter(|u| u.is_expired()).count();

    let mut rows = String::new();
    for u in &users {
        let devices = list_user_devices(&state.pool, u.id).await?;
        let active_dev = devices.iter().filter(|d| d.revoked == 0).count();
        let badge = match u.status.as_str() {
            "active" => ("b-active", if u.is_expired() { "已过期" } else { "正常" }),
            "suspended" => ("b-suspended", "停用"),
            _ => ("b-pending", "待审核"),
        };
        let expires = u.expires_at.as_deref().unwrap_or("长期");
        let dev_list = devices
            .iter()
            .map(|d| {
                format!(
                    "{}{}",
                    html_escape(d.platform.as_deref().unwrap_or("?")),
                    if d.revoked != 0 { "(禁)" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        rows.push_str(&format!(
            r#"<tr>
<td>{id}</td>
<td>{email}<div class="muted">{dev_list}</div></td>
<td><span class="badge {bcls}">{btxt}</span></td>
<td>{active_dev}/{max}</td>
<td>{expires}</td>
<td><div class="row-actions">
  <form class="inline" method="post" action="/admin/users/{id}/authorize">
    设备<input type="number" name="max_devices" value="{max}" min="1">
    天数<input type="number" name="valid_days" value="30" min="0" title="0=长期">
    <button>授权/更新</button>
  </form>
  <form class="inline" method="post" action="/admin/users/{id}/extend">
    +<input type="number" name="days" value="30" min="1">天<button class="ghost">续期</button>
  </form>
  {suspend_btn}
  <form class="inline" method="post" action="/admin/users/{id}/reset-devices" onsubmit="return confirm('解绑该用户所有设备?')"><button class="ghost">解绑设备</button></form>
  <form class="inline" method="post" action="/admin/users/{id}/delete" onsubmit="return confirm('删除该用户?')"><button class="danger">删除</button></form>
</div></td>
</tr>"#,
            id = u.id,
            email = html_escape(&u.email),
            bcls = badge.0,
            btxt = badge.1,
            active_dev = active_dev,
            max = u.max_devices,
            expires = html_escape(expires),
            dev_list = dev_list,
            suspend_btn = if u.status == "suspended" {
                format!(r#"<form class="inline" method="post" action="/admin/users/{}/activate"><button class="ghost">恢复</button></form>"#, u.id)
            } else {
                format!(r#"<form class="inline" method="post" action="/admin/users/{}/suspend"><button class="ghost">停用</button></form>"#, u.id)
            },
        ));
    }

    let body = format!(
        r#"{nav}<main>
<div class="cards">
<div class="card"><div class="n">{total}</div><div class="l">总用户</div></div>
<div class="card"><div class="n">{active}</div><div class="l">正常</div></div>
<div class="card"><div class="n">{pending}</div><div class="l">待审核</div></div>
<div class="card"><div class="n">{expired}</div><div class="l">已过期</div></div>
</div>
<table>
<thead><tr><th>ID</th><th>邮箱 / 设备</th><th>状态</th><th>设备</th><th>到期</th><th>操作</th></tr></thead>
<tbody>{rows}</tbody>
</table>
<p class="muted">授权：设置「可绑定设备数」与「有效天数」（0=长期），保存即把用户置为正常并邮件通知。</p>
</main>"#,
        nav = nav(),
        total = total,
        active = active,
        pending = pending,
        expired = expired,
        rows = rows,
    );
    Ok(page("后台", &body))
}

// ---------------- 用户操作 ----------------

#[derive(Deserialize)]
pub struct AuthorizeForm {
    max_devices: i64,
    valid_days: i64,
}

pub async fn authorize_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(form): Form<AuthorizeForm>,
) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    let expires_at = if form.valid_days > 0 {
        Some((Utc::now() + Duration::days(form.valid_days)).to_rfc3339())
    } else {
        None
    };
    let max_devices = form.max_devices.max(1);
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query("UPDATE users SET status='active', max_devices=?, expires_at=?, authorized_at=? WHERE id=?")
        .bind(max_devices)
        .bind(&expires_at)
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await;
    if let Err(e) = res {
        tracing::error!("authorize 失败: {e:#}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "失败").into_response();
    }
    if let Ok(Some(user)) = find_user_by_id(&state.pool, id).await {
        crate::notify::on_user_authorized(&state.cfg, &state.pool, &user).await;
    }
    Redirect::to("/admin").into_response()
}

#[derive(Deserialize)]
pub struct ExtendForm {
    days: i64,
}

pub async fn extend_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(form): Form<ExtendForm>,
) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    let Ok(Some(user)) = find_user_by_id(&state.pool, id).await else {
        return Redirect::to("/admin").into_response();
    };
    let base = user
        .expires_at
        .as_deref()
        .and_then(crate::models::parse_dt)
        .filter(|d| *d > Utc::now())
        .unwrap_or_else(Utc::now);
    let new_exp = (base + Duration::days(form.days.max(1))).to_rfc3339();
    sqlx::query("UPDATE users SET expires_at=?, status='active' WHERE id=?")
        .bind(&new_exp)
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();
    Redirect::to("/admin").into_response()
}

pub async fn suspend_user(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    sqlx::query("UPDATE users SET status='suspended' WHERE id=?")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();
    Redirect::to("/admin").into_response()
}

pub async fn activate_user(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    sqlx::query("UPDATE users SET status='active' WHERE id=?")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();
    Redirect::to("/admin").into_response()
}

pub async fn reset_devices(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    sqlx::query("UPDATE devices SET revoked=1, token=NULL WHERE user_id=?")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();
    Redirect::to("/admin").into_response()
}

pub async fn delete_user(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    sqlx::query("DELETE FROM devices WHERE user_id=?")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();
    sqlx::query("DELETE FROM users WHERE id=?")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();
    Redirect::to("/admin").into_response()
}

// ---------------- 订阅模板 ----------------

pub async fn template_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    let tpl = clash::get_template(&state.pool).await.unwrap_or_default();
    let body = format!(
        r#"{nav}<main>
<h2>订阅模板（Clash YAML）</h2>
<p class="muted">节点 password 处写占位符 <code>{ph}</code>，下发时会替换为用户设备 Token。</p>
<form method="post" action="/admin/template">
<textarea name="template">{tpl}</textarea>
<p><button type="submit">保存模板</button></p>
</form></main>"#,
        nav = nav(),
        ph = clash::TOKEN_PLACEHOLDER,
        tpl = html_escape(&tpl),
    );
    page("订阅模板", &body).into_response()
}

#[derive(Deserialize)]
pub struct TemplateForm {
    template: String,
}

pub async fn template_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TemplateForm>,
) -> Response {
    if !is_authed(&state, &headers) {
        return Redirect::to("/admin/login").into_response();
    }
    clash::set_template(&state.pool, &form.template).await.ok();
    Redirect::to("/admin/template").into_response()
}
