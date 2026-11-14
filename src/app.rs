//! Terminus Universal Application Engine
//!
//! Provides application bootstrapping, fluent route registration,
//! and multi-platform service orchestration without manual lifetime
//! or concurrency boilerplate.
//!
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, patch, post, put, MethodRouter},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

/// Terminus Application Builder
#[derive(Clone)]
pub struct App {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub router: Router,
    pub cors_enabled: bool,
    pub spec: crate::spec::AppSpec,
    pub spec_endpoint_path: Option<String>,
    pub session_config: Option<crate::session::SessionConfig>,
    pub rate_limiter: Option<crate::ratelimit::RateLimiter>,
    pub metrics_enabled: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Initialize a new Terminus application
    pub fn new() -> Self {
        let default_port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3000);
        let mut spec = crate::spec::AppSpec::new("Terminus Application");
        spec.host = "127.0.0.1".to_string();
        spec.port = default_port;
        Self {
            name: "Terminus Application".to_string(),
            host: "127.0.0.1".to_string(),
            port: default_port,
            router: Router::new(),
            cors_enabled: true,
            spec,
            spec_endpoint_path: None,
            session_config: None,
            rate_limiter: None,
            metrics_enabled: false,
        }
    }

    /// Alias for boot
    pub fn boot() -> Self {
        Self::new()
    }

    /// Set the human-readable application name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self.spec.app_name = self.name.clone();
        self
    }

    /// Set the listening port (default 3000, or from PORT env variable)
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self.spec.port = port;
        self
    }

    /// Preset constructor: Port 3000 (standard web application & admin dashboard)
    pub fn port_3000() -> Self {
        Self::new().port(3000)
    }

    /// Preset constructor: Port 8000 (microservices / alternative backend API)
    pub fn port_8000() -> Self {
        Self::new().port(8000)
    }

    /// Preset constructor: Port 8080 (cloud proxy / enterprise gateway)
    pub fn port_8080() -> Self {
        Self::new().port(8080)
    }

    /// Register the active application database and configure global database access
    pub fn database(mut self, db: crate::orm::Database) -> Self {
        let kind = match db.kind() {
            crate::backend::DatabaseKind::Sqlite => "sqlite",
            crate::backend::DatabaseKind::Postgres => "postgres",
            crate::backend::DatabaseKind::Mysql => "mysql",
        };
        self.spec.origin.database_backend = Some(kind.to_string());
        if self.spec.origin.database_source.is_none() {
            self.spec.origin.database_source = Some("App::database".to_string());
        }
        crate::orm::Database::set_global(db);
        self
    }

    /// Limit each direct socket peer to `max_requests` per `window`;
    /// excess requests receive HTTP 429. Forwarded headers are not trusted.
    /// In-memory requests without a peer share one bucket.
    pub fn rate_limit(mut self, max_requests: usize, window: std::time::Duration) -> Self {
        self.rate_limiter = Some(crate::ratelimit::RateLimiter::new(max_requests, window));
        self
    }

    /// Rate-limit clients behind explicitly trusted reverse proxies.
    ///
    /// The direct TCP peer must match one of `trusted_proxies` before
    /// X-Forwarded-For is consulted. Walk the chain from the right, skipping
    /// known proxy hops; an untrusted client cannot spoof the first hop.
    /// Configure the proxies to append the actual upstream address.
    pub fn rate_limit_with_trusted_proxies(
        mut self,
        max_requests: usize,
        window: std::time::Duration,
        trusted_proxies: &[std::net::IpAddr],
    ) -> Self {
        self.rate_limiter = Some(
            crate::ratelimit::RateLimiter::new(max_requests, window)
                .with_trusted_proxies(trusted_proxies),
        );
        self
    }

    /// Record HTTP metrics and recent requests into the global
    /// `MetricsRegistry` (read with `terminus::metrics::metrics_snapshot`).
    pub fn metrics(mut self, enabled: bool) -> Self {
        self.metrics_enabled = enabled;
        self
    }

    /// Configures signed, expiring sessions with an explicit server-side store.
    pub fn sessions(mut self, config: crate::session::SessionConfig) -> Self {
        self.session_config = Some(config);
        self
    }

    /// Set the host interface (default 127.0.0.1)
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self.spec.host = self.host.clone();
        self
    }

    /// Register a raw Axum method router at a path
    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        let clean = crate::spec::normalize_route_path(path);
        self.spec.add_unavailable(
            "route",
            &clean,
            "Mounted via App::route with opaque axum::routing::MethodRouter; HTTP methods, handler, and auth metadata are not observable.",
        );
        self.router = self.router.route(path, method_router);
        self
    }

    fn record_handler_route<H>(&mut self, method: &str, path: &str) {
        let clean = crate::spec::normalize_route_path(path);
        let handler = short_handler_name(std::any::type_name::<H>());
        self.spec
            .add_route(crate::spec::RouteSpec::new(method, clean, handler));
    }

    /// Register a GET endpoint
    pub fn get<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.record_handler_route::<H>("GET", path);
        self.router = self.router.route(path, get(handler));
        self
    }

    /// Register a POST endpoint
    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.record_handler_route::<H>("POST", path);
        self.router = self.router.route(path, post(handler));
        self
    }

    /// Register a PUT endpoint
    pub fn put<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.record_handler_route::<H>("PUT", path);
        self.router = self.router.route(path, put(handler));
        self
    }

    /// Register a PATCH endpoint
    pub fn patch<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.record_handler_route::<H>("PATCH", path);
        self.router = self.router.route(path, patch(handler));
        self
    }

    /// Register a DELETE endpoint
    pub fn delete<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.record_handler_route::<H>("DELETE", path);
        self.router = self.router.route(path, delete(handler));
        self
    }

    /// Mount the Terminus Admin Panel under `/admin`
    pub fn panel(mut self, panel: crate::panel::AdminPanel) -> Self {
        record_panel_spec(&mut self.spec, &panel);
        self.router = self.router.nest("/admin", panel.into_router());
        self
    }

    /// Merge any nested API sub-router
    pub fn api(mut self, prefix: &str, sub_router: Router) -> Self {
        let clean_prefix = crate::spec::normalize_route_path(prefix);
        self.spec.add_unavailable(
            "router_mount",
            &clean_prefix,
            "Mounted opaque axum::Router via App::api; internal routes are not observable via AppSpec.",
        );
        self.router = if clean_prefix == "/" {
            self.router.merge(sub_router)
        } else {
            self.router.nest(&clean_prefix, sub_router)
        };
        self
    }

    /// Group routes under a common path prefix using a fluent builder
    pub fn group<F>(mut self, prefix: &str, configure: F) -> Self
    where
        F: FnOnce(App) -> App,
    {
        let clean_prefix = crate::spec::normalize_route_path(prefix);
        let sub_app = configure(App::new());
        self.spec.merge_group(&clean_prefix, sub_app.spec);
        self.router = if clean_prefix == "/" {
            self.router.merge(sub_app.router)
        } else {
            self.router.nest(&clean_prefix, sub_app.router)
        };
        self
    }

    /// Merge an existing Axum router into this application
    pub fn merge(mut self, router: Router) -> Self {
        self.spec.add_unavailable(
            "router_mount",
            "/",
            "Merged opaque axum::Router via App::merge; internal routes are not observable via AppSpec.",
        );
        self.router = self.router.merge(router);
        self
    }

    /// Serve static files (HTML, CSS, JS, images) from a local directory
    pub fn static_dir(mut self, path: &str, dir: impl AsRef<std::path::Path>) -> Self {
        use tower_http::services::ServeDir;
        let dir_display = dir.as_ref().display().to_string();
        let clean_path = crate::spec::normalize_route_path(path);
        self.spec.add_route(
            crate::spec::RouteSpec::new(
                "GET",
                crate::spec::join_route_paths(&clean_path, "*path"),
                format!("ServeDir({dir_display})"),
            )
            .public(),
        );
        self.router = self.router.nest_service(&clean_path, ServeDir::new(dir));
        self
    }

    /// Expose an OpenAPI 3.0.3 specification JSON endpoint generated by SdkGenerator
    pub fn openapi(mut self, path: &str, generator: &crate::client::SdkGenerator) -> Self {
        let spec_json = generator.to_openapi_json().to_string();
        let clean_path = crate::spec::normalize_route_path(path);
        self.spec.add_route(
            crate::spec::RouteSpec::new("GET", &clean_path, "terminus::app::openapi").public(),
        );
        self.router = self.router.route(
            &clean_path,
            get(move || {
                let body = spec_json.clone();
                async move {
                    (
                        StatusCode::OK,
                        [("Content-Type", "application/json; charset=utf-8")],
                        body,
                    )
                }
            }),
        );
        self
    }

    /// Retrieve a reference to the machine-readable application specification
    pub fn spec(&self) -> &crate::spec::AppSpec {
        &self.spec
    }

    /// Retrieve a mutable reference to the machine-readable application specification
    pub fn spec_mut(&mut self) -> &mut crate::spec::AppSpec {
        &mut self.spec
    }

    /// Register a domain model into the application specification
    pub fn model(mut self, model: crate::spec::ModelSpec) -> Self {
        self.spec.add_model(model);
        self
    }

    /// Register an administrative resource specification
    pub fn resource_spec(mut self, res: crate::spec::ResourceSpec) -> Self {
        self.spec.add_resource(res);
        self
    }

    /// Mount a modular domain package (AppModule) with routes and admin resources
    pub fn module<M: crate::module::AppModule + 'static>(mut self, module: M) -> Self {
        let slug = crate::spec::normalize_route_path(module.slug());
        let route_specs = module.route_specs();
        if route_specs.is_empty() {
            self.spec.add_unavailable(
                "module_routes",
                &slug,
                format!(
                    "AppModule '{}' mounted an opaque axum::Router without overriding route_specs()",
                    module.name()
                ),
            );
        } else {
            for mut r in route_specs {
                r.path = crate::spec::join_route_paths(&slug, &r.path);
                self.spec.add_route(r);
            }
        }
        if let Some(res) = module.resource() {
            self.spec.add_resource(resource_spec_from_resource(
                res.as_ref(),
                res.slug(),
            ));
        }
        self.router = if slug == "/" {
            self.router.merge(module.routes())
        } else {
            self.router.nest(&slug, module.routes())
        };
        self
    }

    /// Expose an agent-native application introspection endpoint serving AppSpec as JSON
    pub fn spec_endpoint(mut self, path: &str) -> Self {
        let clean_path = crate::spec::normalize_route_path(path);
        self.spec.add_route(
            crate::spec::RouteSpec::new("GET", &clean_path, "terminus::app::spec_endpoint")
                .public(),
        );
        self.spec_endpoint_path = Some(clean_path);
        self
    }

    /// Enable or disable CORS headers for mobile clients and frontend apps
    pub fn cors(mut self, enabled: bool) -> Self {
        self.cors_enabled = enabled;
        self
    }

    /// Return a cloned `AppSpec` annotated as an application export.
    pub fn export_spec(&self) -> crate::spec::AppSpec {
        let mut export = self.spec.clone();
        export.origin.kind = crate::spec::OriginKind::ApplicationExport;
        export.origin.authoritative_app_contract = true;
        export
    }

    /// Write the application's authoritative `AppSpec` manifest to `path`.
    pub fn write_spec(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        self.spec.write_to_path(path)
    }

    /// Finalize the Axum Router with request-scoped helpers.
    /// Sessions are created lazily and receive a cookie when accessed.
    pub fn build(self) -> Router {
        let mut r = self.router;
        if let Some(ref spec_path) = self.spec_endpoint_path {
            let spec_json = self.spec.to_json_pretty();
            r = r.route(
                spec_path,
                get(move || async move {
                    (
                        StatusCode::OK,
                        [("Content-Type", "application/json; charset=utf-8")],
                        spec_json,
                    )
                }),
            );
        }
        if self.cors_enabled {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            r = r.layer(cors);
        }
        if self.metrics_enabled {
            r = r.layer(axum::middleware::from_fn(metrics_middleware));
        }
        if let Some(limiter) = self.rate_limiter {
            r = r.layer(axum::middleware::from_fn_with_state(
                limiter,
                rate_limit_middleware,
            ));
        }
        r.layer(axum::middleware::from_fn_with_state(self.session_config, request_context))
    }

    /// Start the server and listen on the configured interface with graceful shutdown.
    ///
    /// If `TERMINUS_EXPORT_SPEC` or `--export-spec [PATH]` is passed, writes the application's
    /// authoritative `AppSpec` manifest (or prints JSON to stdout when set to `"-"`) and exits
    /// cleanly without binding a listening socket.
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(export_target) = spec_export_target_from_process() {
            if export_target == "-" {
                println!("{}", self.export_spec().to_json_pretty());
            } else {
                self.write_spec(&export_target)?;
                eprintln!("Exported Terminus AppSpec manifest to {}", export_target);
            }
            return Ok(());
        }

        let effective_port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(self.port);
        let addr: SocketAddr = format!("{}:{}", self.host, effective_port).parse()?;
        println!("  Terminus Application: {}", self.name);
        println!("  Listening on: http://{}", addr);

        let router = self.build();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        // Preserve the real TCP peer for proxy-aware rate limiting.
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }
}

