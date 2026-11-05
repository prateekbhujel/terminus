//! Multi-database connection layer.
//!
//! SQLite remains the default, zero-configuration backend. PostgreSQL and
//! MySQL/MariaDB are first-class backends for the ORM, migrations, and CLI,
//! enabled by the default `postgres` and `mysql` Cargo features. Build with
//! `--no-default-features` for a SQLite-only binary.
//!
//! All values cross this layer as strings, matching the ORM's string-based
//! row model. Placeholders are written per-dialect: `?` for SQLite/MySQL,
//! `$1, $2, ...` for PostgreSQL (see [`DatabaseKind::placeholder`]).
//!
//! MongoDB is intentionally not part of this layer: it is a document store,
//! not a SQL dialect, and belongs behind a separate document adapter rather
//! than being forced through relational SQL generation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// The SQL backend a connection or configuration targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatabaseKind {
    #[default]
    Sqlite,
    Postgres,
    Mysql,
}

impl DatabaseKind {
    /// Detect the backend from a connection URL. Plain paths and
    /// `sqlite://` URLs are SQLite; `mariadb://` uses the MySQL backend.
    pub fn from_url(url: &str) -> Self {
        let lower = url.trim().to_ascii_lowercase();
        if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
            Self::Postgres
        } else if lower.starts_with("mysql://") || lower.starts_with("mariadb://") {
            Self::Mysql
        } else {
            Self::Sqlite
        }
    }

    /// The parameter placeholder for the 1-based parameter `index`.
    pub fn placeholder(&self, index: usize) -> String {
        match self {
            Self::Postgres => format!("${index}"),
            Self::Sqlite | Self::Mysql => "?".to_string(),
        }
    }

    /// DDL for the framework migration bookkeeping table on this backend.
    pub(crate) fn migrations_table_ddl(&self) -> &'static str {
        match self {
            Self::Sqlite => {
                "CREATE TABLE IF NOT EXISTS _terminus_migrations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    batch INTEGER NOT NULL,
                    applied_at TEXT NOT NULL
                )"
            }
            Self::Postgres => {
                "CREATE TABLE IF NOT EXISTS _terminus_migrations (
                    id BIGSERIAL PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    batch BIGINT NOT NULL,
                    applied_at TEXT NOT NULL
                )"
            }
            Self::Mysql => {
                "CREATE TABLE IF NOT EXISTS _terminus_migrations (
                    id BIGINT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(255) NOT NULL UNIQUE,
                    batch BIGINT NOT NULL,
                    applied_at VARCHAR(64) NOT NULL
                )"
            }
        }
    }
}

/// A live connection to any supported backend.
#[derive(Clone)]
pub enum AnyConnection {
    Sqlite(Arc<Mutex<rusqlite::Connection>>),
    #[cfg(feature = "postgres")]
    Postgres(Arc<Mutex<postgres::Client>>),
    #[cfg(feature = "mysql")]
    Mysql(Arc<Mutex<mysql::Conn>>),
}

impl AnyConnection {
    /// Connect using a `sqlite://`, `postgres://`, or `mysql://` URL
    /// (a plain file path also means SQLite).
    pub fn connect(url: &str) -> Result<Self, String> {
        match DatabaseKind::from_url(url) {
            DatabaseKind::Sqlite => {
                let path = url
                    .trim()
                    .strip_prefix("sqlite://")
                    .unwrap_or(url.trim());
                let conn = if path == ":memory:" {
                    rusqlite::Connection::open_in_memory()
                } else {
                    rusqlite::Connection::open(path)
                }
                .map_err(|e| e.to_string())?;
                Ok(Self::Sqlite(Arc::new(Mutex::new(conn))))
            }
            DatabaseKind::Postgres => {
                #[cfg(feature = "postgres")]
                {
                    let client = postgres::Client::connect(url, postgres::NoTls)
                        .map_err(|e| e.to_string())?;
                    Ok(Self::Postgres(Arc::new(Mutex::new(client))))
                }
                #[cfg(not(feature = "postgres"))]
                {
                    Err("PostgreSQL support is not compiled in; enable the `postgres` Cargo feature".into())
                }
            }
            DatabaseKind::Mysql => {
                #[cfg(feature = "mysql")]
                {
                    let opts = mysql::Opts::from_url(url).map_err(|e| e.to_string())?;
                    let conn = mysql::Conn::new(opts).map_err(|e| e.to_string())?;
                    Ok(Self::Mysql(Arc::new(Mutex::new(conn))))
                }
                #[cfg(not(feature = "mysql"))]
                {
                    Err("MySQL support is not compiled in; enable the `mysql` Cargo feature".into())
                }
            }
        }
    }

    pub fn kind(&self) -> DatabaseKind {
        match self {
            Self::Sqlite(_) => DatabaseKind::Sqlite,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => DatabaseKind::Postgres,
            #[cfg(feature = "mysql")]
            Self::Mysql(_) => DatabaseKind::Mysql,
        }
    }

