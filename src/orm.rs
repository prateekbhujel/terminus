//! Terminus ORM — Active Record ORM in Rust
//! Models, Active Record pattern, query scopes, relationships (has_many, belongs_to), and schema builders.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Active Record Model trait
pub trait Model: Sized + Clone + Serialize + for<'de> Deserialize<'de> {
    fn table_name() -> &'static str;
    fn primary_key() -> &'static str { "id" }
    fn has_timestamps() -> bool { true }

    fn get_id(&self) -> Option<&str>;
    fn set_id(&mut self, id: String);
    fn to_map(&self) -> HashMap<String, String>;
    fn from_map(map: HashMap<String, String>) -> Self;

    /// Begin a new query builder for this Model
    fn query() -> ModelQuery<Self> {
        ModelQuery::new()
    }

    /// Find a record by its primary key
    fn find(id: &str, conn: &rusqlite::Connection) -> Result<Option<Self>, String> {
        Self::query().find(id, conn)
    }

    /// Find a record by its primary key or return HTTP 404 Not Found error
    fn find_or_fail(id: &str, conn: &rusqlite::Connection) -> Result<Self, crate::helpers::AbortError> {
        match Self::find(id, conn) {
            Ok(Some(model)) => Ok(model),
            Ok(None) => Err(crate::helpers::AbortError::not_found(format!("Record in '{}' with key '{}' not found", Self::table_name(), id))),
            Err(e) => Err(crate::helpers::AbortError::server_error(e)),
        }
    }

    /// Retrieve all records
    fn all(conn: &rusqlite::Connection) -> Result<Vec<Self>, String> {
        Self::query().get(conn)
    }

