//! Terminus Application Topology Specification (AppSpec)
//!
//! Versioned machine-readable application contract used by Terminus tooling,
//! OpenAPI/SDK generators, and AI coding agents with explicit provenance
//! and disclosure of unobservable registrations.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SCHEMA_CONTRACT_VERSION: &str = "1.0";
pub const DEFAULT_SPEC_PATH: &str = ".terminus/app-spec.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    #[default]
    Programmatic,
    ApplicationExport,
    CliInspection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractOrigin {
    pub kind: OriginKind,
    pub authoritative_app_contract: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported_at: Option<String>,
}

impl Default for ContractOrigin {
    fn default() -> Self {
        Self {
            kind: OriginKind::Programmatic,
            authoritative_app_contract: true,
            manifest_path: None,
            database_backend: None,
            database_source: None,
            exported_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnavailableSpecItem {
    pub category: String,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RouteAuth {
    Required,
    Public,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSpec {
    pub contract_version: String,
    pub framework: String,
    pub version: String,
    pub app_name: String,
    pub host: String,
    pub port: u16,
    pub origin: ContractOrigin,
    pub targets: Vec<String>,
    pub routes: Vec<RouteSpec>,
    pub models: Vec<ModelSpec>,
    pub resources: Vec<ResourceSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<UnavailableSpecItem>,
    pub services: ServiceSpec,
}
impl Default for AppSpec {
    fn default() -> Self {
        Self::new("Terminus Application")
    }
}

impl AppSpec {
    /// Initialize a new application specification
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            contract_version: SCHEMA_CONTRACT_VERSION.to_string(),
            framework: "terminus-rs".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_name: app_name.into(),
            host: "127.0.0.1".to_string(),
            port: 3000,
            origin: ContractOrigin::default(),
            targets: vec!["web".to_string()],
            routes: Vec::new(),
            models: Vec::new(),
            resources: Vec::new(),
            unavailable: Vec::new(),
            services: ServiceSpec::default(),
        }
    }

    pub fn add_unavailable(
        &mut self,
        category: impl Into<String>,
        target: impl Into<String>,
        reason: impl Into<String>,
    ) -> &mut Self {
        let item = UnavailableSpecItem {
            category: category.into(),
            target: target.into(),
            reason: reason.into(),
        };
        if !self.unavailable.contains(&item) {
            self.unavailable.push(item);
        }
        self
    }

    /// Serialize this contract to a manifest file on disk, creating parent directories if needed.
    pub fn write_to_path(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut export = self.clone();
        export.origin.kind = OriginKind::ApplicationExport;
        export.origin.authoritative_app_contract = true;
        export.origin.manifest_path = Some(path.display().to_string());
        export.origin.exported_at = Some(chrono::Utc::now().to_rfc3339());
        std::fs::write(path, export.to_json_pretty())
    }

    /// Load an exported `AppSpec` manifest from disk and validate its contract version.
    pub fn read_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read AppSpec manifest '{}': {e}", path.display()))?;
        let spec: Self = serde_json::from_str(&raw)
            .map_err(|e| format!("Invalid AppSpec JSON in '{}': {e}", path.display()))?;
        if spec.contract_version != SCHEMA_CONTRACT_VERSION {
            return Err(format!(
                "Unsupported AppSpec contract_version '{}' in '{}' (expected '{}')",
                spec.contract_version,
                path.display(),
                SCHEMA_CONTRACT_VERSION
            ));
        }
        Ok(spec)
    }

    /// Add a route to the specification (replacing an identical method+path entry if present)
    pub fn add_route(&mut self, route: RouteSpec) -> &mut Self {
        if let Some(existing) = self
            .routes
            .iter_mut()
            .find(|r| r.method == route.method && r.path == route.path)
        {
            *existing = route;
        } else {
            self.routes.push(route);
        }
        self
    }

    /// Add a domain model to the specification
    pub fn add_model(&mut self, model: ModelSpec) -> &mut Self {
        if let Some(existing) = self.models.iter_mut().find(|m| m.table == model.table) {
            *existing = model;
        } else {
            self.models.push(model);
        }
        self
    }

    /// Add an admin resource to the specification
    pub fn add_resource(&mut self, resource: ResourceSpec) -> &mut Self {
        if let Some(existing) = self.resources.iter_mut().find(|r| r.slug == resource.slug) {
            *existing = resource;
        } else {
            self.resources.push(resource);
        }
        self
    }

    /// Merge routes, models, resources, and unavailable entries from a nested group under `prefix`.
    pub fn merge_group(&mut self, prefix: &str, child: AppSpec) -> &mut Self {
        for mut route in child.routes {
            route.path = join_route_paths(prefix, &route.path);
            self.add_route(route);
        }
        for model in child.models {
            self.add_model(model);
        }
        for resource in child.resources {
            self.add_resource(resource);
        }
        for mut item in child.unavailable {
            if item.target.starts_with('/') {
                item.target = join_route_paths(prefix, &item.target);
            }
            if !self.unavailable.contains(&item) {
                self.unavailable.push(item);
            }
        }
        self
    }

    /// Set the target platforms
    pub fn set_targets(&mut self, targets: Vec<String>) -> &mut Self {
        self.targets = targets;
        self
    }

    /// Serialize this specification to pretty JSON for AI agent consumption
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Render a concise ASCII tabular summary of the application topology
    pub fn to_ascii_summary(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "================================================================================\n",
        );
        out.push_str(&format!(
            "  TERMINUS APPLICATION TOPOLOGY ── {}\n",
            self.app_name
        ));
        out.push_str(&format!(
            "  Framework: {} v{} | Contract: v{} | Host: {}:{}\n",
            self.framework, self.version, self.contract_version, self.host, self.port
        ));
        out.push_str(&format!(
            "  Origin: {:?} (authoritative: {}) | Platforms: {}\n",
            self.origin.kind,
            self.origin.authoritative_app_contract,
            self.targets.join(", ")
        ));
        if let Some(ref db_backend) = self.origin.database_backend {
            out.push_str(&format!(
                "  Database Backend: {} ({})\n",
                db_backend,
                self.origin
                    .database_source
                    .as_deref()
                    .unwrap_or("configured")
            ));
        }
        out.push_str(
            "================================================================================\n\n",
        );

        // Routes
        out.push_str("[ROUTES]\n");
        out.push_str("  METHOD   PATH                                HANDLER               AUTH\n");
        out.push_str("  ----------------------------------------------------------------------\n");
        if self.routes.is_empty() {
            out.push_str("  (No explicit routes registered in specification)\n");
        } else {
            for r in &self.routes {
                let auth_str = match r.auth {
                    RouteAuth::Required => "REQUIRED",
                    RouteAuth::Public => "PUBLIC",
                    RouteAuth::Unknown => "UNKNOWN",
                };
                out.push_str(&format!(
                    "  {:<8} {:<35} {:<21} {}\n",
                    r.method, r.path, r.handler_name, auth_str
                ));
            }
        }
        out.push('\n');

        // Models
        out.push_str("[MODELS & TABLES]\n");
        out.push_str("  MODEL                TABLE                FIELDS\n");
        out.push_str("  ----------------------------------------------------------------------\n");
        if self.models.is_empty() {
            out.push_str("  (No models registered in specification)\n");
        } else {
            for m in &self.models {
                let field_count = m.fields.len();
                out.push_str(&format!(
                    "  {:<20} {:<20} {} fields\n",
                    m.name, m.table, field_count
                ));
            }
        }
        out.push('\n');

        // Resources
        out.push_str("[ADMIN RESOURCES]\n");
        out.push_str("  RESOURCE             SLUG                 SEARCHABLE COLUMNS\n");
        out.push_str("  ----------------------------------------------------------------------\n");
        if self.resources.is_empty() {
            out.push_str("  (No admin resources registered in specification)\n");
        } else {
            for res in &self.resources {
                let search_cols = if res.searchable_fields.is_empty() {
                    "None".to_string()
                } else {
                    res.searchable_fields.join(", ")
                };
                out.push_str(&format!(
                    "  {:<20} {:<20} {}\n",
                    res.name, res.slug, search_cols
                ));
            }
        }
        out.push('\n');

        if !self.unavailable.is_empty() {
            out.push_str("[UNAVAILABLE / UNOBSERVABLE REGISTRATIONS]\n");
            out.push_str(
                "  ----------------------------------------------------------------------\n",
            );
            for item in &self.unavailable {
                out.push_str(&format!(
                    "  [{}] {} — {}\n",
                    item.category, item.target, item.reason
                ));
            }
            out.push('\n');
        }

        // Services
        out.push_str("[SERVICES & CLIENT SDKs]\n");
        out.push_str(&format!(
            "  Background Job Workers : {}\n",
            if self.services.background_workers {
                "Enabled"
            } else {
                "Disabled"
            }
        ));
        out.push_str(&format!(
            "  WebSocket Broadcaster  : {}\n",
            if self.services.websocket_broadcasting {
                "Enabled"
            } else {
                "Disabled"
            }
        ));
        out.push_str(&format!(
            "  Active Cache Stores    : {}\n",
            self.services.cache_stores.join(", ")
        ));
        out.push_str(&format!(
            "  Client SDK Targets     : {}\n",
            self.services.client_sdk_languages.join(", ")
        ));
        out.push_str(
            "================================================================================\n",
        );

        out
    }
}

/// Specification for an HTTP route
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteSpec {
    pub method: String,
    pub path: String,
    pub handler_name: String,
    #[serde(default)]
    pub auth: RouteAuth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl RouteSpec {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        handler: impl Into<String>,
    ) -> Self {
        Self {
            method: method.into().to_uppercase(),
            path: path.into(),
            handler_name: handler.into(),
            auth: RouteAuth::Unknown,
            description: None,
        }
    }

    pub fn with_auth(mut self, required: bool) -> Self {
        self.auth = if required {
            RouteAuth::Required
        } else {
            RouteAuth::Public
        };
        self
    }

    pub fn with_auth_status(mut self, auth: RouteAuth) -> Self {
        self.auth = auth;
        self
    }

    pub fn requires_auth(self) -> Self {
        self.with_auth_status(RouteAuth::Required)
    }

    pub fn public(self) -> Self {
        self.with_auth_status(RouteAuth::Public)
    }

    pub fn with_unknown_auth(self) -> Self {
        self.with_auth_status(RouteAuth::Unknown)
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Specification for a domain entity model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: String,
    pub table: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub fields: Vec<FieldSpec>,
}

impl ModelSpec {
    pub fn new(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            table: table.into(),
            source: Some("app_spec".to_string()),
            fields: Vec::new(),
        }
    }

    pub fn from_database(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            table: table.into(),
            source: Some("database_schema".to_string()),
            fields: Vec::new(),
        }
    }

