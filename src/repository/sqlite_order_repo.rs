use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection};
use crate::domain::{Order, OrderStatus};
use crate::repository::OrderRepository;
use crate::resource::QueryState;

pub struct SqliteOrderRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteOrderRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self, rusqlite::Error> {
        {
            let db = conn.lock().unwrap();
            db.execute(
                r#"CREATE TABLE IF NOT EXISTS orders (
                    id TEXT PRIMARY KEY,
                    customer TEXT NOT NULL,
                    amount TEXT NOT NULL,
                    status TEXT NOT NULL,
                    date TEXT NOT NULL
                );"#,
                [],
            )?;
        }

        Ok(Self { conn })
    }
}

impl OrderRepository for SqliteOrderRepository {
    fn list(&self, query: &QueryState) -> (Vec<Order>, usize) {
        let db = self.conn.lock().unwrap();

        let search_pattern = format!("%{}%", query.search.to_lowercase());
        let count_sql = if query.search.is_empty() {
            "SELECT COUNT(*) FROM orders".to_string()
        } else {
            "SELECT COUNT(*) FROM orders WHERE LOWER(id) LIKE ?1 OR LOWER(customer) LIKE ?1 OR LOWER(status) LIKE ?1".to_string()
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
            Some("customer") => "customer",
            Some("amount") => "amount",
            Some("date") => "date",
            _ => "id",
        };
        let direction = if query.sort_desc { "DESC" } else { "ASC" };

        let select_sql = if query.search.is_empty() {
            format!("SELECT id, customer, amount, status, date FROM orders ORDER BY {order_by} {direction} LIMIT ?1 OFFSET ?2")
        } else {
            format!("SELECT id, customer, amount, status, date FROM orders WHERE LOWER(id) LIKE ?1 OR LOWER(customer) LIKE ?1 OR LOWER(status) LIKE ?1 ORDER BY {order_by} {direction} LIMIT ?2 OFFSET ?3")
        };

        let mut stmt = db.prepare(&select_sql).unwrap();

        let order_iter = if query.search.is_empty() {
            stmt.query_map(params![per_page, offset], |row| {
                let id: String = row.get(0)?;
                let customer: String = row.get(1)?;
                let amount: String = row.get(2)?;
                let status_str: String = row.get(3)?;
                let date: String = row.get(4)?;

                Ok(Order {
                    id,
                    customer,
                    amount,
                    status: OrderStatus::from_str_loose(&status_str),
                    date,
                })
            }).unwrap().collect::<Result<Vec<_>, _>>().unwrap_or_default()
        } else {
            stmt.query_map(params![search_pattern, per_page, offset], |row| {
                let id: String = row.get(0)?;
                let customer: String = row.get(1)?;
                let amount: String = row.get(2)?;
                let status_str: String = row.get(3)?;
                let date: String = row.get(4)?;

                Ok(Order {
                    id,
                    customer,
                    amount,
                    status: OrderStatus::from_str_loose(&status_str),
                    date,
                })
            }).unwrap().collect::<Result<Vec<_>, _>>().unwrap_or_default()
        };

        (order_iter, total)
    }

    fn find_by_id(&self, id: &str) -> Option<Order> {
        let db = self.conn.lock().unwrap();

        db.query_row(
            "SELECT id, customer, amount, status, date FROM orders WHERE id = ?1",
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let customer: String = row.get(1)?;
                let amount: String = row.get(2)?;
                let status_str: String = row.get(3)?;
                let date: String = row.get(4)?;

                Ok(Order {
                    id,
                    customer,
                    amount,
                    status: OrderStatus::from_str_loose(&status_str),
                    date,
                })
            },
        ).ok()
    }

    fn save(&self, order: Order) -> Result<String, String> {
        let db = self.conn.lock().unwrap();
        db.execute(
            "INSERT INTO orders (id, customer, amount, status, date) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![order.id, order.customer, order.amount, order.status.as_str(), order.date],
        ).map_err(|e| e.to_string())?;

        Ok(order.id)
    }

    fn update(&self, id: &str, customer: String, amount: String, status: OrderStatus) -> Result<(), String> {
        let db = self.conn.lock().unwrap();

        db.execute(
            "UPDATE orders SET customer = ?1, amount = ?2, status = ?3 WHERE id = ?4",
            params![customer, amount, status.as_str(), id],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let db = self.conn.lock().unwrap();

        db.execute("DELETE FROM orders WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
