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
        while self.work_one().is_some() {
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

/// Default lease granted to a claimed job. Handlers that may run longer must
/// configure a longer lease with `with_lease_duration`.
pub const DEFAULT_LEASE: std::time::Duration = std::time::Duration::from_secs(60);

/// A job atomically reserved by one worker.
///
/// Delivery is **at-least-once**: if the worker crashes, or its lease expires
/// before it acknowledges the job, another worker may reserve and run the same
/// job again. Handlers must therefore be idempotent. Terminus never claims
/// exactly-once execution.
///
/// `complete` and `fail` only take effect while this claim still owns the job.
/// If the lease expired and another worker claimed the job, both return
/// `Ok(false)` and leave the current owner's state untouched.
pub struct ClaimedJob {
    conn: crate::backend::AnyConnection,
    claim_token: String,
    pub id: i64,
    pub name: String,
    pub payload: String,
    pub attempts: u32,
    pub max_attempts: u32,
}

impl ClaimedJob {
    fn placeholder(&self, index: usize) -> String {
        self.conn.kind().placeholder(index)
    }

    /// Mark the job completed. Returns `Ok(false)` when this claim is stale.
    pub fn complete(self) -> Result<bool, String> {
        let sql = format!(
            "UPDATE _terminus_jobs
             SET status = 'completed', claimed_by = NULL, lease_expires_at = NULL
             WHERE id = {} AND claimed_by = {} AND status = 'processing'",
            self.placeholder(1),
            self.placeholder(2)
        );
        let changed = self
            .conn
            .execute(&sql, &[self.id.to_string(), self.claim_token.clone()])?;
        Ok(changed == 1)
    }

    /// Record a handler failure. The job returns to `pending` while attempts
    /// remain, and becomes `failed` once attempts are exhausted. Returns
    /// `Ok(false)` when this claim is stale.
    pub fn fail(self, error: &str) -> Result<bool, String> {
        let status = if self.attempts >= self.max_attempts {
            "failed"
        } else {
            "pending"
        };
        let sql = format!(
            "UPDATE _terminus_jobs
             SET status = '{status}', error = {}, claimed_by = NULL, lease_expires_at = NULL
             WHERE id = {} AND claimed_by = {} AND status = 'processing'",
            self.placeholder(1),
            self.placeholder(2),
            self.placeholder(3)
        );
        let changed = self.conn.execute(
            &sql,
            &[
                error.to_string(),
                self.id.to_string(),
                self.claim_token.clone(),
            ],
        )?;
        Ok(changed == 1)
    }
}

fn queue_error(message: String) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
        Some(message),
    )
}

/// Persistent database-backed job queue.
///
/// Jobs are stored in the `_terminus_jobs` table and survive restarts.
/// SQLite is the default backend (`new`/`from_arc`); PostgreSQL and
/// MySQL/MariaDB connect with [`SqliteJobQueue::connect`] using a server
/// URL. (The type keeps its original name for API compatibility;
/// `PersistentJobQueue` is the backend-neutral alias.)
///
/// Workers reserve jobs atomically: the reservation is committed before the
/// handler runs, and the connection lock is never held while user code runs,
/// so a handler may use the same shared connection without deadlocking.
///
/// Each reservation carries an expiring lease. A worker that crashes leaves
/// its job in `processing` only until the lease expires; after that another
/// worker may claim it. Expired claims whose attempts are exhausted become
/// `failed` instead of being retried forever. This gives at-least-once
/// delivery: handlers must be idempotent, and a job whose handler runs longer
/// than its lease may execute more than once.
#[derive(Clone)]
pub struct SqliteJobQueue {
    conn: crate::backend::AnyConnection,
    lease: std::time::Duration,
}

/// Backend-neutral alias for [`SqliteJobQueue`].
pub type PersistentJobQueue = SqliteJobQueue;

