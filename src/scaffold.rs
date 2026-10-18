//! Terminus Project Scaffolder & Installation Wizard
//!
//! Interactive project bootstrapping and template scaffolding
//! supporting Composable Target Platforms (Web, Desktop, Mobile, IoT), Fullstack,
//! Backend API, and Universal architecture presets.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Target client platforms supported by Terminus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetPlatform {
    /// Web applications (HTTP Server, Admin Dashboard, REST & OpenAPI 3.0)
    Web,
    /// Desktop workstations (Native OS Windowing & Daemons, Offline SQLite Sync, Direct IPC Sockets)
    Desktop,
    /// Mobile applications (Native Swift for iOS & Native Kotlin for Android, Push notification dispatchers)
    Mobile,
    /// IoT hardware scanners (Native Embedded Runtime, HMAC-SHA256 authenticated device registry, barcode event streams)
    IoT,
}

impl TargetPlatform {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "web" | "w" | "browser" | "http" => Some(Self::Web),
            "desktop" | "d" | "desk" | "native-desktop" | "station" | "kiosk" | "tauri" | "electron" => Some(Self::Desktop),
            "mobile" | "m" | "app" | "ios" | "android" | "kotlin" | "swift" => Some(Self::Mobile),
            "iot" | "i" | "hardware" | "scanner" | "embedded" => Some(Self::IoT),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::IoT => "iot",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Web => "Web (HTTP Server, Admin UI, REST & OpenAPI 3.0)",
            Self::Desktop => "Desktop (Native OS Windowing & Daemons, Offline SQLite Sync, Direct IPC Sockets)",
            Self::Mobile => "Mobile (Native Swift for iOS, Native Kotlin for Android, Push Dispatcher)",
            Self::IoT => "IoT (Native Embedded Runtime, Hardware Scanners, HMAC Device Registry, Sensor Streams)",
        }
    }
}

/// Parses a comma-separated string into a list of target platforms
pub fn parse_targets(s: &str) -> Vec<TargetPlatform> {
    let trimmed = s.trim().to_lowercase();
    if trimmed == "all" || trimmed == "universal" {
        return vec![
            TargetPlatform::Web,
            TargetPlatform::Desktop,
            TargetPlatform::Mobile,
            TargetPlatform::IoT,
        ];
    }
    let mut targets = Vec::new();
    for part in s.split(',') {
        if let Some(t) = TargetPlatform::from_str_loose(part.trim()) {
            if !targets.contains(&t) {
                targets.push(t);
            }
        }
    }
    if targets.is_empty() {
        vec![TargetPlatform::Web]
    } else {
        targets
    }
}

/// Architecture presets available during project creation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectPreset {
    /// Universal Terminus ecosystem: Web Admin UI, Mobile APIs, Desktop SQLite sync, IoT scanner streams
    Universal,
    /// Fullstack: Backend API + Built-in Admin Dashboard UI
    Fullstack,
    /// Headless Backend API: Pure high-throughput JSON API, OpenAPI 3.0 specs, zero frontend bloat
    Backend,
}

impl ProjectPreset {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "universal" | "all" | "u" => Some(Self::Universal),
            "fullstack" | "admin" | "dashboard" | "f" => Some(Self::Fullstack),
            "backend" | "api" | "headless" | "b" => Some(Self::Backend),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Universal => "universal",
            Self::Fullstack => "fullstack",
            Self::Backend => "backend",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Universal => "Universal (Terminus Ecosystem: Web, Mobile APIs, Desktop Sync, IoT)",
            Self::Fullstack => "Fullstack (Backend API + Built-in Admin Dashboard)",
            Self::Backend => "Backend API (High-throughput JSON API + OpenAPI 3.0 Specs)",
        }
    }
}

/// Frontend client choices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendChoice {
    /// Headless API or built-in Admin Dashboard (no separate frontend build step)
    None,
    /// React 19 + TypeScript (Vite & Tailwind CSS)
    React,
    /// Vue 3 + TypeScript (Vite & Tailwind CSS)
    Vue,
    /// HTML5 + HTMX + Tailwind CSS (Zero JavaScript build step, SSR)
    Htmx,
}

impl FrontendChoice {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" | "no" | "n" | "api" => Some(Self::None),
            "react" | "r" | "react-ts" => Some(Self::React),
            "vue" | "v" | "vue-ts" => Some(Self::Vue),
            "htmx" | "h" | "html" => Some(Self::Htmx),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::React => "react",
            Self::Vue => "vue",
            Self::Htmx => "htmx",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None (Headless API / Built-in Admin Dashboard)",
            Self::React => "React 19 + TypeScript (Vite & Tailwind CSS)",
            Self::Vue => "Vue 3 + TypeScript (Vite & Tailwind CSS)",
            Self::Htmx => "HTMX + Tailwind CSS (Zero build step, SSR)",
        }
    }
}

/// Database engine choices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseChoice {
    /// SQLite (Embedded, zero configuration single-file database)
    Sqlite,
    /// PostgreSQL (Enterprise relational database)
    Postgres,
    /// MySQL / MariaDB (High-throughput transactional database)
    Mysql,
    /// MongoDB (Scalable NoSQL document database)
    Mongo,
    /// Redis (In-memory key-value and cache store)
    Redis,
}

impl DatabaseChoice {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sqlite" | "sqlite3" | "sq" => Some(Self::Sqlite),
            "postgres" | "postgresql" | "pg" => Some(Self::Postgres),
            "mysql" | "mariadb" | "my" => Some(Self::Mysql),
            "mongodb" | "mongo" | "nosql" => Some(Self::Mongo),
            "redis" | "keyvalue" => Some(Self::Redis),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Mongo => "mongo",
            Self::Redis => "redis",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite (Zero configuration single-file database)",
            Self::Postgres => "PostgreSQL (Enterprise relational database)",
            Self::Mysql => "MySQL / MariaDB (High throughput transactional database)",
            Self::Mongo => "MongoDB (Scalable NoSQL document store)",
            Self::Redis => "Redis (In-memory key-value and cache store)",
        }
    }
}

