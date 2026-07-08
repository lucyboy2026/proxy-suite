//! 每个设备 Token 的「同时在线 IP」跟踪与限制。
//!
//! hysteria2 每次建连都会回调 `/auth`，这里按 Token 记录来源 IP 及其最近活跃时间；
//! 超过 TTL 未再认证的 IP 视为下线并释放名额。限制的是「同一时刻不同来源 IP 数」，
//! 与 IP 是否固定无关：家庭宽带/移动网络换 IP 只是名额内的替换，不受影响。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct OnlineTracker {
    inner: Mutex<HashMap<String, HashMap<String, Instant>>>,
}

impl OnlineTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 判定该 (token, ip) 是否允许建连。`limit == 0` 表示不限制。
    /// 已在线的 IP 刷新活跃时间放行；新 IP 在名额内则登记放行，否则拒绝。
    pub fn admit(&self, token: &str, ip: &str, limit: usize, ttl: Duration) -> bool {
        if limit == 0 {
            return true;
        }
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();
        let ips = map.entry(token.to_string()).or_default();
        ips.retain(|_, seen| now.duration_since(*seen) < ttl);
        if let Some(seen) = ips.get_mut(ip) {
            *seen = now;
            return true;
        }
        if ips.len() >= limit {
            return false;
        }
        ips.insert(ip.to_string(), now);
        true
    }
}

/// 从 hysteria 回调的 `addr`（如 `1.2.3.4:5678`、`[::1]:5678`）里取出纯 IP。
pub fn client_ip(addr: &str) -> &str {
    let addr = addr.trim();
    if let Some(rest) = addr.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return &rest[..end];
        }
    }
    match addr.rsplit_once(':') {
        // 含多个 ':' 且无方括号的裸 IPv6 地址，整体就是 IP
        Some((host, _)) if !host.contains(':') => host,
        _ => addr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ip_parses_v4_v6_and_bare() {
        assert_eq!(client_ip("1.2.3.4:5678"), "1.2.3.4");
        assert_eq!(client_ip("[2001:db8::1]:443"), "2001:db8::1");
        assert_eq!(client_ip("2001:db8::1"), "2001:db8::1");
        assert_eq!(client_ip("1.2.3.4"), "1.2.3.4");
    }

    #[test]
    fn admit_enforces_limit_and_allows_known_ips() {
        let t = OnlineTracker::new();
        let ttl = Duration::from_secs(60);
        assert!(t.admit("tok", "ip1", 2, ttl));
        assert!(t.admit("tok", "ip2", 2, ttl));
        // 第 3 个不同 IP 超限
        assert!(!t.admit("tok", "ip3", 2, ttl));
        // 已在线 IP 重连不受影响
        assert!(t.admit("tok", "ip1", 2, ttl));
        // 不同 Token 互不影响
        assert!(t.admit("tok2", "ip3", 2, ttl));
    }

    #[test]
    fn admit_zero_limit_means_unlimited() {
        let t = OnlineTracker::new();
        let ttl = Duration::from_secs(60);
        for i in 0..10 {
            assert!(t.admit("tok", &format!("ip{i}"), 0, ttl));
        }
    }

    #[test]
    fn admit_frees_slots_after_ttl() {
        let t = OnlineTracker::new();
        let ttl = Duration::from_millis(10);
        assert!(t.admit("tok", "ip1", 1, ttl));
        assert!(!t.admit("tok", "ip2", 1, ttl));
        std::thread::sleep(Duration::from_millis(20));
        assert!(t.admit("tok", "ip2", 1, ttl));
    }
}
