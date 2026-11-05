use std::sync::{Arc, Mutex};
use terminus::prelude::*;
use terminus::resource::QueryState;

fn shared_repository() -> (SqliteUserRepository, Arc<Mutex<rusqlite::Connection>>) {
    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let repo = SqliteUserRepository::new(conn.clone()).unwrap();
    (repo, conn)
}

#[test]
fn fresh_database_contains_no_seeded_accounts() {
    let (repo, _) = shared_repository();
    let (_, total) = repo.list(&QueryState::default());
    assert_eq!(total, 0);
    assert!(repo.find_by_email("pratik@terminus.dev").is_none());
}

#[test]
fn credentials_verify_only_with_the_correct_password() {
    let (repo, conn) = shared_repository();
    let user = User::new("ignored", "Operator", "operator@example.test", Role::Admin);
    let id = repo.save(user).unwrap();

    // Accounts created without a password can never sign in.
    assert!(repo
        .verify_credentials("operator@example.test", "anything")
        .is_none());

    conn.lock()
        .unwrap()
        .execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            rusqlite::params![Hash::make("correct horse battery staple"), id],
        )
        .unwrap();

    let verified = repo
        .verify_credentials("OPERATOR@example.test", "correct horse battery staple")
        .expect("correct password verifies");
    assert_eq!(verified.email, "operator@example.test");
    assert_eq!(verified.role, Role::Admin);
    assert!(repo
        .verify_credentials("operator@example.test", "wrong password")
        .is_none());
    assert!(repo
        .verify_credentials("missing@example.test", "correct horse battery staple")
        .is_none());
}