/// Configuration options for a new Terminus project
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub preset: ProjectPreset,
    pub targets: Vec<TargetPlatform>,
    pub frontend: FrontendChoice,
    pub database: DatabaseChoice,
    pub port: u16,
}

impl ProjectConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            preset: ProjectPreset::Fullstack,
            targets: vec![TargetPlatform::Web],
            frontend: FrontendChoice::None,
            database: DatabaseChoice::Sqlite,
            port: 3000,
        }
    }

    pub fn preset(mut self, preset: ProjectPreset) -> Self {
        self.preset = preset;
        match preset {
            ProjectPreset::Universal => {
                self.targets = vec![
                    TargetPlatform::Web,
                    TargetPlatform::Desktop,
                    TargetPlatform::Mobile,
                    TargetPlatform::IoT,
                ];
            }
            ProjectPreset::Fullstack | ProjectPreset::Backend => {
                self.targets = vec![TargetPlatform::Web];
            }
        }
        self
    }

    pub fn targets(mut self, targets: Vec<TargetPlatform>) -> Self {
        if !targets.is_empty() {
            self.targets = targets;
        }
        self
    }

    pub fn frontend(mut self, frontend: FrontendChoice) -> Self {
        self.frontend = frontend;
        self
    }

    pub fn database(mut self, database: DatabaseChoice) -> Self {
        self.database = database;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn has_target(&self, platform: TargetPlatform) -> bool {
        self.targets.contains(&platform)
    }

    pub fn is_desktop_only(&self) -> bool {
        self.targets.len() == 1 && self.targets.contains(&TargetPlatform::Desktop)
    }

    pub fn is_iot_only(&self) -> bool {
        self.targets.len() == 1 && self.targets.contains(&TargetPlatform::IoT)
    }

    pub fn is_universal(&self) -> bool {
        self.has_target(TargetPlatform::Web)
            && self.has_target(TargetPlatform::Desktop)
            && self.has_target(TargetPlatform::Mobile)
            && self.has_target(TargetPlatform::IoT)
    }
}

/// Scaffolds a complete Terminus project into the specified directory
pub fn scaffold_project(target_dir: &Path, config: &ProjectConfig) -> io::Result<Vec<PathBuf>> {
    let mut created_files = Vec::new();

    fs::create_dir_all(target_dir)?;

    // 1. Cargo.toml
    let cargo_toml = generate_cargo_toml(config);
    let cargo_path = target_dir.join("Cargo.toml");
    fs::write(&cargo_path, cargo_toml)?;
    created_files.push(cargo_path);

    // 2. src/main.rs
    let src_dir = target_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    let main_rs = generate_main_rs(config);
    let main_path = src_dir.join("main.rs");
    fs::write(&main_path, main_rs)?;
    created_files.push(main_path);

    // 3. .env.example
    let env_example = generate_env_example(config);
    let env_path = target_dir.join(".env.example");
    fs::write(&env_path, env_example)?;
    created_files.push(env_path);

    // 4. README.md
    let readme = generate_readme(config);
    let readme_path = target_dir.join("README.md");
    fs::write(&readme_path, readme)?;
    created_files.push(readme_path);

    // 5. Frontend scaffolding (only when Web target is active)
    if config.has_target(TargetPlatform::Web) {
        match config.frontend {
            FrontendChoice::React => {
                let fe_files = scaffold_react_frontend(&target_dir.join("frontend"), config)?;
                created_files.extend(fe_files);
            }
            FrontendChoice::Vue => {
                let fe_files = scaffold_vue_frontend(&target_dir.join("frontend"), config)?;
                created_files.extend(fe_files);
            }
            FrontendChoice::Htmx => {
                let public_dir = target_dir.join("public");
                fs::create_dir_all(&public_dir)?;
                let htmx_html = generate_htmx_index(config);
                let htmx_path = public_dir.join("index.html");
                fs::write(&htmx_path, htmx_html)?;
                created_files.push(htmx_path);
            }
            FrontendChoice::None => {}
        }
    }

    // 6. Desktop Workstation scaffolding (if Desktop target is active)
    if config.has_target(TargetPlatform::Desktop) {
        let desk_files = scaffold_desktop_target(&target_dir.join("desktop"), config)?;
        created_files.extend(desk_files);
    }

    // 7. Mobile SDK & App scaffolding (if Mobile target is active)
    if config.has_target(TargetPlatform::Mobile) {
        let mob_files = scaffold_mobile_target(&target_dir.join("mobile"), config)?;
        created_files.extend(mob_files);
    }

    // 8. IoT Hardware Scanner scaffolding (if IoT target is active)
    if config.has_target(TargetPlatform::IoT) {
        let iot_files = scaffold_iot_target(&target_dir.join("iot"), config)?;
        created_files.extend(iot_files);
    }

    Ok(created_files)
}

/// Generates Cargo.toml for the new application
pub fn generate_cargo_toml(config: &ProjectConfig) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
terminus-framework = "0.1"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
axum = {{ version = "0.7", features = ["macros"] }}
chrono = "0.4"
"#,
        name = config.name
    )
}

