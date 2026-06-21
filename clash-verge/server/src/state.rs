//! 应用共享状态。

use crate::auth::SessionStore;
use crate::config::Config;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<Config>,
    pub sessions: Arc<SessionStore>,
}
