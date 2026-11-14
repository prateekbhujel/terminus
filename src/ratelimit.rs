//! Fixed-window rate limiting with proxy-aware client identification.
//!
//! Client-supplied forwarding headers are untrusted unless the immediate
//! socket peer has been explicitly allowlisted as a reverse proxy.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Thread-safe fixed-window rate limiter.
#[derive(Clone)]
pub struct RateLimiter {
    max_requests: usize,
    window: Duration,
    hits: Arc<Mutex<HashMap<String, (Instant, usize)>>>,
    trusted_proxies: Vec<IpAddr>,
}

impl RateLimiter {
    /// Allow `max_requests` per `window` for each client identity.
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            hits: Arc::new(Mutex::new(HashMap::new())),
            trusted_proxies: Vec::new(),
        }
    }

    /// Trust forwarding headers only when the direct socket peer is one of
    /// these proxy addresses. Use the actual address of your reverse proxy,
    /// not the addresses of clients connecting through it.
    pub fn with_trusted_proxies(mut self, proxies: &[IpAddr]) -> Self {
        self.trusted_proxies = proxies.to_vec();
        self
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

    /// Determine the rate-limit identity without trusting arbitrary headers.
    ///
    /// Walk a trusted forwarding chain from right to left. A proxy can append
    /// its observed client address to an existing X-Forwarded-For header, so
    /// taking the *first* address would allow clients to spoof their identity.
    /// Stop at the first hop that is not a trusted proxy.
    pub(crate) fn client_key(
        &self,
        peer_ip: Option<IpAddr>,
        headers: &axum::http::HeaderMap,
    ) -> String {
        let Some(mut client) = peer_ip else {
            // In-process routers have no socket peer: never manufacture an
            // identity from client-controlled headers.
            return "global".to_string();
        };
        if !self.trusted_proxies.contains(&client) {
            return client.to_string();
        }

        let mut forwarded = Vec::new();
        for value in headers.get_all("x-forwarded-for").iter() {
            let Ok(value) = value.to_str() else {
                return client.to_string();
            };
            for hop in value.split(',') {
                let Ok(ip) = hop.trim().parse::<IpAddr>() else {
                    // Do not use a partially parsed or attacker-invented chain.
                    return client.to_string();
                };
                forwarded.push(ip);
            }
        }

        for hop in forwarded.into_iter().rev() {
            if !self.trusted_proxies.contains(&client) {
                break;
            }
            client = hop;
        }
        client.to_string()
    }
}
