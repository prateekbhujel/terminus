//! Terminus Project Scaffolder & Installation Wizard
//!
//! Interactive project bootstrapping and template scaffolding
//! for batteries-included Axum web applications and REST APIs.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Target platforms supported by Terminus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetPlatform {
    /// Web applications (HTTP Server, Admin Dashboard, REST & OpenAPI 3.0)
    Web,
}

impl TargetPlatform {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "web" | "w" | "browser" | "http" | "api" | "server" => Some(Self::Web),
            _ => Some(Self::Web),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Web => "web",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Web => "Web (HTTP Server, Admin UI, REST & OpenAPI 3.0)",
        }
    }
}

/// Parses a comma-separated string into a list of target platforms
pub fn parse_targets(s: &str) -> Vec<TargetPlatform> {
    let _ = s;
    vec![TargetPlatform::Web]
}

/// Architecture presets available during project creation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectPreset {
    /// Fullstack: Backend API + Built-in Admin Dashboard UI & static assets
    Fullstack,
    /// Headless Backend API: Pure high-throughput JSON API, OpenAPI 3.0 specs, zero frontend bloat
    Backend,
}

impl ProjectPreset {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "fullstack" | "admin" | "dashboard" | "f" | "universal" | "all" => Some(Self::Fullstack),
            "backend" | "api" | "headless" | "b" => Some(Self::Backend),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fullstack => "fullstack",
            Self::Backend => "backend",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Fullstack => "Fullstack (Backend API + Built-in Admin Dashboard)",
            Self::Backend => "Backend API (High-throughput JSON API + OpenAPI 3.0 Specs)",
        }
    }
}

/// Frontend client choices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendChoice {
    /// Headless API or built-in Admin Dashboard
    None,
    /// HTML5 + HTMX + Tailwind CSS (Zero JavaScript build step, SSR)
    Htmx,
}

impl FrontendChoice {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" | "no" | "n" | "api" => Some(Self::None),
            "htmx" | "h" | "html" => Some(Self::Htmx),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Htmx => "htmx",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None (Headless API / Built-in Admin Dashboard)",
            Self::Htmx => "HTMX + Tailwind CSS (Zero build step, SSR)",
        }
    }
}

/// Database engine choices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseChoice {
    /// SQLite (Embedded, zero configuration single-file database)
    Sqlite,
}

impl DatabaseChoice {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sqlite" | "sqlite3" | "sq" | "default" => Some(Self::Sqlite),
            _ => Some(Self::Sqlite),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite (Zero configuration single-file database)",
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
        self.targets = vec![TargetPlatform::Web];
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

    // 5. migrations/001_create_users_table.sql
    let mig_dir = target_dir.join("migrations");
    fs::create_dir_all(&mig_dir)?;
    let mig_sql = r#"-- Terminus Initial Migration: Create Users Table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
"#;
    let mig_path = mig_dir.join("001_create_users_table.sql");
    fs::write(&mig_path, mig_sql)?;
    created_files.push(mig_path);

