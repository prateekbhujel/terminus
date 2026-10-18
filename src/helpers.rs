//! Global application helpers for Terminus.
//! Provides zero-ceremony access to current authenticated user (`auth()`, `user()`),
//! authorization checks (`can()`, `cannot()`), request inspection (`request()`),
//! control flow (`abort()`, `abort_if()`, `abort_unless()`), and system utilities.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use chrono::{DateTime, Utc};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use crate::domain::User;
use crate::session::{Session, SessionStore};

tokio::task_local! {
    pub static TASK_USER: Option<User>;
    pub static TASK_REQUEST: Option<RequestInfo>;
    pub static TASK_SESSION: Option<Session>;
}

thread_local! {
    static THREAD_USER: RefCell<Option<User>> = const { RefCell::new(None) };
    static THREAD_REQUEST: RefCell<Option<RequestInfo>> = const { RefCell::new(None) };
    static THREAD_SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

static GLOBAL_USER: OnceLock<RwLock<Option<User>>> = OnceLock::new();
fn global_user() -> &'static RwLock<Option<User>> {
    GLOBAL_USER.get_or_init(|| RwLock::new(None))
}

static GLOBAL_REQUEST: OnceLock<RwLock<Option<RequestInfo>>> = OnceLock::new();
fn global_request() -> &'static RwLock<Option<RequestInfo>> {
    GLOBAL_REQUEST.get_or_init(|| RwLock::new(None))
}

static GLOBAL_SESSION: OnceLock<RwLock<Option<Session>>> = OnceLock::new();
fn global_session() -> &'static RwLock<Option<Session>> {
    GLOBAL_SESSION.get_or_init(|| RwLock::new(None))
}

static CONFIG_STORE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
fn config_store() -> &'static RwLock<HashMap<String, String>> {
    CONFIG_STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Request metadata envelope providing direct inspection of HTTP request parameters.
#[derive(Debug, Clone, Default)]
pub struct RequestInfo {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub ip: String,
    pub body_json: Option<serde_json::Value>,
}

impl RequestInfo {
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            query: HashMap::new(),
            headers: HashMap::new(),
            ip: "127.0.0.1".to_string(),
            body_json: None,
        }
    }

    pub fn from_headers(method: &str, path: &str, headers: &axum::http::HeaderMap) -> Self {
        let mut req = Self::new(method, path);
        for (k, v) in headers {
            if let Ok(val) = v.to_str() {
                req.headers.insert(k.as_str().to_lowercase(), val.to_string());
            }
        }
        if let Some(ip) = req.header("x-forwarded-for").or_else(|| req.header("x-real-ip")) {
            req.ip = ip.to_string();
        }
        req
    }

    pub fn with_query(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.query.insert(key.into(), val.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.headers.insert(key.into().to_lowercase(), val.into());
        self
    }

    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip = ip.into();
        self
    }

    pub fn input(&self, key: &str) -> Option<&str> {
        self.query.get(key)
            .or_else(|| self.query.get(&crate::resource::to_camel_case(key)))
            .or_else(|| self.query.get(&crate::resource::to_snake_case(key)))
            .map(|s| s.as_str())
    }

    pub fn has(&self, key: &str) -> bool {
        self.input(key).is_some()
    }

    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(&key.to_lowercase()).map(|s| s.as_str())
    }

    pub fn is_json(&self) -> bool {
        self.header("content-type").map(|c| c.contains("application/json")).unwrap_or(false)
    }

    pub fn wants_json(&self) -> bool {
        self.header("accept").map(|a| a.contains("application/json")).unwrap_or(false)
    }

    pub fn is_ajax(&self) -> bool {
        self.header("x-requested-with").map(|h| h.eq_ignore_ascii_case("xmlhttprequest")).unwrap_or(false)
    }
}

/// Fluent accessor for the current HTTP request.
#[derive(Debug, Clone, Default)]
pub struct RequestAccessor;

impl RequestAccessor {
    pub fn info(&self) -> Option<RequestInfo> {
        request_info()
    }

    pub fn input(&self, key: &str) -> Option<String> {
        self.info().and_then(|r| r.input(key).map(|s| s.to_string()))
    }

    pub fn has(&self, key: &str) -> bool {
        self.info().map(|r| r.has(key)).unwrap_or(false)
    }

    pub fn path(&self) -> String {
        self.info().map(|r| r.path).unwrap_or_default()
    }