fn spec_export_target_from_process() -> Option<String> {
    if let Ok(path) = std::env::var("TERMINUS_EXPORT_SPEC") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let args: Vec<String> = std::env::args().collect();
    for (idx, arg) in args.iter().enumerate() {
        if arg == "--export-spec" {
            let next = args
                .get(idx + 1)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && !s.starts_with('-'))
                .unwrap_or(crate::spec::DEFAULT_SPEC_PATH);
            return Some(next.to_string());
        }
        if let Some(val) = arg.strip_prefix("--export-spec=") {
            let trimmed = val.trim();
            let target = if trimmed.is_empty() {
                crate::spec::DEFAULT_SPEC_PATH
            } else {
                trimmed
            };
            return Some(target.to_string());
        }
    }
    None
}

async fn request_context(
    axum::extract::State(session_config): axum::extract::State<Option<crate::session::SessionConfig>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if crate::helpers::has_request_context() { return next.run(request).await; }
    let cookie = request.headers().get(axum::http::header::COOKIE).and_then(|value| value.to_str().ok());
    if let Some(config) = &session_config {
        if config.cookie_id(cookie).is_err() {
            return (StatusCode::BAD_REQUEST, "Invalid session cookie").into_response();
        }
    } else if let Some(key) = crate::helpers::config("app.key").or_else(|| crate::helpers::env("APP_KEY")) {
        if let Some(value) = crate::session::session_cookie_value(cookie) {
            if crate::session::Session::verify_signed_cookie(value, key.as_bytes()).is_none() {
                return (StatusCode::BAD_REQUEST, "Invalid session cookie").into_response();
            }
        }
    }
    let info = crate::helpers::RequestInfo::from_headers(
        request.method().as_str(), request.uri().path(), request.headers(),
    );
    let (mut response, session, error) = crate::helpers::with_request_context(info, session_config.clone(), next.run(request)).await;
    if let Some(error) = error {
        tracing::error!(error = %error, "Session lookup failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Session storage unavailable").into_response();
    }
    let has_session_cookie = response.headers().get_all(axum::http::header::SET_COOKIE).iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.trim_start().starts_with("terminus_session="));
    if let Some(session) = session {
        if session.is_invalidated() { return response; }
        let store = session_config.as_ref().map(|config| config.store.clone())
            .unwrap_or_else(crate::session::SessionStore::global_arc);
        let result = match &session_config {
            Some(config) => store.save_with_ttl(&session, config.ttl_seconds),
            None => store.save(&session),
        };
        if let Err(error) = result {
            tracing::error!(error = %error, "Session persistence failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Session storage unavailable").into_response();
        }
        if !has_session_cookie {
            let cookie = session_config.as_ref().map(|config| config.cookie_header(&session)).unwrap_or_else(|| {
                if let Some(key) = crate::helpers::config("app.key").or_else(|| crate::helpers::env("APP_KEY")) {
                    session.signed_cookie_header(key.as_bytes())
                } else { session.cookie_header() }
            });
            if let Ok(cookie) = cookie.parse() {
                response.headers_mut().append(axum::http::header::SET_COOKIE, cookie);
            }
        }
    }
    response
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    println!("\n  Terminus application shutting down gracefully...");
}

