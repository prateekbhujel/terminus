//! Live-backend tests. Each server test runs only when its
//! `TERMINUS_TEST_*_URL` environment variable is set (CI has no servers);
//! run them locally with dockerized PostgreSQL/MySQL. URL parsing and
//! dialect tests always run.
use terminus::backend::DatabaseKind;
use terminus::orm::{Database, DatabaseConfig};

#[test]
fn urls_select_the_right_backend() {
    assert_eq!(DatabaseKind::from_url("postgres://u:p@h:5432/db"), DatabaseKind::Postgres);
    assert_eq!(DatabaseKind::from_url("postgresql://u:p@h/db"), DatabaseKind::Postgres);
    assert_eq!(DatabaseKind::from_url("mysql://u:p@h/db"), DatabaseKind::Mysql);
    assert_eq!(DatabaseKind::from_url("mariadb://u:p@h/db"), DatabaseKind::Mysql);
    assert_eq!(DatabaseKind::from_url("sqlite://app.db"), DatabaseKind::Sqlite);
    assert_eq!(DatabaseKind::from_url("app.db"), DatabaseKind::Sqlite);

    assert_eq!(DatabaseKind::Postgres.placeholder(2), "$2");
    assert_eq!(DatabaseKind::Mysql.placeholder(2), "?");
    assert_eq!(DatabaseKind::Sqlite.placeholder(2), "?");

    let config = DatabaseConfig::from_url("postgres://u:p@h:5432/db").unwrap();
    assert_eq!(config.kind(), DatabaseKind::Postgres);
    assert_eq!(config.to_url(), "postgres://u:p@h:5432/db");
}

fn exercise_backend(db: &Database, kind: DatabaseKind, table: &str) {
    assert_eq!(db.kind(), kind);
    let ph = |i: usize| kind.placeholder(i);
    db.execute(&format!("DROP TABLE IF EXISTS {table}")).unwrap();
    let id_type = match kind {
        DatabaseKind::Mysql => "VARCHAR(64)",
        _ => "TEXT",
    };
    db.execute(&format!(
        "CREATE TABLE {table} (id {id_type} PRIMARY KEY, qty TEXT NOT NULL)"
    ))
    .unwrap();
    db.execute_params(
        &format!("INSERT INTO {table} (id, qty) VALUES ({}, {})", ph(1), ph(2)),
        &["sku-1".to_string(), "42".to_string()],
    )
    .unwrap();
    let rows = db
        .query_params(
            &format!("SELECT qty FROM {table} WHERE id = {}", ph(1)),
            &["sku-1".to_string()],
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("qty").map(String::as_str), Some("42"));

    // Migrations run end-to-end on the server backend.
    let migration = format!("CREATE TABLE {table}_migrated (id TEXT)");
    assert!(db.apply_migration("001_widgets.sql", &migration, 1).unwrap());
    assert!(!db.apply_migration("001_widgets.sql", &migration, 1).unwrap());
    assert_eq!(db.latest_batch().unwrap(), 1);
    assert_eq!(db.applied_migrations().unwrap().len(), 1);
    let rolled = db
        .rollback_latest_batch_with(|_| Ok(format!("DROP TABLE {table}_migrated")))
        .unwrap();
    assert_eq!(rolled, vec!["001_widgets.sql".to_string()]);
    assert!(db.applied_migrations().unwrap().is_empty());
    db.execute(&format!("DROP TABLE IF EXISTS {table}")).unwrap();
}

#[test]
fn postgres_backend_roundtrip_and_migrations() {
    let Ok(url) = std::env::var("TERMINUS_TEST_POSTGRES_URL") else {
        eprintln!("skipping: TERMINUS_TEST_POSTGRES_URL not set");
        return;
    };
    let db = Database::connect(&url).unwrap();
    // Failed migration SQL rolls its DDL back on PostgreSQL.
    assert!(db
        .apply_migration(
            "bad.sql",
            "CREATE TABLE terminus_bad_partial (id TEXT); INSERT INTO missing_table VALUES (1)",
            9,
        )
        .is_err());
    let rows = db
        .query_params(
            "SELECT table_name AS name FROM information_schema.tables WHERE table_name = 'terminus_bad_partial'",
            &[],
        )
        .unwrap();
    assert!(rows.is_empty(), "failed migration left partial DDL behind");
    exercise_backend(&db, DatabaseKind::Postgres, "terminus_backend_widgets");
}

#[test]
fn mysql_backend_roundtrip_and_migrations() {
    let Ok(url) = std::env::var("TERMINUS_TEST_MYSQL_URL") else {
        eprintln!("skipping: TERMINUS_TEST_MYSQL_URL not set");
        return;
    };
    let db = Database::connect(&url).unwrap();
    exercise_backend(&db, DatabaseKind::Mysql, "terminus_backend_widgets");
}