    pub fn method(&self) -> String {
        self.info().map(|r| r.method).unwrap_or_else(|| "GET".to_string())
    }

    pub fn ip(&self) -> String {
        self.info().map(|r| r.ip).unwrap_or_else(|| "127.0.0.1".to_string())
    }

    pub fn header(&self, key: &str) -> Option<String> {
        self.info().and_then(|r| r.header(key).map(|s| s.to_string()))
    }

    pub fn is_json(&self) -> bool {
        self.info().map(|r| r.is_json()).unwrap_or(false)
    }

    pub fn wants_json(&self) -> bool {
        self.info().map(|r| r.wants_json()).unwrap_or(false)
    }

    pub fn is_ajax(&self) -> bool {
        self.info().map(|r| r.is_ajax()).unwrap_or(false)
    }
}

/// Global helper returning the active request accessor.
pub fn request() -> RequestAccessor {
    RequestAccessor
}

/// Returns the currently active `RequestInfo` across task, thread, or global scope.
pub fn request_info() -> Option<RequestInfo> {
    if let Ok(Some(r)) = TASK_REQUEST.try_with(|r| r.clone()) {
        return Some(r);
    }
    if let Some(r) = THREAD_REQUEST.with(|c| c.borrow().clone()) {
        return Some(r);
    }
    global_request().read().ok().and_then(|r| r.clone())
}

/// Sets the current HTTP request info for the active thread and global scope.
pub fn set_current_request(req: Option<RequestInfo>) {
    THREAD_REQUEST.with(|c| *c.borrow_mut() = req.clone());
    if let Ok(mut g) = global_request().write() {
        *g = req;
    }
}

/// Runs a closure with the specified `RequestInfo` bound to the thread.
pub fn with_request<F, R>(req: RequestInfo, f: F) -> R
where
    F: FnOnce() -> R,
{
    let prev = THREAD_REQUEST.with(|c| c.borrow().clone());
    THREAD_REQUEST.with(|c| *c.borrow_mut() = Some(req));
    let result = f();
    THREAD_REQUEST.with(|c| *c.borrow_mut() = prev);
    result
}

/// Returns the currently authenticated `User` across task, thread, or global scope.
pub fn current_user() -> Option<User> {
    if let Ok(Some(u)) = TASK_USER.try_with(|u| u.clone()) {
        return Some(u);
    }
    if let Some(u) = THREAD_USER.with(|c| c.borrow().clone()) {
        return Some(u);
    }
    global_user().read().ok().and_then(|u| u.clone())
}

/// Global helper returning the currently authenticated `User`.
pub fn auth() -> Option<User> {
    current_user()
}

/// Global helper returning the currently authenticated `User`.
pub fn user() -> Option<User> {
    current_user()
}

/// Returns the ID of the currently authenticated `User`, or `None`.
pub fn user_id() -> Option<String> {
    current_user().map(|u| u.id)
}

/// Sets the authenticated user for the active thread and global scope.
pub fn set_current_user(user: Option<User>) {
    THREAD_USER.with(|c| *c.borrow_mut() = user.clone());
    if let Ok(mut g) = global_user().write() {
        *g = user;
    }
}

/// Runs a closure with the specified `User` bound to the thread.
pub fn with_user<F, R>(user: User, f: F) -> R
where
    F: FnOnce() -> R,
{
    let prev = THREAD_USER.with(|c| c.borrow().clone());
    THREAD_USER.with(|c| *c.borrow_mut() = Some(user));
    let result = f();
    THREAD_USER.with(|c| *c.borrow_mut() = prev);
    result
}

/// Returns the currently active `Session` across task, thread, or global scope.
pub fn session() -> Session {
    if let Ok(Some(s)) = TASK_SESSION.try_with(|s| s.clone()) {
        return s;
    }
    if let Some(s) = THREAD_SESSION.with(|c| c.borrow().clone()) {
        return s;
    }
    if let Some(s) = global_session().read().ok().and_then(|s| s.clone()) {
        return s;
    }
    let s = SessionStore::global().get_or_create(None);
    set_current_session(Some(s.clone()));
    s
}

/// Sets the current `Session` for the active thread and global scope.
pub fn set_current_session(sess: Option<Session>) {
    THREAD_SESSION.with(|c| *c.borrow_mut() = sess.clone());
    if let Ok(mut g) = global_session().write() {
        *g = sess;
    }
}

