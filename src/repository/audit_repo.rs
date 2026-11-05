use std::sync::{Arc, RwLock};
use crate::domain::audit::AuditLog;
use crate::resource::QueryState;

pub trait AuditRepository: Send + Sync {
    fn list(&self, query: &QueryState) -> (Vec<AuditLog>, usize);
    fn save(&self, log: AuditLog) -> Result<String, String>;
    fn find_by_id(&self, id: &str) -> Option<AuditLog>;
}

#[derive(Clone)]
pub struct InMemoryAuditRepository {
    storage: Arc<RwLock<Vec<AuditLog>>>,
}

impl InMemoryAuditRepository {
    pub fn new() -> Self {
        let seed = vec![
            AuditLog {
                id: "1".into(),
                user_name: "Hari Bahadur".into(),
                action: "CREATE".into(),
                resource: "users".into(),
                record_id: "2".into(),
                details: "Created account for Pratik (Superadmin)".into(),
                timestamp: "2026-10-07 14:20:00".into(),
            },
            AuditLog {
                id: "2".into(),
                user_name: "Pratik".into(),
                action: "UPDATE".into(),
                resource: "users".into(),
                record_id: "8".into(),
                details: "Updated role to Editor for Elena Rostova".into(),
                timestamp: "2026-10-07 15:45:12".into(),
            },
            AuditLog {
                id: "3".into(),
                user_name: "Dharma Raj Shrestha".into(),
                action: "CREATE".into(),
                resource: "orders".into(),
                record_id: "ORD-1001".into(),
                details: "Created order for Acme Corp ($250.00)".into(),
                timestamp: "2026-10-07 18:02:40".into(),
            },
            AuditLog {
                id: "4".into(),
                user_name: "Gokul Subedi".into(),
                action: "UPDATE".into(),
                resource: "orders".into(),
                record_id: "ORD-1002".into(),
                details: "Marked order ORD-1002 as Paid".into(),
                timestamp: "2026-10-07 20:10:05".into(),
            },
        ];

        Self {
            storage: Arc::new(RwLock::new(seed)),
        }
    }
}

impl Default for InMemoryAuditRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditRepository for InMemoryAuditRepository {
    fn list(&self, query: &QueryState) -> (Vec<AuditLog>, usize) {
        let store = self.storage.read().unwrap();
        let mut list: Vec<AuditLog> = store.clone();

        if !query.search.is_empty() {
            let q = query.search.to_lowercase();
            list.retain(|a| {
                a.user_name.to_lowercase().contains(&q)
                    || a.action.to_lowercase().contains(&q)
                    || a.resource.to_lowercase().contains(&q)
                    || a.details.to_lowercase().contains(&q)
                    || a.record_id.to_lowercase().contains(&q)
            });
        }

        if let Some(ref sort_col) = query.sort_by {
            list.sort_by(|a, b| {
                let cmp = match sort_col.as_str() {
                    "id" => a.id.parse::<usize>().unwrap_or(0).cmp(&b.id.parse::<usize>().unwrap_or(0)),
                    "timestamp" => a.timestamp.cmp(&b.timestamp),
                    "user_name" => a.user_name.cmp(&b.user_name),
                    "action" => a.action.cmp(&b.action),
                    "resource" => a.resource.cmp(&b.resource),
                    _ => std::cmp::Ordering::Equal,
                };
                if query.sort_desc { cmp.reverse() } else { cmp }
            });
        } else {
            // Default: newest logs first
            list.reverse();
        }

        let total = list.len();
        let page = if query.page == 0 { 1 } else { query.page };
        let per_page = if query.per_page == 0 { 8 } else { query.per_page };
        let skip = (page - 1) * per_page;

        let paginated = list.into_iter().skip(skip).take(per_page).collect();
        (paginated, total)
    }

    fn save(&self, log: AuditLog) -> Result<String, String> {
        let mut store = self.storage.write().unwrap();
        let id = log.id.clone();
        store.push(log);
        Ok(id)
    }

    fn find_by_id(&self, id: &str) -> Option<AuditLog> {
        let store = self.storage.read().unwrap();
        store.iter().find(|a| a.id == id).cloned()
    }
}
