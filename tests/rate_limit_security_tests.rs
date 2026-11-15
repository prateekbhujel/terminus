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
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
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
    assert_eq!(
        status(&router, client, Some("1.1.1.1")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&router, client, Some("2.2.2.2")).await,
        StatusCode::OK
    );
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
    assert_eq!(
        status(&router, peer, Some("203.0.113.10, nonsense")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&router, peer, Some("203.0.113.11, invalid")).await,
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

#[tokio::test]
async fn untrusted_malformed_prefix_does_not_poison_shared_proxy_bucket() {
    let proxies = ["127.0.0.1".parse().unwrap()];
    let router = test_app()
        .rate_limit_with_trusted_proxies(1, Duration::from_secs(60), &proxies)
        .build();
    let proxy = Some("127.0.0.1:8080");
    assert_eq!(
        status(&router, proxy, Some("not-an-ip, 203.0.113.10")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(
            &router,
            proxy,
            Some("another-malformed-prefix, 203.0.113.10")
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        status(&router, proxy, Some("not-an-ip, 203.0.113.11")).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn ipv4_mapped_ipv6_peers_and_hops_share_canonical_identity() {
    let proxies = ["127.0.0.1".parse().unwrap()];
    let router = test_app()
        .rate_limit_with_trusted_proxies(1, Duration::from_secs(60), &proxies)
        .build();
    assert_eq!(
        status(
            &router,
            Some("[::ffff:127.0.0.1]:8080"),
            Some("203.0.113.10")
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        status(&router, Some("127.0.0.1:8081"), Some("::ffff:203.0.113.10")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn real_tcp_listener_populates_connect_info_and_blocks_spoofed_xff() {
    let router = test_app().rate_limit(1, Duration::from_secs(60)).build();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    let send = |xff: &str| {
        let req = format!(
            "GET / HTTP/1.1\r\nHost: localhost\r\nX-Forwarded-For: {xff}\r\nConnection: close\r\n\r\n"
        );
        async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            stream.write_all(req.as_bytes()).await.unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await.unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        }
    };

    assert!(send("1.1.1.1").await.starts_with("HTTP/1.1 200 OK"));
    assert!(send("2.2.2.2")
        .await
        .starts_with("HTTP/1.1 429 Too Many Requests"));
    server.abort();
}

#[tokio::test]
async fn request_ip_ignores_spoofed_headers_from_untrusted_direct_peer() {
    let app = App::new().get("/ip", || async { terminus::helpers::request().ip() });
    let router = app.build();

    let mut request = Request::builder()
        .uri("/ip")
        .method("GET")
        .header("x-forwarded-for", "198.51.100.5")
        .header("x-real-ip", "198.51.100.6")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "203.0.113.10:5000".parse::<SocketAddr>().unwrap(),
    ));

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), "203.0.113.10");
}

#[tokio::test]
async fn request_ip_resolves_forwarded_chain_from_trusted_proxy() {
    let proxies: Vec<IpAddr> = vec!["127.0.0.1".parse().unwrap(), "10.0.0.1".parse().unwrap()];
    let app = App::new()
        .trusted_proxies(&proxies)
        .get("/ip", || async { terminus::helpers::request().ip() });
    let router = app.build();

    let mut request = Request::builder()
        .uri("/ip")
        .method("GET")
        .header("x-forwarded-for", "1.1.1.1, 198.51.100.7, 10.0.0.1")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:443".parse::<SocketAddr>().unwrap()));

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), "198.51.100.7");
}

#[tokio::test]
async fn request_ip_falls_back_to_x_real_ip_when_xff_absent_and_proxy_trusted() {
    let proxies: Vec<IpAddr> = vec!["127.0.0.1".parse().unwrap()];
    let app = App::new()
        .trusted_proxies(&proxies)
        .get("/ip", || async { terminus::helpers::request().ip() });
    let router = app.build();

    let mut request = Request::builder()
        .uri("/ip")
        .method("GET")
        .header("x-real-ip", "198.51.100.99")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:443".parse::<SocketAddr>().unwrap()));

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), "198.51.100.99");
}

#[tokio::test]
async fn request_ip_in_memory_requests_ignore_spoofed_headers() {
    let app = App::new().get("/ip", || async { terminus::helpers::request().ip() });
    let router = app.build();

    let request = Request::builder()
        .uri("/ip")
        .method("GET")
        .header("x-forwarded-for", "198.51.100.5")
        .header("x-real-ip", "198.51.100.6")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), "127.0.0.1");
}
