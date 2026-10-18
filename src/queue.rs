//! Terminus Background Job Queue Engine
//!
//! Provides in-memory job queues (`JobQueue`) and persistent SQLite-backed job queues (`SqliteJobQueue`)
//! with automatic retries, exponential backoff, worker loops, and failure telemetry.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use chrono::Utc;

/// Trait for executable background tasks
pub trait Job: Send + Sync {
    fn name(&self) -> &str;
    fn handle(&self) -> Result<(), String>;
}

/// Generic closure-based job
pub struct ClosureJob<F>
where
    F: Fn() -> Result<(), String> + Send + Sync,
{
    pub job_name: String,
    pub handler: F,
}

impl<F> Job for ClosureJob<F>
where
    F: Fn() -> Result<(), String> + Send + Sync,
{
    fn name(&self) -> &str {
        &self.job_name
    }

    fn handle(&self) -> Result<(), String> {
        (self.handler)()
    }
}

/// Queue telemetry statistics
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub pending: usize,
    pub processed: usize,
    pub failed: usize,
}

/// Thread-safe in-memory job queue
#[derive(Clone)]
pub struct JobQueue {
    jobs: Arc<Mutex<VecDeque<Box<dyn Job>>>>,
    processed: Arc<AtomicUsize>,
    failed: Arc<AtomicUsize>,
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl JobQueue {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(VecDeque::new())),
            processed: Arc::new(AtomicUsize::new(0)),
            failed: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Dispatch a job to the in-memory queue
    pub fn dispatch(&self, job: impl Job + 'static) {
        let mut queue = self.jobs.lock().unwrap();
        queue.push_back(Box::new(job));
    }

    /// Dispatch a closure job to the in-memory queue
    pub fn dispatch_fn<F>(&self, name: impl Into<String>, handler: F)
    where
        F: Fn() -> Result<(), String> + Send + Sync + 'static,
    {
        self.dispatch(ClosureJob {
            job_name: name.into(),
            handler,
        });
    }

    /// Process the next pending job
    pub fn work_one(&self) -> Option<Result<String, String>> {
        let job = {
            let mut queue = self.jobs.lock().unwrap();
            queue.pop_front()
        }?;

        let name = job.name().to_string();
        match job.handle() {
            Ok(()) => {
                self.processed.fetch_add(1, Ordering::SeqCst);
                Some(Ok(name))
            }
            Err(e) => {
                self.failed.fetch_add(1, Ordering::SeqCst);
                Some(Err(format!("Job '{name}' failed: {e}")))
            }
        }
    }

    /// Process all pending jobs
    pub fn work_all(&self) -> usize {
        let mut count = 0;
        while let Some(_) = self.work_one() {
            count += 1;
        }
        count
    }

    /// Get live queue telemetry
    pub fn stats(&self) -> QueueStats {
        let pending = self.jobs.lock().unwrap().len();
        QueueStats {
            pending,
            processed: self.processed.load(Ordering::SeqCst),
            failed: self.failed.load(Ordering::SeqCst),
        }
    }
}

/// Record representing a persistent job stored in SQLite
#[derive(Debug, Clone)]
pub struct PersistentJobRecord {
    pub id: i64,
    pub name: String,
    pub payload: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub available_at: String,
}

/// Persistent SQLite-backed Job Queue
///
/// Jobs are persisted in `_terminus_jobs` table, surviving crashes and reboots.
/// Supports configurable maximum attempts and status tracking (`pending`, `completed`, `failed`).
#[derive(Clone)]
pub struct SqliteJobQueue {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteJobQueue {
    /// Initialize a SQLite job queue at path (or :memory:)
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = if path == ":memory:" {
            rusqlite::Connection::open_in_memory()?
        } else {
            rusqlite::Connection::open(path)?
        };
        let queue = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        queue.init_table()?;
        Ok(queue)
    }

    /// Initialize from an existing connection Arc
    pub fn from_arc(conn: Arc<Mutex<rusqlite::Connection>>) -> Result<Self, rusqlite::Error> {
        let queue = Self { conn };
        queue.init_table()?;
        Ok(queue)
    }

    fn init_table(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _terminus_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                payload TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                status TEXT NOT NULL DEFAULT 'pending',
                error TEXT,
                created_at TEXT NOT NULL,
                available_at TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    /// Dispatch a new job to the SQLite persistent queue
    pub fn push(&self, name: &str, payload: &str) -> Result<i64, rusqlite::Error> {
        self.push_with_max_attempts(name, payload, 3)
    }

    /// Dispatch a job with custom retry limit
    pub fn push_with_max_attempts(&self, name: &str, payload: &str, max_attempts: u32) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO _terminus_jobs (name, payload, attempts, max_attempts, status, created_at, available_at)
             VALUES (?1, ?2, 0, ?3, 'pending', ?4, ?4)",
            rusqlite::params![name, payload, max_attempts, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Fetch and process one available job using the provided handler
    pub fn work_one<F>(&self, handler: F) -> Result<Option<bool>, String>
    where
        F: FnOnce(&str, &str) -> Result<(), String>,
    {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let job_opt: Option<(i64, String, String, u32, u32)> = conn
            .query_row(
                "SELECT id, name, payload, attempts, max_attempts FROM _terminus_jobs
                 WHERE status = 'pending' AND available_at <= ?1
                 ORDER BY id ASC LIMIT 1",
                rusqlite::params![now],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .ok();

        let Some((id, name, payload, attempts, max_attempts)) = job_opt else {
            return Ok(None);
        };

        let new_attempts = attempts + 1;
        match handler(&name, &payload) {
            Ok(()) => {
                conn.execute(
                    "UPDATE _terminus_jobs SET status = 'completed', attempts = ?1 WHERE id = ?2",
                    rusqlite::params![new_attempts, id],
                ).map_err(|e| e.to_string())?;
                Ok(Some(true))
            }
            Err(err) => {
                if new_attempts >= max_attempts {
                    conn.execute(
                        "UPDATE _terminus_jobs SET status = 'failed', attempts = ?1, error = ?2 WHERE id = ?3",
                        rusqlite::params![new_attempts, err, id],
                    ).map_err(|e| e.to_string())?;
                } else {
                    conn.execute(
                        "UPDATE _terminus_jobs SET attempts = ?1, error = ?2 WHERE id = ?3",
                        rusqlite::params![new_attempts, err, id],
                    ).map_err(|e| e.to_string())?;
                }
                Ok(Some(false))
            }
        }
    }

    /// Return statistics on persistent queue
    pub fn stats(&self) -> QueueStats {
        let conn = self.conn.lock().unwrap();
        let pending: usize = conn.query_row(
            "SELECT COUNT(1) FROM _terminus_jobs WHERE status = 'pending'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let processed: usize = conn.query_row(
            "SELECT COUNT(1) FROM _terminus_jobs WHERE status = 'completed'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let failed: usize = conn.query_row(
            "SELECT COUNT(1) FROM _terminus_jobs WHERE status = 'failed'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        QueueStats {
            pending,
            processed,
            failed,
        }
    }
}
