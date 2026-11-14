use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use terminus::app::App;
use tower::ServiceExt;

async fn status(
    router: &axum::Router,
    peer: Option<&str>,
    forwarded_for: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder().uri("/").method("GET");
    if let Some(value) = forwarded_for {
        builder = builder.header("x-forwarded-for", value);
    }
    let mut request = builder.body(Body::empty()).unwrap();
    if let Some(peer) = peer {
        request.extensions_mut().insert(ConnectInfo(
            peer.parse::<SocketAddr>().unwrap(),
        ));
    }
    router.clone().oneshot(request).await.unwrap().status()
}

fn test_app() -> App {
    App::new().get("/", || async { "ok" })
}

#[tokio::test]
async fn direct_clients_cannot_evade_throttle_by_spoofing_forwarded_headers() {
    let router = test_app().rate_limit(2, Duration::from_secs(60)).build();
    let client = Some("203.0.113.10:5000");
    assert_eq!(status(&router, client, Some("1.1.1.1")).await, StatusCode::OK);
    assert_eq!(status(&router, client, Some("2.2.2.2")).await, StatusCode::OK);
    assert_eq!(
        status(&router, client, Some("3.3.3.3")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // Different real peers have independent buckets.
    assert_eq!(
        status(&router, Some("203.0.113.11:5000"), Some("1.1.1.1")).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn trusted_proxy_uses_rightmost_untrusted_hop_not_spoofed_first_hop() {
    let proxies: Vec<IpAddr> = vec!["127.0.0.1".parse().unwrap(), "10.0.0.3".parse().unwrap()];
    let router = test_app()
        .rate_limit_with_trusted_proxies(1, Duration::from_secs(60), &proxies)
        .build();

    // Nginx -> second trusted proxy -> real client; untrusted leftmost
    // addresses cannot make subsequent requests look like new clients.
    let proxy = Some("127.0.0.1:443");
    assert_eq!(
        status(&router, proxy, Some("192.0.2.1, 203.0.113.55, 10.0.0.3")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&router, proxy, Some("192.0.2.2, 203.0.113.55, 10.0.0.3")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        status(&router, proxy, Some("192.0.2.2, 203.0.113.56, 10.0.0.3")).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn forwarding_headers_from_unlisted_peers_are_ignored() {
    let proxies = ["127.0.0.1".parse().unwrap()];
    let router = test_app()
        .rate_limit_with_trusted_proxies(1, Duration::from_secs(60), &proxies)
        .build();
    let peer = Some("198.51.100.10:8080");
    assert_eq!(status(&router, peer, Some("1.1.1.1")).await, StatusCode::OK);
    assert_eq!(
        status(&router, peer, Some("2.2.2.2")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn malformed_forwarding_falls_back_to_socket_peer() {
    let proxies = ["127.0.0.1".parse().unwrap()];
    let router = test_app()
        .rate_limit_with_trusted_proxies(1, Duration::from_secs(60), &proxies)
        .build();
    let peer = Some("127.0.0.1:8080");
    assert_eq!(status(&router, peer, Some("nonsense, 203.0.113.10")).await, StatusCode::OK);
    assert_eq!(
        status(&router, peer, Some("invalid, 203.0.113.11")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn in_memory_requests_cannot_supply_identity_through_headers() {
    let router = test_app().rate_limit(1, Duration::from_secs(60)).build();
    assert_eq!(status(&router, None, Some("1.1.1.1")).await, StatusCode::OK);
    assert_eq!(
        status(&router, None, Some("2.2.2.2")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
}
