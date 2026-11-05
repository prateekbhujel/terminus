use std::collections::HashMap;
use std::sync::Arc;
use axum::{
    extract::{Form, Path, Query, State},
    http::header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE, SET_COOKIE},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use crate::auth;
use crate::domain::audit::AuditLog;
use crate::domain::user::User;
use crate::repository::{AuditRepository, UserRepository};
use crate::resource::{DynResource, QueryState, Resource};
use crate::view::{dashboard_view, dialogs, form_page, layout, table_view};

type CredentialVerifier = dyn Fn(&str, &str) -> Option<User> + Send + Sync;

#[derive(Clone, Default)]
pub struct AdminPanel {
    resources: Vec<DynResource>,
    resource_map: HashMap<String, DynResource>,
    user_repo: Option<Arc<dyn UserRepository>>,
    credential_verifier: Option<Arc<CredentialVerifier>>,
    audit_repo: Option<Arc<dyn AuditRepository>>,
    theme: crate::theme::ThemeConfig,
    queue: Arc<crate::queue::JobQueue>,
    mailer: Arc<crate::mail::Mailer>,
    broadcaster: Arc<crate::ws::Broadcaster>,
    token_store: Option<Arc<dyn crate::token::TokenRepository>>,
    modules: Vec<Arc<dyn crate::module::AppModule>>,
}

