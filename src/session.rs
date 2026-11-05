//! Web session and flash messaging engine for Terminus.
//!
//! Provides thread-safe, cookie-backed session state, one-time flash messages
//! for web redirects, CSRF token management, and automatic lifecycle expiration.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// One-time flash message storage that survives across redirects and is consumed on the next request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    expires_at: Arc<AtomicI64>,
    invalidated: Arc<AtomicBool>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Creates a new Session with a secure random identifier.
    pub fn new() -> Self {
        let random_id = secure_random_hex(32);
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
            expires_at: Arc::new(AtomicI64::new(0)),
            invalidated: Arc::new(AtomicBool::new(false)),
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
        self.invalidated.store(true, Ordering::SeqCst);
        if let Ok(mut d) = self.data.write() {
            d.clear();
        }
        if let Ok(mut f) = self.flash.write() {
            f.sweep();
        }
    }

    /// Whether this session has been revoked in the current process.
    pub fn is_invalidated(&self) -> bool {
        self.invalidated.load(Ordering::SeqCst)
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

/// Computes HMAC-SHA256 using the maintained RustCrypto implementation.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use hmac::{Hmac, KeyInit, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(message);
    let mut output = [0u8; 32];
    output.copy_from_slice(&mac.finalize().into_bytes());
    output
}

/// Session repository. The default backend is process-local; `sqlite` persists
/// server-side data, flash messages, expiry, and revocation across restarts.
#[derive(Debug)]
pub struct SessionStore {
    backend: SessionBackend,
}

enum SessionBackend {
    Memory {
        sessions: RwLock<HashMap<String, Session>>,
        revoked: RwLock<HashSet<String>>,
    },
    Database(crate::backend::AnyConnection),
    #[cfg(feature = "redis")]
    Redis(redis::Client),
}

impl std::fmt::Debug for SessionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory { .. } => write!(f, "Memory"),
            Self::Database(conn) => write!(f, "Database({:?})", conn.kind()),
            #[cfg(feature = "redis")]
            Self::Redis(_) => write!(f, "Redis"),
        }
    }
}

const DEFAULT_TTL: i64 = 2 * 60 * 60;

#[cfg(feature = "redis")]
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredSession {
    data: HashMap<String, String>,
    flashes: FlashBag,
    created_at: i64,
    last_accessed: i64,
    expires_at: i64,
}

impl Default for SessionStore {
    fn default() -> Self { Self::new() }
}

impl SessionStore {
    pub fn new() -> Self {
        Self { backend: SessionBackend::Memory {
            sessions: RwLock::new(HashMap::new()), revoked: RwLock::new(HashSet::new()),
        } }
    }

    /// Opens a durable SQLite store without modifying application tables.
    pub fn sqlite(path: impl AsRef<Path>) -> Result<Self, String> {
        let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS _terminus_sessions (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL,
            flashes TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_accessed INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            revoked INTEGER NOT NULL DEFAULT 0
        ); CREATE INDEX IF NOT EXISTS _terminus_sessions_expiry ON _terminus_sessions(expires_at);")
            .map_err(|e| e.to_string())?;
        Ok(Self {
            backend: SessionBackend::Database(crate::backend::AnyConnection::Sqlite(
                Arc::new(Mutex::new(conn)),
            )),
        })
    }