    /// Create and persist a new model instance
    fn create(attributes: HashMap<&str, &str>, conn: &rusqlite::Connection) -> Result<Self, String> {
        let mut string_map = HashMap::new();
        for (k, v) in attributes {
            string_map.insert(k.to_string(), v.to_string());
        }
        let now = chrono::Utc::now().to_rfc3339();
        if Self::has_timestamps() {
            string_map.entry("created_at".to_string()).or_insert_with(|| now.clone());
            string_map.entry("updated_at".to_string()).or_insert_with(|| now);
        }
        if !string_map.contains_key(Self::primary_key()) {
            let auto_id = format!("{:x}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
            string_map.insert(Self::primary_key().to_string(), auto_id);
        }

        let model = Self::from_map(string_map);
        model.save(conn)?;
        Ok(model)
    }

    /// Save (insert or update) the model instance
    fn save(&self, conn: &rusqlite::Connection) -> Result<(), String> {
        let map = self.to_map();
        let table = Self::table_name();
        let pk = Self::primary_key();
        let pk_val = self.get_id().unwrap_or_default();

        // Check if record exists
        let exists: bool = conn
            .query_row(
                &format!("SELECT COUNT(1) FROM {} WHERE {} = ?1", table, pk),
                rusqlite::params![pk_val],
                |r| r.get::<_, usize>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if exists {
            // UPDATE
            let mut set_clauses = Vec::new();
            let mut values = Vec::new();

            for (k, v) in &map {
                if k != pk {
                    set_clauses.push(format!("{} = ?", k));
                    values.push(v.clone());
                }
            }
            if Self::has_timestamps() && !map.contains_key("updated_at") {
                set_clauses.push("updated_at = ?".to_string());
                values.push(chrono::Utc::now().to_rfc3339());
            }

            let sql = format!("UPDATE {} SET {} WHERE {} = ?", table, set_clauses.join(", "), pk);
            values.push(pk_val.to_string());

            let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
            conn.execute(&sql, params.as_slice()).map_err(|e| e.to_string())?;
        } else {
            // INSERT
            let mut cols = Vec::new();
            let mut placeholders = Vec::new();
            let mut values = Vec::new();

            for (k, v) in &map {
                cols.push(k.clone());
                placeholders.push("?".to_string());
                values.push(v.clone());
            }

            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table,
                cols.join(", "),
                placeholders.join(", ")
            );

            let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
            conn.execute(&sql, params.as_slice()).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Delete the model instance
    fn delete(&self, conn: &rusqlite::Connection) -> Result<(), String> {
        let pk = Self::primary_key();
        let Some(id) = self.get_id() else {
            return Err("Cannot delete model without primary key".into());
        };
        let sql = format!("DELETE FROM {} WHERE {} = ?1", Self::table_name(), pk);
        conn.execute(&sql, rusqlite::params![id]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Dynamic Active Record model with dirty tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicModel {
    pub table: String,
    pub primary_key: String,
    pub attributes: HashMap<String, String>,
    pub original: HashMap<String, String>,
}

impl DynamicModel {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            primary_key: "id".to_string(),
            attributes: HashMap::new(),
            original: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, val: impl Into<String>) -> &mut Self {
        self.attributes.insert(key.into(), val.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|s| s.as_str())
    }

    pub fn is_dirty(&self) -> bool {
        self.attributes != self.original
    }

    pub fn get_dirty(&self) -> HashMap<String, String> {
        let mut dirty = HashMap::new();
        for (k, v) in &self.attributes {
            if self.original.get(k) != Some(v) {
                dirty.insert(k.clone(), v.clone());
            }
        }
        dirty
    }

    pub fn save(&mut self, conn: &rusqlite::Connection) -> Result<(), String> {
        let pk_val = self.get(&self.primary_key).map(String::from);
        let exists = if let Some(ref id) = pk_val {
            conn.query_row(
                &format!("SELECT COUNT(1) FROM {} WHERE {} = ?1", self.table, self.primary_key),
                rusqlite::params![id],
                |r| r.get::<_, usize>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false)
        } else {
            false
        };

        let now = chrono::Utc::now().to_rfc3339();

        if exists {
            let id = pk_val.unwrap();
            let mut set_clauses = Vec::new();
            let mut values = Vec::new();

            for (k, v) in &self.attributes {
                if k != &self.primary_key {
                    set_clauses.push(format!("{} = ?", k));
                    values.push(v.clone());
                }
            }
            set_clauses.push("updated_at = ?".to_string());
            values.push(now.clone());

            let sql = format!("UPDATE {} SET {} WHERE {} = ?", self.table, set_clauses.join(", "), self.primary_key);
            values.push(id);

            let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
            conn.execute(&sql, params.as_slice()).map_err(|e| e.to_string())?;
        } else {
            let auto_id = pk_val.unwrap_or_else(|| format!("{:x}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
            self.attributes.insert(self.primary_key.clone(), auto_id.clone());
            self.attributes.entry("created_at".to_string()).or_insert_with(|| now.clone());
            self.attributes.entry("updated_at".to_string()).or_insert_with(|| now.clone());

            let mut cols = Vec::new();
            let mut placeholders = Vec::new();
            let mut values = Vec::new();

            for (k, v) in &self.attributes {
                cols.push(k.clone());
                placeholders.push("?".to_string());
                values.push(v.clone());
            }

            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                self.table,
                cols.join(", "),
                placeholders.join(", ")
            );

            let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
            conn.execute(&sql, params.as_slice()).map_err(|e| e.to_string())?;
        }

        self.original = self.attributes.clone();
        Ok(())
    }

    pub fn delete(&self, conn: &rusqlite::Connection) -> Result<(), String> {
        let Some(id) = self.get(&self.primary_key) else {
            return Err("Model has no primary key to delete".into());
        };
        let sql = format!("DELETE FROM {} WHERE {} = ?1", self.table, self.primary_key);
        conn.execute(&sql, rusqlite::params![id]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Fluent Model Query Builder
#[derive(Debug, Clone)]
pub struct ModelQuery<M: Model> {
    _marker: std::marker::PhantomData<M>,
    wheres: Vec<(String, String, String)>, // (col, op, val)
    order_by: Option<(String, String)>,
    limit_val: Option<usize>,
    offset_val: Option<usize>,
}

impl<M: Model> ModelQuery<M> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
            wheres: Vec::new(),
            order_by: None,
            limit_val: None,
            offset_val: None,
        }
    }

    pub fn where_eq(mut self, col: impl Into<String>, val: impl Into<String>) -> Self {
        self.wheres.push((col.into(), "=".to_string(), val.into()));
        self
    }

    pub fn where_neq(mut self, col: impl Into<String>, val: impl Into<String>) -> Self {
        self.wheres.push((col.into(), "!=".to_string(), val.into()));
        self
    }

    pub fn where_gt(mut self, col: impl Into<String>, val: impl Into<String>) -> Self {
        self.wheres.push((col.into(), ">".to_string(), val.into()));
        self
    }

    pub fn where_lt(mut self, col: impl Into<String>, val: impl Into<String>) -> Self {
        self.wheres.push((col.into(), "<".to_string(), val.into()));
        self
    }

    pub fn where_like(mut self, col: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.wheres.push((col.into(), "LIKE".to_string(), pattern.into()));
        self
    }

    pub fn order_by(mut self, col: impl Into<String>, dir: &str) -> Self {
        self.order_by = Some((col.into(), dir.to_uppercase()));
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit_val = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset_val = Some(offset);
        self
    }

    fn build_sql(&self) -> (String, Vec<String>) {
        let mut sql = format!("SELECT * FROM {}", M::table_name());
        let mut params = Vec::new();

        if !self.wheres.is_empty() {
            let clauses: Vec<String> = self
                .wheres
                .iter()
                .map(|(c, op, val)| {
                    params.push(val.clone());
                    format!("{} {} ?", c, op)
                })
                .collect();
            sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        }

        if let Some((ref col, ref dir)) = self.order_by {
            sql.push_str(&format!(" ORDER BY {} {}", col, dir));
        }

        if let Some(lim) = self.limit_val {
            sql.push_str(&format!(" LIMIT {}", lim));
        }

        if let Some(off) = self.offset_val {
            sql.push_str(&format!(" OFFSET {}", off));
        }

        (sql, params)
    }

    /// Execute the query and return matching Model instances
    pub fn get(&self, conn: &rusqlite::Connection) -> Result<Vec<M>, String> {
        let (sql, params) = self.build_sql();
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

        let col_names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let mut map = HashMap::new();
            for (i, name) in col_names.iter().enumerate() {
                let val_str: String = row.get(i).unwrap_or_default();
                map.insert(name.clone(), val_str);
            }
            Ok(M::from_map(map))
        }).map_err(|e| e.to_string())?;

        let mut models = Vec::new();
        for r in rows {
            if let Ok(m) = r {
                models.push(m);
            }
        }
        Ok(models)
    }

    /// Execute the query and return the first matching Model instance
    pub fn first(&self, conn: &rusqlite::Connection) -> Result<Option<M>, String> {
        let q = self.clone().limit(1);
        let mut results = q.get(conn)?;
        Ok(results.pop())
    }

    /// Find by primary key
    pub fn find(&self, id: &str, conn: &rusqlite::Connection) -> Result<Option<M>, String> {
        Self::new().where_eq(M::primary_key(), id).first(conn)
    }

    /// Count matching records
    pub fn count(&self, conn: &rusqlite::Connection) -> Result<usize, String> {
        let mut sql = format!("SELECT COUNT(1) FROM {}", M::table_name());
        let mut params = Vec::new();

        if !self.wheres.is_empty() {
            let clauses: Vec<String> = self
                .wheres
                .iter()
                .map(|(c, op, val)| {
                    params.push(val.clone());
                    format!("{} {} ?", c, op)
                })
                .collect();
            sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
        let count: usize = conn
            .query_row(&sql, param_refs.as_slice(), |r| r.get(0))
            .map_err(|e| e.to_string())?;

        Ok(count)
    }

    /// Delete all matching records
    pub fn delete_all(&self, conn: &rusqlite::Connection) -> Result<usize, String> {
        let mut sql = format!("DELETE FROM {}", M::table_name());
        let mut params = Vec::new();

        if !self.wheres.is_empty() {
            let clauses: Vec<String> = self
                .wheres
                .iter()
                .map(|(c, op, val)| {
                    params.push(val.clone());
                    format!("{} {} ?", c, op)
                })
                .collect();
            sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
        let count = conn.execute(&sql, param_refs.as_slice()).map_err(|e| e.to_string())?;
        Ok(count)
    }
}

impl<M: Model> Default for ModelQuery<M> {
    fn default() -> Self {
        Self::new()
    }
}

/// HasMany Relationship
pub struct HasMany<Child: Model> {
    pub parent_id: String,
    pub foreign_key: String,
    _marker: std::marker::PhantomData<Child>,
}

impl<Child: Model> HasMany<Child> {
    pub fn new(parent_id: impl Into<String>, foreign_key: impl Into<String>) -> Self {
        Self {
            parent_id: parent_id.into(),
            foreign_key: foreign_key.into(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Retrieve all related child models
    pub fn get(&self, conn: &rusqlite::Connection) -> Result<Vec<Child>, String> {
        Child::query().where_eq(&self.foreign_key, &self.parent_id).get(conn)
    }

    /// Create and persist a child model with the parent foreign key pre-assigned
    pub fn create(&self, attributes: HashMap<&str, &str>, conn: &rusqlite::Connection) -> Result<Child, String> {
        let mut string_map = HashMap::new();
        for (k, v) in attributes {
            string_map.insert(k.to_string(), v.to_string());
        }
        string_map.insert(self.foreign_key.clone(), self.parent_id.clone());
        let str_ref_map: HashMap<&str, &str> = string_map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        Child::create(str_ref_map, conn)
    }
}

/// BelongsTo Relationship
pub struct BelongsTo<Parent: Model> {
    pub foreign_key_val: String,
    pub owner_key: String,
    _marker: std::marker::PhantomData<Parent>,
}

impl<Parent: Model> BelongsTo<Parent> {
    pub fn new(foreign_key_val: impl Into<String>, owner_key: impl Into<String>) -> Self {
        Self {
            foreign_key_val: foreign_key_val.into(),
            owner_key: owner_key.into(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Retrieve the related parent model
    pub fn get(&self, conn: &rusqlite::Connection) -> Result<Option<Parent>, String> {
        Parent::query().where_eq(&self.owner_key, &self.foreign_key_val).first(conn)
    }
}

/// Blueprint for declarative SQLite schema migrations
pub struct Blueprint {
    pub table_name: String,
    pub columns: Vec<String>,
}

impl Blueprint {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            table_name: name.into(),
            columns: Vec::new(),
        }
    }

    pub fn id(&mut self) -> &mut Self {
        self.columns.push("id TEXT PRIMARY KEY".to_string());
        self
    }

    pub fn string(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} TEXT NOT NULL", col));
        self
    }

    pub fn nullable_string(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} TEXT", col));
        self
    }

    pub fn text(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} TEXT NOT NULL", col));
        self
    }

    pub fn nullable_text(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} TEXT", col));
        self
    }

    pub fn integer(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} INTEGER NOT NULL DEFAULT 0", col));
        self
    }

    pub fn boolean(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} INTEGER NOT NULL DEFAULT 0", col));
        self
    }

    pub fn float(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} REAL NOT NULL DEFAULT 0.0", col));
        self
    }

    pub fn decimal(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} REAL NOT NULL DEFAULT 0.0", col));
        self
    }

    pub fn datetime(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} TEXT NOT NULL", col));
        self
    }

    pub fn json(&mut self, col: &str) -> &mut Self {
        self.columns.push(format!("{} TEXT NOT NULL DEFAULT '{{}}'", col));
        self
    }

    pub fn timestamps(&mut self) -> &mut Self {
        self.columns.push("created_at TEXT NOT NULL".to_string());
        self.columns.push("updated_at TEXT NOT NULL".to_string());
        self
    }

    /// Compile Blueprint schema definition into SQL dialect for SQLite
    pub fn to_sql(&self) -> String {
        format!("CREATE TABLE IF NOT EXISTS {} ({});", self.table_name, self.columns.join(", "))
    }
}

/// Declarative Schema builder
pub struct Schema;

impl Schema {
    pub fn create<F: FnOnce(&mut Blueprint)>(
        conn: &rusqlite::Connection,
        table: &str,
        callback: F,
    ) -> Result<(), rusqlite::Error> {
        let mut bp = Blueprint::new(table);
        callback(&mut bp);

        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} ({});",
            table,
            bp.columns.join(", ")
        );
        conn.execute(&sql, [])?;
        Ok(())
    }

    pub fn compile<F: FnOnce(&mut Blueprint)>(table: &str, callback: F) -> String {
        let mut bp = Blueprint::new(table);
        callback(&mut bp);
        bp.to_sql()
    }

    pub fn drop_if_exists(conn: &rusqlite::Connection, table: &str) -> Result<(), rusqlite::Error> {
        conn.execute(&format!("DROP TABLE IF EXISTS {};", table), [])?;
        Ok(())
    }
}

