//! Multi-database connection layer.
//!
//! SQLite remains the default, zero-configuration backend. PostgreSQL and
//! MySQL/MariaDB are first-class backends for the ORM, migrations, and CLI,
//! enabled by the default `postgres` and `mysql` Cargo features. Build with
//! `--no-default-features` for a SQLite-only binary.
//!
//! Prefer explicit [`SqlValue`] bindings via [`AnyConnection::execute_values`]
//! / [`AnyConnection::query_values`] (or [`crate::orm::Database::execute_values`]
//! / [`crate::orm::Database::query_values`]) whenever queries involve SQL `NULL`,
//! numbers, booleans, JSON, binary payloads, arrays, or numeric-like strings
//! such as `"00123"`.
//!
//! Legacy string-slice helpers (`execute`, `query`) remain supported: on
//! PostgreSQL, statements are prepared by the driver first so parameter binding
//! uses native protocol `ToSql` bindings guided by the server's expected
//! parameter types without SQL text substitution. Placeholders are written
//! per-dialect: `?` for SQLite/MySQL, `$1, $2, ...` for PostgreSQL (see
//! [`DatabaseKind::placeholder`]).
//!
//! MongoDB is intentionally not part of this layer: it is a document store,
//! not a SQL dialect, and belongs behind a separate document adapter rather
//! than being forced through relational SQL generation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// Explicit typed SQL parameter value passed through native driver bindings.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Json(serde_json::Value),
    Bytes(Vec<u8>),
    TextArray(Vec<String>),
    IntArray(Vec<i64>),
}

