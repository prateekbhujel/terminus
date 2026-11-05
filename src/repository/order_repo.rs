use std::sync::{Arc, RwLock};
use crate::domain::{Order, OrderStatus};
use crate::resource::QueryState;

pub trait OrderRepository: Send + Sync {
    fn list(&self, query: &QueryState) -> (Vec<Order>, usize);
    fn find_by_id(&self, id: &str) -> Option<Order>;
    fn save(&self, order: Order) -> Result<String, String>;
    fn update(&self, id: &str, customer: String, amount: String, status: OrderStatus) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}

#[derive(Clone)]
pub struct InMemoryOrderRepository {
    storage: Arc<RwLock<Vec<Order>>>,
}

impl InMemoryOrderRepository {
    pub fn new() -> Self {
        let seed = vec![
            Order {
                id: "ORD-1001".into(),
                customer: "Hari Bahadur".into(),
                amount: "$1,500.00".into(),
                status: OrderStatus::Paid,
                date: "2026-10-01".into(),
            },
            Order {
                id: "ORD-1002".into(),
                customer: "Dharma Raj Shrestha".into(),
                amount: "$750.00".into(),
                status: OrderStatus::Paid,
                date: "2026-10-02".into(),
            },
            Order {
                id: "ORD-1003".into(),
                customer: "Gokul Subedi".into(),
                amount: "$320.00".into(),
                status: OrderStatus::Paid,
                date: "2026-10-03".into(),
            },
            Order {
                id: "ORD-1004".into(),
                customer: "Ranjan Gumanju".into(),
                amount: "$490.00".into(),
                status: OrderStatus::Pending,
                date: "2026-10-04".into(),
            },
            Order {
                id: "ORD-1005".into(),
                customer: "Sampanna Rimal".into(),
                amount: "$850.00".into(),
                status: OrderStatus::Paid,
                date: "2026-10-05".into(),
            },
            Order {
                id: "ORD-1006".into(),
                customer: "Subash Ranabat".into(),
                amount: "$600.00".into(),
                status: OrderStatus::Paid,
                date: "2026-10-06".into(),
            },
            Order {
                id: "ORD-1007".into(),
                customer: "Elena Rostova".into(),
                amount: "$210.00".into(),
                status: OrderStatus::Refunded,
                date: "2026-10-07".into(),
            },
            Order {
                id: "ORD-1008".into(),
                customer: "Ryan Koirala".into(),
                amount: "$990.00".into(),
                status: OrderStatus::Paid,
                date: "2026-10-07".into(),
            },
        ];

        Self {
            storage: Arc::new(RwLock::new(seed)),
        }
    }
}

impl Default for InMemoryOrderRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderRepository for InMemoryOrderRepository {
    fn list(&self, query: &QueryState) -> (Vec<Order>, usize) {
        let store = self.storage.read().unwrap();
        let mut list: Vec<Order> = store.clone();

        if !query.search.is_empty() {
            let q = query.search.to_lowercase();
            list.retain(|o| {
                o.id.to_lowercase().contains(&q)
                    || o.customer.to_lowercase().contains(&q)
                    || o.status.as_str().to_lowercase().contains(&q)
            });
        }

        let total = list.len();
        let page = if query.page == 0 { 1 } else { query.page };
        let per_page = if query.per_page == 0 { 8 } else { query.per_page };
        let skip = (page - 1) * per_page;

        let paginated = list.into_iter().skip(skip).take(per_page).collect();
        (paginated, total)
    }

    fn find_by_id(&self, id: &str) -> Option<Order> {
        let store = self.storage.read().unwrap();
        store.iter().find(|o| o.id == id).cloned()
    }

    fn save(&self, order: Order) -> Result<String, String> {
        let mut store = self.storage.write().unwrap();
        let id = order.id.clone();
        store.push(order);
        Ok(id)
    }

    fn update(&self, id: &str, customer: String, amount: String, status: OrderStatus) -> Result<(), String> {
        let mut store = self.storage.write().unwrap();
        if let Some(o) = store.iter_mut().find(|o| o.id == id) {
            o.customer = customer;
            o.amount = amount;
            o.status = status;
            Ok(())
        } else {
            Err("Order not found".into())
        }
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let mut store = self.storage.write().unwrap();
        store.retain(|o| o.id != id);
        Ok(())
    }
}
