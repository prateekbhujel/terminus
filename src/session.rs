//! Web session and flash messaging engine for Terminus.
//!
//! Provides thread-safe, cookie-backed session state, one-time flash messages
//! for web redirects, CSRF token management, and automatic lifecycle expiration.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};
use chrono::Utc;

/// One-time flash message storage that survives across redirects and is consumed on the next request.
#[derive(Debug, Clone, Default)]
pub struct FlashBag {
    messages: HashMap<String, String>,
    retained_keys: HashSet<String>,
}

impl FlashBag {
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            retained_keys: HashSet::new(),
        }
    }

    /// Stores a flash message for the next request.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let k = key.into();
        self.retained_keys.insert(k.clone());
        self.messages.insert(k, value.into());
    }

    /// Retrieves a flash message by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(|s| s.as_str())
    }

    /// Checks if a flash message exists for the given key.
    pub fn has(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }

    /// Returns a copy of all current flash messages.
    pub fn all(&self) -> HashMap<String, String> {
        self.messages.clone()
    }

    /// Clears flash messages that have already been read.
    pub fn sweep(&mut self) {
        self.messages.clear();
        self.retained_keys.clear();
    }
}

/// Thread-safe web application session.
#[derive(Debug, Clone)]
pub struct Session {
    id: String,
    data: Arc<RwLock<HashMap<String, String>>>,
    flash: Arc<RwLock<FlashBag>>,
    created_at: i64,
    last_accessed: Arc<RwLock<i64>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Creates a new Session with a secure random identifier.
    pub fn new() -> Self {
        let random_id = format!("{:016x}{:016x}", rand_u64(), rand_u64());
        Self::with_id(random_id)
    }

    /// Creates a Session instance with an explicit identifier.
    pub fn with_id(id: impl Into<String>) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: id.into(),
            data: Arc::new(RwLock::new(HashMap::new())),
            flash: Arc::new(RwLock::new(FlashBag::new())),
            created_at: now,
            last_accessed: Arc::new(RwLock::new(now)),
        }
    }

    /// Returns the session ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Retrieves a value from the session.
    pub fn get(&self, key: &str) -> Option<String> {
        self.touch();
        self.data.read().ok().and_then(|d| d.get(key).cloned())
    }

    /// Stores a key-value pair in the session.
    pub fn set(&self, key: impl Into<String>, val: impl Into<String>) {
        self.touch();
        if let Ok(mut d) = self.data.write() {
            d.insert(key.into(), val.into());
        }
    }

    /// Removes a key from the session, returning the previous value if present.
    pub fn remove(&self, key: &str) -> Option<String> {
        self.touch();
        self.data.write().ok().and_then(|mut d| d.remove(key))
    }

    /// Checks if a key exists in the session.
    pub fn has(&self, key: &str) -> bool {
        self.touch();
        self.data.read().ok().map(|d| d.contains_key(key)).unwrap_or(false)
    }

    /// Flashes a message to be read on the next request (e.g. after a redirect).
    pub fn flash(&self, key: impl Into<String>, msg: impl Into<String>) {
        self.touch();
        if let Ok(mut f) = self.flash.write() {
            f.set(key, msg);
        }
    }

    /// Convenience helper to flash a success notification.
    pub fn flash_success(&self, msg: impl Into<String>) {
        self.flash("success", msg);
    }

    /// Convenience helper to flash an error notification.
    pub fn flash_error(&self, msg: impl Into<String>) {
        self.flash("error", msg);
    }

    /// Convenience helper to flash a warning notification.
    pub fn flash_warning(&self, msg: impl Into<String>) {
        self.flash("warning", msg);
    }

    /// Convenience helper to flash an informational notification.
    pub fn flash_info(&self, msg: impl Into<String>) {
        self.flash("info", msg);
    }

    /// Retrieves a flash message by key.
    pub fn get_flash(&self, key: &str) -> Option<String> {
        self.touch();
        self.flash.read().ok().and_then(|f| f.get(key).map(|s| s.to_string()))
    }

    /// Checks if a flash message exists for the given key.
    pub fn has_flash(&self, key: &str) -> bool {
        self.touch();
        self.flash.read().ok().map(|f| f.has(key)).unwrap_or(false)
    }

    /// Returns a copy of all current flash messages.
    pub fn all_flashes(&self) -> HashMap<String, String> {
        self.touch();
        self.flash.read().ok().map(|f| f.all()).unwrap_or_default()
    }

    /// Clears flash messages after they have been rendered.
    pub fn sweep_flashes(&self) {
        if let Ok(mut f) = self.flash.write() {
            f.sweep();
        }
    }

    /// Retrieves or generates a secure CSRF token associated with this session.
    pub fn csrf_token(&self) -> String {
        const CSRF_KEY: &str = "_csrf_token";
        if let Some(token) = self.get(CSRF_KEY) {
            token
        } else {
            let token = format!("{:016x}{:016x}", rand_u64(), rand_u64());
            self.set(CSRF_KEY, &token);
            token
        }
    }

    /// Verifies a submitted CSRF token against this session's active token.
    pub fn verify_csrf_token(&self, submitted_token: &str) -> bool {
        if let Some(active) = self.get("_csrf_token") {
            constant_time_eq(&active, submitted_token)
        } else {
            false
        }
    }

    /// Invalidates and clears all data and flash messages in this session.
    pub fn invalidate(&self) {
        if let Ok(mut d) = self.data.write() {
            d.clear();
        }
        if let Ok(mut f) = self.flash.write() {
            f.sweep();
        }
    }

    /// Formats the Set-Cookie HTTP header value for this session.
    pub fn cookie_header(&self) -> String {
        format!("terminus_session={}; Path=/; HttpOnly; SameSite=Lax", self.id)
    }

    fn touch(&self) {
        if let Ok(mut accessed) = self.last_accessed.write() {
            *accessed = Utc::now().timestamp();
        }
    }
}

