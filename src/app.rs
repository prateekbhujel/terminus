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
    pub fn database(self, db: crate::orm::Database) -> Self {
        crate::orm::Database::set_global(db);
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
        self.router = self.router.route(path, method_router);
        self
    }

    /// Register a GET endpoint
    pub fn get<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.spec.add_route(crate::spec::RouteSpec::new("GET", path, path));
        self.router = self.router.route(path, get(handler));
        self
    }

    /// Register a POST endpoint
    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.spec.add_route(crate::spec::RouteSpec::new("POST", path, path));
        self.router = self.router.route(path, post(handler));
        self
    }

    /// Register a PUT endpoint
    pub fn put<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.spec.add_route(crate::spec::RouteSpec::new("PUT", path, path));
        self.router = self.router.route(path, put(handler));
        self
    }

    /// Register a PATCH endpoint
    pub fn patch<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.spec.add_route(crate::spec::RouteSpec::new("PATCH", path, path));
        self.router = self.router.route(path, patch(handler));
        self
    }

    /// Register a DELETE endpoint
    pub fn delete<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.spec.add_route(crate::spec::RouteSpec::new("DELETE", path, path));
        self.router = self.router.route(path, delete(handler));
        self
    }

    /// Mount the Terminus Admin Panel router seamlessly
    pub fn panel(mut self, panel: crate::panel::AdminPanel) -> Self {
        self.router = self.router.merge(panel.into_router());
        self
    }

    /// Merge any nested API sub-router
    pub fn api(mut self, prefix: &str, sub_router: Router) -> Self {
        let clean_prefix = if prefix.starts_with('/') {
            prefix.to_string()
        } else {
            format!("/{}", prefix)
        };
        self.router = self.router.nest(&clean_prefix, sub_router);
        self
    }

    /// Group routes under a common path prefix using a fluent builder
    pub fn group<F>(mut self, prefix: &str, configure: F) -> Self
    where
        F: FnOnce(App) -> App,
    {
        let clean_prefix = if prefix.starts_with('/') {
            prefix.to_string()
        } else {
            format!("/{}", prefix)
        };
        let sub_app = configure(App::new());
        self.router = self.router.nest(&clean_prefix, sub_app.router);
        self
    }

    /// Merge an existing Axum router into this application
    pub fn merge(mut self, router: Router) -> Self {
        self.router = self.router.merge(router);
        self
    }

    /// Serve static files (HTML, CSS, JS, images) from a local directory
    pub fn static_dir(mut self, path: &str, dir: impl AsRef<std::path::Path>) -> Self {
        use tower_http::services::ServeDir;
        let clean_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
        self.router = self.router.nest_service(&clean_path, ServeDir::new(dir));
        self
    }

    /// Expose an OpenAPI 3.0.3 specification JSON endpoint generated by SdkGenerator
    pub fn openapi(mut self, path: &str, generator: &crate::client::SdkGenerator) -> Self {
        let spec_json = generator.to_openapi_json().to_string();
        let clean_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
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
        let slug = format!("/{}", module.slug().trim_start_matches('/'));
        self = self.api(&slug, module.routes());
        if let Some(res) = module.resource() {
            self.spec.add_resource(crate::spec::ResourceSpec::new(res.name(), res.slug(), res.slug()));
        }
        self
    }

    /// Expose an agent-native application introspection endpoint serving AppSpec as JSON
    pub fn spec_endpoint(mut self, path: &str) -> Self {
        let spec_json = self.spec.to_json_pretty();
        let clean_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
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

    /// Enable or disable CORS headers for mobile clients and frontend apps
    pub fn cors(mut self, enabled: bool) -> Self {
        self.cors_enabled = enabled;
        self
    }

    /// Finalize and build the Axum Router
    pub fn build(self) -> Router {
        let mut r = self.router;
        if self.cors_enabled {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            r = r.layer(cors);
        }
        r
    }

    /// Start the server and listen on the configured interface with graceful shutdown
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let effective_port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(self.port);
        let addr: SocketAddr = format!("{}:{}", self.host, effective_port).parse()?;
        println!("  Terminus Application: {}", self.name);
        println!("  Listening on: http://{}", addr);

        let router = self.build();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }
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
