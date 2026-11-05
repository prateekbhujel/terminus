use terminus::prelude::*;

#[test]
fn test_role_hierarchy_and_permissions() {
    let superadmin_role = Role::Superadmin;
    assert!(superadmin_role.can("users.create"));
    assert!(superadmin_role.can("users.delete"));
    assert!(superadmin_role.can("audit.view"));
    assert!(superadmin_role.can("orders.delete"));

    let admin_role = Role::Admin;
    assert!(admin_role.can("users.create"));
    assert!(admin_role.can("users.edit"));
    assert!(!admin_role.can("users.delete"), "Admins cannot delete users");
    assert!(admin_role.can("orders.delete"));
    assert!(admin_role.can("audit.view"));

    let editor_role = Role::Editor;
    assert!(editor_role.can("users.view"));
    assert!(!editor_role.can("users.create"), "Editors cannot create users");
    assert!(!editor_role.can("users.edit"), "Editors cannot edit users");
    assert!(editor_role.can("orders.create"));
    assert!(editor_role.can("orders.edit"));
    assert!(!editor_role.can("orders.delete"), "Editors cannot delete orders");
    assert!(!editor_role.can("audit.view"), "Editors cannot view audit logs");

    let member_role = Role::Member;
    assert!(member_role.can("users.view"));
    assert!(member_role.can("orders.view"));
    assert!(!member_role.can("users.create"));
    assert!(!member_role.can("orders.create"));
    assert!(!member_role.can("orders.edit"));
    assert!(!member_role.can("orders.delete"));
    assert!(!member_role.can("audit.view"));
}

#[test]
fn test_user_status_guards() {
    let active_admin = User {
        id: "1".into(),
        name: "Active Admin".into(),
        email: "admin@example.com".into(),
        role: Role::Admin,
        status: UserStatus::Active,
        created_at: "2026-10-01".into(),
        ..Default::default()
    };
    assert!(active_admin.can("users.create"));

    let suspended_admin = User {
        id: "2".into(),
        name: "Suspended Admin".into(),
        email: "suspended@example.com".into(),
        role: Role::Admin,
        status: UserStatus::Suspended,
        created_at: "2026-10-01".into(),
        ..Default::default()
    };
    assert!(!suspended_admin.can("users.create"), "Suspended users cannot perform actions");

    let pending_admin = User {
        id: "3".into(),
        name: "Pending Admin".into(),
        email: "pending@example.com".into(),
        role: Role::Admin,
        status: UserStatus::Pending,
        created_at: "2026-10-01".into(),
        ..Default::default()
    };
    assert!(!pending_admin.can("users.create"), "Pending users cannot perform actions");
}

