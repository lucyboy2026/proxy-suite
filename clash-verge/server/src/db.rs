//! 数据库连接与建表（SQLite via sqlx，运行时建表，无需编译期 DATABASE_URL）。

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    email            TEXT NOT NULL UNIQUE,
    password_hash    TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending', -- pending | active | suspended
    max_devices      INTEGER NOT NULL DEFAULT 1,
    expires_at       TEXT,                            -- ISO-8601 (UTC)，授权后才有
    note             TEXT,
    subscription_key TEXT,                            -- 长期固定的订阅密钥（/sub/{key}）
    created_at       TEXT NOT NULL,
    authorized_at    TEXT
);

CREATE TABLE IF NOT EXISTS devices (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id          INTEGER NOT NULL,
    device_fp        TEXT NOT NULL,
    platform         TEXT,
    token            TEXT,                         -- 64-hex 设备 token
    token_expires_at TEXT,
    revoked          INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL,
    last_seen_at     TEXT,
    UNIQUE(user_id, device_fp)
);

CREATE INDEX IF NOT EXISTS idx_devices_token ON devices(token);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 审计/通知日志（便于后台展示与排查）
CREATE TABLE IF NOT EXISTS events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT NOT NULL,
    user_email TEXT,
    detail     TEXT,
    created_at TEXT NOT NULL
);
"#;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool> {
    // 确保 sqlite 文件所在目录存在
    if let Some(path) = database_url.strip_prefix("sqlite://") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
    }

    let opts = SqliteConnectOptions::from_str(database_url)
        .context("无法解析 DATABASE_URL")?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .context("连接 SQLite 失败")?;

    sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await.ok();

    for stmt in SCHEMA.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt)
            .execute(&pool)
            .await
            .with_context(|| format!("建表失败: {stmt}"))?;
    }

    // 针对早期版本库的增量迁移：列已存在时 SQLite 报错，忽略即可。
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN subscription_key TEXT")
        .execute(&pool)
        .await;
    // 列就绪后再建唯一索引（放在迁移之后，兼容旧库升级）。
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_users_subkey ON users(subscription_key)")
        .execute(&pool)
        .await
        .context("建 subscription_key 索引失败")?;

    Ok(pool)
}

/// 读取设置项。
pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// 写入设置项。
pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings(key, value) VALUES(?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// 记录一条事件日志。
pub async fn log_event(pool: &SqlitePool, kind: &str, user_email: Option<&str>, detail: &str) -> Result<()> {
    sqlx::query("INSERT INTO events(kind, user_email, detail, created_at) VALUES(?, ?, ?, ?)")
        .bind(kind)
        .bind(user_email)
        .bind(detail)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let path = std::env::temp_dir().join(format!("nodeauth-test-{}.db", crate::auth::gen_token()));
        init_pool(&format!("sqlite://{}", path.display())).await.unwrap()
    }

    #[tokio::test]
    async fn init_pool_creates_the_expected_tables() {
        let pool = test_pool().await;
        for table in ["users", "devices", "settings", "events"] {
            let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?")
                .bind(table)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(row.0, 1, "table `{table}` should exist");
        }
    }

    #[tokio::test]
    async fn setting_get_returns_none_then_value_and_upserts() {
        let pool = test_pool().await;
        assert!(get_setting(&pool, "k").await.unwrap().is_none());

        set_setting(&pool, "k", "v1").await.unwrap();
        assert_eq!(get_setting(&pool, "k").await.unwrap().as_deref(), Some("v1"));

        // A second write to the same key overwrites rather than duplicating.
        set_setting(&pool, "k", "v2").await.unwrap();
        assert_eq!(get_setting(&pool, "k").await.unwrap().as_deref(), Some("v2"));
    }

    #[tokio::test]
    async fn log_event_inserts_a_row() {
        let pool = test_pool().await;
        log_event(&pool, "register", Some("u@e.com"), "hello").await.unwrap();
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM events WHERE kind = 'register'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 1);
    }
}
