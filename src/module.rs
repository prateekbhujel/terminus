//! Terminus Module — Enterprise Modular Architecture (DDD)
//! Encapsulated domain modules containing DTOs, Models, Repositories, Providers, Controllers, Requests, and Routes.

use axum::{
    body::to_bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;
use crate::resource::Resource;

/// Contract for objects and DTOs that can validate themselves.
pub trait Validate {
    fn validate(&self) -> Result<(), ValidationErrors>;
}

/// Axum Extractor that automatically deserializes and validates a request payload.
/// If JSON deserialization fails, or if validation fails, it short-circuits and
/// returns an HTTP 422 Unprocessable Entity response containing structured JSON errors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Valid<T>(pub T);

impl<T> std::ops::Deref for Valid<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Valid<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Valid<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[axum::async_trait]
impl<T, S> FromRequest<S> for Valid<T>
where
    T: DeserializeOwned + Validate + 'static,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = match to_bytes(req.into_body(), 1024 * 1024 * 2).await {
            Ok(b) => b,
            Err(e) => {
                let mut errs = ValidationErrors::new();
                errs.message = "Failed to read request body".to_string();
                errs.add("body", e.to_string());
                return Err(errs.into_response());
            }
        };

        let parsed: T = match serde_json::from_slice(&bytes) {
            Ok(val) => val,
            Err(e) => {
                let mut errs = ValidationErrors::new();
                errs.message = "Invalid JSON payload".to_string();
                errs.add("payload", e.to_string());
                return Err(errs.into_response());
            }
        };

        if let Err(errors) = parsed.validate() {
            return Err(errors.into_response());
        }

        Ok(Valid(parsed))
    }
}

/// Core interface for an encapsulated domain module
pub trait AppModule: Send + Sync {
    /// Human-readable module name (e.g. "User Management", "Billing")
    fn name(&self) -> &str;

    /// URL/route slug identifier (e.g. "users", "billing")
    fn slug(&self) -> &str;

    /// Optional module description
    fn description(&self) -> Option<&str> {
        None
    }

    /// Axum Router for this module's custom API/web endpoints
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    /// Optional Terminus administrative Resource for this module
    fn resource(&self) -> Option<Arc<dyn Resource>> {
        None
    }
}

/// Standardized enterprise JSON API response envelope
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<String>>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            message: None,
            data: Some(data),
            errors: None,
        }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: Some(data),
            errors: None,
        }
    }
}

impl ApiResponse<()> {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            data: None,
            errors: None,
        }
    }

    pub fn validation_errors(errors: Vec<String>) -> Self {
        Self {
            success: false,
            message: Some("Validation failed".to_string()),
            data: None,
            errors: Some(errors),
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = if self.success {
            StatusCode::OK
        } else if self.errors.is_some() {
            StatusCode::UNPROCESSABLE_ENTITY
        } else {
            StatusCode::BAD_REQUEST
        };
        (status, Json(self)).into_response()
    }
}

/// Validation contract for enterprise Form Request objects
pub trait ValidatedRequest: Sized {
    fn validate(&self) -> Result<(), Vec<String>>;
}

/// Dynamic module wrapper allowing programmatic module construction
#[derive(Clone)]
pub struct DynamicModule {
    name: String,
    slug: String,
    description: Option<String>,
    routes: axum::Router,
    resource: Option<Arc<dyn Resource>>,
}

impl DynamicModule {
    pub fn new(name: impl Into<String>, slug: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            slug: slug.into(),
            description: None,
            routes: axum::Router::new(),
            resource: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_routes(mut self, router: axum::Router) -> Self {
        self.routes = router;
        self
    }

    pub fn with_resource(mut self, res: Arc<dyn Resource>) -> Self {
        self.resource = Some(res);
        self
    }
}

impl AppModule for DynamicModule {
    fn name(&self) -> &str {
        &self.name
    }

