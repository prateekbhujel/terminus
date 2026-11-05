use crate::backend::DatabaseKind;
use crate::domain::{Role, User, UserStatus};
use crate::orm::Database;
use crate::repository::UserRepository;
use crate::resource::QueryState;

/// User repository over any Terminus database backend (SQLite, PostgreSQL,
/// or MySQL/MariaDB), selected by the [`Database`] connection URL. Unlike
/// the SQLite repository it never seeds accounts.
#[derive(Clone)]
pub struct DatabaseUserRepository {
    db: Database,
}

impl DatabaseUserRepository {
    pub fn new(db: Database) -> Result<Self, String> {
        let ddl = match db.kind() {
            DatabaseKind::Postgres => {
                "CREATE TABLE IF NOT EXISTS users (
                    id BIGSERIAL PRIMARY KEY,
                    name TEXT NOT NULL,
                    email TEXT NOT NULL,
                    password_hash TEXT NOT NULL DEFAULT '',
                    role TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )"
            }
            DatabaseKind::Mysql => {
                "CREATE TABLE IF NOT EXISTS users (
                    id BIGINT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(255) NOT NULL,
                    email VARCHAR(255) NOT NULL,
                    password_hash TEXT NOT NULL,
                    role VARCHAR(64) NOT NULL,
                    status VARCHAR(64) NOT NULL,
                    created_at VARCHAR(64) NOT NULL
                )"
            }
            DatabaseKind::Sqlite => {
                "CREATE TABLE IF NOT EXISTS users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    email TEXT NOT NULL,
                    password_hash TEXT NOT NULL DEFAULT '',
                    role TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )"
            }
        };
        db.execute(ddl).map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    fn placeholder(&self, index: usize) -> String {
        self.db.kind().placeholder(index)
    }

    fn row_to_user(row: &std::collections::HashMap<String, String>) -> Option<User> {
        Some(User {
            id: row.get("id")?.clone(),
            name: row.get("name")?.clone(),
            email: row.get("email")?.clone(),
            role: Role::from_str_loose(row.get("role")?),
            status: UserStatus::from_str_loose(row.get("status")?),
            created_at: row.get("created_at")?.clone(),
            ..Default::default()
        })
    }

    /// Verify an email/password pair against the stored Argon2id (or legacy
    /// PBKDF2) hash. Accounts without a password hash can never verify.
    pub fn verify_credentials(&self, email: &str, password: &str) -> Option<User> {
        let sql = format!(
            "SELECT id, name, email, role, status, created_at, password_hash
             FROM users WHERE LOWER(email) = LOWER({})",
            self.placeholder(1)
        );
        let rows = self.db.query_params(&sql, &[email.to_string()]).ok()?;
        let row = rows.into_iter().next()?;
        let password_hash = row.get("password_hash")?.clone();
        if password_hash.is_empty() || !crate::auth::Hash::check(password, &password_hash) {
            return None;
        }
        Self::row_to_user(&row)
    }
}

impl UserRepository for DatabaseUserRepository {
    fn list(&self, query: &QueryState) -> (Vec<User>, usize) {
        let per_page = if query.per_page == 0 { 8 } else { query.per_page };
        let page = if query.page == 0 { 1 } else { query.page };
        let offset = (page - 1) * per_page;
        let order_by = match query.sort_by.as_deref() {
            Some("name") => "name",
            Some("created_at") => "created_at",
            _ => "id",
        };
        let direction = if query.sort_desc { "DESC" } else { "ASC" };

        let (where_clause, search_params) = if query.search.is_empty() {
            (String::new(), Vec::new())
        } else {
            let ph = self.placeholder(1);
            (
                format!(
                    " WHERE LOWER(name) LIKE {ph} OR LOWER(email) LIKE {ph} OR LOWER(role) LIKE {ph}"
                ),
                vec![format!("%{}%", query.search.to_lowercase())],
            )
        };

        let total = self
            .db
            .query_params(
                &format!("SELECT COUNT(*) AS total FROM users{where_clause}"),
                &search_params,
            )
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .and_then(|row| row.get("total").and_then(|v| v.parse::<usize>().ok()))
            .unwrap_or(0);

        let sql = format!(
            "SELECT id, name, email, role, status, created_at FROM users{where_clause}
             ORDER BY {order_by} {direction} LIMIT {per_page} OFFSET {offset}"
        );
        let users = self
            .db
            .query_params(&sql, &search_params)
            .unwrap_or_default()
            .iter()
            .filter_map(Self::row_to_user)
            .collect();
        (users, total)
    }

    fn find_by_id(&self, id: &str) -> Option<User> {
        let sql = format!(
            "SELECT id, name, email, role, status, created_at FROM users WHERE id = {}",
            self.placeholder(1)
        );
        let rows = self.db.query_params(&sql, &[id.to_string()]).ok()?;
        rows.into_iter().next().and_then(|row| Self::row_to_user(&row))
    }

    fn find_by_email(&self, email: &str) -> Option<User> {
        let sql = format!(
            "SELECT id, name, email, role, status, created_at FROM users WHERE LOWER(email) = LOWER({})",
            self.placeholder(1)
        );
        let rows = self.db.query_params(&sql, &[email.to_string()]).ok()?;
        rows.into_iter().next().and_then(|row| Self::row_to_user(&row))
    }

    fn save(&self, user: User) -> Result<String, String> {
        let params = vec![
            user.name.clone(),
            user.email.clone(),
            user.role.as_str().to_string(),
            user.status.as_str().to_string(),
            user.created_at.clone(),
            String::new(),
        ];
        match self.db.kind() {
            DatabaseKind::Postgres => {
                let rows = self.db.query_params(
                    "INSERT INTO users (name, email, role, status, created_at, password_hash)
                     VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
                    &params,
                )?;
                rows.into_iter()
                    .next()
                    .and_then(|row| row.get("id").cloned())
                    .ok_or_else(|| "User insert returned no id".to_string())
            }
            kind => {
                let ph = |i: usize| kind.placeholder(i);
                let sql = format!(
                    "INSERT INTO users (name, email, role, status, created_at, password_hash) VALUES ({}, {}, {}, {}, {}, {})",
                    ph(1),
                    ph(2),
                    ph(3),
                    ph(4),
                    ph(5),
                    ph(6)
                );
                self.db.execute_params(&sql, &params)?;
                let last_id_sql = match kind {
                    DatabaseKind::Mysql => "SELECT LAST_INSERT_ID() AS id",
                    _ => "SELECT last_insert_rowid() AS id",
                };
                let rows = self.db.query_params(last_id_sql, &[])?;
                rows.into_iter()
                    .next()
                    .and_then(|row| row.get("id").cloned())
                    .ok_or_else(|| "User insert returned no id".to_string())
            }
        }
    }

    fn update(
        &self,
        id: &str,
        name: String,
        email: String,
        role: Role,
        status: UserStatus,
    ) -> Result<(), String> {
        let ph = |i: usize| self.placeholder(i);
        let sql = format!(
            "UPDATE users SET name = {}, email = {}, role = {}, status = {} WHERE id = {}",
            ph(1),
            ph(2),
            ph(3),
            ph(4),
            ph(5)
        );
        let changed = self.db.execute_params(
            &sql,
            &[
                name,
                email,
                role.as_str().to_string(),
                status.as_str().to_string(),
                id.to_string(),
            ],
        )?;
        if changed == 0 {
            return Err("User not found".into());
        }
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let sql = format!("DELETE FROM users WHERE id = {}", self.placeholder(1));
        self.db.execute_params(&sql, &[id.to_string()])?;
        Ok(())
    }
}
