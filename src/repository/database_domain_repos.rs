use crate::backend::DatabaseKind;
use crate::domain::audit::AuditLog;
use crate::domain::order::{Order, OrderStatus};
use crate::orm::Database;
use crate::repository::{AuditRepository, OrderRepository};
use crate::resource::QueryState;
use std::collections::HashMap;

fn id_column(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Postgres => "BIGSERIAL PRIMARY KEY",
        DatabaseKind::Mysql => "BIGINT PRIMARY KEY AUTO_INCREMENT",
        DatabaseKind::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
    }
}

fn text_column(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Mysql => "VARCHAR(255)",
        _ => "TEXT",
    }
}

fn insert_id(db: &Database, sql: &str, params: &[String]) -> Result<String, String> {
    match db.kind() {
        DatabaseKind::Postgres => {
            let rows = db.query_params(&format!("{sql} RETURNING id"), params)?;
            rows.into_iter()
                .next()
                .and_then(|row| row.get("id").cloned())
                .ok_or_else(|| "Insert returned no id".to_string())
        }
        kind => {
            db.execute_params(sql, params)?;
            let last_id_sql = match kind {
                DatabaseKind::Mysql => "SELECT LAST_INSERT_ID() AS id",
                _ => "SELECT last_insert_rowid() AS id",
            };
            let rows = db.query_params(last_id_sql, &[])?;
            rows.into_iter()
                .next()
                .and_then(|row| row.get("id").cloned())
                .ok_or_else(|| "Insert returned no id".to_string())
        }
    }
}

fn paginate(query: &QueryState) -> (usize, usize) {
    let per_page = if query.per_page == 0 { 8 } else { query.per_page };
    let page = if query.page == 0 { 1 } else { query.page };
    (per_page, (page - 1) * per_page)
}

/// Audit log repository over any database backend. Never seeds entries.
#[derive(Clone)]
pub struct DatabaseAuditRepository {
    db: Database,
}

impl DatabaseAuditRepository {
    pub fn new(db: Database) -> Result<Self, String> {
        let kind = db.kind();
        let ddl = format!(
            "CREATE TABLE IF NOT EXISTS audit_logs (
                id {},
                user_name {} NOT NULL,
                action {} NOT NULL,
                resource {} NOT NULL,
                record_id {} NOT NULL,
                details TEXT NOT NULL,
                timestamp {} NOT NULL
            )",
            id_column(kind),
            text_column(kind),
            text_column(kind),
            text_column(kind),
            text_column(kind),
            text_column(kind)
        );
        db.execute(&ddl).map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    fn row_to_log(row: &HashMap<String, String>) -> Option<AuditLog> {
        Some(AuditLog {
            id: row.get("id")?.clone(),
            user_name: row.get("user_name")?.clone(),
            action: row.get("action")?.clone(),
            resource: row.get("resource")?.clone(),
            record_id: row.get("record_id")?.clone(),
            details: row.get("details")?.clone(),
            timestamp: row.get("timestamp")?.clone(),
        })
    }
}

impl AuditRepository for DatabaseAuditRepository {
    fn list(&self, query: &QueryState) -> (Vec<AuditLog>, usize) {
        let (per_page, offset) = paginate(query);
        let kind = self.db.kind();
        let (where_clause, params) = if query.search.is_empty() {
            (String::new(), Vec::new())
        } else {
            let ph = kind.placeholder(1);
            (
                format!(
                    " WHERE LOWER(user_name) LIKE {ph} OR LOWER(action) LIKE {ph} OR LOWER(resource) LIKE {ph} OR LOWER(details) LIKE {ph}"
                ),
                vec![format!("%{}%", query.search.to_lowercase())],
            )
        };
        let total = self
            .db
            .query_params(
                &format!("SELECT COUNT(*) AS total FROM audit_logs{where_clause}"),
                &params,
            )
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .and_then(|row| row.get("total").and_then(|v| v.parse::<usize>().ok()))
            .unwrap_or(0);
        let logs = self
            .db
            .query_params(
                &format!(
                    "SELECT id, user_name, action, resource, record_id, details, timestamp
                     FROM audit_logs{where_clause} ORDER BY id DESC LIMIT {per_page} OFFSET {offset}"
                ),
                &params,
            )
            .unwrap_or_default()
            .iter()
            .filter_map(Self::row_to_log)
            .collect();
        (logs, total)
    }

    fn save(&self, log: AuditLog) -> Result<String, String> {
        let ph = |i: usize| self.db.kind().placeholder(i);
        let sql = format!(
            "INSERT INTO audit_logs (user_name, action, resource, record_id, details, timestamp)
             VALUES ({}, {}, {}, {}, {}, {})",
            ph(1),
            ph(2),
            ph(3),
            ph(4),
            ph(5),
            ph(6)
        );
        insert_id(
            &self.db,
            &sql,
            &[
                log.user_name.clone(),
                log.action.clone(),
                log.resource.clone(),
                log.record_id.clone(),
                log.details.clone(),
                log.timestamp.clone(),
            ],
        )
    }