/// JSON HTTP response helper for ergonomic response construction
pub fn json_response<T: serde::Serialize>(data: T) -> Response {
    match serde_json::to_string(&data) {
        Ok(body) => (
            StatusCode::OK,
            [("Content-Type", "application/json; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Serialization error: {}", err),
        )
            .into_response(),
    }
}

/// HTML HTTP response helper
pub fn html_response(html: impl Into<String>) -> Html<String> {
    Html(html.into())
}

/// Redirect helper
pub fn redirect_to(url: &str) -> Redirect {
    Redirect::to(url)
}

async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<crate::ratelimit::RateLimiter>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let key = limiter.client_key(peer, request.headers());
    if limiter.hit(&key) {
        next.run(request).await
    } else {
        (axum::http::StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response()
    }
}

async fn metrics_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let started = std::time::Instant::now();
    let response = next.run(request).await;
    crate::metrics::MetricsRegistry::global().record(crate::metrics::RequestRecord {
        method,
        path,
        status: response.status().as_u16(),
        duration_ms: started.elapsed().as_secs_f64() * 1000.0,
        at_unix: chrono::Utc::now().timestamp(),
    });
    response
}

fn short_handler_name(raw: &str) -> String {
    raw.split("::")
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>()
        .join("::")
}

fn resource_spec_from_resource(
    res: &dyn crate::resource::Resource,
    table_name: &str,
) -> crate::spec::ResourceSpec {
    let table = res.table();
    let searchable: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.searchable)
        .map(|c| c.name.as_str())
        .collect();
    let sortable: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.sortable)
        .map(|c| c.name.as_str())
        .collect();
    crate::spec::ResourceSpec::new(res.name(), res.slug(), table_name)
        .with_searchable(&searchable)
        .with_sortable(&sortable)
}

