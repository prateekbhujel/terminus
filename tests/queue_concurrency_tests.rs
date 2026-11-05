use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use terminus::queue::SqliteJobQueue;

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = COUNTER.fetch_add(1, Ordering::SeqCst);
        Self(
            std::env::temp_dir().join(format!(
                "terminus_queue_{}_{unique}_{sequence}.db",
                std::process::id()
            )),
        )
    }

    fn path(&self) -> String {
        self.0.to_str().unwrap().to_string()
    }

    fn queue(&self) -> SqliteJobQueue {
        SqliteJobQueue::new(&self.path()).unwrap()
    }

    fn expire_claims(&self) {
        rusqlite::Connection::open(&self.0)
            .unwrap()
            .execute("UPDATE _terminus_jobs SET lease_expires_at = 1", [])
            .unwrap();
    }

    fn status_of(&self, id: i64) -> String {
        rusqlite::Connection::open(&self.0)
            .unwrap()
            .query_row(
                "SELECT status FROM _terminus_jobs WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap()
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path()));
        }
    }
}

#[test]
fn handler_runs_without_holding_the_shared_connection_lock() {
    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let queue = SqliteJobQueue::from_arc(conn.clone()).unwrap();
    queue.push("shared_connection_job", "{}").unwrap();

    let result = queue.work_one(|_, _| {
        // Under the old implementation this lock was held for the whole
        // handler, so any database use through the shared connection hung.
        let guard = conn
            .try_lock()
            .expect("connection must be unlocked inside the handler");
        let pending: i64 = guard
            .query_row(
                "SELECT COUNT(1) FROM _terminus_jobs WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending, 0, "the claimed job is already committed as processing");
        Ok(())
    });

    assert_eq!(result, Ok(Some(true)));
    assert_eq!(queue.stats().processed, 1);
}

#[test]
fn second_worker_cannot_claim_a_live_lease() {
    let db = TempDb::new();
    let first = db.queue();
    let second = db.queue();
    let id = first.push("exclusive_job", "{}").unwrap();

    let claim = first.reserve().unwrap().expect("first worker claims the job");
    assert_eq!(second.reserve().unwrap().map(|job| job.id), None);
    assert_eq!(
        second.work_one(|_, _| panic!("live lease was claimed twice")),
        Ok(None)
    );

    assert!(claim.complete().unwrap());
    assert_eq!(db.status_of(id), "completed");
}

#[test]
fn expired_claim_is_recovered_by_another_worker() {
    let db = TempDb::new();
    let crashed = db.queue();
    let recovery = db.queue();
    crashed.push("crash_recovery_job", "{}").unwrap();

    // Simulate a worker that reserved the job and never acknowledged it.
    let stranded = crashed.reserve().unwrap().unwrap();
    drop(stranded);
    db.expire_claims();

    assert_eq!(
        recovery.work_one(|name, _| {
            assert_eq!(name, "crash_recovery_job");
            Ok(())
        }),
        Ok(Some(true))
    );
    assert_eq!(recovery.stats().processed, 1);
}

#[test]
fn exhausted_expired_claim_fails_instead_of_retrying_forever() {
    let db = TempDb::new();
    let queue = db.queue();
    queue.push_with_max_attempts("final_attempt_job", "{}", 1).unwrap();

    let claim = queue.reserve().unwrap().unwrap();
    assert_eq!(claim.attempts, 1);
    drop(claim);
    db.expire_claims();

    assert!(queue.reserve().unwrap().is_none());
    assert_eq!(queue.stats().failed, 1);
    assert_eq!(queue.stats().pending, 0);
}

#[test]
fn stale_claim_cannot_overwrite_reassigned_work() {
    let db = TempDb::new();
    let first = db.queue();
    let second = db.queue();
    let id = first.push("reassigned_job", "{}").unwrap();

    let stale = first.reserve().unwrap().unwrap();
    db.expire_claims();
    let current = second.reserve().unwrap().unwrap();
    assert!(current.complete().unwrap());

    // The stale worker finishes late; neither ack may change the outcome.
    assert!(!stale.complete().unwrap());
    assert_eq!(db.status_of(id), "completed");
}

#[test]
fn stale_failure_cannot_requeue_completed_work() {
    let db = TempDb::new();
    let first = db.queue();
    let second = db.queue();
    let id = first.push("reassigned_failure_job", "{}").unwrap();

    let stale = first.reserve().unwrap().unwrap();
    db.expire_claims();
    assert_eq!(second.work_one(|_, _| Ok(())), Ok(Some(true)));

    assert!(!stale.fail("late failure").unwrap());
    assert_eq!(db.status_of(id), "completed");
}

#[test]
fn legacy_schema_upgrades_without_losing_pending_jobs() {
    let db = TempDb::new();
    {
        let conn = rusqlite::Connection::open(&db.0).unwrap();
        conn.execute_batch(
            "CREATE TABLE _terminus_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                payload TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                status TEXT NOT NULL DEFAULT 'pending',
                error TEXT,
                created_at TEXT NOT NULL,
                available_at TEXT NOT NULL
            );
            INSERT INTO _terminus_jobs
                (name, payload, attempts, max_attempts, status, created_at, available_at)
            VALUES ('legacy_job', '{}', 0, 3, 'pending', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00');",
        )
        .unwrap();
    }

    let queue = db.queue();
    assert_eq!(queue.stats().pending, 1);
    assert_eq!(queue.work_one(|name, _| {
        assert_eq!(name, "legacy_job");
        Ok(())
    }), Ok(Some(true)));
    assert_eq!(queue.stats().processed, 1);
}

#[test]
fn concurrent_workers_each_claim_jobs_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let db = TempDb::new();
    let queue = db.queue();
    for index in 0..20 {
        queue.push(&format!("job_{index}"), "{}").unwrap();
    }

    let handled = Arc::new(AtomicUsize::new(0));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let worker = db.queue();
            let handled = handled.clone();
            std::thread::spawn(move || {
                loop {
                    match worker.work_one(|_, _| {
                        handled.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }) {
                        Ok(Some(_)) => continue,
                        Ok(None) => break,
                        Err(error) => panic!("worker failed: {error}"),
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }

    assert_eq!(handled.load(Ordering::SeqCst), 20);
    assert_eq!(queue.stats().processed, 20);
    assert_eq!(queue.stats().pending, 0);
}

#[test]
fn lease_duration_must_be_positive() {
    let db = TempDb::new();
    assert!(db.queue().with_lease_duration(Duration::ZERO).is_err());
    let queue = db
        .queue()
        .with_lease_duration(Duration::from_secs(5))
        .unwrap();
    assert_eq!(queue.lease_duration(), Duration::from_secs(5));
}
