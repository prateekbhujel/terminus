//! Live-backend tests. Each server test runs only when its
//! `TERMINUS_TEST_*_URL` environment variable is set (CI has no servers);
//! run them locally with dockerized PostgreSQL/MySQL. URL parsing and
//! dialect tests always run.
use serde_json::json;
use terminus::backend::{DatabaseKind, SqlValue};
use terminus::orm::{Database, DatabaseConfig, SqlQuery};

#[test]
fn urls_select_the_right_backend() {
    assert_eq!(
        DatabaseKind::from_url("postgres://u:p@h:5432/db"),
        DatabaseKind::Postgres
    );
    assert_eq!(
        DatabaseKind::from_url("postgresql://u:p@h/db"),
        DatabaseKind::Postgres
    );
    assert_eq!(
        DatabaseKind::from_url("mysql://u:p@h/db"),
        DatabaseKind::Mysql
    );
    assert_eq!(
        DatabaseKind::from_url("mariadb://u:p@h/db"),
        DatabaseKind::Mysql
    );
    assert_eq!(
        DatabaseKind::from_url("sqlite://app.db"),
        DatabaseKind::Sqlite
    );
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
    db.execute(&format!("DROP TABLE IF EXISTS {table}"))
        .unwrap();
    let id_type = match kind {
        DatabaseKind::Mysql => "VARCHAR(64)",
        _ => "TEXT",
    };
    db.execute(&format!(
        "CREATE TABLE {table} (id {id_type} PRIMARY KEY, qty TEXT NOT NULL)"
    ))
    .unwrap();
    db.execute_params(
        &format!(
            "INSERT INTO {table} (id, qty) VALUES ({}, {})",
            ph(1),
            ph(2)
        ),
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
    assert!(db
        .apply_migration("001_widgets.sql", &migration, 1)
        .unwrap());
    assert!(!db
        .apply_migration("001_widgets.sql", &migration, 1)
        .unwrap());
    assert_eq!(db.latest_batch().unwrap(), 1);
    assert_eq!(db.applied_migrations().unwrap().len(), 1);
    let rolled = db
        .rollback_latest_batch_with(|_| Ok(format!("DROP TABLE {table}_migrated")))
        .unwrap();
    assert_eq!(rolled, vec!["001_widgets.sql".to_string()]);
    assert!(db.applied_migrations().unwrap().is_empty());
    db.execute(&format!("DROP TABLE IF EXISTS {table}"))
        .unwrap();
}

#[test]
fn sqlite_backend_roundtrip_and_migrations() {
    let db = Database::memory().unwrap();
    exercise_backend(&db, DatabaseKind::Sqlite, "terminus_backend_widgets");
}

#[test]
fn typed_sql_values_roundtrip_across_sqlite() {
    let db = Database::memory().unwrap();
    db.execute(
        "CREATE TABLE typed_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL,
            note TEXT,
            qty INTEGER,
            active INTEGER NOT NULL,
            ratio REAL,
            meta TEXT,
            raw BLOB
        )",
    )
    .unwrap();

    db.execute_values(
        "INSERT INTO typed_items (code, note, qty, active, ratio, meta, raw)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            SqlValue::Text("00123".into()),
            SqlValue::Null,
            SqlValue::Int(42),
            SqlValue::Bool(true),
            SqlValue::Float(3.25),
            SqlValue::Json(json!({"ok": true, "tags": ["a'b", "c"]})),
            SqlValue::Bytes(b"bin-ok".to_vec()),
        ],
    )
    .unwrap();

    let rows = db
        .query_values(
            "SELECT code, note, qty, active, ratio, meta, raw FROM typed_items WHERE code = ?",
            &[SqlValue::Text("00123".into())],
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("code").map(String::as_str), Some("00123"));
    assert!(!rows[0].contains_key("note"));
    assert_eq!(rows[0].get("qty").map(String::as_str), Some("42"));
    assert_eq!(rows[0].get("active").map(String::as_str), Some("1"));
    assert_eq!(rows[0].get("ratio").map(String::as_str), Some("3.25"));
    assert_eq!(rows[0].get("raw").map(String::as_str), Some("bin-ok"));
    let meta: serde_json::Value = serde_json::from_str(rows[0].get("meta").unwrap()).unwrap();
    assert_eq!(meta, json!({"ok": true, "tags": ["a'b", "c"]}));

    let err = db
        .execute_values(
            "INSERT INTO typed_items (code, active, ratio) VALUES (?, ?, ?)",
            &[
                SqlValue::Text("001".into()),
                SqlValue::Bool(false),
                SqlValue::Float(f64::NAN),
            ],
        )
        .unwrap_err();
    assert!(err.contains("non-finite"), "unexpected error: {err}");
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn external_test_url(var: &str, service: &str) -> Option<String> {
    if let Ok(url) = std::env::var(var) {
        let trimmed = url.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    let required = std::env::var("TERMINUS_REQUIRE_EXTERNAL_TESTS")
        .ok()
        .map(|raw| {
            let val = raw.trim().to_ascii_lowercase();
            match val.as_str() {
                "1" | "true" | "sql" => matches!(service, "postgres" | "mysql" | "redis"),
                "all" => true,
                _ => val.split(',').any(|item| item.trim() == service),
            }
        })
        .unwrap_or(false);
    if required {
        panic!(
            "{var} must be set when TERMINUS_REQUIRE_EXTERNAL_TESTS requires '{service}' (external-service tests must not silently skip)"
        );
    }
    eprintln!("skipping: {var} not set");
    None
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_bound_parameters_cover_types_comments_and_reject_unsafe_coercions() {
    let Some(url) = external_test_url("TERMINUS_TEST_POSTGRES_URL", "postgres") else {
        return;
    };
    let db = Database::connect(&url).unwrap();
    let table = "terminus_pg_bound_params";
    db.execute(&format!("DROP TABLE IF EXISTS {table}"))
        .unwrap();
    db.execute(&format!(
        "CREATE TABLE {table} (
            id BIGSERIAL PRIMARY KEY,
            small SMALLINT,
            text_val TEXT NOT NULL,
            nullable_val TEXT,
            count_val BIGINT,
            ratio DOUBLE PRECISION,
            flag BOOLEAN NOT NULL,
            payload JSONB,
            raw_json JSON,
            tags TEXT[],
            scores BIGINT[],
            raw_bytes BYTEA
        )"
    ))
    .unwrap();

    // Rich typed roundtrip: apostrophes, quotes, numeric-like strings, Unicode,
    // JSON/JSONB, native TEXT[] and BIGINT[], booleans, bytes, and NULL.
    let payload = json!({
        "owner": "O'Reilly",
        "items": ["00123", "$1", true],
        "nested": {"greeting": "नमस्ते 🌍"}
    });
    let inserted = db
        .query_values(
            &format!(
                "INSERT INTO {table}
                    (small, text_val, nullable_val, count_val, ratio, flag, payload, raw_json, tags, scores, raw_bytes)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                 RETURNING id"
            ),
            &[
                SqlValue::Int(7),
                SqlValue::Text("00123 — O'Reilly \"café\" नमस्ते".into()),
                SqlValue::Null,
                SqlValue::Int(9_000_000_000),
                SqlValue::Float(12.5),
                SqlValue::Bool(true),
                SqlValue::Json(payload.clone()),
                SqlValue::Json(json!(["plain", 1])),
                SqlValue::TextArray(vec!["one's $1".into(), "two".into()]),
                SqlValue::IntArray(vec![10, -20, 300]),
                SqlValue::Bytes(b"pg-bytes".to_vec()),
            ],
        )
        .unwrap();
    let id: i64 = inserted[0]["id"].parse().unwrap();

    let fetched = db
        .query_values(
            &format!(
                "SELECT small, text_val, nullable_val, count_val, ratio, flag, payload, raw_json, tags, scores, raw_bytes
                 FROM {table}
                 WHERE id = $1 AND nullable_val IS NULL"
            ),
            &[SqlValue::Int(id)],
        )
        .unwrap();
    assert_eq!(fetched.len(), 1);
    let row = &fetched[0];
    assert_eq!(row.get("small").map(String::as_str), Some("7"));
    assert_eq!(
        row.get("text_val").map(String::as_str),
        Some("00123 — O'Reilly \"café\" नमस्ते")
    );
    assert!(!row.contains_key("nullable_val"));
    assert_eq!(row.get("count_val").map(String::as_str), Some("9000000000"));
    assert_eq!(row.get("ratio").map(String::as_str), Some("12.5"));
    assert_eq!(row.get("flag").map(String::as_str), Some("true"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row["payload"]).unwrap(),
        payload
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row["raw_json"]).unwrap(),
        json!(["plain", 1])
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row["tags"]).unwrap(),
        json!(["one's $1", "two"])
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row["scores"]).unwrap(),
        json!([10, -20, 300])
    );
    assert_eq!(row.get("raw_bytes").map(String::as_str), Some("pg-bytes"));

    // Legacy string API preserves numeric-like strings on TEXT columns verbatim.
    db.execute_params(
        &format!("INSERT INTO {table} (text_val, nullable_val, flag) VALUES ($1, $2, $3)"),
        &["00123".into(), "null".into(), "false".into()],
    )
    .unwrap();
    let numeric_text_rows = db
        .query_values(
            &format!("SELECT text_val, nullable_val, flag FROM {table} WHERE text_val = $1"),
            &[SqlValue::Text("00123".into())],
        )
        .unwrap();
    assert_eq!(numeric_text_rows.len(), 1);
    assert_eq!(numeric_text_rows[0]["text_val"], "00123");
    assert_eq!(
        numeric_text_rows[0].get("nullable_val").map(String::as_str),
        Some("null")
    );
    assert_eq!(numeric_text_rows[0]["flag"], "false");

    // Placeholders inside SQL comments, single-quoted strings, and dollar-quoted
    // blocks are ignored by PostgreSQL's parser and never rewritten.
    let comment_and_dollar_sql = format!(
        "/* block comment $1 $2 */
         SELECT
             $$dollar $1 and $99 literal$$ AS dollar_literal,
             '$2 single quoted' AS quoted_literal,
             text_val AS bound_val
         -- line comment $3 $4
         FROM {table}
         WHERE text_val = $1"
    );
    let comment_rows = db
        .query_values(&comment_and_dollar_sql, &[SqlValue::Text("00123".into())])
        .unwrap();
    assert_eq!(comment_rows.len(), 1);
    assert_eq!(
        comment_rows[0]["dollar_literal"],
        "dollar $1 and $99 literal"
    );
    assert_eq!(comment_rows[0]["quoted_literal"], "$2 single quoted");
    assert_eq!(comment_rows[0]["bound_val"], "00123");

    // Unsafe or ambiguous conversions fail clearly instead of guessing types.
    assert!(
        db.execute_values(
            &format!("INSERT INTO {table} (text_val, count_val, flag) VALUES ($1, $2, $3)"),
            &[
                SqlValue::Text("bad".into()),
                SqlValue::Text("00123".into()),
                SqlValue::Bool(true),
            ],
        )
        .is_err(),
        "SqlValue::Text must not silently coerce into BIGINT"
    );
    assert!(
        db.execute_values(
            &format!("INSERT INTO {table} (small, text_val, flag) VALUES ($1, $2, $3)"),
            &[
                SqlValue::Int(100_000),
                SqlValue::Text("overflow".into()),
                SqlValue::Bool(true),
            ],
        )
        .is_err(),
        "out-of-range integer for SMALLINT must fail"
    );
    assert!(
        db.execute_values(
            &format!("INSERT INTO {table} (text_val, flag) VALUES ($1, $2)"),
            &[SqlValue::Int(123), SqlValue::Bool(true)],
        )
        .is_err(),
        "SqlValue::Int must not silently coerce into TEXT"
    );
    assert!(
        db.execute_params(
            &format!("INSERT INTO {table} (text_val, count_val, flag) VALUES ($1, $2, $3)"),
            &["leading-zero".into(), "00123".into(), "true".into()],
        )
        .is_err(),
        "ambiguous leading-zero integer string must fail on BIGINT"
    );
    assert!(
        db.execute_params(
            &format!("INSERT INTO {table} (text_val, count_val, flag) VALUES ($1, $2, $3)"),
            &["empty-null".into(), "null".into(), "true".into()],
        )
        .is_err(),
        "string \"null\" must fail on BIGINT instead of silently becoming NULL"
    );
    assert!(
        db.execute_params(
            &format!("INSERT INTO {table} (text_val, flag) VALUES ($1, $2)"),
            &["ambig-bool".into(), "1".into()],
        )
        .is_err(),
        "ambiguous boolean string \"1\" must fail"
    );
    assert!(
        db.execute_params(
            &format!("INSERT INTO {table} (text_val, flag, payload) VALUES ($1, $2, $3)"),
            &["json-as-str".into(), "true".into(), "{\"a\":1}".into()],
        )
        .is_err(),
        "string API must reject raw string binding into JSONB and require SqlValue::Json"
    );
    assert!(db
        .execute_values(
            &format!("INSERT INTO {table} (text_val, flag) VALUES ($1, $2)"),
            &[SqlValue::Text("wrong-arity".into())],
        )
        .is_err());

    // Regression: malicious SQL payloads bound through values cannot alter query structure.
    let payloads = [
        "'; DROP TABLE terminus_pg_bound_params; --",
        "$$; DELETE FROM terminus_pg_bound_params; --",
        "x' OR '1'='1",
        "$1 UNION SELECT 'injected'",
    ];
    for injection in payloads {
        db.execute_values(
            &format!("INSERT INTO {table} (text_val, flag) VALUES ($1, $2)"),
            &[SqlValue::Text(injection.into()), SqlValue::Bool(true)],
        )
        .unwrap();
    }
    let attacker_probe = db
        .query_values(
            &format!("SELECT text_val FROM {table} WHERE text_val = $1"),
            &[SqlValue::Text("x' OR '1'='1' OR 'a'='a".into())],
        )
        .unwrap();
    assert!(
        attacker_probe.is_empty(),
        "bound parameter must match only literal equality, never OR 1=1"
    );
    let stored_injection = db
        .query_params(
            &format!("SELECT text_val FROM {table} WHERE text_val = $1"),
            &["'; DROP TABLE terminus_pg_bound_params; --".to_string()],
        )
        .unwrap();
    assert_eq!(stored_injection.len(), 1);

    db.execute(&format!("DROP TABLE IF EXISTS {table}"))
        .unwrap();
}