impl From<String> for SqlValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for SqlValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<bool> for SqlValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i16> for SqlValue {
    fn from(value: i16) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<i32> for SqlValue {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<i64> for SqlValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f32> for SqlValue {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

impl From<f64> for SqlValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<serde_json::Value> for SqlValue {
    fn from(value: serde_json::Value) -> Self {
        Self::Json(value)
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl From<&[u8]> for SqlValue {
    fn from(value: &[u8]) -> Self {
        Self::Bytes(value.to_vec())
    }
}

impl From<Vec<String>> for SqlValue {
    fn from(value: Vec<String>) -> Self {
        Self::TextArray(value)
    }
}

impl From<Vec<i64>> for SqlValue {
    fn from(value: Vec<i64>) -> Self {
        Self::IntArray(value)
    }
}

impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(inner) => inner.into(),
            None => Self::Null,
        }
    }
}

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
                let path = url.trim().strip_prefix("sqlite://").unwrap_or(url.trim());
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
    ///
    /// Prefer [`AnyConnection::execute_values`] for explicit SQL types or `NULL`.
    pub fn execute(&self, sql: &str, params: &[String]) -> Result<u64, String> {
        self.lock().execute(sql, params)
    }

    /// Execute one statement with explicit typed [`SqlValue`] parameters.
    pub fn execute_values(&self, sql: &str, params: &[SqlValue]) -> Result<u64, String> {
        self.lock().execute_values(sql, params)
    }

    /// Run one parameterized query; rows map column names to string values.
    /// SQL NULL values are omitted from the row map.
    ///
    /// Prefer [`AnyConnection::query_values`] for explicit SQL types or `NULL`.
    pub fn query(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<Vec<HashMap<String, String>>, String> {
        self.lock().query(sql, params)
    }

    /// Run one query with explicit typed [`SqlValue`] parameters.
    /// SQL NULL values are omitted from the returned row maps.
    pub fn query_values(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<HashMap<String, String>>, String> {
        self.lock().query_values(sql, params)
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
            Self::Postgres(client) => pg_execute_strings(client, sql, params),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                let values: Vec<mysql::Value> = params
                    .iter()
                    .map(|value| mysql::Value::Bytes(value.clone().into_bytes()))
                    .collect();
                mysql_execute(conn, sql, values)
            }
        }
    }

    pub fn execute_values(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, String> {
        match self {
            Self::Sqlite(conn) => {
                let values = sqlite_values(params)?;
                conn.execute(sql, rusqlite::params_from_iter(values.iter()))
                    .map(|rows| rows as u64)
                    .map_err(|e| e.to_string())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(client) => pg_execute_values(client, sql, params),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                let values = mysql_values(params)?;
                mysql_execute(conn, sql, values)
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
                sqlite_query(conn, sql, rusqlite::params_from_iter(params.iter()))
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(client) => pg_query_strings(client, sql, params),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                let values: Vec<mysql::Value> = params
                    .iter()
                    .map(|value| mysql::Value::Bytes(value.clone().into_bytes()))
                    .collect();
                mysql_query(conn, sql, values)
            }
        }
    }

    pub fn query_values(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<HashMap<String, String>>, String> {
        match self {
            Self::Sqlite(conn) => {
                let values = sqlite_values(params)?;
                sqlite_query(conn, sql, rusqlite::params_from_iter(values.iter()))
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(client) => pg_query_values(client, sql, params),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                let values = mysql_values(params)?;
                mysql_query(conn, sql, values)
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

pub(crate) fn sqlite_values(params: &[SqlValue]) -> Result<Vec<rusqlite::types::Value>, String> {
    params
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let pos = index + 1;
            Ok(match value {
                SqlValue::Null => rusqlite::types::Value::Null,
                SqlValue::Bool(v) => rusqlite::types::Value::Integer(if *v { 1 } else { 0 }),
                SqlValue::Int(v) => rusqlite::types::Value::Integer(*v),
                SqlValue::Float(v) => {
                    if !v.is_finite() {
                        return Err(format!(
                            "Parameter ?{pos}: non-finite float value cannot be bound safely"
                        ));
                    }
                    rusqlite::types::Value::Real(*v)
                }
                SqlValue::Text(v) => rusqlite::types::Value::Text(v.clone()),
                SqlValue::Json(v) => rusqlite::types::Value::Text(v.to_string()),
                SqlValue::Bytes(v) => rusqlite::types::Value::Blob(v.clone()),
                SqlValue::TextArray(v) => rusqlite::types::Value::Text(
                    serde_json::to_string(v).map_err(|e| e.to_string())?,
                ),
                SqlValue::IntArray(v) => rusqlite::types::Value::Text(
                    serde_json::to_string(v).map_err(|e| e.to_string())?,
                ),
            })
        })
        .collect()
}

pub(crate) fn sqlite_query<P: rusqlite::Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: P,
) -> Result<Vec<HashMap<String, String>>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let columns: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    let rows = stmt
        .query_map(params, |row| {
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

#[cfg(feature = "mysql")]
fn mysql_values(params: &[SqlValue]) -> Result<Vec<mysql::Value>, String> {
    params
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let pos = index + 1;
            Ok(match value {
                SqlValue::Null => mysql::Value::NULL,
                SqlValue::Bool(v) => mysql::Value::Int(if *v { 1 } else { 0 }),
                SqlValue::Int(v) => mysql::Value::Int(*v),
                SqlValue::Float(v) => {
                    if !v.is_finite() {
                        return Err(format!(
                            "Parameter ?{pos}: non-finite float value cannot be bound safely"
                        ));
                    }
                    mysql::Value::Double(*v)
                }
                SqlValue::Text(v) => mysql::Value::Bytes(v.clone().into_bytes()),
                SqlValue::Json(v) => mysql::Value::Bytes(v.to_string().into_bytes()),
                SqlValue::Bytes(v) => mysql::Value::Bytes(v.clone()),
                SqlValue::TextArray(v) => {
                    mysql::Value::Bytes(serde_json::to_vec(v).map_err(|e| e.to_string())?)
                }
                SqlValue::IntArray(v) => {
                    mysql::Value::Bytes(serde_json::to_vec(v).map_err(|e| e.to_string())?)
                }
            })
        })
        .collect()
}

#[cfg(feature = "mysql")]
fn mysql_execute(
    conn: &mut mysql::Conn,
    sql: &str,
    values: Vec<mysql::Value>,
) -> Result<u64, String> {
    use mysql::prelude::Queryable;
    conn.exec_drop(sql, mysql::Params::Positional(values))
        .map_err(|e| e.to_string())?;
    Ok(conn.affected_rows())
}

#[cfg(feature = "mysql")]
fn mysql_query(
    conn: &mut mysql::Conn,
    sql: &str,
    values: Vec<mysql::Value>,
) -> Result<Vec<HashMap<String, String>>, String> {
    use mysql::prelude::Queryable;
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

#[cfg(feature = "postgres")]
#[derive(Debug)]
struct PgNull;

#[cfg(feature = "postgres")]
impl postgres::types::ToSql for PgNull {
    fn to_sql(
        &self,
        _ty: &postgres::types::Type,
        _out: &mut postgres::types::private::BytesMut,
    ) -> Result<postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        Ok(postgres::types::IsNull::Yes)
    }

    fn accepts(_ty: &postgres::types::Type) -> bool {
        true
    }

    postgres::types::to_sql_checked!();
}

#[cfg(feature = "postgres")]
fn pg_default_type(value: &SqlValue) -> postgres::types::Type {
    use postgres::types::Type;
    match value {
        SqlValue::Null | SqlValue::Text(_) => Type::TEXT,
        SqlValue::Bool(_) => Type::BOOL,
        SqlValue::Int(_) => Type::INT8,
        SqlValue::Float(_) => Type::FLOAT8,
        SqlValue::Json(_) => Type::JSONB,
        SqlValue::Bytes(_) => Type::BYTEA,
        SqlValue::TextArray(_) => Type::TEXT_ARRAY,
        SqlValue::IntArray(_) => Type::INT8_ARRAY,
    }
}

#[cfg(feature = "postgres")]
fn parse_strict_i64(value: &str) -> Option<i64> {
    if value.is_empty() || value.trim() != value || value.starts_with('+') || value == "-0" {
        return None;
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    value.parse::<i64>().ok()
}

#[cfg(feature = "postgres")]
fn parse_strict_f64(value: &str) -> Option<f64> {
    if value.is_empty() || value.trim() != value || value.starts_with('+') {
        return None;
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let int_part = unsigned.split(['.', 'e', 'E']).next()?;
    if int_part.is_empty()
        || !int_part.bytes().all(|b| b.is_ascii_digit())
        || (int_part.len() > 1 && int_part.starts_with('0'))
    {
        return None;
    }
    let parsed = value.parse::<f64>().ok()?;
    parsed.is_finite().then_some(parsed)
}

#[cfg(feature = "postgres")]
fn pg_prepare_for_values(
    client: &mut postgres::Client,
    sql: &str,
    params: &[SqlValue],
) -> Result<
    (
        postgres::Statement,
        Vec<Box<dyn postgres::types::ToSql + Sync>>,
    ),
    String,
> {
    use postgres::types::Type;
    let inferred = client.prepare(sql).map_err(|e| e.to_string())?;
    if params.len() != inferred.params().len() {
        return Err(format!(
            "Parameter count mismatch: SQL expects {} parameter(s), got {}",
            inferred.params().len(),
            params.len()
        ));
    }
    let stmt = if inferred.params().contains(&Type::UNKNOWN) {
        let resolved: Vec<Type> = inferred
            .params()
            .iter()
            .zip(params.iter())
            .map(|(ty, val)| {
                if *ty == Type::UNKNOWN {
                    pg_default_type(val)
                } else {
                    ty.clone()
                }
            })
            .collect();
        client
            .prepare_typed(sql, &resolved)
            .map_err(|e| e.to_string())?
    } else {
        inferred
    };
    let bound = params
        .iter()
        .zip(stmt.params().iter())
        .enumerate()
        .map(|(index, (val, expected))| pg_bind_sql_value(index + 1, val, expected))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((stmt, bound))
}

#[cfg(feature = "postgres")]
fn pg_prepare_for_strings(
    client: &mut postgres::Client,
    sql: &str,
    params: &[String],
) -> Result<
    (
        postgres::Statement,
        Vec<Box<dyn postgres::types::ToSql + Sync>>,
    ),
    String,
> {
    let stmt = client.prepare(sql).map_err(|e| e.to_string())?;
    if params.len() != stmt.params().len() {
        return Err(format!(
            "Parameter count mismatch: SQL expects {} parameter(s), got {}",
            stmt.params().len(),
            params.len()
        ));
    }
    let bound = params
        .iter()
        .zip(stmt.params().iter())
        .enumerate()
        .map(|(index, (val, expected))| pg_bind_string(index + 1, val, expected))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((stmt, bound))
}

#[cfg(feature = "postgres")]
fn pg_bind_int(
    pos: usize,
    value: i64,
    expected: &postgres::types::Type,
) -> Result<Box<dyn postgres::types::ToSql + Sync>, String> {
    use postgres::types::Type;
    match *expected {
        Type::INT2 => i16::try_from(value)
            .map(|v| Box::new(v) as Box<dyn postgres::types::ToSql + Sync>)
            .map_err(|_| {
                format!("Parameter ${pos}: integer {value} overflows PostgreSQL SMALLINT")
            }),
        Type::INT4 => i32::try_from(value)
            .map(|v| Box::new(v) as Box<dyn postgres::types::ToSql + Sync>)
            .map_err(|_| format!("Parameter ${pos}: integer {value} overflows PostgreSQL INTEGER")),
        Type::INT8 => Ok(Box::new(value)),
        Type::OID => u32::try_from(value)
            .map(|v| Box::new(v) as Box<dyn postgres::types::ToSql + Sync>)
            .map_err(|_| {
                format!("Parameter ${pos}: integer {value} is invalid for PostgreSQL OID")
            }),
        _ => Err(format!(
            "Parameter ${pos}: cannot bind integer {value} to PostgreSQL type {expected}"
        )),
    }
}

#[cfg(feature = "postgres")]
fn pg_bind_float(
    pos: usize,
    value: f64,
    expected: &postgres::types::Type,
) -> Result<Box<dyn postgres::types::ToSql + Sync>, String> {
    use postgres::types::Type;
    if !value.is_finite() {
        return Err(format!(
            "Parameter ${pos}: non-finite float value cannot be bound safely"
        ));
    }
    match *expected {
        Type::FLOAT4 => {
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(format!(
                    "Parameter ${pos}: float {value} overflows PostgreSQL REAL"
                ));
            }
            Ok(Box::new(narrowed))
        }
        Type::FLOAT8 => Ok(Box::new(value)),
        _ => Err(format!(
            "Parameter ${pos}: cannot bind float {value} to PostgreSQL type {expected}"
        )),
    }
}

#[cfg(feature = "postgres")]
fn pg_bind_sql_value(
    pos: usize,
    value: &SqlValue,
    expected: &postgres::types::Type,
) -> Result<Box<dyn postgres::types::ToSql + Sync>, String> {
    use postgres::types::{ToSql, Type};
    match value {
        SqlValue::Null => Ok(Box::new(PgNull)),
        SqlValue::Bool(v) => {
            if *expected == Type::BOOL {
                Ok(Box::new(*v))
            } else {
                Err(format!(
                    "Parameter ${pos}: cannot bind boolean to PostgreSQL type {expected}"
                ))
            }
        }
        SqlValue::Int(v) => pg_bind_int(pos, *v, expected),
        SqlValue::Float(v) => pg_bind_float(pos, *v, expected),
        SqlValue::Text(s) => {
            if <&str as ToSql>::accepts(expected) {
                Ok(Box::new(s.clone()))
            } else {
                Err(format!(
                    "Parameter ${pos}: cannot bind text parameter to PostgreSQL type {expected}; pass an explicit typed SqlValue"
                ))
            }
        }
        SqlValue::Json(v) => {
            if <serde_json::Value as ToSql>::accepts(expected) {
                Ok(Box::new(v.clone()))
            } else {
                Err(format!(
                    "Parameter ${pos}: cannot bind JSON to PostgreSQL type {expected}"
                ))
            }
        }
        SqlValue::Bytes(bytes) => {
            if <Vec<u8> as ToSql>::accepts(expected) {
                Ok(Box::new(bytes.clone()))
            } else {
                Err(format!(
                    "Parameter ${pos}: cannot bind bytes to PostgreSQL type {expected}"
                ))
            }
        }
        SqlValue::TextArray(items) => {
            if <Vec<String> as ToSql>::accepts(expected) {
                Ok(Box::new(items.clone()))
            } else {
                Err(format!(
                    "Parameter ${pos}: cannot bind text array to PostgreSQL type {expected}"
                ))
            }
        }
        SqlValue::IntArray(items) => match *expected {
            Type::INT8_ARRAY => Ok(Box::new(items.clone())),
            Type::INT4_ARRAY => {
                let narrowed = items
                    .iter()
                    .map(|v| {
                        i32::try_from(*v).map_err(|_| {
                            format!(
                                "Parameter ${pos}: array integer {v} overflows PostgreSQL INT4[]"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Box::new(narrowed))
            }
            Type::INT2_ARRAY => {
                let narrowed = items
                    .iter()
                    .map(|v| {
                        i16::try_from(*v).map_err(|_| {
                            format!(
                                "Parameter ${pos}: array integer {v} overflows PostgreSQL INT2[]"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Box::new(narrowed))
            }
            _ => Err(format!(
                "Parameter ${pos}: cannot bind integer array to PostgreSQL type {expected}"
            )),
        },
    }
}

#[cfg(feature = "postgres")]
fn pg_bind_string(
    pos: usize,
    value: &str,
    expected: &postgres::types::Type,
) -> Result<Box<dyn postgres::types::ToSql + Sync>, String> {
    use postgres::types::{ToSql, Type};
    match *expected {
        ref ty if <&str as ToSql>::accepts(ty) => Ok(Box::new(value.to_string())),
        Type::INT2 | Type::INT4 | Type::INT8 | Type::OID => {
            let parsed = parse_strict_i64(value).ok_or_else(|| {
                format!(
                    "Parameter ${pos}: ambiguous or invalid integer string {value:?} for PostgreSQL type {expected}; pass SqlValue::Int via execute_values/query_values"
                )
            })?;
            pg_bind_int(pos, parsed, expected)
        }
        Type::FLOAT4 | Type::FLOAT8 => {
            let parsed = parse_strict_f64(value).ok_or_else(|| {
                format!(
                    "Parameter ${pos}: ambiguous or invalid float string {value:?} for PostgreSQL type {expected}; pass SqlValue::Float via execute_values/query_values"
                )
            })?;
            pg_bind_float(pos, parsed, expected)
        }
        Type::BOOL => match value {
            "true" => Ok(Box::new(true)),
            "false" => Ok(Box::new(false)),
            _ => Err(format!(
                "Parameter ${pos}: ambiguous boolean string {value:?} for PostgreSQL BOOL column; pass SqlValue::Bool via execute_values/query_values"
            )),
        },
        _ => Err(format!(
            "Parameter ${pos}: string-only binding cannot safely convert value for PostgreSQL type {expected}; pass an explicit SqlValue via execute_values/query_values"
        )),
    }
}

#[cfg(feature = "postgres")]
fn pg_execute_values(
    client: &mut postgres::Client,
    sql: &str,
    params: &[SqlValue],
) -> Result<u64, String> {
    let (stmt, bound) = pg_prepare_for_values(client, sql, params)?;
    let refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
        bound.iter().map(|value| value.as_ref()).collect();
    client.execute(&stmt, &refs).map_err(|e| e.to_string())
}

#[cfg(feature = "postgres")]
fn pg_query_values(
    client: &mut postgres::Client,
    sql: &str,
    params: &[SqlValue],
) -> Result<Vec<HashMap<String, String>>, String> {
    let (stmt, bound) = pg_prepare_for_values(client, sql, params)?;
    pg_run_query(client, &stmt, &bound)
}

#[cfg(feature = "postgres")]
fn pg_execute_strings(
    client: &mut postgres::Client,
    sql: &str,
    params: &[String],
) -> Result<u64, String> {
    let (stmt, bound) = pg_prepare_for_strings(client, sql, params)?;
    let refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
        bound.iter().map(|value| value.as_ref()).collect();
    client.execute(&stmt, &refs).map_err(|e| e.to_string())
}

#[cfg(feature = "postgres")]
fn pg_query_strings(
    client: &mut postgres::Client,
    sql: &str,
    params: &[String],
) -> Result<Vec<HashMap<String, String>>, String> {
    let (stmt, bound) = pg_prepare_for_strings(client, sql, params)?;
    pg_run_query(client, &stmt, &bound)
}

#[cfg(feature = "postgres")]
fn pg_run_query(
    client: &mut postgres::Client,
    stmt: &postgres::Statement,
    bound: &[Box<dyn postgres::types::ToSql + Sync>],
) -> Result<Vec<HashMap<String, String>>, String> {
    let refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
        bound.iter().map(|value| value.as_ref()).collect();
    let rows = client.query(stmt, &refs).map_err(|e| e.to_string())?;
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

#[cfg(feature = "postgres")]
fn postgres_value(row: &postgres::Row, index: usize) -> Option<String> {
    use postgres::types::Type;
    match *row.columns()[index].type_() {
        Type::BOOL => row
            .try_get::<_, Option<bool>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::INT2 => row
            .try_get::<_, Option<i16>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::INT4 => row
            .try_get::<_, Option<i32>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::INT8 => row
            .try_get::<_, Option<i64>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::FLOAT4 => row
            .try_get::<_, Option<f32>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::FLOAT8 => row
            .try_get::<_, Option<f64>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::JSON | Type::JSONB => row
            .try_get::<_, Option<serde_json::Value>>(index)
            .ok()
            .flatten()
            .map(|v| v.to_string()),
        Type::BYTEA => row
            .try_get::<_, Option<Vec<u8>>>(index)
            .ok()
            .flatten()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::BPCHAR_ARRAY | Type::NAME_ARRAY => row
            .try_get::<_, Option<Vec<Option<String>>>>(index)
            .ok()
            .flatten()
            .and_then(|arr| serde_json::to_string(&arr).ok()),
        Type::INT2_ARRAY => row
            .try_get::<_, Option<Vec<Option<i16>>>>(index)
            .ok()
            .flatten()
            .and_then(|arr| serde_json::to_string(&arr).ok()),
        Type::INT4_ARRAY => row
            .try_get::<_, Option<Vec<Option<i32>>>>(index)
            .ok()
            .flatten()
            .and_then(|arr| serde_json::to_string(&arr).ok()),
        Type::INT8_ARRAY => row
            .try_get::<_, Option<Vec<Option<i64>>>>(index)
            .ok()
            .flatten()
            .and_then(|arr| serde_json::to_string(&arr).ok()),
        Type::BOOL_ARRAY => row
            .try_get::<_, Option<Vec<Option<bool>>>>(index)
            .ok()
            .flatten()
            .and_then(|arr| serde_json::to_string(&arr).ok()),
        _ => row.try_get::<_, Option<String>>(index).ok().flatten(),
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
