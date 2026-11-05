use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use terminus::prelude::*;
use tower::ServiceExt;

fn router(panel: AdminPanel) -> Router {
    Router::new().nest("/admin", panel.into_router())
}

fn account() -> User {
    User::new(
        "operator",
        "Operator",
        "operator@example.test",
        Role::Member,
    )
    .with_permission("widgets.*")
}

fn configured_panel(user: User) -> AdminPanel {
    AdminPanel::new().authenticate_with(move |email, password| {
        (email == user.email && password == " password with spaces ").then(|| user.clone())
    })
}

fn cookie(response: &Response) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("terminus_session="))
        .expect("session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn text(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn token(html: &str) -> String {
    html.split("name=\"_csrf_token\" value=\"")
        .nth(1)
        .expect("CSRF form field")
        .split('"')
        .next()
        .unwrap()
        .to_string()
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: &str,
    csrf: Option<&str>,
    bearer: Option<&str>,
) -> Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    if let Some(bearer) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    app.clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn login_form(app: &Router) -> (String, String) {
    let response = request(app, "GET", "/admin/login", None, "", None, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = cookie(&response);
    let html = text(response).await;
    assert!(!html.contains("admin123"));
    assert!(!html.contains("setDemo"));
    assert!(!html.contains("/oauth/github"));
    (cookie, token(&html))
}

async fn sign_in(app: &Router) -> (String, String) {
    let (previous, csrf) = login_form(app).await;
    let body = serde_urlencoded::to_string([
        ("email", "operator@example.test"),
        ("password", " password with spaces "),
        ("_csrf_token", &csrf),
    ])
    .unwrap();
    let response = request(
        app,
        "POST",
        "/admin/login",
        Some(&previous),
        &body,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    (previous, cookie(&response))
}

#[tokio::test]
async fn panel_denies_demo_cookies_and_unconfigured_credentials() {
    let app = router(AdminPanel::new());
    for forged in [
        "terminus_session=authenticated",
        "terminus_session=user:pratik@terminus.dev",
        "terminus_session=unknown",
    ] {
        let response = request(
            &app,
            "GET",
            "/admin/dashboard",
            Some(forged),
            "",
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "/admin/login"
        );
    }
    let (cookie, csrf) = login_form(&app).await;
    let body = serde_urlencoded::to_string([
        ("email", "pratik@terminus.dev"),
        ("password", "admin123"),
        ("_csrf_token", &csrf),
    ])
    .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/admin/login",
            Some(&cookie),
            &body,
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn login_requires_csrf_and_correct_nonempty_credentials() {
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let user = account();
    let app = router(AdminPanel::new().authenticate_with(move |email, password| {
        called.fetch_add(1, Ordering::SeqCst);
        (email == user.email && password == " password with spaces ").then(|| user.clone())
    }));
    let (anon_cookie, csrf) = login_form(&app).await;
    let credentials = "email=operator%40example.test&password=+password+with+spaces+";
    for submitted in [None, Some("invalid")] {
        assert_eq!(
            request(
                &app,
                "POST",
                "/admin/login",
                Some(&anon_cookie),
                credentials,
                submitted,
                None
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "CSRF must be checked before credentials"
    );
    for body in [
        "email=operator%40example.test&password=wrong",
        "email=operator%40example.test",
        "password=secret",
    ] {
        assert_eq!(
            request(
                &app,
                "POST",
                "/admin/login",
                Some(&anon_cookie),
                body,
                Some(&csrf),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let response = request(
        &app,
        "POST",
        "/admin/login",
        Some(&anon_cookie),
        credentials,
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let signed_in = cookie(&response);
    assert_ne!(signed_in, anon_cookie);
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/dashboard",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/dashboard",
            Some(&anon_cookie),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn inactive_and_repository_missing_users_cannot_log_in() {
    for status in [UserStatus::Pending, UserStatus::Suspended] {
        let app = router(configured_panel(account().with_status(status)));
        let (cookie, csrf) = login_form(&app).await;
        let response = request(
            &app,
            "POST",
            "/admin/login",
            Some(&cookie),
            "email=operator%40example.test&password=+password+with+spaces+",
            Some(&csrf),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let app = router(configured_panel(account()).users(Arc::new(InMemoryUserRepository::new())));
    let (cookie, csrf) = login_form(&app).await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/admin/login",
            Some(&cookie),
            "email=operator%40example.test&password=+password+with+spaces+",
            Some(&csrf),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn session_identity_rechecks_repository_status_and_deletion() {
    let repo = Arc::new(InMemoryUserRepository::new());
    repo.save(account()).unwrap();
    let app = router(configured_panel(account()).users(repo.clone()));
    let (_, signed_in) = sign_in(&app).await;
    repo.update(
        "operator",
        "Operator".into(),
        "operator@example.test".into(),
        Role::Member,
        UserStatus::Suspended,
    )
    .unwrap();
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/dashboard",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    repo.delete("operator").unwrap();
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/dashboard",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn logout_requires_post_and_csrf_and_revokes_cookie_replay() {
    let app = router(configured_panel(account()));
    let (_, signed_in) = sign_in(&app).await;
    let dashboard = request(
        &app,
        "GET",
        "/admin/dashboard",
        Some(&signed_in),
        "",
        None,
        None,
    )
    .await;
    assert_eq!(dashboard.status(), StatusCode::OK);
    let html = text(dashboard).await;
    assert!(html.contains("method=\"POST\" action=\"/admin/logout\""));
    let csrf = token(&html);
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/logout",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/admin/logout",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = request(
        &app,
        "POST",
        "/admin/logout",
        Some(&signed_in),
        "",
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/dashboard",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/oauth/github",
            Some(&signed_in),
            "",
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

struct Widgets {
    calls: Arc<AtomicUsize>,
}
impl Resource for Widgets {
    fn name(&self) -> &str {
        "Widget"
    }
    fn plural_name(&self) -> &str {
        "Widgets"
    }
    fn slug(&self) -> &str {
        "widgets"
    }
    fn table(&self) -> Table {
        Table::new()
    }
    fn fetch_rows(&self, _: &QueryState) -> (Vec<RowData>, usize) {
        (vec![], 0)
    }
    fn create_row(&self, values: HashMap<String, String>) -> Result<String, String> {
        assert!(!values.contains_key("_csrf_token") && !values.contains_key("csrf_token"));
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("1".into())
    }
    fn update_row(&self, _: &str, values: HashMap<String, String>) -> Result<(), String> {
        assert!(!values.contains_key("_csrf_token") && !values.contains_key("csrf_token"));
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn delete_row(&self, _: &str) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn handle_action(&self, _: &str, _: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("done".into())
    }
    fn handle_bulk_action(&self, _: &str, _: &[String]) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("done".into())
    }
}

#[tokio::test]
async fn every_resource_mutation_requires_csrf_and_preserves_form_bodies() {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = router(configured_panel(account()).resource(Widgets {
        calls: calls.clone(),
    }));
    let (_, signed_in) = sign_in(&app).await;
    let dashboard = request(
        &app,
        "GET",
        "/admin/dashboard",
        Some(&signed_in),
        "",
        None,
        None,
    )
    .await;
    let csrf = token(&text(dashboard).await);
    let paths = [
        "/admin/widgets/create",
        "/admin/widgets/edit/1",
        "/admin/widgets/delete/1",
        "/admin/widgets/action/run/1",
        "/admin/widgets/bulk-action/run",
    ];
    for path in paths {
        assert_eq!(
            request(
                &app,
                "POST",
                path,
                Some(&signed_in),
                "name=Widget&ids=1%2C2",
                None,
                None
            )
            .await
            .status(),
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for (index, path) in paths.into_iter().enumerate() {
        let body = format!("name=Widget&ids=1%2C2&_csrf_token={csrf}");
        let response = request(
            &app,
            "POST",
            path,
            Some(&signed_in),
            &body,
            if index % 2 == 0 { None } else { Some(&csrf) },
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER, "{path}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 5);
    let rendered = request(
        &app,
        "GET",
        "/admin/widgets",
        Some(&signed_in),
        "",
        None,
        None,
    )
    .await;
    let html = text(rendered).await;
    assert!(html.contains("appendCsrfToken(form)"));
    assert!(html.matches("name=\"_csrf_token\"").count() >= 4);
}

#[tokio::test]
async fn bearer_requires_existing_active_user_and_never_grants_fallback_role() {
    let tokens = Arc::new(MemoryTokenStore::new());
    let token = tokens
        .create_token("user", "operator", "test", vec!["*".into()], None)
        .unwrap();
    let without_repo = router(AdminPanel::new().tokens(tokens.clone()));
    assert_eq!(
        request(
            &without_repo,
            "GET",
            "/admin/dashboard",
            None,
            "",
            None,
            Some(&token.plain_text_token)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let repo = Arc::new(InMemoryUserRepository::new());
    repo.save(account()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let app = router(
        AdminPanel::new()
            .users(repo.clone())
            .tokens(tokens)
            .resource(Widgets {
                calls: calls.clone(),
            }),
    );
    let response = request(
        &app,
        "POST",
        "/admin/widgets/create",
        None,
        "name=Widget",
        None,
        Some(&token.plain_text_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // Replace the account with a Member that has no direct widget grant.
    // A bearer token must not fall back to a privileged role, so this
    // request is forbidden even though the token itself is valid.
    repo.delete("operator").unwrap();
    repo.save(User::new(
        "operator",
        "Operator",
        "operator@example.test",
        Role::Member,
    ))
    .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/admin/widgets/create",
            None,
            "name=Widget",
            None,
            Some(&token.plain_text_token)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    repo.update(
        "operator",
        "Operator".into(),
        "operator@example.test".into(),
        Role::Superadmin,
        UserStatus::Suspended,
    )
    .unwrap();
    assert_eq!(
        request(
            &app,
            "GET",
            "/admin/dashboard",
            None,
            "",
            None,
            Some(&token.plain_text_token)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn app_mounts_the_panel_under_admin() {
    let app = App::new().panel(configured_panel(account())).build();
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let unmounted = app
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unmounted.status(), StatusCode::NOT_FOUND);
}

#[test]
fn password_hashing_uses_argon2id_and_rejects_plaintext_or_malformed_legacy_hashes() {
    let first = Hash::make("correct password");
    let second = Hash::make("correct password");
    assert!(first.starts_with("$argon2id$v=19$"));
    assert_ne!(first, second);
    assert!(Hash::check("correct password", &first));
    assert!(!Hash::check("wrong password", &first));
    assert!(!Hash::check("plaintext", "plaintext"));
    for malformed in [
        "$argon2id$invalid",
        "$pbkdf2-sha256$i=0$salt$abc",
        "$pbkdf2-sha256$i=4294967295$salt$abc",
        "$pbkdf2-sha256$i=invalid$salt$abc",
    ] {
        assert!(!Hash::check("password", malformed));
    }
    let legacy = Hash::make_with_salt("password", "legacy-salt", 1000);
    assert!(Hash::check("password", &legacy));
    assert!(!Hash::check("wrong", &legacy));
}