    /// Connect to a session store by URL: a SQLite path/`sqlite://` URL,
    /// `postgres://...`, `mysql://`/`mariadb://...`, or `redis://...`
    /// (with the `redis` feature). Server stores share the
    /// `_terminus_sessions` table and preserve expiry and revocation
    /// semantics across restarts and processes.
    pub fn connect(url: &str) -> Result<Self, String> {
        #[cfg(feature = "redis")]
        if url.trim().to_ascii_lowercase().starts_with("redis://") {
            let client = redis::Client::open(url).map_err(|e| e.to_string())?;
            return Ok(Self {
                backend: SessionBackend::Redis(client),
            });
        }
        if crate::backend::DatabaseKind::from_url(url) == crate::backend::DatabaseKind::Sqlite {
            let path = url.trim().strip_prefix("sqlite://").unwrap_or(url.trim());
            return Self::sqlite(path);
        }
        let conn = crate::backend::AnyConnection::connect(url)?;
        let ddl = match conn.kind() {
            crate::backend::DatabaseKind::Postgres => {
                "CREATE TABLE IF NOT EXISTS _terminus_sessions (
                    id TEXT PRIMARY KEY,
                    data TEXT NOT NULL,
                    flashes TEXT NOT NULL,
                    created_at BIGINT NOT NULL,
                    last_accessed BIGINT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    revoked BIGINT NOT NULL DEFAULT 0
                )"
            }
            crate::backend::DatabaseKind::Mysql => {
                "CREATE TABLE IF NOT EXISTS _terminus_sessions (
                    id VARCHAR(128) PRIMARY KEY,
                    data TEXT NOT NULL,
                    flashes TEXT NOT NULL,
                    created_at BIGINT NOT NULL,
                    last_accessed BIGINT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    revoked BIGINT NOT NULL DEFAULT 0
                )"
            }
            crate::backend::DatabaseKind::Sqlite => unreachable!("handled above"),
        };
        conn.execute(ddl, &[])?;
        Ok(Self {
            backend: SessionBackend::Database(conn),
        })
    }

    pub fn global() -> &'static Self { Self::global_instance().as_ref() }

    fn global_instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<SessionStore>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(Self::new()))
    }

    pub fn global_arc() -> Arc<Self> { Self::global_instance().clone() }

    /// Looks up an existing, unexpired, non-revoked session. Storage failures
    /// return an error; use this fallible API in request authentication.
    pub fn load(&self, id: &str) -> Result<Option<Session>, String> {
        let now = Utc::now().timestamp();
        match &self.backend {
            SessionBackend::Memory { sessions, .. } => {
                Ok(sessions.read().map_err(|_| "Session store lock poisoned")?
                    .get(id).filter(|s| !s.is_invalidated()
                        && s.expires_at.load(Ordering::SeqCst) > now).cloned())
            }
            SessionBackend::Database(conn) => {
                let kind = conn.kind();
                let sql = format!(
                    "SELECT data, flashes, created_at, last_accessed, expires_at FROM _terminus_sessions
                     WHERE id = {} AND revoked = 0 AND expires_at > {}",
                    kind.placeholder(1),
                    kind.placeholder(2)
                );
                let rows = conn.query(&sql, &[id.to_string(), now.to_string()])?;
                let Some(row) = rows.into_iter().next() else {
                    return Ok(None);
                };
                let parse = |key: &str| -> Result<i64, String> {
                    row.get(key)
                        .and_then(|value| value.parse::<i64>().ok())
                        .ok_or_else(|| format!("Corrupt session row: invalid {key}"))
                };
                Ok(Some(Session {
                    id: id.to_string(),
                    data: Arc::new(RwLock::new(
                        serde_json::from_str(row.get("data").map(String::as_str).unwrap_or("{}"))
                            .map_err(|e| e.to_string())?,
                    )),
                    flash: Arc::new(RwLock::new(
                        serde_json::from_str(row.get("flashes").map(String::as_str).unwrap_or("{}"))
                            .map_err(|e| e.to_string())?,
                    )),
                    created_at: parse("created_at")?,
                    last_accessed: Arc::new(RwLock::new(parse("last_accessed")?)),
                    expires_at: Arc::new(AtomicI64::new(parse("expires_at")?)),
                    invalidated: Arc::new(AtomicBool::new(false)),
                }))
            }
            #[cfg(feature = "redis")]
            SessionBackend::Redis(client) => {
                use redis::Commands;
                let mut conn = client
                    .get_connection()
                    .map_err(|e| e.to_string())?;
                let revoked: bool = conn
                    .sismember("terminus:session:revoked", id)
                    .map_err(|e| e.to_string())?;
                if revoked {
                    return Ok(None);
                }
                let stored: Option<String> = conn
                    .get(format!("terminus:session:{id}"))
                    .map_err(|e| e.to_string())?;
                let Some(stored) = stored else {
                    return Ok(None);
                };
                let stored: StoredSession =
                    serde_json::from_str(&stored).map_err(|e| e.to_string())?;
                if stored.expires_at <= now {
                    return Ok(None);
                }
                Ok(Some(Session {
                    id: id.to_string(),
                    data: Arc::new(RwLock::new(stored.data)),
                    flash: Arc::new(RwLock::new(stored.flashes)),
                    created_at: stored.created_at,
                    last_accessed: Arc::new(RwLock::new(stored.last_accessed)),
                    expires_at: Arc::new(AtomicI64::new(stored.expires_at)),
                    invalidated: Arc::new(AtomicBool::new(false)),
                }))
            }
        }
    }

    /// Lookup-only compatibility helper. Storage errors yield no session.
    pub fn get(&self, id: &str) -> Option<Session> { self.load(id).ok().flatten() }

    /// Unknown client-provided IDs never become server session IDs.
    pub fn get_or_create(&self, session_id: Option<&str>) -> Session {
        if let Some(session) = session_id.and_then(|id| self.load(id).expect("Session lookup failed")) {
            return session;
        }
        let session = Session::new();
        self.save(&session).expect("Session storage failed");
        session
    }

    /// Persists session state with the default two-hour idle timeout.
    pub fn save(&self, session: &Session) -> Result<(), String> {
        self.save_with_ttl(session, DEFAULT_TTL)
    }

    pub(crate) fn save_with_ttl(&self, session: &Session, ttl: i64) -> Result<(), String> {
        let now = Utc::now().timestamp();
        let previous_expiry = session.expires_at.load(Ordering::SeqCst);
        if session.is_invalidated() || (previous_expiry != 0 && previous_expiry <= now) {
            return Err("Session has expired or been revoked".into());
        }
        let expires_at = now.checked_add(ttl).ok_or("Session expiry overflow")?;
        match &self.backend {
            SessionBackend::Memory { sessions, revoked } => {
                let revoked = revoked.read().map_err(|_| "Session revocation lock poisoned")?;
                if revoked.contains(session.id()) { return Err("Session has been revoked".into()); }
                sessions.write().map_err(|_| "Session store lock poisoned")?
                    .insert(session.id().to_string(), session.clone());
            }
            SessionBackend::Database(conn) => {
                let data = serde_json::to_string(&*session.data.read().map_err(|_| "Session data lock poisoned")?)
                    .map_err(|e| e.to_string())?;
                let flashes = serde_json::to_string(&*session.flash.read().map_err(|_| "Session flash lock poisoned")?)
                    .map_err(|e| e.to_string())?;
                let kind = conn.kind();
                let mut guard = conn.lock();
                guard.begin()?;
                let work = (|guard: &mut crate::backend::AnyGuard<'_>| -> Result<(), String> {
                    let existing_sql = format!(
                        "SELECT revoked, expires_at FROM _terminus_sessions WHERE id = {}",
                        kind.placeholder(1)
                    );
                    let existing = guard.query(&existing_sql, &[session.id().to_string()])?;
                    if let Some(row) = existing.into_iter().next() {
                        let revoked = row.get("revoked").map(String::as_str) == Some("1");
                        let stored_expiry = row
                            .get("expires_at")
                            .and_then(|value| value.parse::<i64>().ok())
                            .unwrap_or(0);
                        if revoked || stored_expiry <= now {
                            return Err("Session has expired or been revoked".into());
                        }
                        let update_sql = format!(
                            "UPDATE _terminus_sessions SET data = {}, flashes = {},
                                last_accessed = {}, expires_at = {} WHERE id = {}",
                            kind.placeholder(1),
                            kind.placeholder(2),
                            kind.placeholder(3),
                            kind.placeholder(4),
                            kind.placeholder(5)
                        );
                        guard.execute(
                            &update_sql,
                            &[
                                data.clone(),
                                flashes.clone(),
                                now.to_string(),
                                expires_at.to_string(),
                                session.id().to_string(),
                            ],
                        )?;
                    } else {
                        let insert_sql = format!(
                            "INSERT INTO _terminus_sessions
                                (id, data, flashes, created_at, last_accessed, expires_at, revoked)
                             VALUES ({}, {}, {}, {}, {}, {}, 0)",
                            kind.placeholder(1),
                            kind.placeholder(2),
                            kind.placeholder(3),
                            kind.placeholder(4),
                            kind.placeholder(5),
                            kind.placeholder(6)
                        );
                        guard.execute(
                            &insert_sql,
                            &[
                                session.id().to_string(),
                                data,
                                flashes,
                                session.created_at.to_string(),
                                now.to_string(),
                                expires_at.to_string(),
                            ],
                        )?;
                    }
                    Ok(())
                })(&mut guard);
                match work {
                    Ok(()) => guard.commit()?,
                    Err(error) => {
                        let _ = guard.rollback();
                        return Err(error);
                    }
                }
            }
            #[cfg(feature = "redis")]
            SessionBackend::Redis(client) => {
                use redis::Commands;
                let mut conn = client
                    .get_connection()
                    .map_err(|e| e.to_string())?;
                let revoked: bool = conn
                    .sismember("terminus:session:revoked", session.id())
                    .map_err(|e| e.to_string())?;
                if revoked {
                    return Err("Session has been revoked".into());
                }
                let stored = StoredSession {
                    data: session
                        .data
                        .read()
                        .map_err(|_| "Session data lock poisoned")?
                        .clone(),
                    flashes: session
                        .flash
                        .read()
                        .map_err(|_| "Session flash lock poisoned")?
                        .clone(),
                    created_at: session.created_at,
                    last_accessed: now,
                    expires_at,
                };
                let serialized = serde_json::to_string(&stored).map_err(|e| e.to_string())?;
                let _: () = conn
                    .set_ex(
                        format!("terminus:session:{}", session.id()),
                        serialized,
                        ttl.max(1) as u64,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        session.expires_at.store(expires_at, Ordering::SeqCst);
        Ok(())
    }

    /// Compatibility wrapper. Use `save` to handle storage errors explicitly.
    pub fn put(&self, session: Session) { self.save(&session).expect("Session storage failed"); }

    /// Revokes a session durably. Tombstones deliberately prevent stale in-flight
    /// requests from recreating a logged-out session when they later save.
    pub fn remove(&self, id: &str) -> Result<bool, String> {
        match &self.backend {
            SessionBackend::Memory { sessions, revoked } => {
                let mut revoked = revoked.write().map_err(|_| "Session revocation lock poisoned")?;
                revoked.insert(id.to_string());
                let session = sessions.write().map_err(|_| "Session store lock poisoned")?.remove(id);
                if let Some(session) = &session { session.invalidate(); }
                Ok(session.is_some())
            }
            SessionBackend::Database(conn) => {
                let kind = conn.kind();
                let now = Utc::now().timestamp();
                let mut guard = conn.lock();
                guard.begin()?;
                let work = (|guard: &mut crate::backend::AnyGuard<'_>| -> Result<bool, String> {
                    let existing_sql = format!(
                        "SELECT revoked FROM _terminus_sessions WHERE id = {}",
                        kind.placeholder(1)
                    );
                    let existing = guard.query(&existing_sql, &[id.to_string()])?;
                    let existed = !existing.is_empty();
                    if existed {
                        let update_sql = format!(
                            "UPDATE _terminus_sessions SET data = '{{}}', flashes = '{{}}', revoked = 1 WHERE id = {}",
                            kind.placeholder(1)
                        );
                        guard.execute(&update_sql, &[id.to_string()])?;
                    } else {
                        let insert_sql = format!(
                            "INSERT INTO _terminus_sessions
                                (id, data, flashes, created_at, last_accessed, expires_at, revoked)
                             VALUES ({}, '{{}}', '{{}}', {}, {}, {}, 1)",
                            kind.placeholder(1),
                            kind.placeholder(2),
                            kind.placeholder(3),
                            kind.placeholder(4)
                        );
                        guard.execute(
                            &insert_sql,
                            &[
                                id.to_string(),
                                now.to_string(),
                                now.to_string(),
                                now.to_string(),
                            ],
                        )?;
                    }
                    Ok(existed)
                })(&mut guard);
                match work {
                    Ok(existed) => {
                        guard.commit()?;
                        Ok(existed)
                    }
                    Err(error) => {
                        let _ = guard.rollback();
                        Err(error)
                    }
                }
            }
            #[cfg(feature = "redis")]
            SessionBackend::Redis(client) => {
                use redis::Commands;
                let mut conn = client
                    .get_connection()
                    .map_err(|e| e.to_string())?;
                let _: () = conn
                    .sadd("terminus:session:revoked", id)
                    .map_err(|e| e.to_string())?;
                let deleted: usize = conn
                    .del(format!("terminus:session:{id}"))
                    .map_err(|e| e.to_string())?;
                Ok(deleted > 0)
            }
        }
    }

    /// Revokes expired records. Durable revocation tombstones are retained so
    /// an old request snapshot cannot resurrect a session after pruning.
    pub fn prune(&self, ttl_seconds: i64) -> usize {
        let cutoff = Utc::now().timestamp().saturating_sub(ttl_seconds);
        match &self.backend {
            SessionBackend::Memory { sessions, revoked } => {
                let mut revoked = revoked.write().expect("Session revocation lock poisoned");
                let mut sessions = sessions.write().expect("Session store lock poisoned");
                let before = sessions.len();
                sessions.retain(|id, session| {
                    let keep = *session.last_accessed.read().expect("Session access lock poisoned") > cutoff;
                    if !keep { session.invalidate(); revoked.insert(id.clone()); }
                    keep
                });
                before - sessions.len()
            }
            SessionBackend::Database(conn) => {
                let kind = conn.kind();
                let sql = format!(
                    "UPDATE _terminus_sessions SET revoked = 1, data = '{{}}', flashes = '{{}}'
                     WHERE revoked = 0 AND last_accessed <= {}",
                    kind.placeholder(1)
                );
                conn.execute(&sql, &[cutoff.to_string()])
                    .expect("Session pruning failed") as usize
            }
            #[cfg(feature = "redis")]
            SessionBackend::Redis(_) => 0,
        }
    }
}

