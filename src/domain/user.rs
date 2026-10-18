use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Matches a permission pattern against a target permission string.
/// Supports exact match, universal wildcard `*`, and segment prefix wildcards (e.g. `orders.*`).
pub fn matches_permission(pattern: &str, target: &str) -> bool {
    if pattern == "*" || pattern == target {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        let prefix_dot = format!("{}.", prefix);
        return target.starts_with(&prefix_dot) || target == prefix;
    }
    false
}

/// In-memory grant cache with configurable TTL and instant invalidation.
/// Avoids repeated permission evaluations across requests.
#[derive(Debug, Clone)]
pub struct PermissionCache {
    ttl: Duration,
    entries: Arc<RwLock<std::collections::HashMap<String, (bool, Instant)>>>,
}

impl Default for PermissionCache {
    fn default() -> Self {
        Self::new(30)
    }
}

impl PermissionCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            ttl: Duration::from_secs(ttl_seconds),
            entries: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn get(&self, user_id: &str, permission: &str) -> Option<bool> {
        let key = format!("{user_id}:{permission}");
        let lock = self.entries.read().ok()?;
        if let Some(&(granted, timestamp)) = lock.get(&key) {
            if timestamp.elapsed() < self.ttl {
                return Some(granted);
            }
        }
        None
    }

    pub fn set(&self, user_id: &str, permission: &str, granted: bool) {
        let key = format!("{user_id}:{permission}");
        if let Ok(mut lock) = self.entries.write() {
            lock.insert(key, (granted, Instant::now()));
        }
    }

    pub fn invalidate_user(&self, user_id: &str) {
        let prefix = format!("{user_id}:");
        if let Ok(mut lock) = self.entries.write() {
            lock.retain(|k, _| !k.starts_with(&prefix));
        }
    }

    pub fn invalidate_all(&self) {
        if let Ok(mut lock) = self.entries.write() {
            lock.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.entries.read().map(|l| l.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Founder,
    Superadmin,
    Admin,
    Editor,
    Member,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Founder => "Founder",
            Self::Superadmin => "Superadmin",
            Self::Admin => "Admin",
            Self::Editor => "Editor",
            Self::Member => "Member",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "founder" => Self::Founder,
            "superadmin" => Self::Superadmin,
            "admin" => Self::Admin,
            "editor" => Self::Editor,
            _ => Self::Member,
        }
    }

    pub fn permissions(&self) -> &'static [&'static str] {
        match self {
            Self::Founder | Self::Superadmin => &[
                "users.view",
                "users.create",
                "users.edit",
                "users.delete",
                "orders.view",
                "orders.create",
                "orders.edit",
                "orders.delete",
                "audit.view",
            ],
            Self::Admin => &[
                "users.view",
                "users.create",
                "users.edit",
                "orders.view",
                "orders.create",
                "orders.edit",
                "orders.delete",
                "audit.view",
            ],
            Self::Editor => &[
                "users.view",
                "orders.view",
                "orders.create",
                "orders.edit",
            ],
            Self::Member => &[
                "users.view",
                "orders.view",
            ],
        }
    }

    pub fn can(&self, permission: &str) -> bool {
        match self {
            Self::Founder | Self::Superadmin => true,
            _ => self.permissions().iter().any(|&p| matches_permission(p, permission)),
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserStatus {
    Active,
    Pending,
    Suspended,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Pending => "Pending",
            Self::Suspended => "Suspended",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => Self::Pending,
            "suspended" => Self::Suspended,
            _ => Self::Active,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: Role,
    pub status: UserStatus,
    #[serde(default)]
    pub direct_permissions: HashSet<String>,
    #[serde(default)]
    pub denied_permissions: HashSet<String>,
    pub created_at: String,
}

impl Default for User {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            email: String::new(),
            role: Role::Member,
            status: UserStatus::Active,
            direct_permissions: HashSet::new(),
            denied_permissions: HashSet::new(),
            created_at: String::new(),
        }
    }
}