#[tokio::test]
async fn sql_query_bind_value_works_with_global_sqlite() {
    let db = Database::memory().unwrap();
    db.execute("CREATE TABLE sq_items (id INTEGER PRIMARY KEY AUTOINCREMENT, code TEXT NOT NULL, note TEXT, qty INTEGER NOT NULL)")
        .unwrap();
    Database::set_global(db.clone());
    SqlQuery::new("INSERT INTO sq_items (code, note, qty) VALUES (?, ?, ?)")
        .bind_value("00999")
        .bind_value(SqlValue::Null)
        .bind_value(5_i64)
        .execute()
        .await
        .unwrap();
    let rows = SqlQuery::new("SELECT code, note, qty FROM sq_items WHERE code = ?")
        .bind_value("00999")
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["code"], "00999");
    assert!(!rows[0].contains_key("note"));
    assert_eq!(rows[0]["qty"], "5");
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_backend_roundtrip_and_migrations() {
    let Some(url) = external_test_url("TERMINUS_TEST_POSTGRES_URL", "postgres") else {
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

#[cfg(feature = "mysql")]
#[test]
fn mysql_backend_roundtrip_and_migrations() {
    let Some(url) = external_test_url("TERMINUS_TEST_MYSQL_URL", "mysql") else {
        return;
    };
    let db = Database::connect(&url).unwrap();
    exercise_backend(&db, DatabaseKind::Mysql, "terminus_backend_widgets");

    let table = "terminus_mysql_typed_params";
    db.execute(&format!("DROP TABLE IF EXISTS {table}"))
        .unwrap();
    db.execute(&format!(
        "CREATE TABLE {table} (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            code VARCHAR(64) NOT NULL,
            note TEXT,
            qty BIGINT,
            active BOOLEAN NOT NULL,
            meta JSON
        )"
    ))
    .unwrap();
    db.execute_values(
        &format!("INSERT INTO {table} (code, note, qty, active, meta) VALUES (?, ?, ?, ?, ?)"),
        &[
            SqlValue::Text("00123".into()),
            SqlValue::Null,
            SqlValue::Int(42),
            SqlValue::Bool(true),
            SqlValue::Json(json!({"owner": "O'Reilly"})),
        ],
    )
    .unwrap();
    let rows = db
        .query_values(
            &format!("SELECT code, note, qty, active, meta FROM {table} WHERE code = ?"),
            &[SqlValue::Text("00123".into())],
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["code"], "00123");
    assert!(!rows[0].contains_key("note"));
    assert_eq!(rows[0]["qty"], "42");
    assert_eq!(rows[0]["active"], "1");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rows[0]["meta"]).unwrap(),
        json!({"owner": "O'Reilly"})
    );
    db.execute(&format!("DROP TABLE IF EXISTS {table}"))
        .unwrap();
}