/// Flashes a one-time message into the active session for the next web request.
pub fn flash(key: impl Into<String>, msg: impl Into<String>) {
    session().flash(key, msg);
}

/// Flashes a success notification into the active session.
pub fn flash_success(msg: impl Into<String>) {
    session().flash_success(msg);
}

/// Flashes an error notification into the active session.
pub fn flash_error(msg: impl Into<String>) {
    session().flash_error(msg);
}

/// Primary authorization check: determines if the current authenticated user can perform an action.
/// Returns `false` if unauthenticated or if the action is denied.
pub fn can(permission: &str) -> bool {
    current_user().map(|u| u.can(permission)).unwrap_or(false)
}

/// Inverse authorization check: determines if the current user cannot perform an action.
pub fn cannot(permission: &str) -> bool {
    !can(permission)
}

/// Checks if the current authenticated user has any of the listed permissions.
pub fn can_any(permissions: &[&str]) -> bool {
    permissions.iter().any(|&p| can(p))
}

/// Checks if the current authenticated user has all of the listed permissions.
pub fn can_all(permissions: &[&str]) -> bool {
    permissions.iter().all(|&p| can(p))
}

/// Checks if the current authenticated user is a Superadmin or Founder.
pub fn is_superadmin() -> bool {
    current_user().map(|u| u.is_superadmin()).unwrap_or(false)
}

/// Checks if the current authenticated user is an Admin, Superadmin, or Founder.
pub fn is_admin() -> bool {
    current_user().map(|u| matches!(
        u.role,
        crate::domain::Role::Founder | crate::domain::Role::Superadmin | crate::domain::Role::Admin
    )).unwrap_or(false)
}

/// HTTP abort error with status code and error message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbortError {
    pub status: u16,
    pub message: String,
}

impl AbortError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    pub fn unprocessable(message: impl Into<String>) -> Self {
        Self::new(422, message)
    }

    pub fn server_error(message: impl Into<String>) -> Self {
        Self::new(500, message)
    }
}

impl std::fmt::Display for AbortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {} Error: {}", self.status, self.message)
    }
}

impl std::error::Error for AbortError {}

impl From<String> for AbortError {
    fn from(msg: String) -> Self {
        Self::new(500, msg)
    }
}

impl From<&str> for AbortError {
    fn from(msg: &str) -> Self {
        Self::new(500, msg)
    }
}

impl From<rusqlite::Error> for AbortError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(500, err.to_string())
    }
}

impl IntoResponse for AbortError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let wants_json = request_info()
            .map(|r| r.wants_json() || r.is_json())
            .unwrap_or(false);

        if wants_json {
            let body = serde_json::json!({
                "error": true,
                "status": self.status,
                "message": self.message,
            })
            .to_string();
            (
                status,
                [("Content-Type", "application/json; charset=utf-8")],
                body,
            )
                .into_response()
        } else {
            let html = format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{code} - {msg}</title>
  <script src="https://cdn.tailwindcss.com"></script>
</head>
<body class="bg-zinc-950 text-zinc-100 flex items-center justify-center min-h-screen p-4 font-sans antialiased">
  <div class="text-center p-8 max-w-md bg-zinc-900 border border-zinc-800 rounded-2xl shadow-2xl">
    <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-rose-500/10 text-rose-500 mb-6 font-mono font-bold text-2xl">
      {code}
    </div>
    <h1 class="text-2xl font-bold text-white mb-2">{msg}</h1>
    <p class="text-sm text-zinc-400 mb-6">The requested operation could not be completed with status code {code}.</p>
    <a href="/" class="inline-flex items-center justify-center px-5 py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold uppercase tracking-wider transition-colors shadow-lg shadow-indigo-600/20">
      Return to Home
    </a>
  </div>
</body>
</html>"#,
                code = self.status,
                msg = self.message
            );
            (status, axum::response::Html(html)).into_response()
        }
    }
}

/// Aborts execution with an HTTP status code and message.
pub fn abort(status_code: u16, message: impl Into<String>) -> Result<()> {
    Err(AbortError {
        status: status_code,
        message: message.into(),
    })
}

/// Aborts execution if the condition is true.
pub fn abort_if(condition: bool, status_code: u16, message: impl Into<String>) -> Result<()> {
    if condition {
        abort(status_code, message)
    } else {
        Ok(())
    }
}