#[derive(Debug, Deserialize)]
pub struct TableQuery {
    pub page: Option<usize>,
    pub search: Option<String>,
    #[serde(alias = "sort_by")]
    pub sort_by: Option<String>,
    #[serde(alias = "sortBy")]
    pub sort_by_camel: Option<String>,
    #[serde(alias = "sort_desc")]
    pub sort_desc: Option<String>,
    #[serde(alias = "sortDesc")]
    pub sort_desc_camel: Option<String>,
    pub flash: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub email: Option<String>,
    pub password: Option<String>,
    #[serde(alias = "csrf_token")]
    pub _csrf_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
    pub ids: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkActionForm {
    pub ids: Option<String>,
}

impl AdminPanel {
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            resource_map: HashMap::new(),
            user_repo: None,
            credential_verifier: None,
            audit_repo: None,
            theme: crate::theme::ThemeConfig::default(),
            queue: Arc::new(crate::queue::JobQueue::new()),
            mailer: Arc::new(crate::mail::Mailer::new()),
            broadcaster: Arc::new(crate::ws::Broadcaster::default()),
            token_store: None,
            modules: Vec::new(),
        }
    }

    pub fn register<R: Resource + 'static>(mut self, resource: R) -> Self {
        let arc_res: DynResource = Arc::new(resource);
        self.resource_map.insert(arc_res.slug().to_string(), arc_res.clone());
        self.resources.push(arc_res);
        self
    }

    pub fn resource<R: Resource + 'static>(self, resource: R) -> Self {
        self.register(resource)
    }

    pub fn users(mut self, repo: Arc<dyn UserRepository>) -> Self {
        self.user_repo = Some(repo);
        self
    }

    /// Configure application-owned credential verification. Without a verifier, login is denied.
    /// The callback must verify the password and return an existing user. Only active users
    /// with nonempty IDs are accepted. When `users` is set, that repository remains authoritative.
    pub fn authenticate_with<F>(mut self, verifier: F) -> Self
    where
        F: Fn(&str, &str) -> Option<User> + Send + Sync + 'static,
    {
        self.credential_verifier = Some(Arc::new(verifier));
        self
    }

    pub fn audit(mut self, repo: Arc<dyn AuditRepository>) -> Self {
        self.audit_repo = Some(repo);
        self
    }

    pub fn theme(mut self, theme: crate::theme::ThemeConfig) -> Self {
        self.theme = theme;
        self
    }

    pub fn brand_name(mut self, name: impl Into<String>) -> Self {
        self.theme.brand_name = name.into();
        self
    }

    pub fn brand_logo(mut self, logo: impl Into<String>) -> Self {
        self.theme.brand_logo = Some(logo.into());
        self
    }

    pub fn primary_color(mut self, color: crate::theme::PrimaryColor) -> Self {
        self.theme.primary_color = color;
        self
    }

    pub fn font_family(mut self, font: crate::theme::FontFamily) -> Self {
        self.theme.font_family = font;
        self
    }

    pub fn queue(mut self, queue: Arc<crate::queue::JobQueue>) -> Self {
        self.queue = queue;
        self
    }

    pub fn mailer(mut self, mailer: Arc<crate::mail::Mailer>) -> Self {
        self.mailer = mailer;
        self
    }

    pub fn get_theme(&self) -> &crate::theme::ThemeConfig {
        &self.theme
    }

    pub fn get_queue(&self) -> Arc<crate::queue::JobQueue> {
        self.queue.clone()
    }

    pub fn get_mailer(&self) -> Arc<crate::mail::Mailer> {
        self.mailer.clone()
    }

    pub fn broadcaster(mut self, broadcaster: Arc<crate::ws::Broadcaster>) -> Self {
        self.broadcaster = broadcaster;
        self
    }

    pub fn get_broadcaster(&self) -> Arc<crate::ws::Broadcaster> {
        self.broadcaster.clone()
    }

    pub fn tokens(mut self, store: Arc<dyn crate::token::TokenRepository>) -> Self {
        self.token_store = Some(store);
        self
    }

    pub fn get_token_store(&self) -> Option<Arc<dyn crate::token::TokenRepository>> {
        self.token_store.clone()
    }

    pub fn module<M: crate::module::AppModule + 'static>(mut self, module: M) -> Self {
        let arc_mod: Arc<dyn crate::module::AppModule> = Arc::new(module);
        if let Some(res) = arc_mod.resource() {
            self.resource_map.insert(res.slug().to_string(), res.clone());
            self.resources.push(res);
        }
        self.modules.push(arc_mod);
        self
    }

    pub fn get_modules(&self) -> Vec<Arc<dyn crate::module::AppModule>> {
        self.modules.clone()
    }

    /// Broadcast an event across all WebSocket clients
    pub fn broadcast(&self, channel: &str, event: &str, data: serde_json::Value) -> usize {
        self.broadcaster.broadcast(channel, event, data)
    }

    pub fn into_router(self) -> Router {
        let shared_panel = Arc::new(self);

        let mut router = Router::new()
            .route("/ws", get({
                let broadcaster = shared_panel.get_broadcaster();
                move |ws| crate::ws::ws_handler(ws, broadcaster)
            }))
            .route("/login", get(login_page).post({
                let panel = shared_panel.clone();
                move |form| login_submit(panel, form)
            }))
            .route("/logout", post(logout_handler))
            .route("/", get({
                let panel = shared_panel.clone();
                move |headers| root_redirect(panel, headers)
            }))
            .route("/dashboard", get({
                let panel = shared_panel.clone();
                move |headers| dashboard_page(panel, headers)
            }))
            .route("/:slug", get({
                let panel = shared_panel.clone();
                move |headers, path, query| resource_page(panel, headers, path, query)
            }))
            .route("/:slug/table", get({
                let panel = shared_panel.clone();
                move |headers, path, query| resource_table_partial(panel, headers, path, query)
            }))
            .route("/:slug/api", get({
                let panel = shared_panel.clone();
                move |headers, path, query| resource_json_api(panel, headers, path, query)
            }))
            .route("/:slug/create", get({
                let panel = shared_panel.clone();
                move |headers, path| resource_create_page(panel, headers, path)
            }).post({
                let panel = shared_panel.clone();
                move |headers, path, form| resource_create(panel, headers, path, form)
            }))
            .route("/:slug/edit/:id", get({
                let panel = shared_panel.clone();
                move |headers, path| resource_edit_page(panel, headers, path)
            }).post({
                let panel = shared_panel.clone();
                move |headers, path, form| resource_update(panel, headers, path, form)
            }))
            .route("/:slug/delete/:id", post({
                let panel = shared_panel.clone();
                move |headers, path| resource_delete(panel, headers, path)
            }))
            .route("/:slug/export", get({
                let panel = shared_panel.clone();
                move |headers, path, query| resource_export(panel, headers, path, query)
            }))
            .route("/:slug/action/:action_id/:id", post({
                let panel = shared_panel.clone();
                move |headers, path| resource_action(panel, headers, path)
            }))
            .route("/:slug/bulk-action/:action_id", post({
                let panel = shared_panel.clone();
                move |headers, path, form| resource_bulk_action(panel, headers, path, form)
            }));

        for m in &shared_panel.modules {
            let slug = m.slug();
            let mod_routes = m.routes();
            router = router.nest(&format!("/api/{}", slug), mod_routes);
        }

        if let Some(ref store) = shared_panel.token_store {
            router = router.layer(axum::Extension(store.clone()));
        }

        let guarded = router.layer(axum::middleware::from_fn_with_state(shared_panel, admin_guard));
        crate::app::App::new().cors(false).merge(guarded).build()
    }
}

