//! Terminus Token — Personal Access Token authentication for Rust & Axum
//! Lightweight personal access token guard with granular abilities and expiration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

/// SHA-256 using the maintained RustCrypto implementation, returning 32 bytes.
pub fn sha256(input: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(input).into()
}

/// Pure-Rust SHA-256 implementation returning a 64-character hex string
pub fn sha256_hex(input: &[u8]) -> String {
    let bytes = sha256(input);
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// HMAC-SHA256 using the maintained RustCrypto implementation.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use hmac::{KeyInit, Mac};
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(key)
        .expect("HMAC accepts keys of any length");
    mac.update(message);
    mac.finalize().into_bytes().into()
}

/// Compute HMAC-SHA256 returning a 64-character lowercase hex string
pub fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    let bytes = hmac_sha256(key, message);
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Constant-time comparison resistant to timing attacks
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Constant-time string equality check
pub fn constant_time_eq_str(a: &str, b: &str) -> bool {
    constant_time_eq(a.as_bytes(), b.as_bytes())
}

/// A stored Personal Access Token record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonalAccessToken {
    pub id: String,
    pub tokenable_type: String,
    pub tokenable_id: String,
    pub name: String,
    pub token: String, // Hashed token
    pub abilities: Vec<String>,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

impl PersonalAccessToken {
    /// Check if the token has the given ability
    pub fn can(&self, ability: &str) -> bool {
        if self.abilities.iter().any(|a| a == "*") {
            return true;
        }
        self.abilities.iter().any(|a| a == ability)
    }

    /// Check if the token lacks the given ability
    pub fn cant(&self, ability: &str) -> bool {
        !self.can(ability)
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(ref exp) = self.expires_at {
            let now = chrono::Utc::now().to_rfc3339();
            now > *exp
        } else {
            false
        }
    }
}

/// Returned when a new token is created, containing the one-time plain text secret
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAccessToken {
    pub access_token: PersonalAccessToken,
    pub plain_text_token: String,
}

/// Repository interface for managing Personal Access Tokens
pub trait TokenRepository: Send + Sync {
    fn create_token(
        &self,
        tokenable_type: &str,
        tokenable_id: &str,
        name: &str,
        abilities: Vec<String>,
        expires_at: Option<String>,
    ) -> Result<NewAccessToken, String>;

    fn find_token(&self, plain_text_token: &str) -> Option<PersonalAccessToken>;
    fn touch_last_used(&self, id: &str) -> Result<(), String>;
    fn revoke_token(&self, id: &str) -> Result<(), String>;
    fn revoke_all(&self, tokenable_type: &str, tokenable_id: &str) -> Result<usize, String>;
    fn list_for(&self, tokenable_type: &str, tokenable_id: &str) -> Vec<PersonalAccessToken>;
}

/// In-memory token store (ideal for tests and light workloads)
pub struct MemoryTokenStore {
    tokens: Mutex<HashMap<String, PersonalAccessToken>>,
    counter: Mutex<u64>,
}

impl MemoryTokenStore {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            counter: Mutex::new(1),
        }
    }
}

