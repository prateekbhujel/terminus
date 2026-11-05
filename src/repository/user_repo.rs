use std::sync::{Arc, RwLock};
use crate::domain::{Role, User, UserStatus};
use crate::resource::QueryState;

pub trait UserRepository: Send + Sync {
    fn list(&self, query: &QueryState) -> (Vec<User>, usize);
    fn find_by_id(&self, id: &str) -> Option<User>;
    fn find_by_email(&self, email: &str) -> Option<User>;
    fn save(&self, user: User) -> Result<String, String>;
    fn update(&self, id: &str, name: String, email: String, role: Role, status: UserStatus) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}

#[derive(Clone)]
pub struct InMemoryUserRepository {
    storage: Arc<RwLock<Vec<User>>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        let seed = vec![
            User {
                id: "1".into(),
                name: "Hari Bahadur".into(),
                email: "hari@terminus.dev".into(),
                role: Role::Founder,
                status: UserStatus::Active,
                created_at: "2026-09-01".into(),
                ..Default::default()
            },
            User {
                id: "2".into(),
                name: "Pratik".into(),
                email: "pratik@terminus.dev".into(),
                role: Role::Superadmin,
                status: UserStatus::Active,
                created_at: "2026-09-02".into(),
                ..Default::default()
            },
            User {
                id: "3".into(),
                name: "Dharma Raj Shrestha".into(),
                email: "dharma.shrestha@terminus.dev".into(),
                role: Role::Admin,
                status: UserStatus::Active,
                created_at: "2026-09-05".into(),
                ..Default::default()
            },
            User {
                id: "4".into(),
                name: "Gokul Subedi".into(),
                email: "gokul.subedi@terminus.dev".into(),
                role: Role::Admin,
                status: UserStatus::Active,
                created_at: "2026-09-08".into(),
                ..Default::default()
            },
            User {
                id: "5".into(),
                name: "Ranjan Gumanju".into(),
                email: "ranjan.gumanju@terminus.dev".into(),
                role: Role::Member,
                status: UserStatus::Active,
                created_at: "2026-09-12".into(),
                ..Default::default()
            },
            User {
                id: "6".into(),
                name: "Sampanna Rimal".into(),
                email: "sampanna.rimal@terminus.dev".into(),
                role: Role::Member,
                status: UserStatus::Active,
                created_at: "2026-09-15".into(),
                ..Default::default()
            },
            User {
                id: "7".into(),
                name: "Subash Ranabat".into(),
                email: "subash.ranabat@terminus.dev".into(),
                role: Role::Member,
                status: UserStatus::Active,
                created_at: "2026-09-19".into(),
                ..Default::default()
            },
            User {
                id: "8".into(),
                name: "Elena Rostova".into(),
                email: "elena@terminus.dev".into(),
                role: Role::Editor,
                status: UserStatus::Active,
                created_at: "2026-09-22".into(),
                ..Default::default()
            },
            User {
                id: "9".into(),
                name: "Ryan Koirala".into(),
                email: "ryan.koirala@terminus.dev".into(),
                role: Role::Member,
                status: UserStatus::Pending,
                created_at: "2026-10-01".into(),
                ..Default::default()
            },
            User {
                id: "10".into(),
                name: "Alex Vance".into(),
                email: "alex.vance@example.com".into(),
                role: Role::Member,
                status: UserStatus::Suspended,
                created_at: "2026-10-04".into(),
                ..Default::default()
            },
        ];

        Self {
            storage: Arc::new(RwLock::new(seed)),
        }
    }
}

impl Default for InMemoryUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl UserRepository for InMemoryUserRepository {
    fn list(&self, query: &QueryState) -> (Vec<User>, usize) {
        let store = self.storage.read().unwrap();
        let mut list: Vec<User> = store.clone();

        if !query.search.is_empty() {
            let q = query.search.to_lowercase();
            list.retain(|u| {
                u.name.to_lowercase().contains(&q)
                    || u.email.to_lowercase().contains(&q)
                    || u.role.as_str().to_lowercase().contains(&q)
            });
        }

        if let Some(ref sort_col) = query.sort_by {
            list.sort_by(|a, b| {
                let cmp = match sort_col.as_str() {
                    "id" => a.id.parse::<usize>().unwrap_or(0).cmp(&b.id.parse::<usize>().unwrap_or(0)),
                    "name" => a.name.cmp(&b.name),
                    "created_at" => a.created_at.cmp(&b.created_at),
                    _ => std::cmp::Ordering::Equal,
                };
                if query.sort_desc { cmp.reverse() } else { cmp }
            });
        }

        let total = list.len();
        let page = if query.page == 0 { 1 } else { query.page };
        let per_page = if query.per_page == 0 { 8 } else { query.per_page };
        let skip = (page - 1) * per_page;

        let paginated = list.into_iter().skip(skip).take(per_page).collect();
        (paginated, total)
    }

    fn find_by_id(&self, id: &str) -> Option<User> {
        let store = self.storage.read().unwrap();
        store.iter().find(|u| u.id == id).cloned()
    }

    fn find_by_email(&self, email: &str) -> Option<User> {
        let store = self.storage.read().unwrap();
        store.iter().find(|u| u.email.eq_ignore_ascii_case(email)).cloned()
    }

    fn save(&self, user: User) -> Result<String, String> {
        let mut store = self.storage.write().unwrap();
        let id = user.id.clone();
        store.push(user);
        Ok(id)
    }

    fn update(&self, id: &str, name: String, email: String, role: Role, status: UserStatus) -> Result<(), String> {
        let mut store = self.storage.write().unwrap();
        if let Some(u) = store.iter_mut().find(|u| u.id == id) {
            u.name = name;
            u.email = email;
            u.role = role;
            u.status = status;
            Ok(())
        } else {
            Err("User not found".into())
        }
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let mut store = self.storage.write().unwrap();
        store.retain(|u| u.id != id);
        Ok(())
    }
}