/// Explicit per-application session settings. Persistent stores require a signing
/// key of at least 32 bytes. Keep the same key across application restarts.
#[derive(Clone)]
pub struct SessionConfig {
    pub(crate) store: Arc<SessionStore>,
    signing_key: Arc<Vec<u8>>,
    pub(crate) ttl_seconds: i64,
    secure: bool,
}

impl std::fmt::Debug for SessionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionConfig").field("ttl_seconds", &self.ttl_seconds)
            .field("secure", &self.secure).finish_non_exhaustive()
    }
}

impl SessionConfig {
    pub fn new(store: Arc<SessionStore>, signing_key: impl AsRef<[u8]>) -> Result<Self, String> {
        let signing_key = signing_key.as_ref();
        if signing_key.len() < 32 { return Err("Session signing key must contain at least 32 bytes".into()); }
        Ok(Self { store, signing_key: Arc::new(signing_key.to_vec()), ttl_seconds: DEFAULT_TTL, secure: true })
    }

    pub fn sqlite(path: impl AsRef<Path>, signing_key: impl AsRef<[u8]>) -> Result<Self, String> {
        Self::new(Arc::new(SessionStore::sqlite(path)?), signing_key)
    }

    /// Sessions on any database backend by URL (`postgres://...`,
    /// `mysql://...`, or SQLite), sharing the `_terminus_sessions` table.
    pub fn database(url: &str, signing_key: impl AsRef<[u8]>) -> Result<Self, String> {
        Self::new(Arc::new(SessionStore::connect(url)?), signing_key)
    }

