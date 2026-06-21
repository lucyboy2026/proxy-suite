//! 统一错误类型 -> HTTP 响应。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

/// 任何 anyhow 错误 -> 500。
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!("内部错误: {err:#}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "内部服务器错误")
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!("数据库错误: {err:#}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "数据库错误")
    }
}

pub type AppResult<T> = Result<T, AppError>;
