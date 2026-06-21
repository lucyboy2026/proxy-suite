//! 邮件发送（Gmail/通用 SMTP，lettre）。未配置时仅记录日志。

use crate::config::SmtpConfig;
use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

pub async fn send(cfg: &SmtpConfig, to: &str, subject: &str, body: &str) -> Result<()> {
    let email = Message::builder()
        .from(cfg.from.parse().context("非法发件地址")?)
        .to(to.parse().context("非法收件地址")?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .context("构建邮件失败")?;

    let creds = Credentials::new(cfg.username.clone(), cfg.password.clone());

    // 465 = 隐式 TLS（relay）；其它（如 587）走 STARTTLS。
    let builder = if cfg.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host).context("SMTP relay 初始化失败")?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host).context("SMTP starttls 初始化失败")?
    };

    let mailer = builder.port(cfg.port).credentials(creds).build();
    mailer.send(email).await.context("发送邮件失败")?;
    Ok(())
}