impl Default for MemoryTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenRepository for MemoryTokenStore {
    fn create_token(
        &self,
        tokenable_type: &str,
        tokenable_id: &str,
        name: &str,
        abilities: Vec<String>,
        expires_at: Option<String>,
    ) -> Result<NewAccessToken, String> {
        let mut count_guard = self.counter.lock().unwrap();
        let id = count_guard.to_string();
        *count_guard += 1;

        // Generate cryptographically secure token entropy
        let plain_secret = format!("{}_{}", id, crate::session::secure_random_hex(32));
        let hashed = sha256_hex(plain_secret.as_bytes());

        let token_record = PersonalAccessToken {
            id: id.clone(),
            tokenable_type: tokenable_type.to_string(),
            tokenable_id: tokenable_id.to_string(),
            name: name.to_string(),
            token: hashed,
            abilities: if abilities.is_empty() { vec!["*".to_string()] } else { abilities },
            last_used_at: None,
            expires_at,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let plain_text_token = format!("{}|{}", id, plain_secret);
        self.tokens.lock().unwrap().insert(id, token_record.clone());

        Ok(NewAccessToken {
            access_token: token_record,
            plain_text_token,
        })
    }

    fn find_token(&self, plain_text_token: &str) -> Option<PersonalAccessToken> {
        let parts: Vec<&str> = plain_text_token.splitn(2, '|').collect();
        if parts.len() != 2 {
            return None;
        }

        let id = parts[0];
        let plain_secret = parts[1];
        let hashed = sha256_hex(plain_secret.as_bytes());

        let guard = self.tokens.lock().unwrap();
        if let Some(token) = guard.get(id) {
            if token.token == hashed && !token.is_expired() {
                return Some(token.clone());
            }
        }
        None
    }

    fn touch_last_used(&self, id: &str) -> Result<(), String> {
        let mut guard = self.tokens.lock().unwrap();
        if let Some(token) = guard.get_mut(id) {
            token.last_used_at = Some(chrono::Utc::now().to_rfc3339());
            Ok(())
        } else {
            Err("Token not found".into())
        }
    }

    fn revoke_token(&self, id: &str) -> Result<(), String> {
        let mut guard = self.tokens.lock().unwrap();
        guard.remove(id);
        Ok(())
    }

    fn revoke_all(&self, tokenable_type: &str, tokenable_id: &str) -> Result<usize, String> {
        let mut guard = self.tokens.lock().unwrap();
        let initial_len = guard.len();
        guard.retain(|_, t| !(t.tokenable_type == tokenable_type && t.tokenable_id == tokenable_id));
        Ok(initial_len - guard.len())
    }

    fn list_for(&self, tokenable_type: &str, tokenable_id: &str) -> Vec<PersonalAccessToken> {
        let guard = self.tokens.lock().unwrap();
        guard
            .values()
            .filter(|t| t.tokenable_type == tokenable_type && t.tokenable_id == tokenable_id)
            .cloned()
            .collect()
    }
}

/// SQLite-backed token store
pub struct SqliteTokenStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteTokenStore {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Result<Self, rusqlite::Error> {
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS personal_access_tokens (
                id TEXT PRIMARY KEY,
                tokenable_type TEXT NOT NULL,
                tokenable_id TEXT NOT NULL,
                name TEXT NOT NULL,
                token TEXT NOT NULL UNIQUE,
                abilities TEXT NOT NULL DEFAULT '["*"]',
                last_used_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL
            );"#,
            [],
        )?;
        Ok(())
    }
}

impl TokenRepository for SqliteTokenStore {
    fn create_token(
        &self,
        tokenable_type: &str,
        tokenable_id: &str,
        name: &str,
        abilities: Vec<String>,
        expires_at: Option<String>,
    ) -> Result<NewAccessToken, String> {
        let conn = self.conn.lock().unwrap();
        let id = format!("{:x}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let plain_secret = format!("{}_{}", id, crate::session::secure_random_hex(32));
        let hashed = sha256_hex(plain_secret.as_bytes());
        let abilities_json = serde_json::to_string(&abilities).unwrap_or_else(|_| "[\"*\"]".into());
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO personal_access_tokens (id, tokenable_type, tokenable_id, name, token, abilities, last_used_at, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id,
                tokenable_type,
                tokenable_id,
                name,
                hashed,
                abilities_json,
                Option::<String>::None,
                expires_at,
                now,
            ],
        ).map_err(|e| e.to_string())?;

        let token_record = PersonalAccessToken {
            id: id.clone(),
            tokenable_type: tokenable_type.to_string(),
            tokenable_id: tokenable_id.to_string(),
            name: name.to_string(),
            token: hashed,
            abilities,
            last_used_at: None,
            expires_at,
            created_at: now,
        };

        Ok(NewAccessToken {
            access_token: token_record,
            plain_text_token: format!("{id}|{plain_secret}"),
        })
    }