impl SqliteJobQueue {
    /// Initialize a SQLite job queue at path (or :memory:)
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = if path == ":memory:" {
            rusqlite::Connection::open_in_memory()?
        } else {
            rusqlite::Connection::open(path)?
        };
        let queue = Self {
            conn: crate::backend::AnyConnection::Sqlite(Arc::new(Mutex::new(conn))),
            lease: DEFAULT_LEASE,
        };
        queue.init_table().map_err(queue_error)?;
        Ok(queue)
    }

    /// Initialize from an existing SQLite connection Arc
    pub fn from_arc(conn: Arc<Mutex<rusqlite::Connection>>) -> Result<Self, rusqlite::Error> {
        let queue = Self {
            conn: crate::backend::AnyConnection::Sqlite(conn),
            lease: DEFAULT_LEASE,
        };
        queue.init_table().map_err(queue_error)?;
        Ok(queue)
    }

    /// Connect to any supported backend by URL: a SQLite path or
    /// `sqlite://` URL, `postgres://...`, or `mysql://`/`mariadb://...`.
    pub fn connect(url: &str) -> Result<Self, String> {
        let queue = Self {
            conn: crate::backend::AnyConnection::connect(url)?,
            lease: DEFAULT_LEASE,
        };
        queue.init_table()?;
        Ok(queue)
    }

    /// Build a queue over an existing backend connection.
    pub fn from_connection(conn: crate::backend::AnyConnection) -> Result<Self, String> {
        let queue = Self {
            conn,
            lease: DEFAULT_LEASE,
        };
        queue.init_table()?;
        Ok(queue)
    }

    /// The backend this queue is stored on.
    pub fn kind(&self) -> crate::backend::DatabaseKind {
        self.conn.kind()
    }

    /// Set how long a reservation remains valid before another worker may
    /// reclaim the job. The lease must be positive and longer than the
    /// longest expected handler run.
    pub fn with_lease_duration(mut self, lease: std::time::Duration) -> Result<Self, String> {
        if lease.is_zero() {
            return Err("Queue lease duration must be positive".into());
        }
        self.lease = lease;
        Ok(self)
    }

    /// The configured reservation lease.
    pub fn lease_duration(&self) -> std::time::Duration {
        self.lease
    }

    fn init_table(&self) -> Result<(), String> {
        use crate::backend::DatabaseKind;
        match &self.conn {
            crate::backend::AnyConnection::Sqlite(conn) => {
                let conn = conn.lock().unwrap();
                conn.busy_timeout(std::time::Duration::from_secs(5))
                    .map_err(|e| e.to_string())?;
                let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
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
                        available_at TEXT NOT NULL,
                        claimed_by TEXT,
                        lease_expires_at INTEGER
                    )",
                    [],
                )
                .map_err(|e| e.to_string())?;
                // Upgrade queues created before claims existed. Rows and their
                // history are preserved; only the claim columns are added.
                let columns: Vec<String> = {
                    let mut stmt = conn
                        .prepare("PRAGMA table_info(_terminus_jobs)")
                        .map_err(|e| e.to_string())?;
                    let rows = stmt
                        .query_map([], |row| row.get::<_, String>(1))
                        .map_err(|e| e.to_string())?;
                    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
                };
                if !columns.iter().any(|name| name == "claimed_by") {
                    conn.execute("ALTER TABLE _terminus_jobs ADD COLUMN claimed_by TEXT", [])
                        .map_err(|e| e.to_string())?;
                }
                if !columns.iter().any(|name| name == "lease_expires_at") {
                    conn.execute(
                        "ALTER TABLE _terminus_jobs ADD COLUMN lease_expires_at INTEGER",
                        [],
                    )
                    .map_err(|e| e.to_string())?;
                }
                Ok(())
            }
            _ => {
                let ddl = match self.conn.kind() {
                    DatabaseKind::Postgres => {
                        "CREATE TABLE IF NOT EXISTS _terminus_jobs (
                            id BIGSERIAL PRIMARY KEY,
                            name TEXT NOT NULL,
                            payload TEXT NOT NULL,
                            attempts BIGINT NOT NULL DEFAULT 0,
                            max_attempts BIGINT NOT NULL DEFAULT 3,
                            status TEXT NOT NULL DEFAULT 'pending',
                            error TEXT,
                            created_at TEXT NOT NULL,
                            available_at TEXT NOT NULL,
                            claimed_by TEXT,
                            lease_expires_at BIGINT
                        )"
                    }
                    DatabaseKind::Mysql => {
                        "CREATE TABLE IF NOT EXISTS _terminus_jobs (
                            id BIGINT PRIMARY KEY AUTO_INCREMENT,
                            name VARCHAR(255) NOT NULL,
                            payload TEXT NOT NULL,
                            attempts BIGINT NOT NULL DEFAULT 0,
                            max_attempts BIGINT NOT NULL DEFAULT 3,
                            status VARCHAR(32) NOT NULL DEFAULT 'pending',
                            error TEXT,
                            created_at VARCHAR(64) NOT NULL,
                            available_at VARCHAR(64) NOT NULL,
                            claimed_by VARCHAR(128),
                            lease_expires_at BIGINT
                        )"
                    }
                    DatabaseKind::Sqlite => unreachable!("handled above"),
                };
                self.conn.execute(ddl, &[])?;
                Ok(())
            }
        }
    }

    /// Dispatch a new job to the persistent queue
    pub fn push(&self, name: &str, payload: &str) -> Result<i64, rusqlite::Error> {
        self.push_with_max_attempts(name, payload, 3)
    }

    /// Dispatch a job with custom retry limit
    pub fn push_with_max_attempts(
        &self,
        name: &str,
        payload: &str,
        max_attempts: u32,
    ) -> Result<i64, rusqlite::Error> {
        use crate::backend::DatabaseKind;
        let kind = self.conn.kind();
        let now = Utc::now().to_rfc3339();
        let params = vec![
            name.to_string(),
            payload.to_string(),
            max_attempts.to_string(),
            now.clone(),
            now,
        ];
        match kind {
            DatabaseKind::Postgres => {
                let sql = "INSERT INTO _terminus_jobs (name, payload, attempts, max_attempts, status, created_at, available_at)
                     VALUES ($1, $2, 0, $3, 'pending', $4, $5) RETURNING id";
                let rows = self.conn.query(sql, &params).map_err(queue_error)?;
                rows.first()
                    .and_then(|row| row.get("id"))
                    .and_then(|id| id.parse::<i64>().ok())
                    .ok_or_else(|| queue_error("Queue insert returned no id".into()))
            }
            _ => {
                let ph = |i: usize| kind.placeholder(i);
                let sql = format!(
                    "INSERT INTO _terminus_jobs (name, payload, attempts, max_attempts, status, created_at, available_at)
                     VALUES ({}, {}, 0, {}, 'pending', {}, {})",
                    ph(1),
                    ph(2),
                    ph(3),
                    ph(4),
                    ph(5)
                );
                self.conn.execute(&sql, &params).map_err(queue_error)?;
                let last_id_sql = match kind {
                    DatabaseKind::Mysql => "SELECT LAST_INSERT_ID() AS id",
                    _ => "SELECT last_insert_rowid() AS id",
                };
                let rows = self.conn.query(last_id_sql, &[]).map_err(queue_error)?;
                rows.first()
                    .and_then(|row| row.get("id"))
                    .and_then(|id| id.parse::<i64>().ok())
                    .ok_or_else(|| queue_error("Queue insert returned no id".into()))
            }
        }
    }

    /// Atomically reserve the next available job, if any.
    ///
    /// The claim (owner token, lease expiry, incremented attempt count) is
    /// committed before this returns, and no lock is held afterwards, so the
    /// caller may run arbitrary code before acknowledging via
    /// [`ClaimedJob::complete`] or [`ClaimedJob::fail`].
    pub fn reserve(&self) -> Result<Option<ClaimedJob>, String> {
        let kind = self.conn.kind();
        let ph = |i: usize| kind.placeholder(i);
        let mut guard = self.conn.lock();
        guard.begin()?;
        let work = (|guard: &mut crate::backend::AnyGuard<'_>| -> Result<Option<ClaimedJob>, String> {
            let now = Utc::now();
            let now_rfc = now.to_rfc3339();
            let now_ms = now.timestamp_millis();

            // Expired claims that used their final attempt fail permanently
            // instead of being reclaimed forever.
            let reap_sql = format!(
                "UPDATE _terminus_jobs
                 SET status = 'failed',
                     error = COALESCE(error, 'Lease expired after the final attempt'),
                     claimed_by = NULL, lease_expires_at = NULL
                 WHERE status = 'processing' AND lease_expires_at IS NOT NULL
                   AND lease_expires_at <= {} AND attempts >= max_attempts",
                ph(1)
            );
            guard.execute(&reap_sql, &[now_ms.to_string()])?;

            let candidate_sql = format!(
                "SELECT id, name, payload, attempts, max_attempts FROM _terminus_jobs
                 WHERE (status = 'pending' AND available_at <= {})
                    OR (status = 'processing' AND lease_expires_at IS NOT NULL
                        AND lease_expires_at <= {} AND attempts < max_attempts)
                 ORDER BY id ASC LIMIT 1",
                ph(1),
                ph(2)
            );
            let candidates = guard.query(
                &candidate_sql,
                &[now_rfc, now_ms.to_string()],
            )?;
            let Some(row) = candidates.into_iter().next() else {
                return Ok(None);
            };
            let parse = |key: &str| -> Result<i64, String> {
                row.get(key)
                    .and_then(|value| value.parse::<i64>().ok())
                    .ok_or_else(|| format!("Malformed queue row: invalid {key}"))
            };
            let id = parse("id")?;
            let attempts = parse("attempts")? as u32;
            let max_attempts = parse("max_attempts")? as u32;
            let name = row.get("name").cloned().unwrap_or_default();
            let payload = row.get("payload").cloned().unwrap_or_default();

            let claim_token = crate::session::secure_random_hex(16);
            let lease_expires_at = now_ms + self.lease.as_millis() as i64;
            let claim_sql = format!(
                "UPDATE _terminus_jobs
                 SET status = 'processing', claimed_by = {}, lease_expires_at = {},
                     attempts = attempts + 1
                 WHERE id = {}
                   AND (status = 'pending'
                        OR (status = 'processing' AND lease_expires_at <= {}))",
                ph(1),
                ph(2),
                ph(3),
                ph(4)
            );
            let changed = guard.execute(
                &claim_sql,
                &[
                    claim_token.clone(),
                    lease_expires_at.to_string(),
                    id.to_string(),
                    now_ms.to_string(),
                ],
            )?;
            if changed != 1 {
                // Lost a race against another connection; report no claim rather
                // than running a job this worker does not own.
                return Ok(None);
            }
            Ok(Some(ClaimedJob {
                conn: self.conn.clone(),
                claim_token,
                id,
                name,
                payload,
                attempts: attempts + 1,
                max_attempts,
            }))
        })(&mut guard);
        match work {
            Ok(claim) => {
                guard.commit()?;
                Ok(claim)
            }
            Err(error) => {
                let _ = guard.rollback();
                Err(error)
            }
        }
    }

    /// Fetch and process one available job using the provided handler.
    ///
    /// The job is reserved and committed before the handler runs, and the
    /// connection lock is released while the handler executes, so the handler
    /// may query or dispatch through the same connection. Multiple workers on
    /// separate connections may run against one database; each job is claimed
    /// by exactly one worker at a time, with at-least-once delivery if a
    /// lease expires mid-run (see [`SqliteJobQueue`]).
    pub fn work_one<F>(&self, handler: F) -> Result<Option<bool>, String>
    where
        F: FnOnce(&str, &str) -> Result<(), String>,
    {
        let Some(claim) = self.reserve()? else {
            return Ok(None);
        };
        let id = claim.id;
        match handler(&claim.name, &claim.payload) {
            Ok(()) => {
                if claim.complete()? {
                    Ok(Some(true))
                } else {
                    Err(format!(
                        "Job {id} finished after its lease expired and was reclaimed; its result was discarded"
                    ))
                }
            }
            Err(err) => {
                if claim.fail(&err)? {
                    Ok(Some(false))
                } else {
                    Err(format!(
                        "Job {id} failed after its lease expired and was reclaimed; its failure was discarded"
                    ))
                }
            }
        }
    }

    /// Return statistics on persistent queue. In-flight (`processing`) jobs
    /// count as pending because they have not reached a terminal state.
    pub fn stats(&self) -> QueueStats {
        let count = |status_sql: &str| -> usize {
            self.conn
                .query(
                    &format!("SELECT COUNT(1) AS total FROM _terminus_jobs WHERE {status_sql}"),
                    &[],
                )
                .ok()
                .and_then(|rows| rows.into_iter().next())
                .and_then(|row| row.get("total").and_then(|v| v.parse::<usize>().ok()))
                .unwrap_or(0)
        };
        QueueStats {
            pending: count("status IN ('pending', 'processing')"),
            processed: count("status = 'completed'"),
            failed: count("status = 'failed'"),
        }
    }
}