/// SQLite Database Configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub database: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database: "terminus.db".to_string(),
        }
    }
}

impl DatabaseConfig {
    /// Parse SQLite database connection URL or path (e.g. sqlite://app.db, :memory:, app.db)
    pub fn from_url(url: &str) -> Result<Self, String> {
        let trimmed = url.trim();
        if trimmed == ":memory:" || trimmed.starts_with("sqlite::memory:") {
            return Ok(Self {
                database: ":memory:".to_string(),
            });
        }
        let path = trimmed.strip_prefix("sqlite://").unwrap_or(trimmed);
        Ok(Self {
            database: path.to_string(),
        })
    }

    /// Read connection URL from standard DATABASE_URL environment variable
    pub fn from_env() -> Result<Self, String> {
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "terminus.db".to_string());
        Self::from_url(&url)
    }

    /// Serialize configuration back into a standard connection URL string
    pub fn to_url(&self) -> String {
        if self.database == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite://{}", self.database)
        }
    }
}

/// Ergonomic, parameterized SQL Query object with sync & async execution support for SQLite
#[derive(Debug, Clone, Default)]
pub struct SqlQuery {
    pub raw_sql: String,
    pub bindings: Vec<String>,
}

impl SqlQuery {
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            raw_sql: sql.into(),
            bindings: Vec::new(),
        }
    }

    pub fn bind<T: ToString>(mut self, val: T) -> Self {
        self.bindings.push(val.to_string());
        self
    }

    pub fn sql(&self) -> &str {
        &self.raw_sql
    }

    pub fn compiled_sql(&self) -> &str {
        &self.raw_sql
    }

    pub fn execute_sync(&self, conn: &rusqlite::Connection) -> Result<usize, String> {
        let params: Vec<&dyn rusqlite::ToSql> = self.bindings.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        conn.execute(&self.raw_sql, params.as_slice()).map_err(|e| e.to_string())
    }

    pub fn insert_sync(&self, conn: &rusqlite::Connection) -> Result<i64, String> {
        let params: Vec<&dyn rusqlite::ToSql> = self.bindings.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        conn.execute(&self.raw_sql, params.as_slice()).map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn fetch_all_sync(&self, conn: &rusqlite::Connection) -> Result<Vec<HashMap<String, String>>, String> {
        let mut stmt = conn.prepare(&self.raw_sql).map_err(|e| e.to_string())?;
        let col_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();
        let params: Vec<&dyn rusqlite::ToSql> = self.bindings.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            let mut map = HashMap::new();
            for (idx, name) in col_names.iter().enumerate() {
                let val: Result<String, _> = row.get(idx);
                if let Ok(v) = val {
                    map.insert(name.clone(), v);
                }
            }
            Ok(map)
        }).map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Asynchronously insert records into the default application database
    pub async fn insert(&self) -> Result<i64, String> {
        let db = Database::global();
        db.with_conn(|conn| self.insert_sync(conn))
    }

    /// Asynchronously execute an update/delete SQL command on the default database
    pub async fn execute(&self) -> Result<usize, String> {
        let db = Database::global();
        db.with_conn(|conn| self.execute_sync(conn))
    }

    /// Asynchronously retrieve all records matching the query from the default database
    pub async fn fetch_all(&self) -> Result<Vec<HashMap<String, String>>, String> {
        let db = Database::global();
        db.with_conn(|conn| self.fetch_all_sync(conn))
    }

    /// Asynchronously retrieve the first matching record from the default database
    pub async fn fetch_one(&self) -> Result<Option<HashMap<String, String>>, String> {
        let rows = self.fetch_all().await?;
        Ok(rows.into_iter().next())
    }

    /// Asynchronously retrieve the first matching record or abort with HTTP 404
    pub async fn first_or_fail(&self) -> Result<HashMap<String, String>, crate::helpers::AbortError> {
        let opt = self.fetch_one().await.map_err(|e| crate::helpers::AbortError::server_error(e))?;
        opt.ok_or_else(|| crate::helpers::AbortError::not_found("Requested record does not exist in database"))
    }

    /// Asynchronously paginate query results with total row count and limit/offset bounds
    pub async fn paginate(&self, page: usize, per_page: usize) -> Result<Paginated<HashMap<String, String>>, String> {
        let page = if page == 0 { 1 } else { page };
        let per_page = if per_page == 0 { 15 } else { per_page };
        let offset = (page - 1) * per_page;

        // Count query
        let count_sql = format!("SELECT COUNT(*) as total_count FROM ({}) as subquery", self.raw_sql);
        let mut count_query = SqlQuery::new(count_sql);
        count_query.bindings = self.bindings.clone();

        let count_rows = count_query.fetch_all().await?;
        let total = count_rows.first()
            .and_then(|r| r.get("total_count"))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        // Paged items query
        let paged_sql = format!("{} LIMIT {} OFFSET {}", self.raw_sql, per_page, offset);
        let mut paged_query = SqlQuery::new(paged_sql);
        paged_query.bindings = self.bindings.clone();

        let items = paged_query.fetch_all().await?;
        Ok(Paginated::new(items, total, page, per_page))
    }
}

