//! Clash Verge 设备绑定两步鉴权 —— Auth Server + 后台管理平台（组件一）。

mod auth;
mod clash;
mod config;
mod db;
mod email;
mod error;
mod models;
mod notify;
mod routes;
mod state;
mod telegram;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::auth::SessionStore;
use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let cfg = Config::from_env();
    tracing::info!("启动配置: bind={} db={}", cfg.bind_addr, cfg.database_url);
    if cfg.smtp.is_none() {
        tracing::warn!("未配置 SMTP，邮件通知将被跳过（仅记录日志）");
    }
    if cfg.telegram.is_none() {
        tracing::warn!("未配置 Telegram，TG 通知/审批将被跳过");
    }
    if cfg.admin_password == "change-me" {
        tracing::warn!("管理员密码为默认值 change-me，请通过 ADMIN_PASSWORD 修改！");
    }

    let pool = db::init_pool(&cfg.database_url).await?;
    let state = AppState {
        pool,
        cfg: Arc::new(cfg.clone()),
        sessions: Arc::new(SessionStore::new()),
    };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("监听 http://{}", cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// 组装路由（生产与集成测试共用）。
fn build_router(state: AppState) -> Router {
    Router::new()
        // 健康检查
        .route("/healthz", get(|| async { "ok" }))
        // 客户端 API
        .route("/register", post(routes::client::register))
        .route("/login", post(routes::client::login))
        .route("/config", get(routes::client::get_config))
        .route("/sub/:key", get(routes::client::get_subscription))
        .route("/auth", post(routes::client::hysteria_auth))
        // Telegram webhook
        .route("/tg/webhook", post(routes::tg::webhook))
        // 后台
        .route(
            "/admin/login",
            get(routes::admin::login_page).post(routes::admin::login_submit),
        )
        .route("/admin/logout", post(routes::admin::logout))
        .route("/admin", get(routes::admin::dashboard))
        .route(
            "/admin/template",
            get(routes::admin::template_page).post(routes::admin::template_submit),
        )
        .route("/admin/users/:id/authorize", post(routes::admin::authorize_user))
        .route("/admin/users/:id/extend", post(routes::admin::extend_user))
        .route("/admin/users/:id/suspend", post(routes::admin::suspend_user))
        .route("/admin/users/:id/activate", post(routes::admin::activate_user))
        .route("/admin/users/:id/reset-devices", post(routes::admin::reset_devices))
        .route("/admin/users/:id/delete", post(routes::admin::delete_user))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod integration_tests {
    //! 端到端集成测试：把整套路由起在本机随机端口上，用真实 HTTP 走完
    //! 注册 → 管理员授权 → 登录 → 配置/订阅注入 → hysteria2 回调 全流程。
    use super::*;
    use crate::config::Config;
    use serde_json::Value;

    fn test_config() -> Config {
        Config {
            bind_addr: "127.0.0.1:0".into(),
            database_url: String::new(),
            public_base_url: "http://test.local".into(),
            admin_username: "admin".into(),
            admin_password: "test-pw".into(),
            default_max_devices: 1,
            default_valid_days: 30,
            token_ttl_days: 7,
            smtp: None,
            telegram: None,
        }
    }

    /// 起一个隔离的应用实例（独立 SQLite 文件 + 随机端口），返回 base url。
    async fn spawn_app() -> String {
        let path = std::env::temp_dir().join(format!("nodeauth-itest-{}.db", auth::gen_token()));
        let pool = db::init_pool(&format!("sqlite://{}", path.display())).await.unwrap();
        let state = AppState {
            pool,
            cfg: Arc::new(test_config()),
            sessions: Arc::new(SessionStore::new()),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = build_router(state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// reqwest 没启用 cookie store，这里手动从 Set-Cookie 取出 `sid=...`。
    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }

    async fn admin_cookie(c: &reqwest::Client, base: &str) -> String {
        let resp = c
            .post(format!("{base}/admin/login"))
            .form(&[("username", "admin"), ("password", "test-pw")])
            .send()
            .await
            .unwrap();
        let cookie = resp
            .headers()
            .get(reqwest::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        cookie.split(';').next().unwrap().to_string()
    }

    #[tokio::test]
    async fn healthz_ok() {
        let base = spawn_app().await;
        let body = reqwest::get(format!("{base}/healthz"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn full_register_authorize_login_inject_flow() {
        let base = spawn_app().await;
        let c = client();
        let reg_body = serde_json::json!({
            "email": "alice@example.com",
            "password": "hunter2",
            "device_fp": "fp-1",
            "platform": "linux",
        });

        // 1) 注册 -> 202 pending
        let resp = c.post(format!("{base}/register")).json(&reg_body).send().await.unwrap();
        assert_eq!(resp.status(), 202);

        // 2) 授权前登录 -> 403
        let resp = c.post(format!("{base}/login")).json(&reg_body).send().await.unwrap();
        assert_eq!(resp.status(), 403);

        // 3) 管理员登录拿 cookie，授权 user id=1（2 台设备 / 30 天）
        let cookie = admin_cookie(&c, &base).await;
        let resp = c
            .post(format!("{base}/admin/users/1/authorize"))
            .header(reqwest::header::COOKIE, &cookie)
            .form(&[("max_devices", "2"), ("valid_days", "30")])
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 303);

        // 4) 授权后登录 -> 200 + token
        let resp = c.post(format!("{base}/login")).json(&reg_body).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let login: Value = resp.json().await.unwrap();
        let token = login["token"].as_str().unwrap().to_string();
        assert_eq!(token.len(), 64);
        assert_eq!(login["username"], "alice@example.com");
        let sub_url = login["subscription_url"].as_str().unwrap().to_string();

        // 5) /config?token= -> 配置里 hysteria2 password 注入了该 token
        let yaml = c
            .get(format!("{base}/config?token={token}"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(yaml.contains(&format!("password: {token}")));

        // 6) 固定订阅 /sub/{key} -> 注入最近活跃 token
        let sub_path = sub_url.strip_prefix("http://test.local").unwrap();
        let yaml = c
            .get(format!("{base}{sub_path}"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(yaml.contains(&format!("password: {token}")));

        // 7) hysteria2 鉴权回调 -> ok:true
        let resp = c
            .post(format!("{base}/auth"))
            .json(&serde_json::json!({ "auth": token }))
            .send()
            .await
            .unwrap();
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["ok"], true);
        assert_eq!(body["id"], "alice@example.com#fp-1");
    }

    #[tokio::test]
    async fn rejects_bad_credentials_and_tokens() {
        let base = spawn_app().await;
        let c = client();

        // 错误密码登录 -> 401
        let resp = c
            .post(format!("{base}/login"))
            .json(&serde_json::json!({
                "email": "nobody@example.com", "password": "x", "device_fp": "fp"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401);

        // 无效 token 取配置 -> 401
        let resp = c.get(format!("{base}/config?token=deadbeef")).send().await.unwrap();
        assert_eq!(resp.status(), 401);

        // hysteria2 回调坏 token -> ok:false
        let body: Value = c
            .post(format!("{base}/auth"))
            .json(&serde_json::json!({ "auth": "deadbeef" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["ok"], false);
    }

    #[tokio::test]
    async fn enforces_device_cap() {
        let base = spawn_app().await;
        let c = client();
        let reg = |fp: &str| {
            serde_json::json!({
                "email": "bob@example.com", "password": "hunter2", "device_fp": fp, "platform": "linux"
            })
        };

        c.post(format!("{base}/register"))
            .json(&reg("fp-1"))
            .send()
            .await
            .unwrap();
        let cookie = admin_cookie(&c, &base).await;
        // 仅授权 1 台设备
        c.post(format!("{base}/admin/users/1/authorize"))
            .header(reqwest::header::COOKIE, &cookie)
            .form(&[("max_devices", "1"), ("valid_days", "30")])
            .send()
            .await
            .unwrap();

        // 第 1 台登录成功
        assert_eq!(
            c.post(format!("{base}/login"))
                .json(&reg("fp-1"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        // 第 2 台超出上限 -> 403
        assert_eq!(
            c.post(format!("{base}/login"))
                .json(&reg("fp-2"))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
}
