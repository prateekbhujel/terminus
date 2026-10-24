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
        let random_id = secure_random_hex(16);
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
            let token = secure_random_hex(16);
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

    /// Formats a cryptographically signed Set-Cookie HTTP header value for this session.
    pub fn signed_cookie_header(&self, secret: &[u8]) -> String {
        let sig = hmac_sha256(secret, self.id.as_bytes());
        let hex_sig: String = sig.iter().map(|b| format!("{:02x}", b)).collect();
        format!("terminus_session={}.{}; Path=/; HttpOnly; SameSite=Lax", self.id, hex_sig)
    }

    /// Verifies and extracts the session ID from a signed cookie value.
    pub fn verify_signed_cookie(cookie_val: &str, secret: &[u8]) -> Option<String> {
        let (id, submitted_sig) = cookie_val.split_once('.')?;
        let expected_sig = hmac_sha256(secret, id.as_bytes());
        let expected_hex: String = expected_sig.iter().map(|b| format!("{:02x}", b)).collect();
        if constant_time_eq(&expected_hex, submitted_sig) {
            Some(id.to_string())
        } else {
            None
        }
    }

    fn touch(&self) {
        if let Ok(mut accessed) = self.last_accessed.write() {
            *accessed = Utc::now().timestamp();
        }
    }
}

/// Computes HMAC-SHA256 according to RFC 2104 using pure-Rust SHA-256
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hashed = crate::token::sha256(key);
        k[..32].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];
    for i in 0..64 {
        o_key_pad[i] ^= k[i];
        i_key_pad[i] ^= k[i];
    }

    let mut inner = Vec::with_capacity(64 + message.len());
    inner.extend_from_slice(&i_key_pad);
    inner.extend_from_slice(message);
    let inner_hash = crate::token::sha256(&inner);

    let mut outer = Vec::with_capacity(64 + 32);
    outer.extend_from_slice(&o_key_pad);
    outer.extend_from_slice(&inner_hash);
    crate::token::sha256(&outer)
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

/// Axum extractor for `Session`, resolving or provisioning the active web session.
#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> std::result::Result<Self, Self::Rejection> {
        let mut session_id: Option<String> = None;
        if let Some(cookie_val) = parts.headers.get(axum::http::header::COOKIE).and_then(|v| v.to_str().ok()) {
            for part in cookie_val.split(';') {
                let trimmed = part.trim();
                if let Some(val) = trimmed.strip_prefix("terminus_session=") {
                    let val = val.trim();
                    if val.contains('.') {
                        if let Some(app_secret) = crate::helpers::config("app.key").or_else(|| crate::helpers::env("APP_KEY")) {
                            if let Some(verified) = Session::verify_signed_cookie(val, app_secret.as_bytes()) {
                                session_id = Some(verified);
                                break;
                            }
                        }
                    }
                    session_id = Some(val.to_string());
                    break;
                }
            }
        }

        let session = SessionStore::global().get_or_create(session_id.as_deref());
        crate::helpers::set_current_session(Some(session.clone()));
        Ok(session)
    }
}

/// Fills a buffer with cryptographically secure random bytes from the OS entropy source.
/// On Unix, reads directly from `/dev/urandom`. On other targets or as fallback,
/// mixes system time, high-resolution monotonic timestamps, memory addresses,
/// process ID, thread ID, and an atomic sequence into a SHA-256 hash.
pub fn secure_random_bytes(buf: &mut [u8]) {
    #[cfg(unix)]
    {
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            if f.read_exact(buf).is_ok() {
                return;
            }
        }
    }

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let stack_addr = &buf as *const _ as usize;
    let pid = std::process::id();
    let tid = format!("{:?}", std::thread::current().id());

    let mut entropy = Vec::with_capacity(128);
    entropy.extend_from_slice(&now.to_le_bytes());
    entropy.extend_from_slice(&count.to_le_bytes());
    entropy.extend_from_slice(&stack_addr.to_le_bytes());
    entropy.extend_from_slice(&pid.to_le_bytes());
    entropy.extend_from_slice(tid.as_bytes());

    let mut filled = 0;
    let mut round: u64 = 0;
    while filled < buf.len() {
        let mut round_entropy = entropy.clone();
        round_entropy.extend_from_slice(&round.to_le_bytes());
        let hash = crate::token::sha256(&round_entropy);
        let needed = std::cmp::min(buf.len() - filled, hash.len());
        buf[filled..filled + needed].copy_from_slice(&hash[..needed]);
        filled += needed;
        round += 1;
    }
}

/// Returns a cryptographically secure random hexadecimal string of the specified byte length.
pub fn secure_random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    secure_random_bytes(&mut buf);
    buf.iter().map(|b| format!("{:02x}", b)).collect()
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