const AUTH_USER_KEY: &str = "_terminus_auth_user";

fn get_authenticated_user(panel: &AdminPanel, headers: &HeaderMap) -> Option<User> {
    let user = resolve_authenticated_user(panel, headers);
    crate::helpers::set_current_user(user.clone());
    user
}

fn active_user(panel: &AdminPanel, user: User) -> Option<User> {
    let user = if let Some(repo) = &panel.user_repo {
        repo.find_by_id(&user.id)?
    } else {
        user
    };
    (user.status.is_active() && !user.id.is_empty()).then_some(user)
}

fn bearer_user(panel: &AdminPanel, headers: &HeaderMap) -> Option<User> {
    let bearer = headers.get(AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")?;
    let store = panel.token_store.as_ref()?;
    let token = store.find_token(bearer.trim())?;
    let user = panel.user_repo.as_ref()?.find_by_id(&token.tokenable_id)?;
    let user = active_user(panel, user)?;
    let _ = store.touch_last_used(&token.id);
    Some(user)
}

fn resolve_authenticated_user(panel: &AdminPanel, headers: &HeaderMap) -> Option<User> {
    if headers.contains_key(AUTHORIZATION) {
        return bearer_user(panel, headers);
    }
    let serialized = crate::helpers::session().get(AUTH_USER_KEY)?;
    let user = serde_json::from_str::<User>(&serialized).ok()?;
    active_user(panel, user)
}

async fn admin_guard(
    State(panel): State<Arc<AdminPanel>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let is_login = request.uri().path() == "/login";
    let user = if is_login { None } else { get_authenticated_user(&panel, request.headers()) };
    if !is_login && user.is_none() {
        if request.headers().contains_key(AUTHORIZATION) || !matches!(*request.method(), axum::http::Method::GET | axum::http::Method::HEAD) {
            return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
        }
        return Redirect::to("/admin/login").into_response();
    }
    let mut request = request;
    let unsafe_method = !matches!(*request.method(), axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS);
    let bearer_authenticated = !is_login && request.headers().contains_key(AUTHORIZATION) && user.is_some();
    if unsafe_method && !bearer_authenticated {
        let header_token = request.headers().get("x-csrf-token").and_then(|value| value.to_str().ok()).map(str::to_string);
        let (parts, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, 2 * 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "Request body exceeds the limit").into_response(),
        };
        let form_token = if header_token.is_none() && parts.headers.get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.split(';').next().unwrap_or("").trim() == "application/x-www-form-urlencoded") {
            serde_urlencoded::from_bytes::<HashMap<String, String>>(&bytes).ok()
                .and_then(|mut values| values.remove("_csrf_token").or_else(|| values.remove("csrf_token")))
        } else { None };
        let valid = header_token.or(form_token)
            .is_some_and(|token| crate::helpers::session().verify_csrf_token(&token));
        if !valid {
            return (StatusCode::FORBIDDEN, "Invalid CSRF token").into_response();
        }
        request = axum::extract::Request::from_parts(parts, axum::body::Body::from(bytes));
    }
    next.run(request).await
}

async fn login_page() -> Response {
    Html(auth::render_login_page(None)).into_response()
}