    /// Lock the underlying connection. Hold the guard across multi-step
    /// operations or transactions so no other caller interleaves.
    pub fn lock(&self) -> AnyGuard<'_> {
        match self {
            Self::Sqlite(conn) => AnyGuard::Sqlite(conn.lock().unwrap()),
            #[cfg(feature = "postgres")]
            Self::Postgres(conn) => AnyGuard::Postgres(conn.lock().unwrap()),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => AnyGuard::Mysql(conn.lock().unwrap()),
        }
    }

    /// Execute one parameterized statement; returns affected rows.
    pub fn execute(&self, sql: &str, params: &[String]) -> Result<u64, String> {
        self.lock().execute(sql, params)
    }

    /// Run one parameterized query; rows map column names to string values.
    /// SQL NULL values are omitted from the row map.
    pub fn query(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<HashMap<String, String>>, String> {
        self.lock().query(sql, params)
    }

    /// Execute a script of one or more semicolon-separated statements.
    pub fn execute_batch(&self, sql: &str) -> Result<(), String> {
        self.lock().execute_batch(sql)
    }
}

/// A locked connection guard; see [`AnyConnection::lock`].
pub enum AnyGuard<'a> {
    Sqlite(MutexGuard<'a, rusqlite::Connection>),
    #[cfg(feature = "postgres")]
    Postgres(MutexGuard<'a, postgres::Client>),
    #[cfg(feature = "mysql")]
    Mysql(MutexGuard<'a, mysql::Conn>),
}

impl AnyGuard<'_> {
    pub fn execute(&mut self, sql: &str, params: &[String]) -> Result<u64, String> {
        match self {
            Self::Sqlite(conn) => conn
                .execute(sql, rusqlite::params_from_iter(params.iter()))
                .map(|rows| rows as u64)
                .map_err(|e| e.to_string()),
            #[cfg(feature = "postgres")]
            Self::Postgres(client) => {
                let sql = pg_inline_params(sql, params)?;
                client.execute(sql.as_str(), &[]).map_err(|e| e.to_string())
            }
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                use mysql::prelude::Queryable;
                let values: Vec<mysql::Value> = params
                    .iter()
                    .map(|value| mysql::Value::Bytes(value.clone().into_bytes()))
                    .collect();
                conn.exec_drop(sql, mysql::Params::Positional(values))
                    .map_err(|e| e.to_string())?;
                Ok(conn.affected_rows())
            }
        }
    }

    pub fn query(
        &mut self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<HashMap<String, String>>, String> {
        match self {
            Self::Sqlite(conn) => {
                let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
                let columns: Vec<String> =
                    stmt.column_names().iter().map(|name| name.to_string()).collect();
                let rows = stmt
                    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                        let mut map = HashMap::new();
                        for (index, column) in columns.iter().enumerate() {
                            let value = match row.get::<_, rusqlite::types::Value>(index) {
                                Ok(rusqlite::types::Value::Null) => None,
                                Ok(rusqlite::types::Value::Integer(v)) => Some(v.to_string()),
                                Ok(rusqlite::types::Value::Real(v)) => Some(v.to_string()),
                                Ok(rusqlite::types::Value::Text(v)) => Some(v),
                                Ok(rusqlite::types::Value::Blob(bytes)) => {
                                    Some(String::from_utf8_lossy(&bytes).into_owned())
                                }
                                Err(_) => None,
                            };
                            if let Some(value) = value {
                                map.insert(column.clone(), value);
                            }
                        }
                        Ok(map)
                    })
                    .map_err(|e| e.to_string())?;
                rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(client) => {
                let sql = pg_inline_params(sql, params)?;
                let rows = client.query(sql.as_str(), &[]).map_err(|e| e.to_string())?;
                let mut out = Vec::with_capacity(rows.len());
                for row in &rows {
                    let mut map = HashMap::new();
                    for (index, column) in row.columns().iter().enumerate() {
                        if let Some(value) = postgres_value(row, index) {
                            map.insert(column.name().to_string(), value);
                        }
                    }
                    out.push(map);
                }
                Ok(out)
            }
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                use mysql::prelude::Queryable;
                let values: Vec<mysql::Value> = params
                    .iter()
                    .map(|value| mysql::Value::Bytes(value.clone().into_bytes()))
                    .collect();
                let rows: Vec<mysql::Row> = conn
                    .exec(sql, mysql::Params::Positional(values))
                    .map_err(|e| e.to_string())?;
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    let columns: Vec<String> = row
                        .columns_ref()
                        .iter()
                        .map(|column| column.name_str().into_owned())
                        .collect();
                    let unwrapped = row.unwrap();
                    let mut map = HashMap::new();
                    for (index, column) in columns.into_iter().enumerate() {
                        if let Some(value) = mysql_value(&unwrapped[index]) {
                            map.insert(column, value);
                        }
                    }
                    out.push(map);
                }
                Ok(out)
            }
        }
    }

    pub fn execute_batch(&mut self, sql: &str) -> Result<(), String> {
        match self {
            Self::Sqlite(conn) => conn.execute_batch(sql).map_err(|e| e.to_string()),
            #[cfg(feature = "postgres")]
            Self::Postgres(client) => client.batch_execute(sql).map_err(|e| e.to_string()),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                use mysql::prelude::Queryable;
                for statement in split_statements(sql) {
                    conn.query_drop(statement).map_err(|e| e.to_string())?;
                }
                Ok(())
            }
        }
    }

    pub fn begin(&mut self) -> Result<(), String> {
        self.execute_batch("BEGIN")
    }

    pub fn commit(&mut self) -> Result<(), String> {
        self.execute_batch("COMMIT")
    }

    pub fn rollback(&mut self) -> Result<(), String> {
        self.execute_batch("ROLLBACK")
    }
}


