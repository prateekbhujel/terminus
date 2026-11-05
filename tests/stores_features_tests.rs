use terminus::prelude::*;
use terminus::storage::LocalStorage;

#[test]
fn events_dispatch_to_registered_listeners_in_order() {
    let bus = EventBus::new();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    for label in ["first", "second"] {
        let seen = seen.clone();
        bus.listen("order.created", move |event: &Event| {
            seen.lock()
                .unwrap()
                .push(format!("{label}:{}", event.payload["id"]));
        });
    }
    assert_eq!(
        bus.dispatch(Event::new(
            "order.created",
            serde_json::json!({ "id": 7 })
        )),
        2
    );
    assert_eq!(seen.lock().unwrap().join(","), "first:7,second:7");
    assert_eq!(bus.dispatch(Event::new("unknown", serde_json::json!({}))), 0);
}

#[test]
fn local_storage_roundtrips_and_rejects_traversal() {
    let dir = std::env::temp_dir().join(format!(
        "terminus_storage_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::new(&dir).unwrap();
    storage.put("docs/readme.txt", b"hello").unwrap();
    assert_eq!(storage.get("docs/readme.txt").unwrap().as_deref(), Some(b"hello".as_slice()));
    assert!(storage.exists("docs/readme.txt"));
    assert_eq!(storage.url("docs/readme.txt"), "/storage/docs/readme.txt");
    assert!(storage.put("../escape.txt", b"x").is_err());
    assert!(storage.get("/etc/passwd").is_err());
    assert!(storage.delete("docs/readme.txt").unwrap());
    assert!(!storage.exists("docs/readme.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rate_limiter_allows_up_to_the_limit_then_blocks() {
    let limiter = RateLimiter::new(2, std::time::Duration::from_secs(60));
    assert!(limiter.hit("client-a"));
    assert!(limiter.hit("client-a"));
    assert!(!limiter.hit("client-a"));
    assert!(limiter.hit("client-b"), "limits are per key");
    assert_eq!(limiter.attempts("client-a"), 3);
}

#[tokio::test]
async fn app_rate_limit_returns_429_after_the_limit() {
    use axum::{body::Body, http::Request, http::StatusCode};
    use tower::ServiceExt;
    let app = App::new()
        .rate_limit(2, std::time::Duration::from_secs(60))
        .get("/", || async { "ok" })
        .build();
    for expected in [StatusCode::OK, StatusCode::OK, StatusCode::TOO_MANY_REQUESTS] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[cfg(feature = "redis")]
#[test]
fn redis_cache_roundtrip_when_configured() {
    let Ok(url) = std::env::var("TERMINUS_TEST_REDIS_URL") else {
        eprintln!("skipping: TERMINUS_TEST_REDIS_URL not set");
        return;
    };
    let cache = RedisCache::new(&url).unwrap();
    cache.flush();
    cache.put("greeting", "hello", Some(60));
    assert_eq!(cache.get("greeting").as_deref(), Some("hello"));
    assert!(cache.forget("greeting"));
    assert_eq!(cache.get("greeting"), None);
}

#[cfg(feature = "mongodb")]
#[tokio::test]
async fn mongo_store_roundtrip_when_configured() {
    let Ok(url) = std::env::var("TERMINUS_TEST_MONGO_URL") else {
        eprintln!("skipping: TERMINUS_TEST_MONGO_URL not set");
        return;
    };
    let store = MongoStore::connect(&url, "terminus_test").await.unwrap();
    store
        .delete("widgets", serde_json::json!({}))
        .await
        .unwrap();
    store
        .insert("widgets", serde_json::json!({ "sku": "KB-1", "qty": 3 }))
        .await
        .unwrap();
    assert_eq!(store.count("widgets", serde_json::json!({})).await.unwrap(), 1);
    let found = store
        .find("widgets", serde_json::json!({ "sku": "KB-1" }))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["sku"], serde_json::json!("KB-1"));
    assert_eq!(
        store
            .delete("widgets", serde_json::json!({ "sku": "KB-1" }))
            .await
            .unwrap(),
        1
    );
}