async fn login_submit(panel: Arc<AdminPanel>, Form(form): Form<LoginForm>) -> Response {
    let email = form.email.unwrap_or_default().trim().to_string();
    let password = form.password.unwrap_or_default();
    if email.is_empty() || password.is_empty() {
        return (StatusCode::UNAUTHORIZED, Html(auth::render_login_page(Some("Invalid email or password.")))).into_response();
    }
    let Some(verifier) = panel.credential_verifier.clone() else {
        return (StatusCode::UNAUTHORIZED, Html(auth::render_login_page(Some("Sign-in is not configured.")))).into_response();
    };
    let user = match tokio::task::spawn_blocking(move || verifier(&email, &password)).await {
        Ok(Some(user)) => active_user(&panel, user),
        Ok(None) => None,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Unable to verify credentials").into_response(),
    };
    let Some(user) = user else {
        return (StatusCode::UNAUTHORIZED, Html(auth::render_login_page(Some("Invalid email or password.")))).into_response();
    };
    let previous = crate::helpers::session();
    previous.invalidate();
    if crate::helpers::session_store().remove(previous.id()).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Unable to rotate session").into_response();
    }
    let session = crate::session::Session::new();
    let serialized = match serde_json::to_string(&user) {
        Ok(serialized) => serialized,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Unable to start session").into_response(),
    };
    session.set(AUTH_USER_KEY, serialized);
    crate::helpers::set_current_session(Some(session.clone()));
    crate::helpers::set_current_user(Some(user));
    let mut response = Redirect::to("/admin").into_response();
    if let Ok(cookie) = crate::helpers::session_cookie_header(&session).parse() {
        response.headers_mut().insert(SET_COOKIE, cookie);
    }
    response
}

async fn logout_handler() -> Response {
    let session = crate::helpers::session();
    session.invalidate();
    if crate::helpers::session_store().remove(session.id()).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Unable to revoke session").into_response();
    }
    crate::helpers::set_current_user(None);
    let expired = crate::helpers::clear_session_cookie_header();
    let mut response = Redirect::to("/admin/login").into_response();
    if let Ok(cookie) = expired.parse() {
        response.headers_mut().insert(SET_COOKIE, cookie);
    }
    response
}

async fn root_redirect(panel: Arc<AdminPanel>, headers: HeaderMap) -> Response {
    let _user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    Redirect::to("/admin/dashboard").into_response()
}

async fn dashboard_page(panel: Arc<AdminPanel>, headers: HeaderMap) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let empty_query = QueryState {
        page: 1,
        per_page: 500,
        search: String::new(),
        sort_by: None,
        sort_desc: false,
        filters: HashMap::new(),
    };

    let (total_orders, gross_rev_str) = if let Some(order_res) = panel.resource_map.get("orders") {
        let (rows, total) = order_res.fetch_rows(&empty_query);
        let mut total_cents: i64 = 0;
        for row in &rows {
            if let Some(val) = row.get("amount") {
                let clean = val.replace(['$', ','], "").trim().to_string();
                if let Ok(f) = clean.parse::<f64>() {
                    total_cents += (f * 100.0) as i64;
                }
            }
        }
        let formatted = format!("${:.2}", (total_cents as f64) / 100.0);
        (total, formatted)
    } else {
        (0, "$0.00".to_string())
    };

    let total_users = if let Some(user_res) = panel.resource_map.get("users") {
        let (_, total) = user_res.fetch_rows(&empty_query);
        total
    } else if let Some(ref repo) = panel.user_repo {
        repo.list(&empty_query).1
    } else {
        1
    };

    let recent_audits = if let Some(ref repo) = panel.audit_repo {
        repo.list(&empty_query).0
    } else {
        Vec::new()
    };

    let dashboard_content = dashboard_view::render_dashboard(
        &user,
        total_orders,
        &gross_rev_str,
        total_users,
        &recent_audits,
    );

    let full_html = layout::render_page(
        "Executive Dashboard",
        "dashboard",
        &panel.resources,
        &dashboard_content,
        "",
        None,
        &user,
        &panel.theme,
    );

    Html(full_html).into_response()
}

