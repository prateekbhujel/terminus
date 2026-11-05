//! Fixed-window rate limiting (Laravel-style throttle).
//!
//! [`RateLimiter`] is a thread-safe per-key counter. [`crate::app::App::rate_limit`]
//! applies it as HTTP middleware keyed by client address (`x-forwarded-for`
//! first hop when behind a proxy, otherwise a single shared bucket, since
//! the socket peer is not visible to in-process handlers).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Thread-safe fixed-window rate limiter.
#[derive(Clone)]
pub struct RateLimiter {
    max_requests: usize,
    window: Duration,
    hits: Arc<Mutex<HashMap<String, (Instant, usize)>>>,
}

impl RateLimiter {
    /// Allow `max_requests` per `window` for each key.
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            hits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record a hit for `key`. Returns true when the hit is allowed.
    pub fn hit(&self, key: &str) -> bool {
        let mut hits = self.hits.lock().unwrap();
        let now = Instant::now();
        let entry = hits.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.max_requests
    }

    /// Hits recorded for `key` in its current window.
    pub fn attempts(&self, key: &str) -> usize {
        let hits = self.hits.lock().unwrap();
        hits.get(key).map(|(_, count)| *count).unwrap_or(0)
    }

    pub fn max_requests(&self) -> usize {
        self.max_requests
    }
}

pub(crate) fn client_key(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "global".to_string())
}