/// Aborts execution unless the condition is true.
pub fn abort_unless(condition: bool, status_code: u16, message: impl Into<String>) -> Result<()> {
    if !condition {
        abort(status_code, message)
    } else {
        Ok(())
    }
}

/// Aborts with HTTP 403 Forbidden.
pub fn abort_403(message: impl Into<String>) -> Result<()> {
    abort(403, message)
}

/// Aborts with HTTP 404 Not Found.
pub fn abort_404(message: impl Into<String>) -> Result<()> {
    abort(404, message)
}

/// Returns current UTC DateTime.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Returns today's date formatted as YYYY-MM-DD.
pub fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

/// Returns current Unix timestamp in seconds.
pub fn timestamp() -> i64 {
    Utc::now().timestamp()
}

/// Reads configuration value from in-memory store or environment variable `TERMINUS_<KEY>` / `<KEY>`.
pub fn config(key: &str) -> Option<String> {
    if let Ok(store) = config_store().read() {
        if let Some(val) = store.get(key) {
            return Some(val.clone());
        }
    }
    let env_key = format!("TERMINUS_{}", key.to_uppercase().replace('.', "_"));
    std::env::var(&env_key).ok().or_else(|| std::env::var(key.to_uppercase().replace('.', "_")).ok())
}

/// Sets an in-memory configuration value.
pub fn config_set(key: impl Into<String>, val: impl Into<String>) {
    if let Ok(mut store) = config_store().write() {
        store.insert(key.into(), val.into());
    }
}

/// Reads an environment variable.
pub fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}



#[macro_export]
macro_rules! abort_if {
    ($cond:expr, $code:expr, $msg:expr) => {
        if $cond {
            return Err($crate::helpers::AbortError {
                status: $code,
                message: $msg.into(),
            });
        }
    };
}

#[macro_export]
macro_rules! abort_unless {
    ($cond:expr, $code:expr, $msg:expr) => {
        if !$cond {
            return Err($crate::helpers::AbortError {
                status: $code,
                message: $msg.into(),
            });
        }
    };
}

/// Fluent redirect response supporting chained flash messages (`redirect("/posts").with("success", "Saved!")`).
#[derive(Debug, Clone)]
pub struct RedirectResponse {
    pub url: String,
    pub status: StatusCode,
    pub flash: Option<(String, String)>,
}

impl RedirectResponse {
    pub fn to(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            status: StatusCode::SEE_OTHER,
            flash: None,
        }
    }

    /// Chains a one-time flash notification to be rendered on the redirected page.
    pub fn with(mut self, key: impl Into<String>, msg: impl Into<String>) -> Self {
        self.flash = Some((key.into(), msg.into()));
        self
    }

    /// Convenience helper to chain a success notification.
    pub fn with_success(self, msg: impl Into<String>) -> Self {
        self.with("success", msg)
    }

    /// Convenience helper to chain an error notification.
    pub fn with_error(self, msg: impl Into<String>) -> Self {
        self.with("error", msg)
    }

    /// Convenience helper to chain a warning notification.
    pub fn with_warning(self, msg: impl Into<String>) -> Self {
        self.with("warning", msg)
    }

    /// Convenience helper to chain an informational notification.
    pub fn with_info(self, msg: impl Into<String>) -> Self {
        self.with("info", msg)
    }
}

impl IntoResponse for RedirectResponse {
    fn into_response(self) -> Response {
        let session = session();
        if let Some((k, v)) = self.flash {
            session.flash(k, v);
        }
        let mut headers = axum::http::HeaderMap::new();
        if let Ok(val) = axum::http::HeaderValue::from_str(&self.url) {
            headers.insert(axum::http::header::LOCATION, val);
        }
        if let Ok(val) = axum::http::HeaderValue::from_str(&session.cookie_header()) {
            headers.insert(axum::http::header::SET_COOKIE, val);
        }
        (self.status, headers, "").into_response()
    }
}

impl From<RedirectResponse> for axum::response::Redirect {
    fn from(r: RedirectResponse) -> Self {
        axum::response::Redirect::to(&r.url)
    }
}

pub type Redirect = RedirectResponse;

/// Fluent redirect helper returning an HTTP 303 Redirect response with optional flash chaining.
pub fn redirect(to_url: impl AsRef<str>) -> RedirectResponse {
    RedirectResponse::to(to_url.as_ref())
}

/// Standard Terminus Result type for controller handlers returning AbortError on failure by default
pub type Result<T, E = AbortError> = std::result::Result<T, E>;