    fn find_token(&self, plain_text_token: &str) -> Option<PersonalAccessToken> {
        let parts: Vec<&str> = plain_text_token.splitn(2, '|').collect();
        if parts.len() != 2 {
            return None;
        }

        let id = parts[0];
        let plain_secret = parts[1];
        let hashed = sha256_hex(plain_secret.as_bytes());

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tokenable_type, tokenable_id, name, token, abilities, last_used_at, expires_at, created_at
             FROM personal_access_tokens WHERE id = ?1 AND token = ?2"
        ).ok()?;

        let mut rows = stmt.query(rusqlite::params![id, hashed]).ok()?;
        if let Some(row) = rows.next().ok()? {
            let abilities_str: String = row.get(5).unwrap_or_default();
            let abilities: Vec<String> = serde_json::from_str(&abilities_str).unwrap_or_else(|_| vec!["*".into()]);
            let token = PersonalAccessToken {
                id: row.get(0).unwrap_or_default(),
                tokenable_type: row.get(1).unwrap_or_default(),
                tokenable_id: row.get(2).unwrap_or_default(),
                name: row.get(3).unwrap_or_default(),
                token: row.get(4).unwrap_or_default(),
                abilities,
                last_used_at: row.get(6).ok(),
                expires_at: row.get(7).ok(),
                created_at: row.get(8).unwrap_or_default(),
            };

            if !token.is_expired() {
                return Some(token);
            }
        }
        None
    }

    fn touch_last_used(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE personal_access_tokens SET last_used_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn revoke_token(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM personal_access_tokens WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn revoke_all(&self, tokenable_type: &str, tokenable_id: &str) -> Result<usize, String> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM personal_access_tokens WHERE tokenable_type = ?1 AND tokenable_id = ?2",
            rusqlite::params![tokenable_type, tokenable_id],
        ).map_err(|e| e.to_string())?;
        Ok(count)
    }

    fn list_for(&self, tokenable_type: &str, tokenable_id: &str) -> Vec<PersonalAccessToken> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, tokenable_type, tokenable_id, name, token, abilities, last_used_at, expires_at, created_at
             FROM personal_access_tokens WHERE tokenable_type = ?1 AND tokenable_id = ?2 ORDER BY created_at DESC"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map(rusqlite::params![tokenable_type, tokenable_id], |row| {
            let abilities_str: String = row.get(5)?;
            let abilities: Vec<String> = serde_json::from_str(&abilities_str).unwrap_or_else(|_| vec!["*".into()]);
            Ok(PersonalAccessToken {
                id: row.get(0)?,
                tokenable_type: row.get(1)?,
                tokenable_id: row.get(2)?,
                name: row.get(3)?,
                token: row.get(4)?,
                abilities,
                last_used_at: row.get(6)?,
                expires_at: row.get(7)?,
                created_at: row.get(8)?,
            })
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        rows.filter_map(|r| r.ok()).collect()
    }
}

/// Trait implemented by Models capable of issuing API tokens
pub trait HasApiTokens {
    fn tokenable_type(&self) -> &'static str { "User" }
    fn tokenable_id(&self) -> &str;

    fn create_token(
        &self,
        store: &dyn TokenRepository,
        name: &str,
        abilities: Vec<&str>,
    ) -> Result<NewAccessToken, String> {
        store.create_token(
            self.tokenable_type(),
            self.tokenable_id(),
            name,
            abilities.into_iter().map(String::from).collect(),
            None,
        )
    }

    fn create_token_with_expiry(
        &self,
        store: &dyn TokenRepository,
        name: &str,
        abilities: Vec<&str>,
        expires_at: Option<String>,
    ) -> Result<NewAccessToken, String> {
        store.create_token(
            self.tokenable_type(),
            self.tokenable_id(),
            name,
            abilities.into_iter().map(String::from).collect(),
            expires_at,
        )
    }
}

impl HasApiTokens for crate::domain::User {
    fn tokenable_id(&self) -> &str {
        &self.id
    }
}

/// Axum Extractor: Authenticated Bearer Token
#[derive(Debug, Clone)]
pub struct BearerToken(pub PersonalAccessToken);