/// Global session repository managing session lifecycle across threads.
#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the global singleton `SessionStore`.
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<SessionStore> = OnceLock::new();
        INSTANCE.get_or_init(SessionStore::new)
    }

    /// Retrieves an existing session by ID or creates a new one.
    pub fn get_or_create(&self, session_id: Option<&str>) -> Session {
        if let Some(id) = session_id {
            if let Ok(store) = self.sessions.read() {
                if let Some(s) = store.get(id) {
                    return s.clone();
                }
            }
        }

        let new_session = match session_id {
            Some(id) if !id.trim().is_empty() => Session::with_id(id),
            _ => Session::new(),
        };

        if let Ok(mut store) = self.sessions.write() {
            store.insert(new_session.id().to_string(), new_session.clone());
        }

        new_session
    }

    /// Stores a session explicitly.
    pub fn put(&self, session: Session) {
        if let Ok(mut store) = self.sessions.write() {
            store.insert(session.id().to_string(), session);
        }
    }

    /// Removes expired sessions older than the given TTL in seconds.
    pub fn prune(&self, ttl_seconds: i64) -> usize {
        let now = Utc::now().timestamp();
        let mut pruned = 0;
        if let Ok(mut store) = self.sessions.write() {
            store.retain(|_, s| {
                let last = s.last_accessed.read().ok().map(|g| *g).unwrap_or(s.created_at);
                let is_alive = (now - last) < ttl_seconds;
                if !is_alive {
                    pruned += 1;
                }
                is_alive
            });
        }
        pruned
    }
}

/// Helper returning pseudo-random 64-bit unsigned integer using nanosecond timestamps and memory entropy.
fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(123456789);
    let mut x = (seed as u64) ^ 0x517cc1b727220a95;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545f4914f6cdd1d)
}

/// Constant-time string equality check preventing timing side-channel attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}