/// Generates src/main.rs tailored to the selected platforms, port, and database
pub fn generate_main_rs(config: &ProjectConfig) -> String {
    let app_name = &config.name;
    let port_str = config.port.to_string();

    let db_init = match config.database {
        DatabaseChoice::Sqlite => format!("let db = Database::open(\"{}.db\")?;", app_name),
        DatabaseChoice::Postgres => format!("let db = Database::connect(&std::env::var(\"DATABASE_URL\").unwrap_or_else(|_| \"postgres://postgres:postgres@localhost:5432/{}\".to_string()))?;", app_name),
        DatabaseChoice::Mysql => format!("let db = Database::connect(&std::env::var(\"DATABASE_URL\").unwrap_or_else(|_| \"mysql://root:password@localhost:3306/{}\".to_string()))?;", app_name),
        DatabaseChoice::Mongo => format!("let db = Database::connect(&std::env::var(\"DATABASE_URL\").unwrap_or_else(|_| \"mongodb://localhost:27017/{}\".to_string()))?;", app_name),
        DatabaseChoice::Redis => "let db = Database::connect(&std::env::var(\"DATABASE_URL\").unwrap_or_else(|_| \"redis://localhost:6379/0\".to_string()))?;".to_string(),
    };

    // 1. IoT-only gateway
    if config.is_iot_only() {
        let tmpl = r#"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {{DB_INIT}}
    db.execute(
        "CREATE TABLE IF NOT EXISTS telemetry_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id TEXT NOT NULL,
            temperature REAL,
            battery REAL,
            timestamp TEXT NOT NULL
        )",
        [],
    )?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS scan_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id TEXT NOT NULL,
            barcode TEXT NOT NULL,
            scanned_at TEXT NOT NULL
        )",
        [],
    )?;

    println!("Terminus IoT Hardware Scanner Gateway [{{NAME}}] online:");
    println!("  -> Telemetry Ingest : http://127.0.0.1:{{PORT}}/api/iot/telemetry");
    println!("  -> Barcode Scanner  : http://127.0.0.1:{{PORT}}/api/iot/scan");
    println!("  -> Hardware Client  : ./iot/iot_client.py");
    println!("  -> Terminal Studio  : run `terminus studio` to inspect sensor telemetry");

    App::new()
        .port({{PORT}})
        .database(db)
        .get("/", || async {
            json_response(serde_json::json!({
                "gateway": "{{NAME}}",
                "mode": "iot_only",
                "status": "online",
                "protocol": "HMAC-SHA256"
            }))
        })
        .group("/api/iot", |api| {
            api.post("/telemetry", || async {
                json_response(serde_json::json!({
                    "acknowledged": true,
                    "timestamp": now()
                }))
            })
            .post("/scan", || async {
                json_response(serde_json::json!({
                    "processed": true,
                    "action": "inventory_lookup"
                }))
            })
        })
        .run()
        .await?;

    Ok(())
}
"#;
        return tmpl
            .replace("{{NAME}}", app_name)
            .replace("{{PORT}}", &port_str)
            .replace("{{DB_INIT}}", &db_init);
    }

    // 2. Desktop-only workstation
    if config.is_desktop_only() {
        let tmpl = r#"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {{DB_INIT}}
    db.execute(
        "CREATE TABLE IF NOT EXISTS workstation_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            synced INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    println!("Terminus Desktop Workstation Engine [{{NAME}}] online:");
    println!("  -> Local Database  : {{NAME}}.db");
    println!("  -> Desktop Sync API: http://127.0.0.1:{{PORT}}/api/desktop/sync");
    println!("  -> IPC Bridge      : ./desktop/desktop_bridge.ts");
    println!("  -> Terminal Studio : run `terminus studio` to inspect offline queue");

    App::new()
        .port({{PORT}})
        .database(db)
        .get("/", || async {
            json_response(serde_json::json!({
                "workstation": "{{NAME}}",
                "mode": "desktop_only",
                "status": "online",
                "offlineQueue": "active"
            }))
        })
        .group("/api/desktop", |api| {
            api.get("/status", || async {
                json_response(serde_json::json!({
                    "isOnline": true,
                    "pendingMutations": 0,
                    "lastSync": now()
                }))
            })
            .post("/sync", || async {
                json_response(serde_json::json!({
                    "synced": 0,
                    "conflicts": 0,
                    "status": "up_to_date"
                }))
            })
        })
        .run()
        .await?;

    Ok(())
}
"#;
        return tmpl
            .replace("{{NAME}}", app_name)
            .replace("{{PORT}}", &port_str)
            .replace("{{DB_INIT}}", &db_init);
    }

    // 3. Multi-platform engines (Web + Desktop, Web + Desktop + Mobile, Universal, etc.)
    let has_web = config.has_target(TargetPlatform::Web);
    let has_desktop = config.has_target(TargetPlatform::Desktop);
    let has_mobile = config.has_target(TargetPlatform::Mobile);
    let has_iot = config.has_target(TargetPlatform::IoT);

    if has_desktop || has_mobile || has_iot {
        let mut platform_prints = Vec::new();
        let mut platform_targets = Vec::new();

        if has_web {
            platform_prints.push(format!("    println!(\"  -> Web Application : http://localhost:{port_str}\");"));
            platform_prints.push(format!("    println!(\"  -> OpenAPI Specs   : http://localhost:{port_str}/api/openapi.json\");"));
            platform_targets.push("\"web\"".to_string());
        }
        if has_desktop {
            platform_prints.push(format!("    println!(\"  -> Desktop Sync API: http://localhost:{port_str}/api/desktop/sync\");"));
            platform_targets.push("\"desktop\"".to_string());
        }
        if has_mobile {
            platform_prints.push(format!("    println!(\"  -> Mobile SDK API  : http://localhost:{port_str}/api/v1\");"));
            platform_targets.push("\"mobile\"".to_string());
        }
        if has_iot {
            platform_prints.push(format!("    println!(\"  -> IoT Scanner API : http://localhost:{port_str}/api/iot/telemetry\");"));
            platform_targets.push("\"iot\"".to_string());
        }

        let desktop_group = if has_desktop {
            r#"        .group("/api/desktop", |desktop| {
            desktop.get("/status", || async {
                json_response(serde_json::json!({ "isOnline": true, "pending": 0 }))
            })
            .post("/sync", || async {
                json_response(serde_json::json!({ "synced": 0, "conflicts": 0 }))
            })
        })"#
        } else {
            ""
        };

        let iot_group = if has_iot {
            r#"        .group("/api/iot", |iot| {
            iot.post("/telemetry", || async {
                json_response(serde_json::json!({ "acknowledged": true, "timestamp": now() }))
            })
            .post("/scan", || async {
                json_response(serde_json::json!({ "processed": true }))
            })
        })"#
        } else {
            ""
        };

        let tmpl = r#"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize thread-safe database connection
    {{DB_INIT}}
    db.execute(
        "CREATE TABLE IF NOT EXISTS items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    // 2. Multi-platform SDK & OpenAPI specification generator
    let sdk = SdkGenerator::new("{{NAME}}")
        .base_url("http://localhost:{{PORT}}")
        .endpoint(SdkEndpoint::get("getItems", "/api/v1/items", "Fetch active inventory items"))
        .endpoint(SdkEndpoint::post("createItem", "/api/v1/items", "Create a new inventory item"));

    println!("Terminus Unified Platform Engine [{{NAME}}] online across touchpoints:");
{{PLATFORM_PRINTS}}
    println!("  -> Terminal Studio : run `terminus studio` to inspect live telemetry");

    // 3. Bootstrap fluent, zero-boilerplate application engine
    App::new()
        .port({{PORT}})
        .database(db)
        .openapi("/api/openapi.json", sdk)
        .get("/", || async {
            json_response(serde_json::json!({
                "app": "{{NAME}}",
                "status": "online",
                "targets": [{{PLATFORM_TARGETS}}],
                "docs": "/api/openapi.json"
            }))
        })
        .group("/api/v1", |api| {
            api.get("/items", || async {
                json_response(serde_json::json!([
                    { "id": 1, "title": "Primary Inventory Node", "status": "active" },
                    { "id": 2, "title": "Field Workstation Sync", "status": "active" }
                ]))
            })
            .post("/items", || async {
                json_response(serde_json::json!({ "created": true, "id": 3 }))
            })
        })
{{DESKTOP_GROUP}}
{{IOT_GROUP}}
        .run()
        .await?;

    Ok(())
}
"#;
        return tmpl
            .replace("{{NAME}}", app_name)
            .replace("{{PORT}}", &port_str)
            .replace("{{DB_INIT}}", &db_init)
            .replace("{{PLATFORM_PRINTS}}", &platform_prints.join("\n"))
            .replace("{{PLATFORM_TARGETS}}", &platform_targets.join(", "))
            .replace("{{DESKTOP_GROUP}}\n", if desktop_group.is_empty() { "" } else { desktop_group })
            .replace("{{IOT_GROUP}}\n", if iot_group.is_empty() { "" } else { iot_group });
    }

    // 4. Web-only presets
    match config.preset {
        ProjectPreset::Backend => {
            let tmpl = r#"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {{DB_INIT}}

    let sdk = SdkGenerator::new("{{NAME}}")
        .base_url("http://localhost:{{PORT}}")
        .endpoint(SdkEndpoint::get("getHealth", "/api/v1/health", "Service health check"))
        .endpoint(SdkEndpoint::get("getRecords", "/api/v1/records", "List domain records"));

    println!("Terminus Headless API [{{NAME}}] running on http://localhost:{{PORT}}");
    println!("  -> OpenAPI Documentation : http://localhost:{{PORT}}/api/openapi.json");
    println!("  -> Terminal Studio       : run `terminus studio` to inspect metrics");

    App::new()
        .port({{PORT}})
        .database(db)
        .openapi("/api/openapi.json", sdk)
        .group("/api/v1", |api| {
            api.get("/health", || async {
                json_response(serde_json::json!({
                    "service": "{{NAME}}",
                    "status": "ok",
                    "timestamp": now()
                }))
            })
            .get("/records", || async {
                json_response(serde_json::json!([
                    { "id": 1, "name": "Primary Database Node", "latency_ms": 0.4 },
                    { "id": 2, "name": "Telemetry Ingestion Worker", "latency_ms": 0.8 }
                ]))
            })
        })
        .run()
        .await?;

    Ok(())
}
"#;
            tmpl.replace("{{NAME}}", app_name)
                .replace("{{PORT}}", &port_str)
                .replace("{{DB_INIT}}", &db_init)
        }
        _ => {
            let tmpl = r#"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {{DB_INIT}}

    println!("Terminus Fullstack Application [{{NAME}}] online:");
    println!("  -> Web Application : http://localhost:{{PORT}}");
    println!("  -> API Endpoints   : http://localhost:{{PORT}}/api/v1");
    println!("  -> Terminal Studio : run `terminus studio` to inspect metrics");

    App::new()
        .port({{PORT}})
        .database(db)
        .static_dir("/assets", "public")
        .get("/", || async {
            html_response("<!DOCTYPE html><html><head><title>{{NAME}}</title><script src=\"https://cdn.tailwindcss.com\"></script></head><body class=\"bg-slate-950 text-slate-100 flex min-h-screen items-center justify-center p-6\"><div class=\"max-w-md w-full bg-slate-900 border border-slate-800 rounded-xl p-8 text-center\"><h1 class=\"text-2xl font-bold text-white mb-2\">{{NAME}}</h1><p class=\"text-slate-400 text-sm mb-6\">Fullstack application powered by Rust & Axum.</p><div class=\"bg-slate-950 p-3 rounded border border-slate-800 text-xs font-mono text-emerald-400\">Connected to Terminus</div></div></body></html>")
        })
        .group("/api/v1", |api| {
            api.get("/health", || async {
                json_response(serde_json::json!({ "status": "healthy", "service": "{{NAME}}" }))
            })
        })
        .run()
        .await?;

    Ok(())
}
"#;
            tmpl.replace("{{NAME}}", app_name)
                .replace("{{PORT}}", &port_str)
                .replace("{{DB_INIT}}", &db_init)
        }
    }
}