    fn slug(&self) -> &str {
        &self.slug
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn routes(&self) -> axum::Router {
        self.routes.clone()
    }

    fn resource(&self) -> Option<Arc<dyn Resource>> {
        self.resource.clone()
    }
}

/// Structured validation errors keyed by field name
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ValidationErrors {
    pub message: String,
    pub errors: std::collections::BTreeMap<String, Vec<String>>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self {
            message: "Validation failed".to_string(),
            errors: std::collections::BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.errors.entry(field.into()).or_default().push(message.into());
    }

    pub fn has_errors_for(&self, field: &str) -> bool {
        self.errors.contains_key(field)
    }

    pub fn first(&self, field: &str) -> Option<&str> {
        self.errors.get(field).and_then(|v| v.first().map(|s| s.as_str()))
    }

    pub fn all_messages(&self) -> Vec<String> {
        self.errors.values().flatten().cloned().collect()
    }
}

impl IntoResponse for ValidationErrors {
    fn into_response(self) -> Response {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            [("Content-Type", "application/json; charset=utf-8")],
            serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string()),
        )
            .into_response()
    }
}

/// Fluent type-safe validator for domain request objects
pub struct Validator<'a, T> {
    target: &'a T,
    errors: ValidationErrors,
}

impl<'a, T> Validator<'a, T> {
    pub fn new(target: &'a T) -> Self {
        Self {
            target,
            errors: ValidationErrors::new(),
        }
    }

    /// Add a validation rule evaluated via closure
    pub fn rule<F>(mut self, field: &str, predicate: F, message: &str) -> Self
    where
        F: FnOnce(&T) -> bool,
    {
        if !predicate(self.target) {
            self.errors.add(field, message);
        }
        self
    }

    /// Add a conditional validation rule
    pub fn rule_when<F, C>(mut self, condition: C, field: &str, predicate: F, message: &str) -> Self
    where
        C: FnOnce(&T) -> bool,
        F: FnOnce(&T) -> bool,
    {
        if condition(self.target) && !predicate(self.target) {
            self.errors.add(field, message);
        }
        self
    }

    /// Direct error addition
    pub fn add_error(mut self, field: &str, message: &str) -> Self {
        self.errors.add(field, message);
        self
    }

    /// Finalize validation, returning Ok(&T) or Err(ValidationErrors)
    pub fn validate(self) -> Result<&'a T, ValidationErrors> {
        if self.errors.is_empty() {
            Ok(self.target)
        } else {
            Err(self.errors)
        }
    }
}

/// Fluent validator for untyped or semi-structured JSON payloads
pub struct JsonValidator<'a> {
    data: &'a serde_json::Value,
    errors: ValidationErrors,
}

impl<'a> JsonValidator<'a> {
    pub fn new(data: &'a serde_json::Value) -> Self {
        Self {
            data,
            errors: ValidationErrors::new(),
        }
    }

    pub fn require_str(&mut self, field: &str) -> Option<&'a str> {
        match self.data.get(field).and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => Some(s),
            _ => {
                self.errors.add(field, format!("{field} is required"));
                None
            }
        }
    }

    pub fn require_email(&mut self, field: &str) -> Option<&'a str> {
        let val = self.require_str(field)?;
        if !val.contains('@') || !val.contains('.') {
            self.errors.add(field, format!("{field} must be a valid email address"));
            return None;
        }
        Some(val)
    }

    pub fn require_min_len(&mut self, field: &str, min: usize) -> Option<&'a str> {
        let val = self.require_str(field)?;
        if val.len() < min {
            self.errors.add(field, format!("{field} must be at least {min} characters"));
            return None;
        }
        Some(val)
    }

    pub fn require_number(&mut self, field: &str) -> Option<f64> {
        match self.data.get(field).and_then(|v| v.as_f64()) {
            Some(n) => Some(n),
            _ => {
                self.errors.add(field, format!("{field} must be a valid number"));
                None
            }
        }
    }

    pub fn validate(self) -> Result<&'a serde_json::Value, ValidationErrors> {
        if self.errors.is_empty() {
            Ok(self.data)
        } else {
            Err(self.errors)
        }
    }
}