fn record_panel_spec(spec: &mut crate::spec::AppSpec, panel: &crate::panel::AdminPanel) {
    const PANEL_ROUTES: &[(&str, &str, &str, bool)] = &[
        ("GET", "/admin/login", "panel::login_page", false),
        ("POST", "/admin/login", "panel::login_submit", false),
        ("POST", "/admin/logout", "panel::logout_handler", true),
        ("GET", "/admin", "panel::root_redirect", true),
        ("GET", "/admin/dashboard", "panel::dashboard_page", true),
        ("GET", "/admin/ws", "panel::ws_handler", true),
        ("GET", "/admin/:slug", "panel::resource_page", true),
        ("GET", "/admin/:slug/table", "panel::resource_table_partial", true),
        ("GET", "/admin/:slug/api", "panel::resource_json_api", true),
        ("GET", "/admin/:slug/create", "panel::resource_create_page", true),
        ("POST", "/admin/:slug/create", "panel::resource_create", true),
        ("GET", "/admin/:slug/edit/:id", "panel::resource_edit_page", true),
        ("POST", "/admin/:slug/edit/:id", "panel::resource_update", true),
        ("POST", "/admin/:slug/delete/:id", "panel::resource_delete", true),
        ("GET", "/admin/:slug/export", "panel::resource_export", true),
        ("POST", "/admin/:slug/action/:action_id/:id", "panel::resource_action", true),
        ("POST", "/admin/:slug/bulk-action/:action_id", "panel::resource_bulk_action", true),
    ];

    for (method, path, handler, auth_required) in PANEL_ROUTES {
        spec.add_route(
            crate::spec::RouteSpec::new(*method, *path, *handler).with_auth(*auth_required),
        );
    }

    for res in panel.resources() {
        spec.add_resource(resource_spec_from_resource(res.as_ref(), res.slug()));
    }

    for module in panel.modules() {
        let prefix = crate::spec::join_route_paths("/admin/api", module.slug());
        let routes = module.route_specs();
        if routes.is_empty() {
            spec.add_unavailable(
                "module_routes",
                &prefix,
                format!(
                    "AdminPanel module '{}' mounted an opaque axum::Router without overriding route_specs()",
                    module.name()
                ),
            );
        } else {
            for mut r in routes {
                r.path = crate::spec::join_route_paths(&prefix, &r.path);
                if matches!(r.auth, crate::spec::RouteAuth::Unknown) {
                    r.auth = crate::spec::RouteAuth::Required;
                }
                spec.add_route(r);
            }
        }
    }
}
