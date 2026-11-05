use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection};
use crate::domain::audit::AuditLog;
use crate::repository::AuditRepository;
use crate::resource::QueryState;

pub struct SqliteAuditRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteAuditRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self, rusqlite::Error> {
        {
            let db = conn.lock().unwrap();
            db.execute(
                r#"CREATE TABLE IF NOT EXISTS audit_logs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_name TEXT NOT NULL,
                    action TEXT NOT NULL,
                    resource TEXT NOT NULL,
                    record_id TEXT NOT NULL,
                    details TEXT NOT NULL,
                    timestamp TEXT NOT NULL
                );"#,
                [],
            )?;
        }

        Ok(Self { conn })
    }
}

impl AuditRepository for SqliteAuditRepository {
    fn list(&self, query: &QueryState) -> (Vec<AuditLog>, usize) {
        let db = self.conn.lock().unwrap();

        let search_pattern = format!("%{}%", query.search.to_lowercase());
        let count_sql = if query.search.is_empty() {
            "SELECT COUNT(*) FROM audit_logs".to_string()
        } else {
            "SELECT COUNT(*) FROM audit_logs WHERE LOWER(user_name) LIKE ?1 OR LOWER(action) LIKE ?1 OR LOWER(resource) LIKE ?1 OR LOWER(details) LIKE ?1 OR LOWER(record_id) LIKE ?1".to_string()
        };

        let total: usize = if query.search.is_empty() {
            db.query_row(&count_sql, [], |r| r.get(0)).unwrap_or(0)
        } else {
            db.query_row(&count_sql, params![search_pattern], |r| r.get(0)).unwrap_or(0)
        };

        let per_page = if query.per_page == 0 { 8 } else { query.per_page };
        let page = if query.page == 0 { 1 } else { query.page };
        let offset = (page - 1) * per_page;

        let order_by = match query.sort_by.as_deref() {
            Some("timestamp") => "timestamp",
            Some("user_name") => "user_name",
            Some("action") => "action",
            Some("resource") => "resource",
            _ => "id",
        };
        let direction = if query.sort_desc { "ASC" } else { "DESC" }; // default newest first

        let select_sql = if query.search.is_empty() {
            format!("SELECT id, user_name, action, resource, record_id, details, timestamp FROM audit_logs ORDER BY {order_by} {direction} LIMIT ?1 OFFSET ?2")
        } else {
            format!("SELECT id, user_name, action, resource, record_id, details, timestamp FROM audit_logs WHERE LOWER(user_name) LIKE ?1 OR LOWER(action) LIKE ?1 OR LOWER(resource) LIKE ?1 OR LOWER(details) LIKE ?1 OR LOWER(record_id) LIKE ?1 ORDER BY {order_by} {direction} LIMIT ?2 OFFSET ?3")
        };

        let mut stmt = db.prepare(&select_sql).unwrap();

        let log_iter = if query.search.is_empty() {
            stmt.query_map(params![per_page, offset], |row| {
                let id: i64 = row.get(0)?;
                let user_name: String = row.get(1)?;
                let action: String = row.get(2)?;
                let resource: String = row.get(3)?;
                let record_id: String = row.get(4)?;
                let details: String = row.get(5)?;
                let timestamp: String = row.get(6)?;

                Ok(AuditLog {
                    id: id.to_string(),
                    user_name,
                    action,
                    resource,
                    record_id,
                    details,
                    timestamp,
                })
            }).unwrap().collect::<Result<Vec<_>, _>>().unwrap_or_default()
        } else {
            stmt.query_map(params![search_pattern, per_page, offset], |row| {
                let id: i64 = row.get(0)?;
                let user_name: String = row.get(1)?;
                let action: String = row.get(2)?;
                let resource: String = row.get(3)?;
                let record_id: String = row.get(4)?;
                let details: String = row.get(5)?;
                let timestamp: String = row.get(6)?;

                Ok(AuditLog {
                    id: id.to_string(),
                    user_name,
                    action,
                    resource,
                    record_id,
                    details,
                    timestamp,
                })
            }).unwrap().collect::<Result<Vec<_>, _>>().unwrap_or_default()
        };

        (log_iter, total)
    }

    fn save(&self, log: AuditLog) -> Result<String, String> {
        let db = self.conn.lock().unwrap();
        db.execute(
            "INSERT INTO audit_logs (user_name, action, resource, record_id, details, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![log.user_name, log.action, log.resource, log.record_id, log.details, log.timestamp],
        ).map_err(|e| e.to_string())?;

        let last_id = db.last_insert_rowid();
        Ok(last_id.to_string())
    }

    fn find_by_id(&self, id: &str) -> Option<AuditLog> {
        let db = self.conn.lock().unwrap();
        let parsed_id: i64 = id.parse().ok()?;

        db.query_row(
            "SELECT id, user_name, action, resource, record_id, details, timestamp FROM audit_logs WHERE id = ?1",
            params![parsed_id],
            |row| {
                let id: i64 = row.get(0)?;
                let user_name: String = row.get(1)?;
                let action: String = row.get(2)?;
                let resource: String = row.get(3)?;
                let record_id: String = row.get(4)?;
                let details: String = row.get(5)?;
                let timestamp: String = row.get(6)?;

                Ok(AuditLog {
                    id: id.to_string(),
                    user_name,
                    action,
                    resource,
                    record_id,
                    details,
                    timestamp,
                })
            },
        ).ok()
    }
}