impl User {
    pub fn new(id: impl Into<String>, name: impl Into<String>, email: impl Into<String>, role: Role) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            email: email.into(),
            role,
            status: UserStatus::Active,
            direct_permissions: HashSet::new(),
            denied_permissions: HashSet::new(),
            created_at: "2026-09-02".into(),
        }
    }

    pub fn with_status(mut self, status: UserStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = created_at.into();
        self
    }

    pub fn with_permission(mut self, perm: impl Into<String>) -> Self {
        self.give_permission_to(perm);
        self
    }

    pub fn with_deny(mut self, perm: impl Into<String>) -> Self {
        self.deny_permission(perm);
        self
    }

    /// Primary authorization check implementing Spatie/Rustlavel RBAC principles:
    /// 1. Explicit Deny Precedence: direct deny strictly overrides Superadmin and role grants.
    /// 2. Silence is Never Permission: suspended or pending users cannot perform actions.
    /// 3. Direct Grants: explicit grants take precedence over role membership.
    /// 4. Super Role: Superadmin/Founder bypass standard checks unless explicitly denied.
    /// 5. Role Grants: role permissions evaluated with wildcard support.
    /// 6. Silence is Never Permission: default closed (returns false).
    pub fn can(&self, permission: &str) -> bool {
        // 1. Explicit Deny Precedence
        for denied in &self.denied_permissions {
            if matches_permission(denied, permission) {
                return false;
            }
        }

        // 2. Silence is Never Permission: Status check
        if self.status != UserStatus::Active {
            return false;
        }

        // 3. Direct Permission Grants
        for granted in &self.direct_permissions {
            if matches_permission(granted, permission) {
                return true;
            }
        }

        // 4. Super Role Bypass
        if self.is_superadmin() {
            return true;
        }

        // 5. Role Grants
        for role_perm in self.role.permissions() {
            if matches_permission(role_perm, permission) {
                return true;
            }
        }

        // 6. Default Closed
        false
    }

    pub fn can_cached(&self, permission: &str, cache: &PermissionCache) -> bool {
        if let Some(cached) = cache.get(&self.id, permission) {
            return cached;
        }
        let res = self.can(permission);
        cache.set(&self.id, permission, res);
        res
    }

    pub fn give_permission_to(&mut self, perm: impl Into<String>) -> &mut Self {
        let p = perm.into();
        self.denied_permissions.remove(&p);
        self.direct_permissions.insert(p);
        self
    }

    pub fn deny_permission(&mut self, perm: impl Into<String>) -> &mut Self {
        let p = perm.into();
        self.direct_permissions.remove(&p);
        self.denied_permissions.insert(p);
        self
    }

    pub fn revoke_permission_to(&mut self, perm: &str) -> &mut Self {
        self.direct_permissions.remove(perm);
        self
    }

    pub fn remove_deny(&mut self, perm: &str) -> &mut Self {
        self.denied_permissions.remove(perm);
        self
    }

    pub fn has_direct_permission(&self, perm: &str) -> bool {
        self.direct_permissions.iter().any(|p| matches_permission(p, perm))
    }

    pub fn has_explicit_deny(&self, perm: &str) -> bool {
        self.denied_permissions.iter().any(|p| matches_permission(p, perm))
    }

    pub fn has_any_permission(&self, perms: &[&str]) -> bool {
        perms.iter().any(|&p| self.can(p))
    }

    pub fn has_all_permissions(&self, perms: &[&str]) -> bool {
        perms.iter().all(|&p| self.can(p))
    }

    pub fn assign_role(&mut self, role: Role) -> &mut Self {
        self.role = role;
        self
    }

    pub fn has_role(&self, role: &Role) -> bool {
        &self.role == role
    }

    pub fn is_superadmin(&self) -> bool {
        matches!(self.role, Role::Founder | Role::Superadmin)
    }

    pub fn initials(&self) -> String {
        let parts: Vec<&str> = self.name.split_whitespace().collect();
        match parts.len() {
            0 => "U".to_string(),
            1 => parts[0].chars().take(2).collect::<String>().to_uppercase(),
            _ => format!(
                "{}{}",
                parts[0].chars().next().unwrap_or('U'),
                parts.last().and_then(|p| p.chars().next()).unwrap_or(' ')
            ).to_uppercase(),
        }
    }

    pub fn default_admin() -> Self {
        Self {
            id: "2".into(),
            name: "Pratik".into(),
            email: "pratik@terminus.dev".into(),
            role: Role::Superadmin,
            status: UserStatus::Active,
            direct_permissions: HashSet::new(),
            denied_permissions: HashSet::new(),
            created_at: "2026-09-02".into(),
        }
    }
}