async fn resource_create_page(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_create(&user) {
        let forbidden_body = layout::render_forbidden_page(&user, res.name());
        let full_html = layout::render_page(
            "Access Restricted",
            &slug,
            &panel.resources,
            &forbidden_body,
            "",
            None,
            &user,
            &panel.theme,
        );
        return (StatusCode::FORBIDDEN, Html(full_html)).into_response();
    }

    let form_page_content = form_page::render_form_page(res.as_ref(), None, None);
    let title = format!("Create {}", res.name());
    let full_html = layout::render_page(
        &title,
        &slug,
        &panel.resources,
        &form_page_content,
        "",
        None,
        &user,
        &panel.theme,
    );
    Html(full_html).into_response()
}

async fn resource_edit_page(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path((slug, id)): Path<(String, String)>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_edit(&user) {
        let forbidden_body = layout::render_forbidden_page(&user, res.name());
        let full_html = layout::render_page(
            "Access Restricted",
            &slug,
            &panel.resources,
            &forbidden_body,
            "",
            None,
            &user,
            &panel.theme,
        );
        return (StatusCode::FORBIDDEN, Html(full_html)).into_response();
    }

    let row_data = res.get_row(&id);
    if let Some(ref r) = row_data {
        if !res.can_edit_row(&user, r) {
            let forbidden_body = layout::render_forbidden_page(&user, res.name());
            let full_html = layout::render_page(
                "Access Restricted",
                &slug,
                &panel.resources,
                &forbidden_body,
                "",
                None,
                &user,
                &panel.theme,
            );
            return (StatusCode::FORBIDDEN, Html(full_html)).into_response();
        }
    }
    let values_map = row_data.map(|r| r.values);

    let form_page_content = form_page::render_form_page(res.as_ref(), Some(&id), values_map.as_ref());
    let title = format!("Edit {} #{}", res.name(), id);
    let full_html = layout::render_page(
        &title,
        &slug,
        &panel.resources,
        &form_page_content,
        "",
        None,
        &user,
        &panel.theme,
    );
    Html(full_html).into_response()
}

