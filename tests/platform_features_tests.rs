use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use terminus::prelude::*;

/// Minimal HTTP server: captures requests and replays canned responses
/// in order, then answers 200/empty. Lives until the test process ends.
fn fake_server(responses: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = captured.clone();
    let queue = Arc::new(Mutex::new(std::collections::VecDeque::from(responses)));
    std::thread::spawn(move || loop {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let queue = queue.clone();
        let sink = sink.clone();
        std::thread::spawn(move || {
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            let mut content_length = 0usize;
            loop {
                let Ok(read) = stream.read(&mut buf) else {
                    return;
                };
                if read == 0 {
                    return;
                }
                request.extend_from_slice(&buf[..read]);
                let text = String::from_utf8_lossy(&request);
                if let Some(headers_end) = text.find("\r\n\r\n") {
                    for line in text[..headers_end].lines() {
                        if let Some(value) =
                            line.to_ascii_lowercase().strip_prefix("content-length:")
                        {
                            content_length = value.trim().parse().unwrap_or(0);
                        }
                    }
                    if text.len() >= headers_end + 4 + content_length {
                        break;
                    }
                }
            }
            sink.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&request).into_owned());
            let response = queue
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| http_ok(""));
            let _ = stream.write_all(response.as_bytes());
        });
    });
    (format!("http://{addr}"), captured)
}