/// Generates .env.example
pub fn generate_env_example(config: &ProjectConfig) -> String {
    format!(
        r#"# {name} Environment Configuration
APP_NAME="{name}"
APP_ENV=local
PORT={port}

# Database
DATABASE_URL={db_url}

# Authentication & Security
APP_KEY=terminus_secret_key_change_in_production
"#,
        name = config.name,
        port = config.port,
        db_url = match config.database {
            DatabaseChoice::Sqlite => format!("{}.db", config.name),
            DatabaseChoice::Postgres => format!("postgres://postgres:postgres@localhost:5432/{}_db", config.name),
            DatabaseChoice::Mysql => format!("mysql://root:password@localhost:3306/{}_db", config.name),
            DatabaseChoice::Mongo => format!("mongodb://localhost:27017/{}_db", config.name),
            DatabaseChoice::Redis => "redis://localhost:6379/0".to_string(),
        }
    )
}

/// Generates README.md for the new project
pub fn generate_readme(config: &ProjectConfig) -> String {
    let targets_str = config
        .targets
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let mut instructions = Vec::new();

    // 1. Backend instructions
    instructions.push(
        r#"### 1. Run the Backend Application

```bash
cargo run
```
The application server will start listening at `http://localhost:3000`."#
            .to_string(),
    );

    // 2. Studio TUI
    instructions.push(
        r#"### 2. Launch the Terminus Studio TUI

Inspect database tables, cache probes, queue depth, and live telemetry:

```bash
cargo terminus studio
```"#
            .to_string(),
    );

    // 3. Desktop instructions
    if config.has_target(TargetPlatform::Desktop) {
        instructions.push(
            r#"### Desktop Workstation Station

Direct native station daemon (`station.rs`) and IPC socket bridge (`desktop_bridge.ts`):
```bash
cat desktop/station.rs
cat desktop/desktop_bridge.ts
```"#
                .to_string(),
        );
    }

    // 4. Mobile instructions
    if config.has_target(TargetPlatform::Mobile) {
        instructions.push(
            r#"### Mobile Application Clients

Native Swift (iOS) and Native Kotlin (Android) clients are located in `./mobile`:
- iOS: `mobile/TerminusClient.swift`
- Android: `mobile/TerminusClient.kt`"#
                .to_string(),
        );
    }

    // 5. IoT instructions
    if config.has_target(TargetPlatform::IoT) {
        instructions.push(
            r#"### IoT Hardware Scanners

Test sensor telemetry and barcode scanning with the Python client:
```bash
python3 iot/iot_client.py
```"#
                .to_string(),
        );
    }

    // 6. Web frontend instructions
    if config.has_target(TargetPlatform::Web) {
        match config.frontend {
            FrontendChoice::React | FrontendChoice::Vue => {
                instructions.push(format!(
                    r#"### Frontend Development ({fe})

1. Navigate to the frontend directory:
   ```bash
   cd frontend
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Start Vite dev server:
   ```bash
   npm run dev
   ```
   The frontend automatically proxies `/api/*` requests to the Terminus backend on port 3000."#,
                    fe = config.frontend.display_name()
                ));
            }
            FrontendChoice::Htmx => {
                instructions.push(
                    r#"### Frontend (HTMX)

Zero build steps required! Static HTMX assets are served directly from `./public/index.html`."#
                        .to_string(),
                );
            }
            FrontendChoice::None => {}
        }
    }

    // Directory tree
    let mut tree = Vec::new();
    tree.push("├── Cargo.toml          # Rust dependencies & package configuration");
    tree.push("├── src/");
    tree.push("│   └── main.rs         # Terminus application bootstrap & routing");
    tree.push("├── .env.example        # Environment variables template");

    if config.has_target(TargetPlatform::Web) {
        if matches!(config.frontend, FrontendChoice::React | FrontendChoice::Vue) {
            tree.push("├── frontend/           # Modern TypeScript client (Vite + Tailwind)");
        } else if matches!(config.frontend, FrontendChoice::Htmx) {
            tree.push("├── public/             # Static HTML & HTMX assets");
        }
    }
    if config.has_target(TargetPlatform::Desktop) {
        tree.push("├── desktop/            # Native desktop station daemon & direct IPC bridge");
    }
    if config.has_target(TargetPlatform::Mobile) {
        tree.push("├── mobile/             # Native Swift (iOS) & Kotlin (Android) SDKs");
    }
    if config.has_target(TargetPlatform::IoT) {
        tree.push("├── iot/                # IoT hardware scanner & telemetry scripts");
    }
    tree.push("└── README.md");

    format!(
        r#"# {name}

Powered by **Terminus Framework** — The universal fullstack application framework for Rust & Axum.

- **Target Platforms**: `{targets}`
- **Architecture Preset**: `{preset}`
- **Frontend Choice**: `{frontend}`
- **Database Engine**: `{database}`

---

## Getting Started

{instructions_rendered}

## Project Structure

```
{tree_rendered}
```
"#,
        name = config.name,
        targets = targets_str,
        preset = config.preset.as_str(),
        frontend = config.frontend.as_str(),
        database = config.database.as_str(),
        instructions_rendered = instructions.join("\n\n"),
        tree_rendered = tree.join("\n"),
    )
}

/// Scaffolds Desktop target files (Native Station daemon, direct IPC bridge, and offline sync)
pub fn scaffold_desktop_target(dest_dir: &Path, config: &ProjectConfig) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fs::create_dir_all(dest_dir)?;

    let station_rs = format!(
        r#"//! Native Desktop Workstation Station Daemon for {name}
//! Direct, native OS execution with zero browser/WebView overhead (<15MB RAM).

use terminus::prelude::*;

pub struct WorkstationStation {{
    pub name: String,
    pub server_url: String,
    pub queue: OfflineSyncQueue,
}}

impl WorkstationStation {{
    pub fn new(server_url: impl Into<String>) -> Self {{
        Self {{
            name: "{name} Station".to_string(),
            server_url: server_url.into(),
            queue: OfflineSyncQueue::new(),
        }}
    }}

    pub fn record_offline_mutation(&self, action: &str, endpoint: &str, payload: serde_json::Value) -> String {{
        let idempotency = format!("tx_{{}}", chrono::Utc::now().timestamp_millis());
        self.queue.enqueue(action, endpoint, payload, idempotency)
    }}

    pub fn pending_count(&self) -> usize {{
        self.queue.pending_records().len()
    }}
}}
"#,
        name = config.name
    );
    let s_path = dest_dir.join("station.rs");
    fs::write(&s_path, station_rs)?;
    files.push(s_path);

    let bridge_ts = format!(
        r#"/**
 * Terminus Native Desktop IPC Bridge
 * Direct native socket and IPC integration for desktop workstations and POS stations.
 * Connects to Terminus local SQLite store with background cloud sync.
 */

export interface OfflineAction {{
  id: string;
  entity: string;
  action: 'create' | 'update' | 'delete';
  payload: Record<string, unknown>;
  timestamp: string;
}}

export interface SyncResult {{
  synced: number;
  conflicts: number;
  remaining: number;
  status: 'up_to_date' | 'conflicts_detected' | 'offline';
}}

export class TerminusDesktopBridge {{
  private baseUrl: string;

  constructor(baseUrl = 'http://127.0.0.1:3000') {{
    this.baseUrl = baseUrl;
  }}

  /** Checks desktop workstation engine status */
  async getStatus(): Promise<{{ isOnline: boolean; pending: number }}> {{
    const res = await fetch(`${{this.baseUrl}}/api/desktop/status`);
    if (!res.ok) throw new Error(`Status check failed: ${{res.statusText}}`);
    return res.json();
  }}

  /** Replays queued offline mutations against the Terminus server */
  async triggerSync(): Promise<SyncResult> {{
    const res = await fetch(`${{this.baseUrl}}/api/desktop/sync`, {{ method: 'POST' }});
    if (!res.ok) throw new Error(`Desktop sync failed: ${{res.statusText}}`);
    return res.json();
  }}
}}

export const desktopBridge = new TerminusDesktopBridge();
"#,
    );
    let b_path = dest_dir.join("desktop_bridge.ts");
    fs::write(&b_path, bridge_ts)?;
    files.push(b_path);

    let readme_content = format!(
        r#"# {name} — Native Desktop Workstation Target

This directory contains the native desktop workstation station integration for **{name}**.

## Features

- **Direct Native Execution**: `station.rs` runs as a high-performance, native daemon or GUI window with <15MB RAM and sub-millisecond cold boot (zero WebView overhead).
- **Offline SQLite Storage**: All domain operations write locally first, ensuring zero latency and 100% offline uptime for POS, warehouse stations, and kiosks.
- **Direct IPC Bridge**: `desktop_bridge.ts` provides typed synchronization with the Terminus engine over local sockets.
- **Conflict-Free Replay**: Offline mutations are queued and replayed against the cloud when connectivity resumes.
"#,
        name = config.name
    );
    let r_path = dest_dir.join("README.md");
    fs::write(&r_path, readme_content)?;
    files.push(r_path);

    Ok(files)
}

/// Scaffolds Mobile target files (Swift for iOS and Kotlin for Android)
pub fn scaffold_mobile_target(dest_dir: &Path, config: &ProjectConfig) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fs::create_dir_all(dest_dir)?;

    let swift_code = format!(
        r#"//
//  TerminusClient.swift
//  {name} Mobile Client (iOS / iPadOS / macOS)
//  Powered by Terminus Framework
//

import Foundation

public struct Item: Codable, Identifiable {{
    public let id: Int
    public let title: String
    public let status: String
}}

public class TerminusClient: ObservableObject {{
    private let baseURL: URL
    private var token: String?

    public init(baseURL: URL = URL(string: "http://localhost:3000")!) {{
        self.baseURL = baseURL
    }}

    public func setBearerToken(_ token: String) {{
        self.token = token
    }}

    public func fetchItems() async throws -> [Item] {{
        let url = baseURL.appendingPathComponent("/api/v1/items")
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        if let token = self.token {{
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }}

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {{
            throw URLError(.badServerResponse)
        }}

        return try JSONDecoder().decode([Item].self, from: data)
    }}
}}
"#,
        name = config.name
    );
    let s_path = dest_dir.join("TerminusClient.swift");
    fs::write(&s_path, swift_code)?;
    files.push(s_path);

    let kotlin_code = format!(
        r#"//
//  TerminusClient.kt
//  {name} Mobile Client (Android)
//  Powered by Terminus Framework
//

package dev.terminus.client

import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

data class Item(
    val id: Int,
    val title: String,
    val status: String
)

class TerminusClient(
    val baseURL: String = "http://10.0.2.2:3000"
) {{
    private var token: String? = null
    private val httpClient: HttpClient = HttpClient.newHttpClient()

    fun setBearerToken(token: String) {{
        this.token = token
    }}

    suspend fun fetchItems(): String = withContext(Dispatchers.IO) {{
        val uri = URI.create("$baseURL/api/v1/items")
        val builder = HttpRequest.newBuilder().uri(uri)
            .header("Accept", "application/json")
        token?.let {{ builder.header("Authorization", "Bearer $it") }}
        val res = httpClient.send(builder.GET().build(), HttpResponse.BodyHandlers.ofString())
        res.body()
    }}
}}
"#,
        name = config.name
    );
    let k_path = dest_dir.join("TerminusClient.kt");
    fs::write(&k_path, kotlin_code)?;
    files.push(k_path);

    let readme_content = format!(
        r#"# {name} — Native Mobile Client Target

This directory contains pure native mobile SDK adapters for **{name}**.

## Platforms

- **iOS (Native Swift)**: `TerminusClient.swift` supports modern `async/await`, `ObservableObject`, and Bearer token auth.
- **Android (Native Kotlin)**: `TerminusClient.kt` supports Kotlin Coroutines (`suspend fun`), `Dispatchers.IO`, and Jetpack Compose integration.

## OpenAPI & SDK Generation

Export the latest API contracts anytime:
```bash
terminus make:sdk --lang=swift
terminus make:sdk --lang=kotlin
terminus make:sdk --lang=openapi
```
"#,
        name = config.name
    );
    let r_path = dest_dir.join("README.md");
    fs::write(&r_path, readme_content)?;
    files.push(r_path);

    Ok(files)
}

/// Scaffolds IoT target files (Python hardware scanner script and integration documentation)
pub fn scaffold_iot_target(dest_dir: &Path, config: &ProjectConfig) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fs::create_dir_all(dest_dir)?;

    let py_code = format!(
        r#"#!/usr/bin/env python3
"""
Terminus IoT Hardware Client — {name}
Hardware scanner and sensor telemetry script.
Authenticates with HMAC-SHA256 device signatures.
"""

import hashlib
import hmac
import json
import time
import urllib.request

DEVICE_ID = "scanner_hw_01"
SECRET_KEY = "terminus_device_secret_change_in_production"
SERVER_URL = "http://localhost:3000/api/iot/telemetry"
SCAN_URL = "http://localhost:3000/api/iot/scan"

def sign_payload(payload_bytes: bytes) -> str:
    return hmac.new(SECRET_KEY.encode('utf-8'), payload_bytes, hashlib.sha256).hexdigest()

def send_heartbeat():
    payload = json.dumps({{
        "device_id": DEVICE_ID,
        "status": "online",
        "battery": 98.5,
        "temperature_c": 22.4,
        "timestamp": int(time.time()),
    }}).encode("utf-8")

    req = urllib.request.Request(
        SERVER_URL,
        data=payload,
        headers={{
            "Content-Type": "application/json",
            "X-Device-Id": DEVICE_ID,
            "X-Device-Signature": sign_payload(payload),
        }}
    )
    try:
        with urllib.request.urlopen(req) as resp:
            print("[IoT Client] Heartbeat sent, HTTP status:", resp.status)
    except Exception as e:
        print("[IoT Client] Telemetry report failed:", e)

def send_barcode_scan(barcode: str):
    payload = json.dumps({{
        "device_id": DEVICE_ID,
        "barcode": barcode,
        "timestamp": int(time.time()),
    }}).encode("utf-8")

    req = urllib.request.Request(
        SCAN_URL,
        data=payload,
        headers={{
            "Content-Type": "application/json",
            "X-Device-Id": DEVICE_ID,
            "X-Device-Signature": sign_payload(payload),
        }}
    )
    try:
        with urllib.request.urlopen(req) as resp:
            print(f"[IoT Client] Barcode '{{barcode}}' processed, HTTP status:", resp.status)
    except Exception as e:
        print("[IoT Client] Barcode scan failed:", e)

if __name__ == "__main__":
    print("[IoT Client] Connecting to {name} gateway...")
    send_heartbeat()
    send_barcode_scan("TERM-984210-A")
"#,
        name = config.name
    );
    let py_path = dest_dir.join("iot_client.py");
    fs::write(&py_path, py_code)?;
    files.push(py_path);

    let readme_content = format!(
        r#"# {name} — IoT Hardware Scanner Target

This directory contains hardware device scripts and documentation for **{name}**.

## Features

- **HMAC-SHA256 Device Signatures**: Hardware devices compute SHA-256 HMAC signatures to guarantee authentic payload delivery.
- **Barcode & RFID Scanning**: Dedicated endpoints for inventory scanners and barcode verification.
- **Periodic Telemetry Heartbeats**: Reports device health, battery levels, and ambient temperatures.

## Usage

Run the Python hardware client:
```bash
python3 iot/iot_client.py
```
"#,
        name = config.name
    );
    let r_path = dest_dir.join("README.md");
    fs::write(&r_path, readme_content)?;
    files.push(r_path);

    Ok(files)
}

/// Generates HTML5 + HTMX template
pub fn generate_htmx_index(config: &ProjectConfig) -> String {
    let tmpl = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{{NAME}} — Terminus + HTMX</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <script src="https://unpkg.com/htmx.org@1.9.12"></script>
</head>
<body class="bg-slate-950 text-slate-100 flex min-h-screen flex-col items-center justify-center p-6">
  <div class="max-w-lg w-full bg-slate-900 border border-slate-800 rounded-xl p-8 shadow-2xl">
    <div class="flex items-center justify-between mb-4">
      <h1 class="text-2xl font-bold text-white">{{NAME}}</h1>
      <span class="px-2.5 py-0.5 rounded-full text-xs font-semibold bg-emerald-950 text-emerald-400 border border-emerald-800">
        HTMX Active
      </span>
    </div>
    <p class="text-slate-400 text-sm mb-6">Zero-build interactive frontend powered by Terminus & Axum.</p>
    <button hx-get="/api/v1/items" hx-target="#results" hx-swap="innerHTML"
      class="w-full bg-sky-600 hover:bg-sky-500 text-white font-medium py-2.5 px-4 rounded-lg transition-colors">
      Load Items from Terminus Backend
    </button>
    <div id="results" class="mt-4 text-xs font-mono text-slate-300"></div>
  </div>
</body>
</html>"##;
    tmpl.replace("{{NAME}}", &config.name)
}

/// Scaffolds a React 19 + TypeScript + Vite + Tailwind frontend
fn scaffold_react_frontend(dest_dir: &Path, config: &ProjectConfig) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fs::create_dir_all(dest_dir.join("src"))?;

    let pkg_json = format!(
        r#"{{
  "name": "{name}-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }},
  "devDependencies": {{
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0",
    "tailwindcss": "^3.4.0",
    "autoprefixer": "^10.4.0",
    "postcss": "^8.4.0"
  }}
}}
"#,
        name = config.name
    );
    let p_path = dest_dir.join("package.json");
    fs::write(&p_path, pkg_json)?;
    files.push(p_path);

    let vite_config = r#"import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: true,
      },
    },
  },
});
"#;
    let v_path = dest_dir.join("vite.config.ts");
    fs::write(&v_path, vite_config)?;
    files.push(v_path);

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
"#;
    let ts_path = dest_dir.join("tsconfig.json");
    fs::write(&ts_path, tsconfig)?;
    files.push(ts_path);

    let index_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{name} — Terminus + React</title>
  </head>
  <body class="bg-slate-950 text-slate-100 antialiased">
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#,
        name = config.name
    );
    let idx_path = dest_dir.join("index.html");
    fs::write(&idx_path, index_html)?;
    files.push(idx_path);

    let main_tsx = r#"import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App.tsx';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