#[async_trait]
impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract Authorization: Bearer <token>
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "Unauthenticated. Missing Authorization header." })),
                )
                    .into_response()
            })?;

        let token_str = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Invalid Authorization header format. Expected 'Bearer <token>'." })),
            )
                .into_response()
        })?;

        // Retrieve Token store from request extensions
        let store = parts
            .extensions
            .get::<Arc<dyn TokenRepository>>()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "TokenRepository not configured in request extensions." })),
                )
                    .into_response()
            })?;

        if let Some(token) = store.find_token(token_str) {
            let _ = store.touch_last_used(&token.id);
            Ok(BearerToken(token))
        } else {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Unauthenticated. Token is invalid or expired." })),
            )
                .into_response())
        }
    }
}

/// Token store over any database backend (SQLite, PostgreSQL, MySQL),
/// selected by the [`Database`](crate::orm::Database) connection URL.
pub struct DatabaseTokenStore {
    db: crate::orm::Database,
}

impl DatabaseTokenStore {
    pub fn new(db: crate::orm::Database) -> Result<Self, String> {
        let ddl = match db.kind() {
            crate::backend::DatabaseKind::Mysql => {
                "CREATE TABLE IF NOT EXISTS personal_access_tokens (
                    id VARCHAR(128) PRIMARY KEY,
                    tokenable_type VARCHAR(128) NOT NULL,
                    tokenable_id VARCHAR(128) NOT NULL,
                    name VARCHAR(255) NOT NULL,
                    token VARCHAR(255) NOT NULL UNIQUE,
                    abilities TEXT NOT NULL,
                    last_used_at VARCHAR(64),
                    expires_at VARCHAR(64),
                    created_at VARCHAR(64) NOT NULL
                )"
            }
            _ => {
                "CREATE TABLE IF NOT EXISTS personal_access_tokens (
                    id TEXT PRIMARY KEY,
                    tokenable_type TEXT NOT NULL,
                    tokenable_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    token TEXT NOT NULL UNIQUE,
                    abilities TEXT NOT NULL DEFAULT '[\"*\"]',
                    last_used_at TEXT,
                    expires_at TEXT,
                    created_at TEXT NOT NULL
                )"
            }
        };
        db.execute(ddl).map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    fn placeholder(&self, index: usize) -> String {
        self.db.kind().placeholder(index)
    }

    fn row_to_token(
        row: &std::collections::HashMap<String, String>,
    ) -> Option<PersonalAccessToken> {
        let abilities: Vec<String> = row
            .get("abilities")
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_else(|| vec!["*".into()]);
        Some(PersonalAccessToken {
            id: row.get("id")?.clone(),
            tokenable_type: row.get("tokenable_type")?.clone(),
            tokenable_id: row.get("tokenable_id")?.clone(),
            name: row.get("name")?.clone(),
            token: row.get("token")?.clone(),
            abilities,
            last_used_at: row.get("last_used_at").cloned(),
            expires_at: row.get("expires_at").cloned(),
            created_at: row.get("created_at")?.clone(),
        })
    }
}