    pub fn add_field(mut self, field: FieldSpec) -> Self {
        self.fields.push(field);
        self
    }
}

/// Specification for an entity field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldSpec {
    pub name: String,
    #[serde(default)]
    pub rust_type: Option<String>,
    pub sql_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

impl FieldSpec {
    pub fn new(
        name: impl Into<String>,
        rust_type: impl Into<String>,
        sql_type: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            rust_type: Some(rust_type.into()),
            sql_type: sql_type.into(),
            nullable: false,
            primary_key: false,
        }
    }

    /// Construct a field observed from database metadata without inventing a Rust type.
    pub fn from_database(name: impl Into<String>, sql_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rust_type: None,
            sql_type: sql_type.into(),
            nullable: false,
            primary_key: false,
        }
    }

    pub fn primary(mut self) -> Self {
        self.primary_key = true;
        self
    }

    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }
}

/// Specification for an administrative Resource
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceSpec {
    pub name: String,
    pub slug: String,
    pub table: String,
    pub searchable_fields: Vec<String>,
    pub sortable_fields: Vec<String>,
}

impl ResourceSpec {
    pub fn new(name: impl Into<String>, slug: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            slug: slug.into(),
            table: table.into(),
            searchable_fields: Vec::new(),
            sortable_fields: Vec::new(),
        }
    }

    pub fn with_searchable(mut self, fields: &[&str]) -> Self {
        self.searchable_fields = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_sortable(mut self, fields: &[&str]) -> Self {
        self.sortable_fields = fields.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// Specification for background and support services
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSpec {
    pub background_workers: bool,
    pub websocket_broadcasting: bool,
    pub cache_stores: Vec<String>,
    pub client_sdk_languages: Vec<String>,
}

impl Default for ServiceSpec {
    fn default() -> Self {
        Self {
            background_workers: true,
            websocket_broadcasting: true,
            cache_stores: vec!["memory".to_string()],
            client_sdk_languages: vec!["typescript".to_string(), "openapi".to_string()],
        }
    }
}

/// Normalize a route path to start with `/` and omit redundant trailing slashes (except root `/`).
pub fn normalize_route_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let clean = trimmed.trim_matches('/');
    if clean.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", clean)
    }
}

/// Join a route group prefix with a child route path cleanly.
pub fn join_route_paths(prefix: &str, path: &str) -> String {
    let clean_prefix = normalize_route_path(prefix);
    let clean_path = normalize_route_path(path);
    match (clean_prefix.as_str(), clean_path.as_str()) {
        ("/", _) => clean_path,
        (_, "/") => clean_prefix,
        _ => format!("{}{}", clean_prefix, clean_path),
    }
}
