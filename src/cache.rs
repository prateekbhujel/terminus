//! Terminus Cache Engine
//!
//! Provides thread-safe in-memory caching with TTL expiration (`MemoryCache`)
//! and persistent SQLite-backed caching (`SqliteCache`) that survives server restarts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    expires_at: Option<u64>,
}

/// Unified cache driver contract
pub trait CacheDriver: Send + Sync {
    /// Retrieve a cached value by key
    fn get(&self, key: &str) -> Option<String>;

    /// Store a value in the cache with an optional TTL in seconds
    fn put(&self, key: &str, value: &str, ttl_secs: Option<u64>);

    /// Remove a cached value by key. Returns true if key was present.
    fn forget(&self, key: &str) -> bool;

    /// Wipe all cached keys
    fn flush(&self);

    /// Retrieve an item from the cache, or execute the given closure and store the result
    fn remember<F>(&self, key: &str, ttl_secs: u64, callback: F) -> String
    where
        F: FnOnce() -> String,
    {
        if let Some(val) = self.get(key) {
            return val;
        }
        let fresh = callback();
        self.put(key, &fresh, Some(ttl_secs));
        fresh
    }
}

/// Thread-safe in-memory cache with TTL expiration
#[derive(Clone, Default)]
pub struct MemoryCache {
    store: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn now_secs() -> u64 {
        chrono::Utc::now().timestamp().max(0) as u64
    }
}

impl CacheDriver for MemoryCache {
    fn get(&self, key: &str) -> Option<String> {
        let mut store = self.store.lock().unwrap();
        if let Some(entry) = store.get(key) {
            if let Some(exp) = entry.expires_at {
                if exp <= Self::now_secs() {
                    store.remove(key);
                    return None;
                }
            }
            return Some(entry.value.clone());
        }
        None
    }

    fn put(&self, key: &str, value: &str, ttl_secs: Option<u64>) {
        let expires_at = ttl_secs.map(|secs| Self::now_secs() + secs);
        let mut store = self.store.lock().unwrap();
        store.insert(
            key.to_string(),
            CacheEntry {
                value: value.to_string(),
                expires_at,
            },
        );
    }

    fn forget(&self, key: &str) -> bool {
        let mut store = self.store.lock().unwrap();
        store.remove(key).is_some()
    }

    fn flush(&self) {
        let mut store = self.store.lock().unwrap();
        store.clear();
    }
}

/// Persistent SQLite Cache Driver
///
/// Stores cached keys and TTLs in a dedicated SQLite table (`_terminus_cache`).
/// Data is persisted across server restarts and process lifecycles.
#[derive(Clone)]
pub struct SqliteCache {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteCache {
    /// Initialize SQLite cache pointing to a database path (or :memory:)
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = if path == ":memory:" {
            rusqlite::Connection::open_in_memory()?
        } else {
            rusqlite::Connection::open(path)?
        };
        let cache = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        cache.init_table()?;
        Ok(cache)
    }

    /// Initialize SQLite cache from an existing connection Arc
    pub fn from_arc(conn: Arc<Mutex<rusqlite::Connection>>) -> Result<Self, rusqlite::Error> {
        let cache = Self { conn };
        cache.init_table()?;
        Ok(cache)
    }

    fn init_table(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _terminus_cache (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                expires_at INTEGER
            )",
            [],
        )?;
        Ok(())
    }

    fn now_secs() -> i64 {
        chrono::Utc::now().timestamp().max(0)
    }
}

impl CacheDriver for SqliteCache {
    fn get(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_secs();

        let row: Result<(String, Option<i64>), rusqlite::Error> = conn.query_row(
            "SELECT value, expires_at FROM _terminus_cache WHERE key = ?1",
            rusqlite::params![key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );

        match row {
            Ok((value, expires_at)) => {
                if let Some(exp) = expires_at {
                    if exp <= now {
                        let _ = conn.execute("DELETE FROM _terminus_cache WHERE key = ?1", rusqlite::params![key]);
                        return None;
                    }
                }
                Some(value)
            }
            Err(_) => None,
        }
    }

    fn put(&self, key: &str, value: &str, ttl_secs: Option<u64>) {
        let conn = self.conn.lock().unwrap();
        let expires_at = ttl_secs.map(|s| Self::now_secs() + s as i64);

        let _ = conn.execute(
            "INSERT INTO _terminus_cache (key, value, expires_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, expires_at = excluded.expires_at",
            rusqlite::params![key, value, expires_at],
        );
    }

    fn forget(&self, key: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM _terminus_cache WHERE key = ?1", rusqlite::params![key])
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    fn flush(&self) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM _terminus_cache", []);
    }
}