async fn resource_page(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(query_params): Query<TableQuery>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_view(&user) {
        let forbidden_body = layout::render_forbidden_page(&user, res.name());
        let full_html = layout::render_page(
            "Access Restricted",
            &slug,
            &panel.resources,
            &forbidden_body,
            "",
            None,
            &user,
            &panel.theme,
        );
        return (StatusCode::FORBIDDEN, Html(full_html)).into_response();
    }

    let flash = query_params.flash.clone();
    let query_state = parse_query(query_params);
    let table_html = table_view::render_table_partial(res.as_ref(), &query_state, &user);
    let dialogs_html = dialogs::render_dialogs(res.as_ref());
    let wrapped_content = format!(r#"<div id="table-wrapper">{table_html}</div>"#);

    let full_html = layout::render_page(
        res.plural_name(),
        &slug,
        &panel.resources,
        &wrapped_content,
        &dialogs_html,
        flash.as_deref(),
        &user,
        &panel.theme,
    );

    Html(full_html).into_response()
}

async fn resource_table_partial(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(query_params): Query<TableQuery>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_view(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    let query_state = parse_query(query_params);
    let table_html = table_view::render_table_partial(res.as_ref(), &query_state, &user);

    Html(table_html).into_response()
}

async fn resource_create(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Form(mut values): Form<HashMap<String, String>>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_create(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    values.remove("_csrf_token");
    values.remove("csrf_token");
    match res.create_row(values) {
        Ok(new_id) => {
            if let Some(ref audit) = panel.audit_repo {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let _ = audit.save(AuditLog {
                    id: String::new(),
                    user_name: user.name.clone(),
                    action: "CREATE".to_string(),
                    resource: slug.clone(),
                    record_id: new_id.clone(),
                    details: format!("Created new {} record #{}", res.name(), new_id),
                    timestamp: now,
                });
            }
            panel.broadcast("admin", "record:created", serde_json::json!({ "slug": &slug, "id": &new_id }));
            panel.broadcast(&slug, "created", serde_json::json!({ "id": &new_id }));
            Redirect::to(&format!("/admin/{slug}?flash=Record+created+successfully")).into_response()
        }
        Err(err) => {
            Redirect::to(&format!("/admin/{slug}?flash=Error:+{}", err.replace(' ', "+"))).into_response()
        }
    }
}

async fn resource_update(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path((slug, id)): Path<(String, String)>,
    Form(mut values): Form<HashMap<String, String>>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_edit(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    if let Some(existing_row) = res.get_row(&id) {
        if !res.can_edit_row(&user, &existing_row) {
            return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
        }
    }

    values.remove("_csrf_token");
    values.remove("csrf_token");
    match res.update_row(&id, values) {
        Ok(()) => {
            if let Some(ref audit) = panel.audit_repo {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let _ = audit.save(AuditLog {
                    id: String::new(),
                    user_name: user.name.clone(),
                    action: "UPDATE".to_string(),
                    resource: slug.clone(),
                    record_id: id.clone(),
                    details: format!("Updated {} record #{}", res.name(), id),
                    timestamp: now,
                });
            }
            panel.broadcast("admin", "record:updated", serde_json::json!({ "slug": &slug, "id": &id }));
            panel.broadcast(&slug, "updated", serde_json::json!({ "id": &id }));
            Redirect::to(&format!("/admin/{slug}?flash=Record+updated+successfully")).into_response()
        }
        Err(err) => {
            Redirect::to(&format!("/admin/{slug}?flash=Error:+{}", err.replace(' ', "+"))).into_response()
        }
    }
}

async fn resource_delete(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path((slug, id)): Path<(String, String)>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_delete(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    if let Some(existing_row) = res.get_row(&id) {
        if !res.can_delete_row(&user, &existing_row) {
            return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
        }
    }

    match res.delete_row(&id) {
        Ok(()) => {
            if let Some(ref audit) = panel.audit_repo {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let _ = audit.save(AuditLog {
                    id: String::new(),
                    user_name: user.name.clone(),
                    action: "DELETE".to_string(),
                    resource: slug.clone(),
                    record_id: id.clone(),
                    details: format!("Deleted {} record #{}", res.name(), id),
                    timestamp: now,
                });
            }
            panel.broadcast("admin", "record:deleted", serde_json::json!({ "slug": &slug, "id": &id }));
            panel.broadcast(&slug, "deleted", serde_json::json!({ "id": &id }));
            Redirect::to(&format!("/admin/{slug}?flash=Record+deleted+successfully")).into_response()
        }
        Err(err) => {
            Redirect::to(&format!("/admin/{slug}?flash=Error:+{}", err.replace(' ', "+"))).into_response()
        }
    }
}

async fn resource_export(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(query): Query<ExportQuery>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_view(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    let id_list: Vec<String> = query.ids
        .map(|s| s.split(',').map(|id| id.trim().to_string()).filter(|id| !id.is_empty()).collect())
        .unwrap_or_default();

    let fmt = query.format.unwrap_or_else(|| "csv".to_string());
    match res.bulk_export(&id_list, &fmt) {
        Ok(data) => {
            let (mime, ext) = if fmt.eq_ignore_ascii_case("json") {
                ("application/json; charset=utf-8", "json")
            } else {
                ("text/csv; charset=utf-8", "csv")
            };
            let filename = format!("{}-export.{}", slug, ext);
            (
                StatusCode::OK,
                [
                    (CONTENT_TYPE, mime),
                    (
                        CONTENT_DISPOSITION,
                        &format!("attachment; filename=\"{}\"", filename),
                    ),
                ],
                data,
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<h1>Export error: {}</h1>", err)),
        )
            .into_response(),
    }
}

async fn resource_action(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path((slug, action_id, id)): Path<(String, String, String)>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_edit(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    match res.handle_action(&action_id, &id) {
        Ok(msg) => {
            panel.broadcast(
                "admin",
                "record:action",
                serde_json::json!({ "slug": &slug, "id": &id, "action": &action_id }),
            );
            panel.broadcast(&slug, "action", serde_json::json!({ "id": &id, "action": &action_id }));
            let flash = if msg.is_empty() {
                format!("Action `{}` completed", action_id)
            } else {
                msg
            };
            Redirect::to(&format!("/admin/{}?flash={}", slug, flash.replace(' ', "+"))).into_response()
        }
        Err(err) => Redirect::to(&format!(
            "/admin/{}?flash=Error:+{}",
            slug,
            err.replace(' ', "+")
        ))
        .into_response(),
    }
}

async fn resource_bulk_action(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path((slug, action_id)): Path<(String, String)>,
    Form(form): Form<BulkActionForm>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return Redirect::to("/admin/login").into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, Html("<h1>404 Resource Not Found</h1>")).into_response();
    };

    if !res.can_edit(&user) {
        return (StatusCode::FORBIDDEN, Html(layout::render_forbidden_page(&user, res.name()))).into_response();
    }

    let id_list: Vec<String> = form.ids
        .map(|s| s.split(',').map(|id| id.trim().to_string()).filter(|id| !id.is_empty()).collect())
        .unwrap_or_default();

    match res.handle_bulk_action(&action_id, &id_list) {
        Ok(msg) => {
            panel.broadcast(
                "admin",
                "record:bulk_action",
                serde_json::json!({ "slug": &slug, "action": &action_id, "count": id_list.len() }),
            );
            panel.broadcast(&slug, "bulk_action", serde_json::json!({ "action": &action_id, "count": id_list.len() }));
            Redirect::to(&format!("/admin/{}?flash={}", slug, msg.replace(' ', "+"))).into_response()
        }
        Err(err) => Redirect::to(&format!(
            "/admin/{}?flash=Error:+{}",
            slug,
            err.replace(' ', "+")
        ))
        .into_response(),
    }
}

async fn resource_json_api(
    panel: Arc<AdminPanel>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(query_params): Query<TableQuery>,
) -> Response {
    let user = match get_authenticated_user(&panel, &headers) {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({ "error": "Unauthorized" }))).into_response(),
    };

    let Some(res) = panel.resource_map.get(&slug) else {
        return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({ "error": "Not Found" }))).into_response();
    };

    if !res.can_view(&user) {
        return (StatusCode::FORBIDDEN, axum::Json(serde_json::json!({ "error": "Forbidden" }))).into_response();
    }

    let query_state = parse_query(query_params);
    let (rows, total) = res.fetch_rows(&query_state);

    let json_rows: Vec<serde_json::Value> = rows.into_iter().map(|r| {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), serde_json::Value::String(r.id));
        for (k, v) in r.values {
            let camel_k = crate::resource::to_camel_case(&k);
            map.insert(camel_k, serde_json::Value::String(v));
        }
        serde_json::Value::Object(map)
    }).collect();

    (StatusCode::OK, axum::Json(serde_json::json!({
        "data": json_rows,
        "total": total,
        "page": query_state.page,
        "perPage": query_state.per_page,
        "sortBy": query_state.sort_by,
        "sortDesc": query_state.sort_desc,
    }))).into_response()
}

fn parse_query(q: TableQuery) -> QueryState {
    let mut filters = HashMap::new();
    for (k, v) in q.extra {
        if let Some(filter_name) = k.strip_prefix("filter_") {
            if !v.is_empty() {
                filters.insert(filter_name.to_string(), v);
            }
        } else if let Some(rest) = k.strip_prefix("filter") {
            if !rest.is_empty() && rest.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                let name = rest.to_lowercase();
                if !v.is_empty() {
                    filters.insert(name, v);
                }
            }
        } else if !k.is_empty() && k != "page" && k != "search" && k != "sort_by" && k != "sortBy" && k != "sort_desc" && k != "sortDesc" && k != "flash"
            && !v.is_empty() {
                filters.insert(k, v);
            }
    }

    let sort_col = q.sort_by_camel.or(q.sort_by).filter(|s| !s.is_empty());
    let sort_desc_val = q.sort_desc_camel.or(q.sort_desc).map(|v| v == "true").unwrap_or(false);

    QueryState {
        page: q.page.unwrap_or(1),
        per_page: 8,
        search: q.search.unwrap_or_default().trim().to_string(),
        sort_by: sort_col,
        sort_desc: sort_desc_val,
        filters,
    }
}
