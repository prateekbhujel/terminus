use terminus::prelude::*;
use std::sync::{Arc, Mutex};

#[test]
fn test_in_memory_audit_repo() {
    let repo = InMemoryAuditRepository::new();
    let (logs, total) = repo.list(&QueryState::default());
    assert!(total >= 4);
    assert!(!logs.is_empty());

    let new_log = AuditLog {
        id: "99".into(),
        user_name: "Test Auditor".into(),
        action: "CREATE".into(),
        resource: "orders".into(),
        record_id: "ORD-999".into(),
        details: "Created test order".into(),
        timestamp: "2026-10-08 09:00:00".into(),
    };
    let saved_id = repo.save(new_log).unwrap();
    assert_eq!(saved_id, "99");

    let query_search = QueryState {
        search: "Test Auditor".into(),
        ..Default::default()
    };
    let (searched, count) = repo.list(&query_search);
    assert_eq!(count, 1);
    assert_eq!(searched[0].user_name, "Test Auditor");
}

#[test]
fn test_sqlite_audit_repo_in_memory_db() {
    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let repo = SqliteAuditRepository::new(conn).expect("Should initialize audit table");

    let (seeded_logs, total) = repo.list(&QueryState::default());
    assert_eq!(total, 0, "Fresh audit repositories start empty; nothing is seeded");
    assert!(seeded_logs.is_empty());

    let new_log = AuditLog {
        id: "".into(),
        user_name: "Superadmin User".into(),
        action: "DELETE".into(),
        resource: "users".into(),
        record_id: "42".into(),
        details: "Removed user account #42".into(),
        timestamp: "2026-10-08 10:15:00".into(),
    };

    let log_id = repo.save(new_log).expect("Should save to SQLite");
    assert!(!log_id.is_empty());

    let (updated_logs, updated_total) = repo.list(&QueryState::default());
    assert_eq!(updated_total, 1);
    assert_eq!(updated_logs[0].action, "DELETE"); // newest first
    assert_eq!(updated_logs[0].user_name, "Superadmin User");
}
