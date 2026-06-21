//! Clash 配置（YAML）生成：把用户的设备 Token 注入 hysteria2 节点的 `password`。
//!
//! 管理员在后台维护一份「模板」，其中节点密码处写占位符 `__NODE_TOKEN__`，
//! 下发时替换为该用户当前设备的 64 位 Token。

use crate::db;
use anyhow::Result;
use sqlx::SqlitePool;

pub const TOKEN_PLACEHOLDER: &str = "__NODE_TOKEN__";
pub const SETTING_KEY: &str = "clash_template";

/// 默认模板：一个最简 hysteria2 订阅示例，管理员应在后台替换为真实节点。
pub const DEFAULT_TEMPLATE: &str = r#"# Clash Verge 订阅模板（请在后台「订阅模板」中编辑为真实节点）
# 节点 password 处填占位符（见后台提示），下发时会被替换为用户设备 Token。
mixed-port: 7890
allow-lan: false
mode: rule
log-level: info

proxies:
  - name: "Hysteria2-示例"
    type: hysteria2
    server: example.com
    port: 443
    password: __NODE_TOKEN__
    sni: example.com
    skip-cert-verify: false

proxy-groups:
  - name: "PROXY"
    type: select
    proxies:
      - "Hysteria2-示例"

rules:
  - MATCH,PROXY
"#;

/// 取得当前模板（无则返回默认）。
pub async fn get_template(pool: &SqlitePool) -> Result<String> {
    Ok(db::get_setting(pool, SETTING_KEY)
        .await?
        .unwrap_or_else(|| DEFAULT_TEMPLATE.to_string()))
}

/// 保存模板。
pub async fn set_template(pool: &SqlitePool, template: &str) -> Result<()> {
    db::set_setting(pool, SETTING_KEY, template).await
}

/// 渲染给定 Token 的配置。
pub fn render(template: &str, token: &str) -> String {
    template.replace(TOKEN_PLACEHOLDER, token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_every_placeholder() {
        let template = "a: __NODE_TOKEN__\nb: __NODE_TOKEN__";
        assert_eq!(render(template, "tok"), "a: tok\nb: tok");
    }

    #[test]
    fn render_is_noop_without_placeholder() {
        let template = "password: literal";
        assert_eq!(render(template, "tok"), template);
    }

    #[test]
    fn default_template_carries_the_placeholder() {
        assert!(DEFAULT_TEMPLATE.contains(TOKEN_PLACEHOLDER));
        let rendered = render(DEFAULT_TEMPLATE, "abc123");
        assert!(!rendered.contains(TOKEN_PLACEHOLDER));
        assert!(rendered.contains("password: abc123"));
    }

    async fn test_pool() -> SqlitePool {
        let path = std::env::temp_dir().join(format!("nodeauth-test-{}.db", crate::auth::gen_token()));
        db::init_pool(&format!("sqlite://{}", path.display())).await.unwrap()
    }

    #[tokio::test]
    async fn get_template_defaults_then_persists_override() {
        let pool = test_pool().await;
        // With nothing stored, the default template is returned.
        assert_eq!(get_template(&pool).await.unwrap(), DEFAULT_TEMPLATE);

        set_template(&pool, "proxies: []  # custom").await.unwrap();
        assert_eq!(get_template(&pool).await.unwrap(), "proxies: []  # custom");
    }
}