fn http_ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn local_signed_urls_verify_and_reject_tampering() {
    let dir = std::env::temp_dir().join(format!(
        "terminus_signed_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::new(&dir).unwrap();
    let url = terminus::storage::signed_url(&storage, "docs/a.txt", b"test-secret", 600);
    let expires: i64 = url
        .split("expires=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let signature = url.split("signature=").nth(1).unwrap().to_string();
    let now = chrono::Utc::now().timestamp();
    assert!(terminus::storage::verify_signed_url(
        "docs/a.txt", expires, &signature, b"test-secret", now
    ));
    assert!(!terminus::storage::verify_signed_url(
        "docs/other.txt", expires, &signature, b"test-secret", now
    ));
    assert!(!terminus::storage::verify_signed_url(
        "docs/a.txt", expires, &signature, b"wrong-secret", now
    ));
    assert!(!terminus::storage::verify_signed_url(
        "docs/a.txt", expires, &signature, b"test-secret", expires + 1
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(feature = "s3")]
#[test]
fn s3_storage_signs_requests_and_roundtrips() {
    let (base, captured) = fake_server(vec![
        http_ok(""),
        http_ok("s3-body"),
        http_ok(""),
        http_ok(""),
    ]);
    let s3 = S3Storage::new("my-bucket", "us-east-1", "AKID", "secret").endpoint(&base);
    s3.put("docs/a.txt", b"hello").unwrap();
    assert_eq!(s3.get("docs/a.txt").unwrap().as_deref(), Some(b"s3-body".as_slice()));
    assert!(s3.exists("docs/a.txt"));
    assert!(s3.delete("docs/a.txt").unwrap());

    let requests = captured.lock().unwrap();
    assert!(requests[0].starts_with("PUT /my-bucket/docs/a.txt"), "{}", requests[0]);
    assert!(
        requests[0].contains("Authorization: AWS4-HMAC-SHA256 Credential=AKID/"),
        "{}",
        requests[0]
    );
    assert!(requests[1].starts_with("GET /my-bucket/docs/a.txt"));
    let signed = s3.temporary_url("docs/a.txt", 300);
    assert!(signed.contains("X-Amz-Signature="), "{signed}");
    assert!(signed.contains("X-Amz-Expires=300"), "{signed}");
}

#[test]
fn presence_channels_track_members_and_private_channels_deny_by_default() {
    let broadcaster = Broadcaster::new(16);
    assert_eq!(terminus::ws::channel_kind("presence-lobby"), terminus::ws::ChannelKind::Presence);
    assert!(broadcaster.authorize("u1", "public-news"));
    assert!(!broadcaster.authorize("u1", "private-chat"), "no authorizer means deny");
    assert!(!broadcaster.join_presence("presence-lobby", "u1", serde_json::json!({ "name": "Ada" })));

    broadcaster.authorize_with(|user, channel| user == "u1" && channel == "presence-lobby");
    assert!(!broadcaster.authorize("u1", "private-chat"));
    assert!(broadcaster.join_presence("presence-lobby", "u1", serde_json::json!({ "name": "Ada" })));
    let members = broadcaster.presence_members("presence-lobby");
    assert_eq!(members["u1"]["name"], serde_json::json!("Ada"));
    assert!(broadcaster.leave_presence("presence-lobby", "u1"));
    assert!(broadcaster.presence_members("presence-lobby").is_empty());
}

#[test]
fn translator_falls_back_and_interpolates() {
    let translator = Translator::new("en");
    translator.add_locale(
        "en",
        serde_json::json!({ "messages": { "welcome": "Welcome, :name!" } }),
    );
    translator.add_locale(
        "ne",
        serde_json::json!({ "messages": { "welcome": "Namaste, {name}!" } }),
    );
    translator.set_locale("ne");
    assert_eq!(
        translator.translate("messages.welcome", &[("name", "Pratik")]),
        "Namaste, Pratik!"
    );
    translator.set_locale("fr");
    assert_eq!(
        translator.translate("messages.welcome", &[("name", "Ada")]),
        "Welcome, Ada!",
        "unknown locales fall back"
    );
    assert_eq!(translator.translate("missing.key", &[]), "missing.key");
    assert!(translator.has("messages.welcome"));
}

#[cfg(feature = "oauth")]
#[test]
fn oauth_exchange_and_userinfo_against_provider() {
    let (base, captured) = fake_server(vec![
        http_ok(r#"{"access_token":"tok-123","token_type":"bearer"}"#),
        http_ok(r#"{"id":42,"email":"ada@example.test","name":"Ada","avatar_url":"https://x.test/a.png"}"#),
    ]);
    let client = OAuthClient::custom(
        format!("{base}/authorize"),
        format!("{base}/token"),
        format!("{base}/userinfo"),
        "client-id",
        "client-secret",
    );
    let url = client.authorization_url("https://app.test/callback", "state-1");
    assert!(url.contains("client_id=client-id"), "{url}");
    assert!(url.contains("state=state-1"), "{url}");

    let token = client.exchange_code("code-abc", "https://app.test/callback").unwrap();
    assert_eq!(token, "tok-123");
    let user = client.userinfo(&token).unwrap();
    assert_eq!(user.id, "42");
    assert_eq!(user.email.as_deref(), Some("ada@example.test"));
    assert_eq!(user.name.as_deref(), Some("Ada"));

    let requests = captured.lock().unwrap();
    assert!(requests[0].contains("code=code-abc"), "{}", requests[0]);
    assert!(requests[0].contains("grant_type=authorization_code"), "{}", requests[0]);
    assert!(requests[1].contains("Authorization: Bearer tok-123"), "{}", requests[1]);
}

#[test]
fn database_search_ranks_exact_then_prefix_then_contains() {
    let db = Database::memory().unwrap();
    db.execute("CREATE TABLE widgets (id TEXT PRIMARY KEY, name TEXT, sku TEXT)")
        .unwrap();
    db.execute("INSERT INTO widgets VALUES ('1', 'Keyboard', 'KB-1')").unwrap();
    db.execute("INSERT INTO widgets VALUES ('2', 'Keyboard Pro', 'KB-2')").unwrap();
    db.execute("INSERT INTO widgets VALUES ('3', 'Pro Keyboard Case', 'KB-3')").unwrap();
    db.execute("INSERT INTO widgets VALUES ('4', 'Mouse', 'MS-1')").unwrap();

    let hits = terminus::search::search_database(&db, "widgets", &["name"], "keyboard", 10).unwrap();
    let ids: Vec<&str> = hits.iter().map(|hit| hit.id.as_str()).collect();
    assert_eq!(ids, vec!["1", "2", "3"]);
    assert!(hits[0].score > hits[1].score && hits[1].score > hits[2].score);
    assert!(terminus::search::search_database(&db, "widgets; DROP TABLE widgets", &["name"], "x", 1).is_err());
}

#[tokio::test]
async fn app_metrics_record_requests() {
    use axum::{body::Body, http::Request, http::StatusCode};
    use tower::ServiceExt;
    let app = App::new()
        .metrics(true)
        .get("/", || async { "ok" })
        .build();
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = app
        .oneshot(Request::builder().uri("/missing").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let snapshot = terminus::metrics::metrics_snapshot();
    assert!(snapshot["total_requests"].as_u64().unwrap() >= 3);
    assert_eq!(snapshot["by_status_class"]["2xx"], serde_json::json!(2));
    assert_eq!(snapshot["by_status_class"]["4xx"], serde_json::json!(1));
    assert_eq!(snapshot["recent"][0]["path"], serde_json::json!("/missing"));
}