    // 6. public/index.html (for Fullstack or HTMX)
    if config.preset == ProjectPreset::Fullstack || config.frontend == FrontendChoice::Htmx {
        let pub_dir = target_dir.join("public");
        fs::create_dir_all(&pub_dir)?;
        let index_html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{name}</title>
  <script src="https://cdn.tailwindcss.com"></script>
</head>
<body class="bg-slate-950 text-slate-100 flex min-h-screen items-center justify-center p-6">
  <div class="max-w-md w-full bg-slate-900 border border-slate-800 rounded-xl p-8 text-center shadow-2xl">
    <h1 class="text-2xl font-bold text-white mb-2">{name}</h1>
    <p class="text-slate-400 text-sm mb-6">High-performance web application powered by Terminus & Axum.</p>
    <div class="bg-slate-950 p-3 rounded border border-slate-800 text-xs font-mono text-emerald-400">
      SQLite Persistence & HMAC Sessions Active
    </div>
  </div>
</body>
</html>
"#,
            name = config.name
        );
        let index_path = pub_dir.join("index.html");
        fs::write(&index_path, index_html)?;
        created_files.push(index_path);
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

/// Generates src/main.rs tailored to the selected preset and port
pub fn generate_main_rs(config: &ProjectConfig) -> String {
    let app_name = &config.name;
    let port_str = config.port.to_string();

    match config.preset {
        ProjectPreset::Backend => {
            let tmpl = r#"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("{{NAME}}.db")?;

    let sdk = SdkGenerator::new("{{NAME}}", "1.0.0")
        .endpoint("GET", "/api/v1/health", None, "{ status: string }", "Service health check")
        .endpoint("GET", "/api/v1/records", None, "Record[]", "List domain records");

    println!("Terminus Headless API [{{NAME}}] running on http://127.0.0.1:{{PORT}}");
    println!("  -> OpenAPI Documentation : http://127.0.0.1:{{PORT}}/api/openapi.json");
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
                    { "id": 2, "name": "Worker Node", "latency_ms": 0.8 }
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
        }
        ProjectPreset::Fullstack => {
            let tmpl = r##"use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("{{NAME}}.db")?;

    println!("Terminus Fullstack Application [{{NAME}}] online:");
    println!("  -> Web Application : http://127.0.0.1:{{PORT}}");
    println!("  -> API Endpoints   : http://127.0.0.1:{{PORT}}/api/v1");
    println!("  -> Terminal Studio : run `terminus studio` to inspect metrics");

    App::new()
        .port({{PORT}})
        .database(db)
        .static_dir("/assets", "public")
        .get("/", || async {
            html_response(r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>{{NAME}}</title>
  <script src="https://cdn.tailwindcss.com"></script>
</head>
<body class="bg-slate-950 text-slate-100 flex min-h-screen items-center justify-center p-6">
  <div class="max-w-md w-full bg-slate-900 border border-slate-800 rounded-xl p-8 text-center shadow-2xl">
    <h1 class="text-2xl font-bold text-white mb-2">{{NAME}}</h1>
    <p class="text-slate-400 text-sm mb-6">High-performance web application powered by Terminus & Axum.</p>
    <div class="bg-slate-950 p-3 rounded border border-slate-800 text-xs font-mono text-emerald-400">
      SQLite Persistence & HMAC Sessions Active
    </div>
  </div>
</body>
</html>"#)
        })
        .group("/api/v1", |api| {
            api.get("/health", || async {
                json_response(serde_json::json!({
                    "service": "{{NAME}}",
                    "status": "healthy"
                }))
            })
        })
        .run()
        .await?;

    Ok(())
}
"##;
            tmpl.replace("{{NAME}}", app_name)
                .replace("{{PORT}}", &port_str)
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
DATABASE_URL=sqlite://{name}.db

# Authentication & Security
APP_KEY=terminus_secret_key_change_in_production
"#,
        name = config.name,
        port = config.port
    )
}

/// Generates README.md for the new project
pub fn generate_readme(config: &ProjectConfig) -> String {
    format!(
        r#"# {name}

Powered by **Terminus Framework** — The agent-native application framework for Rust & Axum.

- **Preset**: `{preset}`
- **Database**: `{database}`
- **Port**: `{port}`

---

## Getting Started

### 1. Run Migrations

```bash
cargo terminus migrate
```

### 2. Run the Application

```bash
cargo run
```

The application server will start listening at `http://127.0.0.1:{port}`.

### 3. Launch the Terminus Studio TUI

Inspect database tables, cache, routes, and application metrics:

```bash
cargo terminus studio
```

## Project Structure

```
├── Cargo.toml          # Rust dependencies & package configuration
├── src/
│   └── main.rs         # Terminus application bootstrap & routing
├── migrations/         # SQL schema migration files
├── public/             # Static HTML assets
├── .env.example        # Environment variables template
└── README.md
```
"#,
        name = config.name,
        preset = config.preset.as_str(),
        database = config.database.as_str(),
        port = config.port,
    )
}
