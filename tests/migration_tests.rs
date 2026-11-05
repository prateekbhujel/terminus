use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};
use terminus::orm::Database;

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "terminus_migrations_{}_{unique}_{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(path.join("migrations")).unwrap();
        Self(path)
    }

    fn db(&self) -> Database {
        Database::open(self.0.join("terminus.db")).unwrap()
    }

    fn run(&self, command: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_terminus"))
            .arg(command)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }

    fn migrate(&self) {
        let output = self.run("migrate");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn migration(&self, name: &str, sql: &str) {
        fs::write(self.0.join("migrations").join(name), sql).unwrap();
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn table_exists(db: &Database, name: &str) -> bool {
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )
        .unwrap()
    })
}

#[test]
fn legacy_cli_bookkeeping_preserves_migration_records() {
    let db = Database::memory().unwrap();
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TABLE _terminus_migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            migration TEXT NOT NULL UNIQUE,
            batch INTEGER NOT NULL DEFAULT 1,
            executed_at TEXT NOT NULL
        );
        INSERT INTO _terminus_migrations VALUES (7, 'existing.sql', 3, '2026-10-24T00:00:00Z');",
        )
        .unwrap()
    });

    assert_eq!(
        db.applied_migrations().unwrap(),
        vec![("existing.sql".into(), 3, "2026-10-24T00:00:00Z".into(),)]
    );
    db.ensure_migrations_table().unwrap();
    db.with_conn(|conn| {
        let id: i64 = conn
            .query_row(
                "SELECT id FROM _terminus_migrations WHERE name = 'existing.sql'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(id, 7);
    });
    db.record_migration("next.sql", 4).unwrap();
    assert_eq!(db.latest_batch().unwrap(), 4);
}

#[test]
fn cli_and_library_share_migration_bookkeeping_in_both_orders() {
    let cli_first = TempProject::new();
    cli_first.migrate();
    assert_eq!(cli_first.db().applied_migrations().unwrap().len(), 4);

    let library_first = TempProject::new();
    library_first
        .db()
        .record_migration("existing.sql", 9)
        .unwrap();
    library_first.migrate();
    let records = library_first.db().applied_migrations().unwrap();
    assert_eq!(records.len(), 5);
    assert_eq!(records[0].0, "existing.sql");
    assert!(records[1..].iter().all(|(_, batch, _)| *batch == 10));
    let status = library_first.run("migrate:status");
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("existing.sql"));
}

#[test]
fn cli_migration_failure_exits_nonzero_and_rolls_back_partial_sql() {
    let project = TempProject::new();
    project.migrate();
    project.migration(
        "001_failure.sql",
        "CREATE TABLE partial_write (id INTEGER); INSERT INTO missing_table VALUES (1);",
    );

    let output = project.run("migrate");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("001_failure.sql"));
    let db = project.db();
    assert!(!table_exists(&db, "partial_write"));
    assert!(db
        .applied_migrations()
        .unwrap()
        .iter()
        .all(|(name, _, _)| name != "001_failure.sql"));
}

#[test]
fn migration_sql_and_bookkeeping_commit_together() {
    let db = Database::memory().unwrap();
    db.ensure_migrations_table().unwrap();
    db.with_conn(|conn| conn.execute_batch(
        "CREATE TRIGGER reject_history BEFORE INSERT ON _terminus_migrations BEGIN SELECT RAISE(ABORT, 'cannot record'); END;"
    ).unwrap());

    assert!(db
        .apply_migration("new.sql", "CREATE TABLE partial_write (id INTEGER)", 1)
        .is_err());
    assert!(!table_exists(&db, "partial_write"));
    assert!(db.applied_migrations().unwrap().is_empty());
}

#[test]
fn cli_executes_down_sql_only_during_rollback() {
    let project = TempProject::new();
    project.migrate();
    project.migration(
        "001_widgets.sql",
        "-- [UP]\nCREATE TABLE widgets (id INTEGER);\n-- [DOWN]\nDROP TABLE widgets;\n",
    );
    project.migrate();
    assert!(table_exists(&project.db(), "widgets"));

    let output = project.run("migrate:rollback");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!table_exists(&project.db(), "widgets"));
    assert_eq!(project.db().applied_migrations().unwrap().len(), 4);
}

#[test]
fn cli_refuses_rollback_without_executable_down_sql() {
    for suffix in [
        "",
        "\n-- [DOWN]\n-- DROP TABLE widgets;",
        "\n-- [DOWN]\n/* no reversal */",
    ] {
        let project = TempProject::new();
        project.migrate();
        project.migration(
            "001_widgets.sql",
            &format!("CREATE TABLE widgets (id INTEGER);{suffix}"),
        );
        project.migrate();
        let before = project.db().applied_migrations().unwrap();

        let output = project.run("migrate:rollback");

        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .to_ascii_lowercase()
            .contains("down"));
        assert!(table_exists(&project.db(), "widgets"));
        assert_eq!(project.db().applied_migrations().unwrap(), before);
    }
}

#[test]
fn rollback_failure_preserves_entire_batch() {
    let db = Database::memory().unwrap();
    db.apply_migration("first", "CREATE TABLE first_table (id INTEGER)", 1)
        .unwrap();
    db.apply_migration("second", "CREATE TABLE second_table (id INTEGER)", 1)
        .unwrap();
    let before = db.applied_migrations().unwrap();

    let result = db.rollback_latest_batch_with(|name| {
        Ok(match name {
            "second" => "DROP TABLE second_table".into(),
            _ => "INSERT INTO missing_table VALUES (1)".into(),
        })
    });

    assert!(result.is_err());
    assert!(table_exists(&db, "first_table"));
    assert!(table_exists(&db, "second_table"));
    assert_eq!(db.applied_migrations().unwrap(), before);
}

#[test]
fn rollback_requires_explicit_sql_for_nonempty_batches() {
    let db = Database::memory().unwrap();
    assert!(db.rollback_latest_batch().unwrap().is_empty());
    db.apply_migration("first", "CREATE TABLE first_table (id INTEGER)", 1)
        .unwrap();
    assert!(db
        .rollback_latest_batch()
        .unwrap_err()
        .to_string()
        .contains("Explicit down SQL"));
    assert!(table_exists(&db, "first_table"));
    assert_eq!(db.applied_migrations().unwrap().len(), 1);
}

#[test]
fn custom_migration_name_cannot_trigger_builtin_table_drop() {
    let project = TempProject::new();
    project.migrate();
    project.migration(
        "001_create_users_table_extra.sql",
        "-- [UP]\nCREATE TABLE widgets (id INTEGER);\n-- [DOWN]\nDROP TABLE widgets;",
    );
    project.migrate();

    let output = project.run("migrate:rollback");

    assert!(output.status.success());
    assert!(table_exists(&project.db(), "users"));
    assert!(!table_exists(&project.db(), "widgets"));
}

#[test]
fn unreadable_or_invalid_database_reports_cli_failure() {
    let project = TempProject::new();
    fs::create_dir_all(project.0.join("terminus.db")).unwrap();
    for command in ["migrate", "migrate:status", "migrate:rollback"] {
        assert!(!project.run(command).status.success(), "{command}");
    }
}

#[test]
fn empty_down_sql_preserves_history() {
    let db = Database::memory().unwrap();
    db.apply_migration("first", "CREATE TABLE first_table (id INTEGER)", 1)
        .unwrap();
    assert!(db
        .rollback_latest_batch_with(|_| Ok(String::new()))
        .is_err());
    assert!(table_exists(&db, "first_table"));
    assert_eq!(db.applied_migrations().unwrap().len(), 1);
}
