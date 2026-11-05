//! Full-stack backend tests: queue, sessions, and repositories must behave
//! the same on PostgreSQL and MySQL as on SQLite. Each test runs only when
//! its `TERMINUS_TEST_*_URL` variable is set.
use terminus::backend::DatabaseKind;
use terminus::orm::Database;
use terminus::prelude::*;
use terminus::queue::PersistentJobQueue;
use terminus::resource::QueryState;
use terminus::session::SessionStore;

fn exercise_database(url: &str, kind: DatabaseKind) {
    let db = Database::connect(url).unwrap();
    assert_eq!(db.kind(), kind);
    for table in ["users", "_terminus_jobs", "_terminus_sessions", "personal_access_tokens", "audit_logs", "orders"] {
        db.execute(&format!("DROP TABLE IF EXISTS {table}")).unwrap();
    }

    // Queue: claims are exclusive across connections; work completes once.
    let first = PersistentJobQueue::connect(url).unwrap();
    let second = PersistentJobQueue::connect(url).unwrap();
    assert_eq!(first.kind(), kind);
    first.push("fullstack_job", "{}").unwrap();
    let claim = first.reserve().unwrap().expect("first worker claims");
    assert!(second.reserve().unwrap().is_none(), "live lease claimed twice");
    assert!(claim.complete().unwrap());
    assert_eq!(first.work_one(|_, _| Ok(())), Ok(None));
    first.push("fullstack_job_2", "{}").unwrap();
    assert_eq!(first.work_one(|_, _| Ok(())), Ok(Some(true)));
    assert_eq!(first.stats().processed, 2);

    // Sessions: save/load/revoke with stale-save rejection.
    let store = SessionStore::connect(url).unwrap();
    let session = Session::new();
    session.set("user_id", "7");
    store.save(&session).unwrap();
    let loaded = store.load(session.id()).unwrap().expect("session persists");
    assert_eq!(loaded.get("user_id").as_deref(), Some("7"));
    assert!(store.remove(session.id()).unwrap());
    loaded.set("user_id", "8");
    assert!(store.save(&loaded).is_err(), "revoked session resurrected");
    assert!(store.load(session.id()).unwrap().is_none());

    // Users: no seeded accounts; credentials verify only with a password.
    let users = DatabaseUserRepository::new(db.clone()).unwrap();
    assert_eq!(users.list(&QueryState::default()).1, 0);
    let id = users
        .save(User::new("ignored", "Operator", "op@example.test", Role::Admin))
        .unwrap();
    assert!(users.verify_credentials("op@example.test", "anything").is_none());
    let ph = |i: usize| kind.placeholder(i);
    db.execute_params(
        &format!("UPDATE users SET password_hash = {} WHERE id = {}", ph(1), ph(2)),
        &[Hash::make("correct horse"), id.clone()],
    )
    .unwrap();
    let verified = users
        .verify_credentials("op@example.test", "correct horse")
        .expect("credentials verify on server backend");
    assert_eq!(verified.id, id);
    assert_eq!(users.find_by_email("op@example.test").unwrap().id, id);

    // Tokens: create/find/revoke on the server backend.
    let tokens = DatabaseTokenStore::new(db.clone()).unwrap();
    let token = tokens
        .create_token("user", &id, "cli", vec!["*".to_string()], None)
        .unwrap();
    assert_eq!(
        tokens.find_token(&token.plain_text_token).unwrap().tokenable_id,
        id
    );
    tokens.revoke_token(&token.access_token.id).unwrap();
    assert!(tokens.find_token(&token.plain_text_token).is_none());

    // Audit + orders on the server backend, with no seeded rows.
    let audit = DatabaseAuditRepository::new(db.clone()).unwrap();
    assert_eq!(audit.list(&QueryState::default()).1, 0);
    let log_id = audit
        .save(AuditLog {
            id: String::new(),
            user_name: "Operator".into(),
            action: "CREATE".into(),
            resource: "widgets".into(),
            record_id: "1".into(),
            details: "Created widget".into(),
            timestamp: "2026-11-05T00:00:00Z".into(),
        })
        .unwrap();
    assert_eq!(audit.find_by_id(&log_id).unwrap().resource, "widgets");

    let orders = DatabaseOrderRepository::new(db.clone()).unwrap();
    assert_eq!(orders.list(&QueryState::default()).1, 0);
    let order_id = orders
        .save(Order {
            id: String::new(),
            customer: "Acme".into(),
            amount: "100".into(),
            status: OrderStatus::Pending,
            date: "2026-11-05".into(),
        })
        .unwrap();
    orders
        .update(&order_id, "Acme".into(), "100".into(), OrderStatus::Paid)
        .unwrap();
    assert_eq!(orders.find_by_id(&order_id).unwrap().status, OrderStatus::Paid);
    orders.delete(&order_id).unwrap();
    assert!(orders.find_by_id(&order_id).is_none());
}

#[test]
fn postgres_full_stack() {
    let Ok(url) = std::env::var("TERMINUS_TEST_POSTGRES_URL") else {
        eprintln!("skipping: TERMINUS_TEST_POSTGRES_URL not set");
        return;
    };
    exercise_database(&url, DatabaseKind::Postgres);
}

#[test]
fn mysql_full_stack() {
    let Ok(url) = std::env::var("TERMINUS_TEST_MYSQL_URL") else {
        eprintln!("skipping: TERMINUS_TEST_MYSQL_URL not set");
        return;
    };
    exercise_database(&url, DatabaseKind::Mysql);
}

#[cfg(feature = "redis")]
#[test]
fn redis_sessions_roundtrip_and_revoke() {
    let Ok(url) = std::env::var("TERMINUS_TEST_REDIS_URL") else {
        eprintln!("skipping: TERMINUS_TEST_REDIS_URL not set");
        return;
    };
    let store = SessionStore::connect(&url).unwrap();
    let session = Session::new();
    session.set("user_id", "9");
    store.save(&session).unwrap();
    let loaded = store.load(session.id()).unwrap().expect("redis session persists");
    assert_eq!(loaded.get("user_id").as_deref(), Some("9"));
    assert!(store.remove(session.id()).unwrap());
    loaded.set("user_id", "10");
    assert!(store.save(&loaded).is_err(), "revoked redis session resurrected");
    assert!(store.load(session.id()).unwrap().is_none());
}
