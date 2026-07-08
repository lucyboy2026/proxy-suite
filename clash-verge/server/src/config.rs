//! 运行时配置：全部来自环境变量（支持 `.env`）。

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    /// 监听地址，例如 `0.0.0.0:8080`
    pub bind_addr: String,
    /// SQLite 数据库文件路径，例如 `data/nodeauth.db`
    pub database_url: String,
    /// 对外可访问的基础地址（用于邮件里的链接、订阅地址），例如 `https://auth.example.com`
    pub public_base_url: String,

    /// 管理员登录用户名（Web 后台）
    pub admin_username: String,
    /// 管理员登录密码（Web 后台）
    pub admin_password: String,

    /// 默认授权设备数（管理员未指定时）
    pub default_max_devices: u32,
    /// 默认授权天数（管理员未指定时）
    pub default_valid_days: i64,
    /// 设备 Token 有效期（天），不超过用户到期时间
    pub token_ttl_days: i64,

    /// `/sub/:key` 是否要求携带已绑定设备的指纹（防订阅链接外传）
    pub sub_require_fp: bool,
    /// 每个 Token 允许的同时在线来源 IP 数（0 = 不限制）
    pub max_online_ips: u32,
    /// 在线 IP 记录的空闲过期时间（秒）
    pub online_ip_ttl_secs: u64,

    /// 邮件配置（可选）
    pub smtp: Option<SmtpConfig>,
    /// Telegram 配置（可选）
    pub telegram: Option<TelegramConfig>,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// 发件人地址（通常等于 username）
    pub from: String,
    /// 管理员收件地址（收注册通知）
    pub admin_to: String,
}

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub admin_chat_id: String,
}

fn var(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

fn var_or(key: &str, default: &str) -> String {
    var(key).unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let smtp = match (var("SMTP_HOST"), var("SMTP_USERNAME"), var("SMTP_PASSWORD")) {
            (Some(host), Some(username), Some(password)) => {
                let from = var_or("SMTP_FROM", &username);
                let admin_to = var_or("ADMIN_EMAIL", &from);
                Some(SmtpConfig {
                    host,
                    port: var_or("SMTP_PORT", "465").parse().unwrap_or(465),
                    username,
                    password,
                    from,
                    admin_to,
                })
            }
            _ => None,
        };

        let telegram = match (var("TELEGRAM_BOT_TOKEN"), var("TELEGRAM_ADMIN_CHAT_ID")) {
            (Some(bot_token), Some(admin_chat_id)) => Some(TelegramConfig {
                bot_token,
                admin_chat_id,
            }),
            _ => None,
        };

        Self {
            bind_addr: var_or("BIND_ADDR", "0.0.0.0:8080"),
            database_url: var_or("DATABASE_URL", "sqlite://data/nodeauth.db"),
            public_base_url: var_or("PUBLIC_BASE_URL", "http://127.0.0.1:8080"),
            admin_username: var_or("ADMIN_USERNAME", "admin"),
            admin_password: var_or("ADMIN_PASSWORD", "change-me"),
            default_max_devices: var_or("DEFAULT_MAX_DEVICES", "1").parse().unwrap_or(1),
            default_valid_days: var_or("DEFAULT_VALID_DAYS", "30").parse().unwrap_or(30),
            token_ttl_days: var_or("TOKEN_TTL_DAYS", "7").parse().unwrap_or(7),
            sub_require_fp: var_or("SUB_REQUIRE_FP", "true").parse().unwrap_or(true),
            max_online_ips: var_or("MAX_ONLINE_IPS", "3").parse().unwrap_or(3),
            online_ip_ttl_secs: var_or("ONLINE_IP_TTL_SECS", "600").parse().unwrap_or(600),
            smtp,
            telegram,
        }
    }
}