    /// Sessions in Redis (`redis` feature), with durable revocation.
    #[cfg(feature = "redis")]
    pub fn redis(url: &str, signing_key: impl AsRef<[u8]>) -> Result<Self, String> {
        Self::new(Arc::new(SessionStore::connect(url)?), signing_key)
    }

    pub fn ttl(mut self, ttl: Duration) -> Result<Self, String> {
        let seconds = i64::try_from(ttl.as_secs()).map_err(|_| "Session TTL is too large")?;
        if seconds == 0 { return Err("Session TTL must be at least one second".into()); }
        self.ttl_seconds = seconds;
        Ok(self)
    }

    /// Disable only for local HTTP development; enabled by default for HTTPS.
    pub fn secure_cookie(mut self, secure: bool) -> Self { self.secure = secure; self }

    pub fn store(&self) -> Arc<SessionStore> { self.store.clone() }

    pub(crate) fn cookie_header(&self, session: &Session) -> String {
        let mut cookie = session.signed_cookie_header(&self.signing_key);
        cookie.push_str(&format!("; Max-Age={}", self.ttl_seconds));
        if self.secure { cookie.push_str("; Secure"); }
        cookie
    }

    pub(crate) fn clear_cookie_header(&self) -> String {
        let mut cookie = "terminus_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax".to_string();
        if self.secure { cookie.push_str("; Secure"); }
        cookie
    }