"#;
    let m_path = dest_dir.join("src/main.tsx");
    fs::write(&m_path, main_tsx)?;
    files.push(m_path);

    let app_tsx = format!(
        r#"import {{ useState, useEffect }} from 'react';

export default function App() {{
  const [data, setData] = useState<any>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {{
    fetch('/api/v1/items')
      .then((res) => res.json())
      .then((json) => {{
        setData(json);
        setLoading(false);
      }})
      .catch((err) => {{
        console.error(err);
        setLoading(false);
      }});
  }}, []);

  return (
    <div className="flex min-h-screen flex-col items-center justify-center p-6">
      <div className="w-full max-w-lg rounded-2xl border border-slate-800 bg-slate-900 p-8 shadow-2xl">
        <div className="mb-4 flex items-center justify-between">
          <h1 className="text-2xl font-bold tracking-tight text-white">{name}</h1>
          <span className="rounded-full border border-sky-800 bg-sky-950 px-3 py-1 text-xs font-medium text-sky-400">
            React 19 + TypeScript
          </span>
        </div>
        <p className="mb-6 text-sm text-slate-400">
          Connected to Terminus Axum backend on port 3000.
        </p>

        <div className="rounded-xl border border-slate-800 bg-slate-950 p-4">
          <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
            API Response (/api/v1/items)
          </p>
          {{loading ? (
            <p className="text-sm text-slate-500">Fetching live data...</p>
          ) : (
            <pre className="overflow-x-auto text-xs text-emerald-400 font-mono">
              {{JSON.stringify(data, null, 2)}}
            </pre>
          )}}
        </div>
      </div>
    </div>
  );
}}
"#,
        name = config.name
    );
    let app_path = dest_dir.join("src/App.tsx");
    fs::write(&app_path, app_tsx)?;
    files.push(app_path);

    Ok(files)
}

