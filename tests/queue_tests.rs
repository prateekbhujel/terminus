use std::sync::{Arc, Mutex};
use terminus::queue::SqliteJobQueue;

#[test]
fn empty_queue_does_not_call_handler() {
    let queue = SqliteJobQueue::new(":memory:").unwrap();

    assert_eq!(
        queue.work_one(|_, _| panic!("empty queue called its handler")),
        Ok(None)
    );
}

#[test]
fn missing_jobs_table_returns_database_error() {
    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let queue = SqliteJobQueue::from_arc(conn.clone()).unwrap();
    conn.lock()
        .unwrap()
        .execute("DROP TABLE _terminus_jobs", [])
        .unwrap();

    let result = queue.work_one(|_, _| panic!("invalid queue called its handler"));

    assert!(result.unwrap_err().contains("no such table"));
}

#[test]
fn malformed_job_returns_database_error() {
    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let queue = SqliteJobQueue::from_arc(conn.clone()).unwrap();
    let id = queue.push("invalid_job", "{}").unwrap();
    conn.lock()
        .unwrap()
        .execute(
            "UPDATE _terminus_jobs SET attempts = 'invalid' WHERE id = ?1",
            [id],
        )
        .unwrap();

    let result = queue.work_one(|_, _| panic!("malformed job called its handler"));

    assert!(result.is_err());
}

#[test]
fn failed_job_retries_until_attempt_limit() {
    let queue = SqliteJobQueue::new(":memory:").unwrap();
    queue.push_with_max_attempts("retry_job", "{}", 2).unwrap();

    assert_eq!(
        queue.work_one(|_, _| Err("try again".into())),
        Ok(Some(false))
    );
    assert_eq!(queue.stats().pending, 1);
    assert_eq!(queue.stats().failed, 0);

    assert_eq!(
        queue.work_one(|_, _| Err("still unavailable".into())),
        Ok(Some(false))
    );
    assert_eq!(queue.stats().pending, 0);
    assert_eq!(queue.stats().failed, 1);
    assert_eq!(
        queue.work_one(|_, _| panic!("exhausted job was retried")),
        Ok(None)
    );
}