#[cfg(feature = "postgres")]
fn pg_literal(value: &str) -> String {
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        value.to_string()
    } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        value.to_ascii_uppercase()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

/// Inline `$n` placeholders as escaped literals. The ORM layer is
/// string-based and PostgreSQL parameters are strictly typed client-side;
/// literal binding keeps one semantics across backends. Quotes inside the
/// SQL itself are respected, and values are single-quote escaped.
#[cfg(feature = "postgres")]
fn pg_inline_params(sql: &str, params: &[String]) -> Result<String, String> {
    let mut out = String::with_capacity(sql.len() + 32);
    let mut chars = sql.chars().peekable();
    let mut in_quote = false;
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            out.push(ch);
            if in_quote && chars.peek() == Some(&'\'') {
                out.push(chars.next().unwrap());
            } else {
                in_quote = !in_quote;
            }
            continue;
        }
        if ch == '$' && !in_quote {
            let mut digits = String::new();
            while let Some(next) = chars.peek() {
                if next.is_ascii_digit() {
                    digits.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if digits.is_empty() {
                out.push(ch);
                continue;
            }
            let index: usize = digits
                .parse()
                .map_err(|_| "Invalid PostgreSQL placeholder".to_string())?;
            let value = params
                .get(index.wrapping_sub(1))
                .ok_or_else(|| format!("Missing value for placeholder ${index}"))?;
            out.push_str(&pg_literal(value));
            continue;
        }
        out.push(ch);
    }
    Ok(out)
}

#[cfg(feature = "postgres")]
fn postgres_value(row: &postgres::Row, index: usize) -> Option<String> {
    use postgres::types::Type;
    match *row.columns()[index].type_() {
        Type::BOOL => row.try_get::<_, bool>(index).ok().map(|v| v.to_string()),
        Type::INT2 => row.try_get::<_, i16>(index).ok().map(|v| v.to_string()),
        Type::INT4 => row.try_get::<_, i32>(index).ok().map(|v| v.to_string()),
        Type::INT8 => row.try_get::<_, i64>(index).ok().map(|v| v.to_string()),
        Type::FLOAT4 => row.try_get::<_, f32>(index).ok().map(|v| v.to_string()),
        Type::FLOAT8 => row.try_get::<_, f64>(index).ok().map(|v| v.to_string()),
        _ => row.try_get::<_, String>(index).ok(),
    }
}

#[cfg(feature = "mysql")]
fn mysql_value(value: &mysql::Value) -> Option<String> {
    match value {
        mysql::Value::NULL => None,
        mysql::Value::Bytes(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        mysql::Value::Int(v) => Some(v.to_string()),
        mysql::Value::UInt(v) => Some(v.to_string()),
        mysql::Value::Float(v) => Some(v.to_string()),
        mysql::Value::Double(v) => Some(v.to_string()),
        mysql::Value::Date(year, month, day, hour, minute, second, micros) => Some(format!(
            "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{micros:06}"
        )),
        mysql::Value::Time(negative, days, hour, minute, second, micros) => Some(format!(
            "{}{days} {hour:02}:{minute:02}:{second:02}.{micros:06}",
            if *negative { "-" } else { "" }
        )),
    }
}

/// Split a script on top-level semicolons (quote-aware) for drivers that
/// accept one statement at a time.
#[cfg(feature = "mysql")]
fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        current.push(ch);
        match quote {
            Some(active) if ch == active => {
                if chars.peek() == Some(&active) {
                    if let Some(escaped) = chars.next() {
                        current.push(escaped);
                    }
                } else {
                    quote = None;
                }
            }
            Some(_) => {}
            None if ch == '\'' || ch == '"' || ch == '`' => quote = Some(ch),
            None if ch == ';' => {
                let statement = current.trim().trim_end_matches(';').trim().to_string();
                if !statement.is_empty() {
                    statements.push(statement);
                }
                current = String::new();
            }
            None => {}
        }
    }
    let tail = current.trim().trim_end_matches(';').trim().to_string();
    if !tail.is_empty() {
        statements.push(tail);
    }
    statements
}
