//! 密码哈希、设备 Token 生成、管理员会话。

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use rand::RngCore;
use std::collections::HashMap;
use std::sync::Mutex;

/// 对密码做 Argon2 哈希。
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash 失败: {e}"))?
        .to_string();
    Ok(hash)
}

/// 校验密码。
pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// 生成 64 位（hex）设备 Token。
pub fn gen_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// 管理员会话存储（内存级，单进程足够）。
#[derive(Default)]
pub struct SessionStore {
    inner: Mutex<HashMap<String, chrono::DateTime<Utc>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新建会话，返回 session id；有效期 7 天。
    pub fn create(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let sid = hex::encode(bytes);
        let exp = Utc::now() + Duration::days(7);
        self.inner.lock().unwrap().insert(sid.clone(), exp);
        sid
    }

    /// 校验会话是否有效。
    pub fn validate(&self, sid: &str) -> bool {
        let mut guard = self.inner.lock().unwrap();
        match guard.get(sid) {
            Some(exp) if *exp > Utc::now() => true,
            Some(_) => {
                guard.remove(sid);
                false
            }
            None => false,
        }
    }

    pub fn destroy(&self, sid: &str) {
        self.inner.lock().unwrap().remove(sid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_round_trips() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn each_hash_is_salted_differently() {
        let a = hash_password("same").unwrap();
        let b = hash_password("same").unwrap();
        assert_ne!(a, b, "salt should make identical passwords hash differently");
        assert!(verify_password("same", &a));
        assert!(verify_password("same", &b));
    }

    #[test]
    fn verify_rejects_malformed_hash() {
        assert!(!verify_password("whatever", "not-a-phc-hash"));
        assert!(!verify_password("whatever", ""));
    }

    #[test]
    fn gen_token_is_64_hex_chars_and_unique() {
        let t = gen_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(gen_token(), gen_token());
    }

    #[test]
    fn session_create_validate_destroy() {
        let store = SessionStore::new();
        let sid = store.create();
        assert_eq!(sid.len(), 64);
        assert!(store.validate(&sid));
        store.destroy(&sid);
        assert!(!store.validate(&sid));
    }

    #[test]
    fn session_validate_rejects_unknown_id() {
        let store = SessionStore::new();
        assert!(!store.validate("nonexistent"));
    }

    #[test]
    fn session_validate_drops_expired_entry() {
        let store = SessionStore::new();
        let sid = "expired".to_string();
        store
            .inner
            .lock()
            .unwrap()
            .insert(sid.clone(), Utc::now() - Duration::seconds(1));
        assert!(!store.validate(&sid));
        // The expired entry should have been evicted on validation.
        assert!(!store.inner.lock().unwrap().contains_key(&sid));
    }
}
