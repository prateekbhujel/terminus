//! Terminus Application Topology Specification (AppSpec)
//!
//! Provides deterministic machine-readable introspection for autonomous coding agents,
//! tooling, and client SDK generators. AI agents can query AppSpec to discover
//! all models, routes, admin resources, and service configurations without hallucination.

use serde::{Deserialize, Serialize};

/// Top-level application specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSpec {
    pub framework: String,
    pub version: String,
    pub app_name: String,
    pub host: String,
    pub port: u16,
    pub targets: Vec<String>,
    pub routes: Vec<RouteSpec>,
    pub models: Vec<ModelSpec>,
    pub resources: Vec<ResourceSpec>,
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
            framework: "terminus-framework".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_name: app_name.into(),
            host: "127.0.0.1".to_string(),
            port: 3000,
            targets: vec!["web".to_string()],
            routes: Vec::new(),
            models: Vec::new(),
            resources: Vec::new(),
            services: ServiceSpec::default(),
        }
    }

    /// Add a route to the specification
    pub fn add_route(&mut self, route: RouteSpec) -> &mut Self {
        self.routes.push(route);
        self
    }

    /// Add a domain model to the specification
    pub fn add_model(&mut self, model: ModelSpec) -> &mut Self {
        self.models.push(model);
        self
    }

    /// Add an admin resource to the specification
    pub fn add_resource(&mut self, resource: ResourceSpec) -> &mut Self {
        self.resources.push(resource);
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
        out.push_str("================================================================================\n");
        out.push_str(&format!("  TERMINUS APPLICATION TOPOLOGY ── {}\n", self.app_name));
        out.push_str(&format!("  Framework: {} v{} | Host: {}:{}\n", self.framework, self.version, self.host, self.port));
        out.push_str(&format!("  Target Platforms: {}\n", self.targets.join(", ")));
        out.push_str("================================================================================\n\n");

        // Routes
        out.push_str("[ROUTES]\n");
        out.push_str("  METHOD   PATH                                HANDLER               AUTH\n");
        out.push_str("  ----------------------------------------------------------------------\n");
        if self.routes.is_empty() {
            out.push_str("  (No explicit routes registered in specification)\n");
        } else {
            for r in &self.routes {
                let auth_str = if r.auth_required { "YES" } else { "NO" };
                out.push_str(&format!("  {:<8} {:<35} {:<21} {}\n", r.method, r.path, r.handler_name, auth_str));
            }
        }
        out.push_str("\n");

        // Models
        out.push_str("[MODELS & TABLES]\n");
        out.push_str("  MODEL                TABLE                FIELDS\n");
        out.push_str("  ----------------------------------------------------------------------\n");
        if self.models.is_empty() {
            out.push_str("  (No models registered in specification)\n");
        } else {
            for m in &self.models {
                let field_count = m.fields.len();
                out.push_str(&format!("  {:<20} {:<20} {} fields\n", m.name, m.table, field_count));
            }
        }
        out.push_str("\n");

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
                out.push_str(&format!("  {:<20} {:<20} {}\n", res.name, res.slug, search_cols));
            }
        }
        out.push_str("\n");

        // Services
        out.push_str("[SERVICES & CLIENT SDKs]\n");
        out.push_str(&format!("  Background Job Workers : {}\n", if self.services.background_workers { "Enabled" } else { "Disabled" }));
        out.push_str(&format!("  WebSocket Broadcaster  : {}\n", if self.services.websocket_broadcasting { "Enabled" } else { "Disabled" }));
        out.push_str(&format!("  Active Cache Stores    : {}\n", self.services.cache_stores.join(", ")));
        out.push_str(&format!("  Client SDK Targets     : {}\n", self.services.client_sdk_languages.join(", ")));
        out.push_str("================================================================================\n");

        out
    }
}

/// Specification for an HTTP route
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteSpec {
    pub method: String,
    pub path: String,
    pub handler_name: String,
    pub auth_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl RouteSpec {
    pub fn new(method: impl Into<String>, path: impl Into<String>, handler: impl Into<String>) -> Self {
        Self {
            method: method.into().to_uppercase(),
            path: path.into(),
            handler_name: handler.into(),
            auth_required: false,
            description: None,
        }
    }

    pub fn with_auth(mut self, required: bool) -> Self {
        self.auth_required = required;
        self
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
    pub fields: Vec<FieldSpec>,
}

impl ModelSpec {
    pub fn new(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            table: table.into(),
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
    pub rust_type: String,
    pub sql_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

impl FieldSpec {
    pub fn new(name: impl Into<String>, rust_type: impl Into<String>, sql_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rust_type: rust_type.into(),
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
            cache_stores: vec!["memory".to_string(), "redis".to_string()],
            client_sdk_languages: vec![
                "typescript".to_string(),
                "swift".to_string(),
                "kotlin".to_string(),
                "python".to_string(),
            ],
        }
    }
}