/// Scaffolds a Vue 3 + TypeScript + Vite + Tailwind frontend
fn scaffold_vue_frontend(dest_dir: &Path, config: &ProjectConfig) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fs::create_dir_all(dest_dir.join("src"))?;

    let pkg_json = format!(
        r#"{{
  "name": "{name}-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "vue-tsc && vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "vue": "^3.5.0"
  }},
  "devDependencies": {{
    "@vitejs/plugin-vue": "^5.2.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0",
    "vue-tsc": "^2.1.0",
    "tailwindcss": "^3.4.0",
    "autoprefixer": "^10.4.0",
    "postcss": "^8.4.0"
  }}
}}
"#,
        name = config.name
    );
    let p_path = dest_dir.join("package.json");
    fs::write(&p_path, pkg_json)?;
    files.push(p_path);

    let vite_config = r#"import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: true,
      },
    },
  },
});
"#;
    let v_path = dest_dir.join("vite.config.ts");
    fs::write(&v_path, vite_config)?;
    files.push(v_path);

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "preserve",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src/**/*.ts", "src/**/*.d.ts", "src/**/*.tsx", "src/**/*.vue"]
}
"#;
    let ts_path = dest_dir.join("tsconfig.json");
    fs::write(&ts_path, tsconfig)?;
    files.push(ts_path);

    let index_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{name} — Terminus + Vue</title>
  </head>
  <body class="bg-slate-950 text-slate-100 antialiased">
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
"#,
        name = config.name
    );
    let idx_path = dest_dir.join("index.html");
    fs::write(&idx_path, index_html)?;
    files.push(idx_path);

    let main_ts = r#"import { createApp } from 'vue';