    pub(crate) fn cookie_id(&self, cookie: Option<&str>) -> Result<Option<String>, String> {
        session_cookie_value(cookie).map(|value| {
            Session::verify_signed_cookie(value, &self.signing_key)
                .ok_or_else(|| "Invalid session cookie signature".to_string())
        }).transpose()
    }
}

pub(crate) fn session_cookie_value(cookie: Option<&str>) -> Option<&str> {
    cookie?.split(';').find_map(|part| part.trim().strip_prefix("terminus_session="))
}

/// Axum extractor for `Session`, resolving or provisioning the active web session.
/// `App::build` scopes helper access to the request and adds session response cookies.
#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> std::result::Result<Self, Self::Rejection> {
        if let Some(session) = crate::helpers::explicit_session() {
            return Ok(session);
        }
        let session = parts.extensions.get::<Session>().cloned()
            .or_else(crate::helpers::scoped_session)
            .unwrap_or_else(|| resolve_session_cookie(
                parts.headers.get(axum::http::header::COOKIE).and_then(|value| value.to_str().ok())
            ));
        parts.extensions.insert(session.clone());
        crate::helpers::set_current_session(Some(session.clone()));
        Ok(session)
    }
}

pub(crate) fn resolve_session_cookie(cookie: Option<&str>) -> Session {
    let value = session_cookie_value(cookie);
    let key = crate::helpers::config("app.key").or_else(|| crate::helpers::env("APP_KEY"));
    let id = match (value, key) {
        (Some(value), Some(key)) => Session::verify_signed_cookie(value, key.as_bytes()),
        (Some(value), None) if !value.contains('.') => Some(value.to_string()),
        _ => None,
    };
    SessionStore::global().get_or_create(id.as_deref())
}

/// Fills a buffer from the operating system's cryptographic random source on
/// every supported platform. OS entropy failure stops execution rather than
/// substituting predictable bytes.
pub fn secure_random_bytes(buf: &mut [u8]) {
    getrandom::fill(buf).expect("Operating system entropy unavailable");
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