#[test]
fn test_user_initials_and_helpers() {
    let u1 = User {
        id: "1".into(),
        name: "Hari Bahadur".into(),
        email: "hari@example.com".into(),
        role: Role::Founder,
        status: UserStatus::Active,
        created_at: "2026-10-01".into(),
        ..Default::default()
    };
    assert_eq!(u1.initials(), "HB");
    assert!(u1.is_superadmin());

    let u2 = User {
        id: "2".into(),
        name: "Pratik".into(),
        email: "pratik@example.com".into(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        created_at: "2026-10-01".into(),
        ..Default::default()
    };
    assert_eq!(u2.initials(), "PR");
    assert!(u2.is_superadmin());

    let u3 = User {
        id: "3".into(),
        name: "Elena Rostova".into(),
        email: "elena@example.com".into(),
        role: Role::Editor,
        status: UserStatus::Active,
        created_at: "2026-10-01".into(),
        ..Default::default()
    };
    assert_eq!(u3.initials(), "ER");
    assert!(!u3.is_superadmin());
}

#[test]
fn test_dynamic_permission_grants_and_revocations() {
    let mut member = User::new("m_1", "Member User", "member@example.com", Role::Member);

    // Member initially cannot create users
    assert!(!member.can("users.create"));
    assert!(!member.has_direct_permission("users.create"));

    // Grant direct permission
    member.give_permission_to("users.create");
    assert!(member.can("users.create"));
    // Multiple permission checks
    assert!(member.has_any_permission(&["users.create", "orders.delete"]));
    assert!(!member.has_all_permissions(&["users.create", "orders.delete"]));

    // Revoke direct permission
    member.revoke_permission_to("users.create");
    assert!(!member.can("users.create"));
    assert!(!member.has_direct_permission("users.create"));

    // Grant and revoke another permission
    member.give_permission_to("users.export");
    assert!(member.can("users.export"));
    member.revoke_permission_to("users.export");
    assert!(!member.can("users.export"));
}

#[test]
fn test_explicit_deny_strictly_overrides_superadmin() {
    let mut superadmin = User::new("sa_1", "Super Admin", "super@example.com", Role::Superadmin);

    // Superadmin has all permissions by default
    assert!(superadmin.can("orders.delete"));
    assert!(superadmin.can("users.delete"));

    // Explicit deny overrides superadmin privilege!
    superadmin.deny_permission("orders.delete");
    assert!(!superadmin.can("orders.delete"), "Explicit deny strictly overrides superadmin");
    assert!(superadmin.has_explicit_deny("orders.delete"));

    // Other permissions remain granted to superadmin
    assert!(superadmin.can("users.delete"));
    assert!(superadmin.can("orders.view"));

    // Removing deny restores superadmin access
    superadmin.remove_deny("orders.delete");
    assert!(superadmin.can("orders.delete"));
    assert!(!superadmin.has_explicit_deny("orders.delete"));

    // Deny and restore another permission
    superadmin.deny_permission("users.delete");
    assert!(!superadmin.can("users.delete"));
    superadmin.remove_deny("users.delete");
    assert!(superadmin.can("users.delete"));
}

#[test]
fn test_wildcard_permissions_and_hierarchies() {
    use terminus::domain::matches_permission;

    assert!(matches_permission("*", "orders.view"));
    assert!(matches_permission("*", "users.delete"));
    assert!(matches_permission("orders.*", "orders.view"));
    assert!(matches_permission("orders.*", "orders.create"));
    assert!(matches_permission("orders.*", "orders.delete"));
    assert!(matches_permission("orders.*", "orders"));
    assert!(!matches_permission("orders.*", "users.view"));

    // User with wildcard grant
    let mut user = User::new("u_wild", "Wildcard User", "wild@example.com", Role::Member);
    assert!(!user.can("orders.refund"));
    user.give_permission_to("orders.*");
    assert!(user.can("orders.refund"));
    assert!(user.can("orders.cancel"));
    assert!(!user.can("users.delete"));

    // Superadmin with wildcard deny
    let mut sa = User::new("sa_2", "Super Admin", "sa2@example.com", Role::Superadmin);
    sa.deny_permission("orders.*");
    assert!(!sa.can("orders.view"), "Wildcard deny blocks orders.view");
    assert!(!sa.can("orders.delete"), "Wildcard deny blocks orders.delete");
    assert!(sa.can("users.view"), "Wildcard deny does not affect users.view");
}

#[test]
fn test_permission_cache_ttl_and_invalidation() {
    use terminus::domain::PermissionCache;

    let cache = PermissionCache::new(30);
    assert_eq!(cache.len(), 0);

    let mut user = User::new("c_1", "Cached User", "cache@example.com", Role::Member);

    // Initial check caches false
    assert!(!user.can_cached("orders.delete", &cache));
    assert_eq!(cache.get(&user.id, "orders.delete"), Some(false));
    assert_eq!(cache.len(), 1);

    // Give permission and invalidate user cache
    user.give_permission_to("orders.delete");
    cache.invalidate_user(&user.id);
    assert_eq!(cache.len(), 0);

    // Next check populates cache with true
    assert!(user.can_cached("orders.delete", &cache));
    assert_eq!(cache.get(&user.id, "orders.delete"), Some(true));

    // Clear all
    cache.invalidate_all();
    assert_eq!(cache.len(), 0);
}