impl TokenRepository for DatabaseTokenStore {
    fn create_token(
        &self,
        tokenable_type: &str,
        tokenable_id: &str,
        name: &str,
        abilities: Vec<String>,
        expires_at: Option<String>,
    ) -> Result<NewAccessToken, String> {
        let id = format!("{:x}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let plain_secret = format!("{}_{}", id, crate::session::secure_random_hex(32));
        let hashed = sha256_hex(plain_secret.as_bytes());
        let abilities = if abilities.is_empty() {
            vec!["*".to_string()]
        } else {
            abilities
        };
        let abilities_json =
            serde_json::to_string(&abilities).unwrap_or_else(|_| "[\"*\"]".into());
        let now = chrono::Utc::now().to_rfc3339();
        let ph = |i: usize| self.placeholder(i);
        let sql = format!(
            "INSERT INTO personal_access_tokens
                (id, tokenable_type, tokenable_id, name, token, abilities, expires_at, created_at)
             VALUES ({}, {}, {}, {}, {}, {}, {}, {})",
            ph(1),
            ph(2),
            ph(3),
            ph(4),
            ph(5),
            ph(6),
            ph(7),
            ph(8)
        );
        // Nullable expires_at goes through a text parameter; empty means NULL
        // is emulated by storing NULL via a second statement when absent.
        if let Some(expires) = &expires_at {
            self.db.execute_params(
                &sql,
                &[
                    id.clone(),
                    tokenable_type.to_string(),
                    tokenable_id.to_string(),
                    name.to_string(),
                    hashed.clone(),
                    abilities_json,
                    expires.clone(),
                    now.clone(),
                ],
            )?;
        } else {
            let sql_null = format!(
                "INSERT INTO personal_access_tokens
                    (id, tokenable_type, tokenable_id, name, token, abilities, expires_at, created_at)
                 VALUES ({}, {}, {}, {}, {}, {}, NULL, {})",
                ph(1),
                ph(2),
                ph(3),
                ph(4),
                ph(5),
                ph(6),
                ph(7)
            );
            self.db.execute_params(
                &sql_null,
                &[
                    id.clone(),
                    tokenable_type.to_string(),
                    tokenable_id.to_string(),
                    name.to_string(),
                    hashed.clone(),
                    abilities_json,
                    now.clone(),
                ],
            )?;
        }
        let token_record = PersonalAccessToken {
            id: id.clone(),
            tokenable_type: tokenable_type.to_string(),
            tokenable_id: tokenable_id.to_string(),
            name: name.to_string(),
            token: hashed,
            abilities,
            last_used_at: None,
            expires_at,
            created_at: now,
        };
        Ok(NewAccessToken {
            access_token: token_record,
            plain_text_token: format!("{id}|{plain_secret}"),
        })
    }

    fn find_token(&self, plain_text_token: &str) -> Option<PersonalAccessToken> {
        let (id, plain_secret) = plain_text_token.split_once('|')?;
        let hashed = sha256_hex(plain_secret.as_bytes());
        let sql = format!(
            "SELECT id, tokenable_type, tokenable_id, name, token, abilities, last_used_at, expires_at, created_at
             FROM personal_access_tokens WHERE id = {} AND token = {}",
            self.placeholder(1),
            self.placeholder(2)
        );
        let rows = self
            .db
            .query_params(&sql, &[id.to_string(), hashed])
            .ok()?;
        let token = rows.into_iter().next().and_then(|row| Self::row_to_token(&row))?;
        if token.is_expired() {
            return None;
        }
        Some(token)
    }

    fn touch_last_used(&self, id: &str) -> Result<(), String> {
        let sql = format!(
            "UPDATE personal_access_tokens SET last_used_at = {} WHERE id = {}",
            self.placeholder(1),
            self.placeholder(2)
        );
        self.db
            .execute_params(&sql, &[chrono::Utc::now().to_rfc3339(), id.to_string()])?;
        Ok(())
    }

    fn revoke_token(&self, id: &str) -> Result<(), String> {
        let sql = format!(
            "DELETE FROM personal_access_tokens WHERE id = {}",
            self.placeholder(1)
        );
        self.db.execute_params(&sql, &[id.to_string()])?;
        Ok(())
    }

    fn revoke_all(&self, tokenable_type: &str, tokenable_id: &str) -> Result<usize, String> {
        let sql = format!(
            "DELETE FROM personal_access_tokens WHERE tokenable_type = {} AND tokenable_id = {}",
            self.placeholder(1),
            self.placeholder(2)
        );
        let changed = self.db.execute_params(
            &sql,
            &[tokenable_type.to_string(), tokenable_id.to_string()],
        )?;
        Ok(changed as usize)
    }

    fn list_for(&self, tokenable_type: &str, tokenable_id: &str) -> Vec<PersonalAccessToken> {
        let sql = format!(
            "SELECT id, tokenable_type, tokenable_id, name, token, abilities, last_used_at, expires_at, created_at
             FROM personal_access_tokens WHERE tokenable_type = {} AND tokenable_id = {} ORDER BY created_at DESC",
            self.placeholder(1),
            self.placeholder(2)
        );
        self.db
            .query_params(&sql, &[tokenable_type.to_string(), tokenable_id.to_string()])
            .unwrap_or_default()
            .iter()
            .filter_map(Self::row_to_token)
            .collect()
    }
}