/// Standard paginated dataset container with metadata and HTML navigation links
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, total: usize, page: usize, per_page: usize) -> Self {
        let per_page = if per_page == 0 { 15 } else { per_page };
        let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };
        let page = if page == 0 { 1 } else { page.min(total_pages) };
        Self {
            items,
            total,
            page,
            per_page,
            total_pages,
        }
    }

    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }

    pub fn has_prev(&self) -> bool {
        self.page > 1
    }

    pub fn next_page(&self) -> Option<usize> {
        if self.has_next() { Some(self.page + 1) } else { None }
    }

    pub fn prev_page(&self) -> Option<usize> {
        if self.has_prev() { Some(self.page - 1) } else { None }
    }

    pub fn from_index(&self) -> usize {
        if self.total == 0 { 0 } else { (self.page - 1) * self.per_page + 1 }
    }

    pub fn to_index(&self) -> usize {
        (self.page * self.per_page).min(self.total)
    }

    /// Renders accessible, styled Tailwind CSS pagination controls with Previous/Next buttons and page numbers
    pub fn links_html(&self, base_url: &str) -> String {
        let separator = if base_url.contains('?') { "&" } else { "?" };
        let from_item = if self.total == 0 { 0 } else { (self.page - 1) * self.per_page + 1 };
        let to_item = (self.page * self.per_page).min(self.total);

        let prev_btn = if self.has_prev() {
            format!(
                r#"<a href="{}{sep}page={}" class="px-3 py-1.5 rounded-lg border border-zinc-700 bg-zinc-850 hover:bg-zinc-800 text-xs text-zinc-300 font-medium transition-colors">Previous</a>"#,
                base_url, self.page - 1, sep = separator
            )
        } else {
            r#"<span class="px-3 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900/50 text-xs text-zinc-600 cursor-not-allowed">Previous</span>"#.to_string()
        };

        let next_btn = if self.has_next() {
            format!(
                r#"<a href="{}{sep}page={}" class="px-3 py-1.5 rounded-lg border border-zinc-700 bg-zinc-850 hover:bg-zinc-800 text-xs text-zinc-300 font-medium transition-colors">Next</a>"#,
                base_url, self.page + 1, sep = separator
            )
        } else {
            r#"<span class="px-3 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900/50 text-xs text-zinc-600 cursor-not-allowed">Next</span>"#.to_string()
        };

        let mut pages_html = String::new();
        for p in 1..=self.total_pages {
            if p == self.page {
                pages_html.push_str(&format!(
                    r#"<span class="w-8 h-8 flex items-center justify-center rounded-lg bg-indigo-600 text-white text-xs font-bold shadow-sm">{}</span>"#,
                    p
                ));
            } else if p <= 3 || p > self.total_pages.saturating_sub(2) || (p as isize - self.page as isize).abs() <= 1 {
                pages_html.push_str(&format!(
                    r#"<a href="{}{sep}page={}" class="w-8 h-8 flex items-center justify-center rounded-lg border border-zinc-800 hover:border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-xs text-zinc-400 hover:text-white transition-colors">{}</a>"#,
                    base_url, p, p, sep = separator
                ));
            } else if !pages_html.ends_with(r#"<span class="px-1 text-zinc-600">...</span>"#) {
                pages_html.push_str(r#"<span class="px-1 text-zinc-600">...</span>"#);
            }
        }

        format!(
            r#"<div class="flex items-center justify-between px-4 py-3 border-t border-zinc-800/80 bg-zinc-900/40 rounded-b-xl">
                <div class="text-xs text-zinc-400">
                    Showing <span class="font-semibold text-zinc-200">{}</span> to <span class="font-semibold text-zinc-200">{}</span> of <span class="font-semibold text-zinc-200">{}</span> results
                </div>
                <div class="flex items-center gap-1.5">
                    {}
                    <div class="flex items-center gap-1 mx-2">{}</div>
                    {}
                </div>
            </div>"#,
            from_item, to_item, self.total, prev_btn, pages_html, next_btn
        )
    }
}

