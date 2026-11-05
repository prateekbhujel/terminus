use axum::{body::{to_bytes, Body}, http::{header, Request, StatusCode}, response::Response};
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use terminus::prelude::*;
use terminus::session::SessionConfig;
use tower::ServiceExt;

const KEY: &[u8] = b"a-test-only-signing-key-at-least-32-bytes";

struct TempStore(PathBuf);
impl TempStore {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::SeqCst);
        Self(std::env::temp_dir().join(format!("terminus_sessions_{}_{}_{sequence}.db",
            std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())))
    }
    fn config(&self) -> SessionConfig { SessionConfig::sqlite(&self.0, KEY).unwrap() }
}
impl Drop for TempStore {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

fn cookie(response: &Response) -> String {
    response.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}

async fn text(response: Response) -> String {
    String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap()
}

#[tokio::test]
async fn sessions_and_one_time_flashes_survive_store_and_app_restart() {
    let db = TempStore::new();
    let app = App::new().sessions(db.config()).post("/save", || async {
        session().set("name", "persistent user");
        redirect("/read").with_success("Saved persistently")
    }).build();
    let response = app.oneshot(Request::builder().method("POST").uri("/save")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = cookie(&response);
    let value = cookie.strip_prefix("terminus_session=").unwrap();
    assert!(Session::verify_signed_cookie(value, KEY).is_some());
    assert!(response.headers()[header::SET_COOKIE].to_str().unwrap().contains("; Secure"));

    let restarted = App::new().sessions(db.config()).get("/read", || async {
        assert_eq!(session().get("name").as_deref(), Some("persistent user"));
        let message = session().get_flash("success").unwrap_or_default();
        session().sweep_flashes();
        message
    }).build();
    let read = restarted.oneshot(Request::builder().uri("/read")
        .header(header::COOKIE, &cookie).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(text(read).await, "Saved persistently");

    let reopened = App::new().sessions(db.config()).get("/", || async {
        assert_eq!(session().get("name").as_deref(), Some("persistent user"));
        session().get_flash("success").unwrap_or_default()
    }).build();
    let read = reopened.oneshot(Request::builder().header(header::COOKIE, cookie)
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(text(read).await, "");
}

#[tokio::test]
async fn signed_configuration_rejects_forged_and_unsigned_cookies_before_handler() {
    let db = TempStore::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let app = App::new().sessions(db.config()).get("/", {
        let calls = calls.clone();
        move || { let calls = calls.clone(); async move {
            calls.fetch_add(1, Ordering::SeqCst);
            "handler ran"
        }}
    }).build();
    for value in ["terminus_session=chosen-id", "terminus_session=chosen-id.invalid-signature"] {
        let response = app.clone().oneshot(Request::builder().header(header::COOKIE, value)
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn expired_and_unknown_ids_receive_fresh_server_generated_identifiers() {
    let db = TempStore::new();
    let config = db.config();
    let stored = Session::new();
    stored.set("identity", "must expire");
    config.store().save(&stored).unwrap();
    rusqlite::Connection::open(&db.0).unwrap().execute(
        "UPDATE _terminus_sessions SET expires_at = 1 WHERE id = ?1", [stored.id()]).unwrap();
    drop(config);
    let app = App::new().sessions(db.config()).get("/", |extracted: Session| async move {
        assert!(extracted.get("identity").is_none());
        extracted.id().to_string()
    }).build();
    let expired_cookie = stored.signed_cookie_header(KEY);
    let unknown = Session::with_id("client-selected-unknown-id");
    for (header, old_id) in [(expired_cookie, stored.id()), (unknown.signed_cookie_header(KEY), unknown.id())] {
        let response = app.clone().oneshot(Request::builder().header(header::COOKIE, header)
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let new_id = text(response).await;
        assert_ne!(new_id, old_id);
        assert_eq!(new_id.len(), 64);
    }
    let memory = SessionStore::new();
    assert_ne!(memory.get_or_create(Some("unknown-memory-id")).id(), "unknown-memory-id");
}

#[test]
fn revocation_survives_restart_and_rejects_stale_request_saves() {
    let db = TempStore::new();
    let first = SessionStore::sqlite(&db.0).unwrap();
    let session = Session::new();
    session.set("authenticated", "yes");
    first.save(&session).unwrap();
    let second = SessionStore::sqlite(&db.0).unwrap();
    let stale = second.load(session.id()).unwrap().unwrap();
    assert!(first.remove(session.id()).unwrap());
    stale.set("authenticated", "resurrected");
    assert!(second.save(&stale).is_err());
    drop((first, second));
    let reopened = SessionStore::sqlite(&db.0).unwrap();
    assert!(reopened.load(session.id()).unwrap().is_none());
    let replacement = reopened.get_or_create(Some(session.id()));
    assert_ne!(replacement.id(), session.id());
    assert!(replacement.get("authenticated").is_none());
}

#[test]
fn config_rejects_short_keys_and_invalid_ttl_and_store_reports_corrupt_data() {
    let db = TempStore::new();
    assert!(SessionConfig::new(Arc::new(SessionStore::new()), b"short").is_err());
    assert!(db.config().ttl(Duration::ZERO).is_err());
    let store = SessionStore::sqlite(&db.0).unwrap();
    let session = Session::new();
    store.save(&session).unwrap();
    rusqlite::Connection::open(&db.0).unwrap().execute(
        "UPDATE _terminus_sessions SET data = 'invalid-json' WHERE id = ?1", [session.id()]).unwrap();
    assert!(store.load(session.id()).is_err());
}
