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

/// Pure-Rust SHA-256 implementation conforming to FIPS 180-4 / RFC 6234 returning 32 bytes
pub fn sha256(input: &[u8]) -> [u8; 32] {
    let mut h = [
        0x6a09e667u32, 0xbb67ae85u32, 0x3c6ef372u32, 0xa54ff53au32,
        0x510e527fu32, 0x9b05688cu32, 0x1f83d9abu32, 0x5be0cd19u32,
    ];

    let k = [
        0x428a2f98u32, 0x71374491u32, 0xb5c0fbcfu32, 0xe9b5dba5u32,
        0x3956c25bu32, 0x59f111f1u32, 0x923f82a4u32, 0xab1c5ed5u32,
        0xd807aa98u32, 0x12835b01u32, 0x243185beu32, 0x550c7dc3u32,
        0x72be5d74u32, 0x80deb1feu32, 0x9bdc06a7u32, 0xc19bf174u32,
        0xe49b69c1u32, 0xefbe4786u32, 0x0fc19dc6u32, 0x240ca1ccu32,
        0x2de92c6fu32, 0x4a7484aau32, 0x5cb0a9dcu32, 0x76f988dau32,
        0x983e5152u32, 0xa831c66du32, 0xb00327c8u32, 0xbf597fc7u32,
        0xc6e00bf3u32, 0xd5a79147u32, 0x06ca6351u32, 0x14292967u32,
        0x27b70a85u32, 0x2e1b2138u32, 0x4d2c6dfcu32, 0x53380d13u32,
        0x650a7354u32, 0x766a0abbu32, 0x81c2c92eu32, 0x92722c85u32,
        0xa2bfe8a1u32, 0xa81a664bu32, 0xc24b8b70u32, 0xc76c51a3u32,
        0xd192e819u32, 0xd6990624u32, 0xf40e3585u32, 0x106aa070u32,
        0x19a4c116u32, 0x1e376c08u32, 0x2748774cu32, 0x34b0bcb5u32,
        0x391c0cb3u32, 0x4ed8aa4au32, 0x5b9cca4fu32, 0x682e6ff3u32,
        0x748f82eeu32, 0x78a5636fu32, 0x84c87814u32, 0x8cc70208u32,
        0x90befffau32, 0xa4506cebu32, 0xbef9a3f7u32, 0xc67178f2u32,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut padded = input.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_val = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val.wrapping_add(s1).wrapping_add(ch).wrapping_add(k[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
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

/// Compute HMAC-SHA256 conforming to RFC 2104 returning raw 32 bytes
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k_prime = [0u8; 64];
    if key.len() > 64 {
        let key_hash = sha256(key);
        k_prime[..32].copy_from_slice(&key_hash);
    } else {
        k_prime[..key.len()].copy_from_slice(key);
    }

    let mut k_ipad = [0u8; 64];
    let mut k_opad = [0u8; 64];
    for i in 0..64 {
        k_ipad[i] = k_prime[i] ^ 0x36;
        k_opad[i] = k_prime[i] ^ 0x5c;
    }

    let mut inner_input = Vec::with_capacity(64 + message.len());
    inner_input.extend_from_slice(&k_ipad);
    inner_input.extend_from_slice(message);
    let inner_hash = sha256(&inner_input);

    let mut outer_input = Vec::with_capacity(64 + 32);
    outer_input.extend_from_slice(&k_opad);
    outer_input.extend_from_slice(&inner_hash);
    sha256(&outer_input)
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