/// Trait for defining database seeders
pub trait Seeder: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, db: &Database) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Registry of database seeders that can be run in batch or individually
#[derive(Default)]
pub struct SeederRegistry {
    seeders: Vec<Box<dyn Seeder>>,
}

impl SeederRegistry {
    pub fn new() -> Self {
        Self { seeders: Vec::new() }
    }

    pub fn register<S: Seeder + 'static>(&mut self, seeder: S) -> &mut Self {
        self.seeders.push(Box::new(seeder));
        self
    }

    pub fn len(&self) -> usize {
        self.seeders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seeders.is_empty()
    }

    pub fn run_all(&self, db: &Database) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut ran = 0;
        for s in &self.seeders {
            s.run(db)?;
            ran += 1;
        }
        Ok(ran)
    }

    pub fn run_class(&self, class_name: &str, db: &Database) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        for s in &self.seeders {
            if s.name().eq_ignore_ascii_case(class_name) {
                s.run(db)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Global Application Database Singleton
static GLOBAL_DB: OnceLock<RwLock<Option<Database>>> = OnceLock::new();
fn global_db_lock() -> &'static RwLock<Option<Database>> {
    GLOBAL_DB.get_or_init(|| RwLock::new(None))
}

/// SQLite Database Manager for Terminus applications
#[derive(Clone)]
pub struct Database {
    pub config: DatabaseConfig,
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl Database {
    /// Create a fast in-memory SQLite database
    pub fn memory() -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open_in_memory()?;
        let config = DatabaseConfig {
            database: ":memory:".to_string(),
        };
        Ok(Self {
            config,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open or create a file-based SQLite database
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, rusqlite::Error> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let conn = rusqlite::Connection::open(path)?;
        let config = DatabaseConfig {
            database: path_str,
        };
        Ok(Self {
            config,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Connect to SQLite database via connection URL (sqlite://path/to.db, :memory:, or file path)
    pub fn connect(url: &str) -> Result<Self, String> {
        let config = DatabaseConfig::from_url(url)?;
        let sqlite_conn = if config.database == ":memory:" {
            rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?
        } else {
            rusqlite::Connection::open(&config.database).map_err(|e| e.to_string())?
        };

        Ok(Self {
            config,
            conn: Arc::new(Mutex::new(sqlite_conn)),
        })
    }

    /// Set the active application database globally
    pub fn set_global(db: Database) {
        let mut guard = global_db_lock().write().unwrap();
        *guard = Some(db);
    }

    /// Retrieve the global application database (defaults to in-memory if uninitialized)
    pub fn global() -> Database {
        let read_guard = global_db_lock().read().unwrap();
        if let Some(db) = read_guard.as_ref() {
            return db.clone();
        }
        drop(read_guard);
        let mut write_guard = global_db_lock().write().unwrap();
        if let Some(db) = write_guard.as_ref() {
            return db.clone();
        }
        let db = Database::memory().unwrap_or_else(|_| Database::open("terminus.db").unwrap());
        *write_guard = Some(db.clone());
        db
    }

    /// Access the underlying connection in a closure
    pub fn with_conn<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&rusqlite::Connection) -> R,
    {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    /// Access the underlying connection mutably in a closure
    pub fn with_conn_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut rusqlite::Connection) -> R,
    {
        let mut conn = self.conn.lock().unwrap();
        f(&mut conn)
    }

    /// Execute a closure inside a SQLite transaction with automatic rollback on error
    pub fn transaction<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<R, String>,
    {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let res = f(&tx)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(res)
    }

    /// Ensure the internal migration tracking table exists
    pub fn ensure_migrations_table(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _terminus_migrations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                batch INTEGER NOT NULL,
                applied_at TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    /// Retrieve list of all applied migrations (name, batch, applied_at)
    pub fn applied_migrations(&self) -> Result<Vec<(String, i64, String)>, rusqlite::Error> {
        self.ensure_migrations_table()?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name, batch, applied_at FROM _terminus_migrations ORDER BY id ASC")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    /// Get the highest migration batch number
    pub fn latest_batch(&self) -> Result<i64, rusqlite::Error> {
        self.ensure_migrations_table()?;
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(MAX(batch), 0) FROM _terminus_migrations",
            [],
            |r| r.get(0),
        )
    }

    /// Record a newly executed migration into the tracking table
    pub fn record_migration(&self, name: &str, batch: i64) -> Result<(), rusqlite::Error> {
        self.ensure_migrations_table()?;
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO _terminus_migrations (name, batch, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, batch, now],
        )?;
        Ok(())
    }

    /// Roll back all migrations in the latest batch and return their names
    pub fn rollback_latest_batch(&self) -> Result<Vec<String>, rusqlite::Error> {
        self.ensure_migrations_table()?;
        let batch = self.latest_batch()?;
        if batch == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name FROM _terminus_migrations WHERE batch = ?1")?;
        let names: Vec<String> = stmt.query_map(rusqlite::params![batch], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        conn.execute("DELETE FROM _terminus_migrations WHERE batch = ?1", rusqlite::params![batch])?;
        Ok(names)
    }

    /// Execute a raw SQL statement
    pub fn execute(&self, sql: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(sql, [])
    }

    /// Get raw connection reference Arc
    pub fn connection_arc(&self) -> Arc<Mutex<rusqlite::Connection>> {
        self.conn.clone()
    }
}

/// Macro for ergonomic, parameterized SQL query construction
#[macro_export]
macro_rules! query {
    ($sql:literal) => {
        $crate::orm::SqlQuery::new($sql)
    };
    ($sql:expr, $($val:expr),* $(,)?) => {
        $crate::orm::SqlQuery::new($sql)$(.bind($val))*
    };
}