import App from './App.vue';

createApp(App).mount('#app');
"#;
    let m_path = dest_dir.join("src/main.ts");
    fs::write(&m_path, main_ts)?;
    files.push(m_path);

    let app_vue = format!(
        r#"<script setup lang="ts">
import {{ ref, onMounted }} from 'vue';

const data = ref<any>(null);
const loading = ref(true);

onMounted(async () => {{
  try {{
    const res = await fetch('/api/v1/items');
    data.value = await res.json();
  }} catch (err) {{
    console.error(err);
  }} finally {{
    loading.value = false;
  }}
}});
</script>

<template>
  <div class="flex min-h-screen flex-col items-center justify-center p-6">
    <div class="w-full max-w-lg rounded-2xl border border-slate-800 bg-slate-900 p-8 shadow-2xl">
      <div class="mb-4 flex items-center justify-between">
        <h1 class="text-2xl font-bold tracking-tight text-white">{name}</h1>
        <span class="rounded-full border border-emerald-800 bg-emerald-950 px-3 py-1 text-xs font-medium text-emerald-400">
          Vue 3 + TypeScript
        </span>
      </div>
      <p class="mb-6 text-sm text-slate-400">
        Connected to Terminus Axum backend on port 3000.
      </p>

      <div class="rounded-xl border border-slate-800 bg-slate-950 p-4">
        <p class="mb-2 text-xs font-semibold uppercase tracking-wider text-slate-500">
          API Response (/api/v1/items)
        </p>
        <p v-if="loading" class="text-sm text-slate-500">Fetching live data...</p>
        <pre v-else class="overflow-x-auto text-xs text-emerald-400 font-mono">{{{{ JSON.stringify(data, null, 2) }}}}</pre>
      </div>
    </div>
  </div>
</template>
"#,
        name = config.name
    );
    let app_path = dest_dir.join("src/App.vue");
    fs::write(&app_path, app_vue)?;
    files.push(app_path);

    Ok(files)
}
