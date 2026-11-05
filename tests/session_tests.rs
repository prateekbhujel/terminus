use axum::{
    body::{to_bytes, Body},
    extract::Path,
    http::{header, Request, StatusCode},
    response::Response,
};
use std::sync::Arc;
use terminus::prelude::*;
use tower::ServiceExt;

async fn body_text(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn session_cookie(response: &Response) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("terminus_session="))
        .expect("web responses must provision a session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn app_sessions_are_isolated_and_the_extractor_matches_helpers() {
    let app = App::new()
        .get("/", |extracted: Session| async move {
            assert_eq!(extracted.id(), session().id());
            session().id().to_string()
        })
        .build();

    let first = app
        .clone()
        .oneshot(Request::new(Body::empty()))
        .await
        .unwrap();
    let first_cookie = session_cookie(&first);
    let first_id = body_text(first).await;
    assert_eq!(first_cookie, format!("terminus_session={first_id}"));

    let second = app
        .clone()
        .oneshot(Request::new(Body::empty()))
        .await
        .unwrap();
    assert_ne!(body_text(second).await, first_id);

    let returning = app
        .oneshot(
            Request::builder()
                .header(header::COOKIE, first_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_text(returning).await, first_id);
}

#[tokio::test]
async fn view_and_redirect_helpers_preserve_sessions_and_one_time_flashes() {
    let app = App::new()
        .get("/", || async { view("Home", "Welcome") })
        .post("/save", || async {
            session().set("saved", "yes");
            redirect("/result").with_success("Saved through redirect")
        })
        .get("/result", || async {
            assert_eq!(session().get("saved").as_deref(), Some("yes"));
            view("Result", "Stored")
        })
        .build();
    let first = app
        .clone()
        .oneshot(Request::new(Body::empty()))
        .await
        .unwrap();
    let cookie = session_cookie(&first);
    assert!(body_text(first).await.contains("name=\"csrf-token\""));

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/save")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::SEE_OTHER);
    assert_eq!(session_cookie(&save), cookie);

    let result = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/result")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(body_text(result).await.contains("Saved through redirect"));
    let next = app
        .oneshot(
            Request::builder()
                .uri("/result")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!body_text(next).await.contains("Saved through redirect"));
}

#[tokio::test(flavor = "current_thread")]
async fn concurrent_requests_scope_user_request_and_session_across_awaits() {
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let app = App::new()
        .get("/:id", {
            let barrier = barrier.clone();
            move |Path(id): Path<String>| {
                let barrier = barrier.clone();
                async move {
                    assert!(auth().is_none());
                    let mut user = User::new("2", "Test Admin", "admin@example.test", Role::Superadmin);
                    user.id = id.clone();
                    set_current_user(Some(user));
                    let mut nested_user = User::new("2", "Test Admin", "admin@example.test", Role::Superadmin);
                    nested_user.id = "nested".into();
                    with_user(nested_user, || {
                        assert_eq!(user_id().as_deref(), Some("nested"))
                    });
                    assert_eq!(user_id().as_deref(), Some(id.as_str()));
                    with_request(RequestInfo::new("PUT", "/nested"), || {
                        assert_eq!(request().path(), "/nested");
                    });
                    session().set("request_id", &id);
                    assert_eq!(request().path(), format!("/{id}"));
                    barrier.wait().await;
                    tokio::task::yield_now().await;
                    assert_eq!(user_id().as_deref(), Some(id.as_str()));
                    assert_eq!(session().get("request_id").as_deref(), Some(id.as_str()));
                    assert_eq!(request().path(), format!("/{id}"));
                    id
                }
            }
        })
        .build();
    set_current_user(Some(User::new("2", "Test Admin", "admin@example.test", Role::Superadmin)));
    let (first, second) = tokio::join!(
        app.clone().oneshot(
            Request::builder()
                .uri("/first")
                .body(Body::empty())
                .unwrap()
        ),
        app.oneshot(
            Request::builder()
                .uri("/second")
                .body(Body::empty())
                .unwrap()
        ),
    );
    assert_eq!(body_text(first.unwrap()).await, "first");
    assert_eq!(body_text(second.unwrap()).await, "second");
    assert!(
        auth().is_some(),
        "request scope must preserve the caller's thread context"
    );
    set_current_user(None);
}

#[tokio::test]
async fn middleware_preserves_explicit_session_cookies_and_unrelated_cookies() {
    let app = App::new()
        .get("/", || async {
            session().set("used", "yes");
            let mut response = "ok".into_response();
            response
                .headers_mut()
                .append(header::SET_COOKIE, "other=value; Path=/".parse().unwrap());
            response.headers_mut().append(
                header::SET_COOKIE,
                "terminus_session=explicit; Path=/admin".parse().unwrap(),
            );
            response
        })
        .build();
    let response = app.oneshot(Request::new(Body::empty())).await.unwrap();
    let cookies: Vec<_> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect();
    assert_eq!(
        cookies,
        vec![
            "other=value; Path=/",
            "terminus_session=explicit; Path=/admin"
        ]
    );
}

#[tokio::test]
async fn stateless_requests_do_not_receive_session_cookies() {
    let app = App::new().get("/health", || async { "ok" }).build();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    assert_eq!(body_text(response).await, "ok");
}

#[tokio::test]
async fn explicit_task_scopes_override_and_restore_app_request_context() {
    use axum::extract::FromRequestParts;
    use terminus::helpers::{TASK_REQUEST, TASK_SESSION, TASK_USER};

    let app = App::new()
        .get("/", || async {
            let base_session = session();
            let scoped_session = Session::new();
            let scoped_id = scoped_session.id().to_string();
            let mut scoped_user = User::new("2", "Test Admin", "admin@example.test", Role::Superadmin);
            scoped_user.id = "task-user".into();
            TASK_USER
                .scope(
                    Some(scoped_user),
                    TASK_REQUEST.scope(
                        Some(RequestInfo::new("PUT", "/task-request")),
                        TASK_SESSION.scope(Some(scoped_session), async {
                            assert_eq!(user_id().as_deref(), Some("task-user"));
                            assert_eq!(request().path(), "/task-request");
                            assert_eq!(session().id(), scoped_id);
                            let (mut parts, _) = Request::new(()).into_parts();
                            let extracted =
                                Session::from_request_parts(&mut parts, &()).await.unwrap();
                            assert_eq!(extracted.id(), scoped_id);
                        }),
                    ),
                )
                .await;
            assert!(auth().is_none());
            assert_eq!(request().path(), "/");
            assert_eq!(session().id(), base_session.id());

            set_current_user(Some(User::new("2", "Test Admin", "admin@example.test", Role::Superadmin)));
            TASK_USER
                .scope(None, async {
                    assert!(auth().is_none());
                })
                .await;
            TASK_REQUEST
                .scope(None, async {
                    assert!(terminus::helpers::request_info().is_none());
                })
                .await;
            assert!(auth().is_some());
            assert_eq!(request().path(), "/");
            base_session.id().to_string()
        })
        .build();
    let response = app.oneshot(Request::new(Body::empty())).await.unwrap();
    let cookie = session_cookie(&response);
    assert_eq!(
        cookie,
        format!("terminus_session={}", body_text(response).await)
    );
}

#[tokio::test]
async fn session_used_by_an_explicit_task_scope_roundtrips_through_its_cookie() {
    use terminus::helpers::TASK_SESSION;
    let app = App::new()
        .get("/", || async {
            let scoped = Session::new();
            TASK_SESSION
                .scope(Some(scoped), async {
                    session().set("marker", "scoped-value");
                    session().id().to_string()
                })
                .await
        })
        .get("/read", |extracted: Session| async move {
            extracted.get("marker").unwrap_or_default()
        })
        .build();
    let response = app
        .clone()
        .oneshot(Request::new(Body::empty()))
        .await
        .unwrap();
    let cookie = session_cookie(&response);
    assert_eq!(
        cookie,
        format!("terminus_session={}", body_text(response).await)
    );
    let returning = app
        .oneshot(
            Request::builder()
                .uri("/read")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_text(returning).await, "scoped-value");
}

#[tokio::test]
async fn session_provided_by_an_extension_roundtrips_through_its_cookie() {
    let provided = Session::new();
    provided.set("marker", "extension-value");
    let app = App::new()
        .route(
            "/",
            axum::routing::get(|extracted: Session| async move {
                assert_eq!(extracted.id(), session().id());
                extracted.id().to_string()
            })
            .layer(axum::Extension(provided)),
        )
        .get("/read", |extracted: Session| async move {
            extracted.get("marker").unwrap_or_default()
        })
        .build();
    let response = app
        .clone()
        .oneshot(Request::new(Body::empty()))
        .await
        .unwrap();
    let cookie = session_cookie(&response);
    assert_eq!(
        cookie,
        format!("terminus_session={}", body_text(response).await)
    );
    let returning = app
        .oneshot(
            Request::builder()
                .uri("/read")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_text(returning).await, "extension-value");
}