    fn find_by_id(&self, id: &str) -> Option<AuditLog> {
        let sql = format!(
            "SELECT id, user_name, action, resource, record_id, details, timestamp FROM audit_logs WHERE id = {}",
            self.db.kind().placeholder(1)
        );
        self.db
            .query_params(&sql, &[id.to_string()])
            .ok()?
            .into_iter()
            .next()
            .and_then(|row| Self::row_to_log(&row))
    }
}

/// Order repository over any database backend. Never seeds entries.
#[derive(Clone)]
pub struct DatabaseOrderRepository {
    db: Database,
}

impl DatabaseOrderRepository {
    pub fn new(db: Database) -> Result<Self, String> {
        let kind = db.kind();
        let ddl = format!(
            "CREATE TABLE IF NOT EXISTS orders (
                id {},
                customer {} NOT NULL,
                amount {} NOT NULL,
                status {} NOT NULL,
                date {} NOT NULL
            )",
            id_column(kind),
            text_column(kind),
            text_column(kind),
            text_column(kind),
            text_column(kind)
        );
        db.execute(&ddl).map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    fn row_to_order(row: &HashMap<String, String>) -> Option<Order> {
        Some(Order {
            id: row.get("id")?.clone(),
            customer: row.get("customer")?.clone(),
            amount: row.get("amount")?.clone(),
            status: OrderStatus::from_str_loose(row.get("status")?),
            date: row.get("date")?.clone(),
        })
    }
}

impl OrderRepository for DatabaseOrderRepository {
    fn list(&self, query: &QueryState) -> (Vec<Order>, usize) {
        let (per_page, offset) = paginate(query);
        let kind = self.db.kind();
        let (where_clause, params) = if query.search.is_empty() {
            (String::new(), Vec::new())
        } else {
            let ph = kind.placeholder(1);
            (
                format!(
                    " WHERE LOWER(customer) LIKE {ph} OR LOWER(status) LIKE {ph}"
                ),
                vec![format!("%{}%", query.search.to_lowercase())],
            )
        };
        let total = self
            .db
            .query_params(
                &format!("SELECT COUNT(*) AS total FROM orders{where_clause}"),
                &params,
            )
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .and_then(|row| row.get("total").and_then(|v| v.parse::<usize>().ok()))
            .unwrap_or(0);
        let orders = self
            .db
            .query_params(
                &format!(
                    "SELECT id, customer, amount, status, date FROM orders{where_clause}
                     ORDER BY id DESC LIMIT {per_page} OFFSET {offset}"
                ),
                &params,
            )
            .unwrap_or_default()
            .iter()
            .filter_map(Self::row_to_order)
            .collect();
        (orders, total)
    }

    fn find_by_id(&self, id: &str) -> Option<Order> {
        let sql = format!(
            "SELECT id, customer, amount, status, date FROM orders WHERE id = {}",
            self.db.kind().placeholder(1)
        );
        self.db
            .query_params(&sql, &[id.to_string()])
            .ok()?
            .into_iter()
            .next()
            .and_then(|row| Self::row_to_order(&row))
    }

    fn save(&self, order: Order) -> Result<String, String> {
        let ph = |i: usize| self.db.kind().placeholder(i);
        let sql = format!(
            "INSERT INTO orders (customer, amount, status, date) VALUES ({}, {}, {}, {})",
            ph(1),
            ph(2),
            ph(3),
            ph(4)
        );
        insert_id(
            &self.db,
            &sql,
            &[
                order.customer.clone(),
                order.amount.clone(),
                order.status.as_str().to_string(),
                order.date.clone(),
            ],
        )
    }

    fn update(
        &self,
        id: &str,
        customer: String,
        amount: String,
        status: OrderStatus,
    ) -> Result<(), String> {
        let ph = |i: usize| self.db.kind().placeholder(i);
        let sql = format!(
            "UPDATE orders SET customer = {}, amount = {}, status = {} WHERE id = {}",
            ph(1),
            ph(2),
            ph(3),
            ph(4)
        );
        let changed = self.db.execute_params(
            &sql,
            &[
                customer,
                amount,
                status.as_str().to_string(),
                id.to_string(),
            ],
        )?;
        if changed == 0 {
            return Err("Order not found".into());
        }
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let sql = format!(
            "DELETE FROM orders WHERE id = {}",
            self.db.kind().placeholder(1)
        );
        self.db.execute_params(&sql, &[id.to_string()])?;
        Ok(())
    }
}
