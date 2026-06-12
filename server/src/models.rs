//! 数据模型与查询辅助。

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub status: String,
    pub max_devices: i64,
    pub expires_at: Option<String>,
    pub note: Option<String>,
    #[serde(skip_serializing)]
    pub subscription_key: Option<String>,
    pub created_at: String,
    pub authorized_at: Option<String>,
}

impl User {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at.as_deref().and_then(parse_dt) {
            Some(exp) => Utc::now() >= exp,
            None => false,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Device {
    pub id: i64,
    pub user_id: i64,
    pub device_fp: String,
    pub platform: Option<String>,
    #[serde(skip_serializing)]
    #[allow(dead_code)] // 由 SQL 按 token 查询使用，结构体字段本身不直接读取
    pub token: Option<String>,
    pub token_expires_at: Option<String>,
    pub revoked: i64,
    pub created_at: String,
    pub last_seen_at: Option<String>,
}

pub fn parse_dt(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

pub async fn find_user_by_email(pool: &SqlitePool, email: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn find_user_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn find_user_by_subscription_key(pool: &SqlitePool, key: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE subscription_key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

/// 返回该用户已绑定的、最近活跃且未过期的设备 Token（用于固定订阅链接注入）。
pub async fn latest_active_token(pool: &SqlitePool, user_id: i64) -> Result<Option<String>> {
    let now = Utc::now().to_rfc3339();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT token FROM devices
         WHERE user_id = ? AND revoked = 0 AND token IS NOT NULL
           AND (token_expires_at IS NULL OR token_expires_at > ?)
         ORDER BY last_seen_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// 确保用户拥有固定订阅密钥；缺失时生成并持久化，返回该密钥。
pub async fn ensure_subscription_key(pool: &SqlitePool, user_id: i64) -> Result<String> {
    if let Some(user) = find_user_by_id(pool, user_id).await? {
        if let Some(key) = user.subscription_key.filter(|s| !s.is_empty()) {
            return Ok(key);
        }
    }
    let key = crate::auth::gen_token();
    sqlx::query("UPDATE users SET subscription_key = ? WHERE id = ?")
        .bind(&key)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(key)
}

pub async fn list_users(pool: &SqlitePool) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(users)
}

pub async fn count_user_devices(pool: &SqlitePool, user_id: i64) -> Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE user_id = ? AND revoked = 0")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn list_user_devices(pool: &SqlitePool, user_id: i64) -> Result<Vec<Device>> {
    let devices = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE user_id = ? ORDER BY created_at")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(devices)
}

pub async fn find_device(pool: &SqlitePool, user_id: i64, device_fp: &str) -> Result<Option<Device>> {
    let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE user_id = ? AND device_fp = ?")
        .bind(user_id)
        .bind(device_fp)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}

pub async fn find_device_by_token(pool: &SqlitePool, token: &str) -> Result<Option<Device>> {
    let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE token = ? AND revoked = 0")
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn user(status: &str, expires_at: Option<&str>) -> User {
        User {
            id: 1,
            email: "u@e.com".into(),
            password_hash: "x".into(),
            status: status.into(),
            max_devices: 1,
            expires_at: expires_at.map(|s| s.into()),
            note: None,
            subscription_key: None,
            created_at: Utc::now().to_rfc3339(),
            authorized_at: None,
        }
    }

    #[test]
    fn parse_dt_accepts_rfc3339_and_rejects_garbage() {
        assert!(parse_dt("2999-01-01T00:00:00Z").is_some());
        assert!(parse_dt("not-a-date").is_none());
        assert!(parse_dt("").is_none());
    }

    #[test]
    fn is_active_tracks_status() {
        assert!(user("active", None).is_active());
        assert!(!user("pending", None).is_active());
        assert!(!user("suspended", None).is_active());
    }

    #[test]
    fn is_expired_handles_past_future_and_missing() {
        let past = (Utc::now() - Duration::days(1)).to_rfc3339();
        let future = (Utc::now() + Duration::days(1)).to_rfc3339();
        assert!(user("active", Some(&past)).is_expired());
        assert!(!user("active", Some(&future)).is_expired());
        // No deadline means the account never expires.
        assert!(!user("active", None).is_expired());
    }

    /// Build a fresh, isolated SQLite database (its own temp file) using the
    /// production `init_pool`, so the real schema/migrations are exercised.
    async fn test_pool() -> SqlitePool {
        let path = std::env::temp_dir().join(format!("nodeauth-test-{}.db", crate::auth::gen_token()));
        let url = format!("sqlite://{}", path.display());
        crate::db::init_pool(&url).await.unwrap()
    }

    async fn insert_user(pool: &SqlitePool, email: &str, status: &str) -> i64 {
        sqlx::query(
            "INSERT INTO users(email, password_hash, status, max_devices, created_at)
             VALUES(?, ?, ?, 1, ?)",
        )
        .bind(email)
        .bind("hash")
        .bind(status)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
    }

    #[tokio::test]
    async fn find_user_by_email_and_id_round_trip() {
        let pool = test_pool().await;
        let id = insert_user(&pool, "a@e.com", "active").await;

        let by_email = find_user_by_email(&pool, "a@e.com").await.unwrap().unwrap();
        assert_eq!(by_email.id, id);
        assert_eq!(by_email.status, "active");

        let by_id = find_user_by_id(&pool, id).await.unwrap().unwrap();
        assert_eq!(by_id.email, "a@e.com");

        assert!(find_user_by_email(&pool, "missing@e.com").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ensure_subscription_key_is_idempotent() {
        let pool = test_pool().await;
        let id = insert_user(&pool, "k@e.com", "active").await;

        let first = ensure_subscription_key(&pool, id).await.unwrap();
        assert!(!first.is_empty());
        let second = ensure_subscription_key(&pool, id).await.unwrap();
        assert_eq!(first, second, "an existing key must be reused, not regenerated");

        // The key is discoverable via its dedicated lookup.
        let found = find_user_by_subscription_key(&pool, &first).await.unwrap().unwrap();
        assert_eq!(found.id, id);
    }

    #[tokio::test]
    async fn count_user_devices_ignores_revoked() {
        let pool = test_pool().await;
        let uid = insert_user(&pool, "d@e.com", "active").await;
        let now = Utc::now().to_rfc3339();
        for (fp, revoked) in [("fp1", 0), ("fp2", 0), ("fp3", 1)] {
            sqlx::query(
                "INSERT INTO devices(user_id, device_fp, revoked, created_at)
                 VALUES(?, ?, ?, ?)",
            )
            .bind(uid)
            .bind(fp)
            .bind(revoked)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        }
        assert_eq!(count_user_devices(&pool, uid).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn latest_active_token_picks_recent_unexpired_unrevoked() {
        let pool = test_pool().await;
        let uid = insert_user(&pool, "t@e.com", "active").await;
        let future = (Utc::now() + Duration::days(7)).to_rfc3339();
        let past = (Utc::now() - Duration::days(1)).to_rfc3339();

        // Three devices: revoked, expired, and a valid recent one.
        let rows = [
            ("revoked-tok", 1, Some(future.as_str()), "2000-01-01T00:00:00Z"),
            ("expired-tok", 0, Some(past.as_str()), "2001-01-01T00:00:00Z"),
            ("good-tok", 0, Some(future.as_str()), "2099-01-01T00:00:00Z"),
        ];
        for (i, (tok, revoked, exp, last_seen)) in rows.iter().enumerate() {
            sqlx::query(
                "INSERT INTO devices(user_id, device_fp, token, token_expires_at, revoked, created_at, last_seen_at)
                 VALUES(?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uid)
            .bind(format!("fp{i}"))
            .bind(tok)
            .bind(exp)
            .bind(revoked)
            .bind(Utc::now().to_rfc3339())
            .bind(last_seen)
            .execute(&pool)
            .await
            .unwrap();
        }

        let token = latest_active_token(&pool, uid).await.unwrap();
        assert_eq!(token.as_deref(), Some("good-tok"));

        // And it is reachable through the token lookup.
        let dev = find_device_by_token(&pool, "good-tok").await.unwrap().unwrap();
        assert_eq!(dev.user_id, uid);
        assert!(find_device_by_token(&pool, "revoked-tok").await.unwrap().is_none());
    }
}
