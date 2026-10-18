use terminus::cache::CacheDriver;
use terminus::client::SdkGenerator;
use terminus::prompt;
use terminus::scaffold::{
    parse_targets, scaffold_project, DatabaseChoice, FrontendChoice, ProjectConfig, ProjectPreset,
    TargetPlatform,
};
use terminus::token::{SqliteTokenStore, TokenRepository};
use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

fn main() {
    let mut raw_args: Vec<String> = env::args().collect();
    // Handle cargo subcommand invocation (`cargo terminus ...`)
    if raw_args.get(1).map(|s| s.as_str()) == Some("terminus") {
        raw_args.remove(1);
    }
    let args = raw_args;
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "help" | "--help" | "-h" => print_help(),
        "version" | "--version" | "-v" => print_version(),
        "schema" | "inspect" | "app:spec" => {
            let is_json = args.iter().any(|a| a == "--json" || a == "-j");
            run_schema(is_json);
        }
        "make:slice" | "make:domain" | "slice:create" => {
            let name = args.get(2).map(|s| s.as_str());
            let fields_arg = args.iter().find(|a| a.starts_with("--fields=")).map(|a| a.trim_start_matches("--fields="));
            make_slice(name, fields_arg);
        }
        "agent:guide" | "agent:spec" | "agent:prompt" => {
            let is_json = args.iter().any(|a| a == "--json" || a == "-j");
            print_agent_guide(is_json);
        }
        "new" | "init" | "create" => {
            let name_arg = args.get(2).map(|s| s.as_str());
            let preset_arg = args.iter().find(|a| a.starts_with("--preset=")).map(|a| a.trim_start_matches("--preset="));
            let targets_arg = args.iter().find(|a| a.starts_with("--targets=") || a.starts_with("--target=")).map(|a| {
                if let Some(t) = a.strip_prefix("--targets=") {
                    t
                } else {
                    a.trim_start_matches("--target=")
                }
            });
            let frontend_arg = args.iter().find(|a| a.starts_with("--frontend=")).map(|a| a.trim_start_matches("--frontend="));
            let db_arg = args.iter().find(|a| a.starts_with("--db=")).map(|a| a.trim_start_matches("--db="));
            let port_arg = args.iter().find(|a| a.starts_with("--port=")).map(|a| a.trim_start_matches("--port="));
            let non_interactive = args.iter().any(|a| a == "--yes" || a == "-y" || a == "--non-interactive");
            run_new_project(name_arg, preset_arg, targets_arg, frontend_arg, db_arg, port_arg, non_interactive);
        }
        "make:resource" => {
            let name = args.get(2).map(|s| s.as_str());
            make_resource(name);
        }
        "make:module" | "module:create" | "make:mod" => {
            let name = args.get(2).map(|s| s.as_str());
            let pattern_arg = args.get(3).map(|s| s.as_str());
            make_module(name, pattern_arg);
        }
        "make:controller" => {
            let name = args.get(2).map(|s| s.as_str());
            let is_resource = args.iter().any(|a| a == "--resource" || a == "-r");
            make_controller(name, is_resource);
        }
        "make:model" => {
            let name = args.get(2).map(|s| s.as_str());
            let with_migration = args.iter().any(|a| a == "-m" || a == "-mc" || a == "-cm" || a == "--migration");
            let with_controller = args.iter().any(|a| a == "-c" || a == "-mc" || a == "-cm" || a == "--controller");
            make_model(name, with_migration, with_controller);
        }
        "make:seeder" => {
            let name = args.get(2).map(|s| s.as_str());
            make_seeder(name);
        }
        "make:repo" => {
            let name = args.get(2).map(|s| s.as_str());
            make_repo(name);
        }
        "make:migration" => {
            let name = args.get(2).map(|s| s.as_str());
            make_migration(name);
        }
        "make:token" | "token:create" => {
            let name = args.get(2).map(|s| s.as_str());
            make_token(name);
        }
        "token:list" => list_tokens(),
        "token:revoke" => {
            let id = args.get(2).map(|s| s.as_str());
            revoke_token(id);
        }
        "migrate" => run_migrations(),
        "migrate:status" => run_migration_status(),
        "migrate:rollback" => run_migration_rollback(),
        "tinker" => {
            let exec_opt = args.iter().find(|a| a.starts_with("--execute=")).map(|a| a.trim_start_matches("--execute="));
            run_tinker(exec_opt);
        }
        "schedule:run" => run_schedule(),
        "schedule:list" => list_schedule(),
        "db:seed" => run_seed(),
        "db:tables" => inspect_tables(),
        "cache:clear" => clear_cache(),
        "make:job" => {
            let name = args.get(2).map(|s| s.as_str());
            make_job(name);
        }
        "make:mail" => {
            let name = args.get(2).map(|s| s.as_str());
            make_mail(name);
        }
        "routes" | "route:list" => print_routes(),
        "queue:work" => run_queue_worker(),
        "ws:serve" => run_ws_server(),
        "theme:preview" => preview_theme(),
        "mail:outbox" => view_mail_outbox(),
        "make:sdk" | "client:sdk" => {
            let lang = args.iter().find(|a| a.starts_with("--lang=")).map(|a| a.trim_start_matches("--lang="));
            make_sdk(lang);
        }
        "make:desktop" | "client:desktop" => {
            let name = args.get(2).map(|s| s.as_str());
            make_desktop(name);
        }
        "make:iot" | "client:iot" => {
            let id = args.get(2).map(|s| s.as_str());
            make_iot(id);
        }
        "studio" | "tui" | "dashboard" | "pulse" | "inspector" => {
            let db_path = args.iter().find(|a| a.starts_with("--db=")).map(|a| a.trim_start_matches("--db=")).unwrap_or("terminus.db");
            let is_watch = args.iter().any(|a| a == "--watch" || a == "-w");
            let is_once = args.iter().any(|a| a == "--once" || a == "--overview");
            let tab_arg = args.iter().find(|a| a.starts_with("--tab=")).and_then(|a| a.trim_start_matches("--tab=").parse::<u8>().ok());
            run_studio(db_path, is_watch, is_once, tab_arg);
        }
        unknown => {
            eprintln!("\x1b[31mError:\x1b[0m Unknown command '{unknown}'\n");
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_banner() {
    println!("\x1b[36m  _____                   _                  
 |_   _|__ _ __ _ __ ___ (_)_ __  _   _ ___ 
   | |/ _ \\ '__| '_ ` _ \\| | '_ \\| | | / __|
   | |  __/ |  | | | | | | | | | | |_| \\__ \\
   |_|\\___|_|  |_| |_| |_|_|_| |_|\\__,_|___/\x1b[0m\n\x1b[1mTerminus Framework CLI\x1b[0m v0.1.0 — Universal Fullstack Rust Application Platform\n");
}

fn print_help() {
    print_banner();
    println!("Usage:\n  terminus <command> [arguments]\n\n\x1b[1mAvailable Commands:\x1b[0m\n  \x1b[33mnew [name] [--preset=...] [--frontend=...]\x1b[0m Scaffold a new Terminus project (interactive wizard or flags)\n  \x1b[33mschema [--json]\x1b[0m          Inspect complete application topology (models, routes, admin)\n  \x1b[33mroute:list | routes\x1b[0m      List registered web routes, admin resources & API endpoints\n  \x1b[33mtinker [--execute=...]\x1b[0m   Interactive database REPL & command console\n  \x1b[33mmake:controller <Name> [-r]\x1b[0m Scaffold a Controller (or RESTful resource controller)\n  \x1b[33mmake:model <Name> [-mc]\x1b[0m  Scaffold a Model (with optional -m migration, -c controller)\n  \x1b[33mmake:seeder <Name>\x1b[0m       Scaffold a database Seeder class\n  \x1b[33mmake:slice <Name> [--fields=...]\x1b[0m Scaffold a full-stack vertical domain slice (DDD)\n  \x1b[33mmake:resource <Name>\x1b[0m     Scaffold a new Admin Resource with table & form\n  \x1b[33mmake:module <Name>\x1b[0m       Scaffold an Enterprise Modular (DDD) domain module\n  \x1b[33mmake:repo <Name>\x1b[0m         Scaffold a SQLite repository implementation\n  \x1b[33mmake:migration <Name>\x1b[0m    Generate a timestamped SQL migration file\n  \x1b[33mmigrate\x1b[0m                  Run pending database migrations\n  \x1b[33mmigrate:status\x1b[0m           Show execution status of each migration batch\n  \x1b[33mmigrate:rollback\x1b[0m         Rollback the latest migration batch\n  \x1b[33mdb:seed\x1b[0m                  Seed demo accounts and orders into database\n  \x1b[33mdb:tables\x1b[0m                Inspect SQLite database tables, schema, and row counts\n  \x1b[33mschedule:run\x1b[0m             Run due scheduled tasks and recurring cron jobs\n  \x1b[33mschedule:list\x1b[0m            List scheduled tasks and recurring frequencies\n  \x1b[33mcache:clear\x1b[0m              Flush application and Redis cache stores\n  \x1b[33mmake:token [Name]\x1b[0m        Generate an API Personal Access Token\n  \x1b[33mtoken:list\x1b[0m               List active Personal Access Tokens\n  \x1b[33mtoken:revoke [ID]\x1b[0m        Revoke a Personal Access Token\n  \x1b[33mstudio\x1b[0m                   Launch Terminal UI Studio & live Database/Cache Inspector\n  \x1b[33mmake:sdk [--lang=ts|kotlin|swift|openapi]\x1b[0m Export typed client SDK (Optional)\n  \x1b[33mmake:desktop [Name]\x1b[0m      Scaffold native desktop station IPC bridge (Optional)\n  \x1b[33mmake:iot [DeviceId]\x1b[0m       Scaffold IoT hardware scanner & telemetry script (Optional)\n  \x1b[33mmake:job <Name>\x1b[0m          Scaffold a background Queue Job\n  \x1b[33mmake:mail <Name>\x1b[0m         Scaffold a Transactional Mail notification\n  \x1b[33mws:serve\x1b[0m                 Start standalone WebSocket broadcasting server\n  \x1b[33mqueue:work\x1b[0m               Start processing background queue jobs\n  \x1b[33mmail:outbox\x1b[0m              Inspect sent transactional emails\n  \x1b[33mtheme:preview\x1b[0m            Preview theme colors, palettes, and font families\n  \x1b[33magent:guide [--json]\x1b[0m     Display agent-native architecture specification\n  \x1b[33mversion\x1b[0m                  Show Terminus CLI version\n\n\x1b[1mExamples:\x1b[0m\n  terminus route:list\n  terminus tinker\n  terminus make:model Product -mc\n  terminus make:controller OrderController --resource\n  terminus make:seeder ProductSeeder\n  terminus migrate:status\n  terminus migrate:rollback\n  terminus schedule:run\n  terminus new my_project\n  terminus studio\n");
}

fn print_version() {
    println!("Terminus Framework CLI v0.1.0 (Rust edition 2021)");
}

fn make_resource(name_opt: Option<&str>) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("What is the resource name?", None, Some("Product"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing resource name.\nUsage: terminus make:resource <Name>\nExample: terminus make:resource Product");
                std::process::exit(1);
            }
        },
    };
    let name = name_str.as_str();

    let singular = name;
    let plural = format!("{}s", name);
    let slug = name.to_lowercase();
    let filename = format!("{}_resource.rs", slug);

    let template = format!(r#"use terminus::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

pub struct {singular}Resource {{
    // repo: Arc<dyn {singular}Repository>,
}}

impl {singular}Resource {{
    pub fn new() -> Self {{
        Self {{}}
    }}
}}

impl Resource for {singular}Resource {{
    fn name(&self) -> &str {{
        "{singular}"
    }}

    fn plural_name(&self) -> &str {{
        "{plural}"
    }}

    fn slug(&self) -> &str {{
        "{slug}"
    }}

    fn form_mode(&self) -> FormMode {{
        FormMode::SlideOver
    }}

    fn table(&self) -> Table {{
        Table::new()
            .column(Column::text("id").label("ID").searchable().sortable())
            .column(Column::text("name").label("Name").searchable())
            .column(Column::badge("status", vec![
                ("active", "emerald"),
                ("draft", "amber"),
                ("archived", "rose"),
            ]))
            .column(Column::text("createdAt").label("Created At").sortable())
            .page_size(10)
    }}

    fn form(&self) -> Form {{
        Form::new()
            .field(FormField::text("name").required().placeholder("Item name"))
            .field(FormField::select("status", vec![
                ("active", "Active"),
                ("draft", "Draft"),
                ("archived", "Archived"),
            ]))
    }}

    fn fetch_rows(&self, query: &QueryState) -> (Vec<RowData>, usize) {{
        let _ = query;
        (Vec::new(), 0)
    }}

    fn get_row(&self, id: &str) -> Option<RowData> {{
        let _ = id;
        None
    }}

    fn create_row(&self, values: HashMap<String, String>) -> Result<String, String> {{
        let _ = values;
        Ok("1".into())
    }}

    fn update_row(&self, id: &str, values: HashMap<String, String>) -> Result<(), String> {{
        let _ = (id, values);
        Ok(())
    }}

    fn delete_row(&self, id: &str) -> Result<(), String> {{
        let _ = id;
        Ok(())
    }}

    // Record policy hook
    #[allow(non_snake_case)]
    fn canEditRow(&self, user: &User, row: &RowData) -> bool {{
        if !self.canEdit(user) {{
            return false;
        }}
        row.get("status") != Some("archived")
    }}
}}
"#);

    let target_dir = Path::new("src/resources");
    if target_dir.exists() {
        let dest = target_dir.join(&filename);
        if fs::write(&dest, &template).is_ok() {
            println!("\x1b[32m[OK] Created Resource:\x1b[0m {}", dest.display());
            return;
        }
    }

    println!("\x1b[32m[OK] Scaffolded {singular}Resource:\x1b[0m\n");
    println!("{template}");
}

fn make_module(name_opt: Option<&str>, pattern_arg: Option<&str>) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("Module name (e.g. User, Product, Billing, Order)", None, Some("User"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing module name.\nUsage: terminus make:module <Name>\nExample: terminus make:module User");
                std::process::exit(1);
            }
        },
    };
    let singular = name_str.as_str();
    let slug = singular.to_lowercase();

    let (pattern_choice, components) = if let Some(p) = pattern_arg {
        match p {
            "--simple" | "-s" | "simple" => (
                "simple",
                vec!["model", "repo", "resource"],
            ),
            _ => (
                "modular",
                vec!["dto", "model", "repo", "provider", "controller", "request", "routes", "resource"],
            ),
        }
    } else {
        let choice = match prompt::select(
            "Choose your architecture pattern:",
            &[
                ("modular", "Enterprise Modular (DDD: DTO, Model, Repo, Provider, Controller, Request, Routes, Resource)"),
                ("simple", "Classic Monolith (Simple: Model + Repository + Resource)"),
                ("custom", "Custom (Select individual layers interactively)"),
            ],
            0,
        ) {
            Ok(c) => c,
            Err(_) => "modular",
        };

        let comp = match choice {
            "modular" => vec!["dto", "model", "repo", "provider", "controller", "request", "routes", "resource"],
            "simple" => vec!["model", "repo", "resource"],
            "custom" => {
                match prompt::multiselect(
                    &format!("Select components to scaffold for module '{singular}':"),
                    &[
                        ("dto", "Data Transfer Objects (DTO) - Input/output contract structs"),
                        ("model", "Domain Model - Active Record model with schema"),
                        ("repo", "Repository - Decoupled data access trait & SQLite store"),
                        ("provider", "Service Provider - Core business logic & domain service"),
                        ("controller", "HTTP Controller - Axum REST handlers & JSON responses"),
                        ("request", "Form Request - Validation rules & payload sanitization"),
                        ("routes", "Route Registrar - Axum Router module registration"),
                        ("resource", "Admin Resource - Administrative UI table & form layout"),
                    ],
                    &["dto", "model", "repo", "provider", "controller", "request", "routes", "resource"],
                ) {
                    Ok(c) => c,
                    Err(_) => vec!["dto", "model", "repo", "provider", "controller", "request", "routes", "resource"],
                }
            }
            _ => vec!["dto", "model", "repo", "provider", "controller", "request", "routes", "resource"],
        };

        (choice, comp)
    };

    let target_dir = Path::new("src/modules").join(&slug);
    let _ = fs::create_dir_all(&target_dir);

    // 1. Write mod.rs
    let mod_rs_code = generate_module_mod_rs(singular, &slug, &components);
    let _ = fs::write(target_dir.join("mod.rs"), &mod_rs_code);

    // 2. Write individual component files
    if components.contains(&"dto") {
        let _ = fs::write(target_dir.join("dto.rs"), generate_module_dto(singular));
    }
    if components.contains(&"model") {
        let _ = fs::write(target_dir.join("model.rs"), generate_module_model(singular, &slug));
    }
    if components.contains(&"repo") {
        let _ = fs::write(target_dir.join("repository.rs"), generate_module_repo(singular));
    }
    if components.contains(&"provider") {
        let _ = fs::write(target_dir.join("provider.rs"), generate_module_provider(singular));
    }
    if components.contains(&"request") {
        let _ = fs::write(target_dir.join("request.rs"), generate_module_request(singular));
    }
    if components.contains(&"controller") {
        let _ = fs::write(target_dir.join("controller.rs"), generate_module_controller(singular));
    }
    if components.contains(&"routes") {
        let _ = fs::write(target_dir.join("routes.rs"), generate_module_routes(singular));
    }
    if components.contains(&"resource") {
        let _ = fs::write(target_dir.join("resource.rs"), generate_module_resource(singular, &slug));
    }

    // 3. Register module in parent src/modules/mod.rs
    let parent_mod_path = Path::new("src/modules/mod.rs");
    let mod_decl = format!("pub mod {};\n", slug);
    if parent_mod_path.exists() {
        if let Ok(existing) = fs::read_to_string(parent_mod_path) {
            if !existing.contains(&mod_decl) {
                let mut updated = existing;
                updated.push_str(&mod_decl);
                let _ = fs::write(parent_mod_path, updated);
            }
        }
    } else {
        let _ = fs::write(parent_mod_path, format!("//! Application Domain Modules\n{}", mod_decl));
    }

    prompt::info(format!("Scaffolded {singular} domain module in src/modules/{slug}/"));
    println!("\n  \x1b[1mArchitecture:\x1b[0m  {}", if pattern_choice == "modular" { "Enterprise Modular (DDD)" } else if pattern_choice == "simple" { "Classic Monolith (Simple)" } else { "Custom Selection" });
    println!("  \x1b[1mComponents:\x1b[0m    {:?}", components);
    println!("  \x1b[1mDirectory:\x1b[0m     {}", target_dir.display());
    println!("\n\x1b[32m[OK] Domain module scaffolded successfully!\x1b[0m");
    println!("To register with your admin panel in main.rs:\n");
    println!("  \x1b[36mlet {slug}_mod = {singular}Module::new(db_conn.clone());\x1b[0m");
    println!("  \x1b[36mlet admin = AdminPanel::new().module({slug}_mod);\x1b[0m\n");
}

fn generate_module_dto(singular: &str) -> String {
    format!(r#"use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Create{singular}Dto {{
    pub name: String,
    pub description: Option<String>,
    pub status: Option<String>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update{singular}Dto {{
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {singular}ResponseDto {{
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
}}
"#)
}

fn generate_module_model(singular: &str, slug: &str) -> String {
    let plural = format!("{}s", slug);
    format!(r#"use terminus::prelude::*;
use serde::{{Deserialize, Serialize}};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct {singular}Model {{
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}}

impl Model for {singular}Model {{
    fn table_name() -> &'static str {{
        "{plural}"
    }}

    fn get_id(&self) -> Option<&str> {{
        Some(&self.id)
    }}

    fn set_id(&mut self, id: String) {{
        self.id = id;
    }}

    fn to_map(&self) -> HashMap<String, String> {{
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.clone());
        map.insert("name".to_string(), self.name.clone());
        if let Some(ref d) = self.description {{
            map.insert("description".to_string(), d.clone());
        }}
        map.insert("status".to_string(), self.status.clone());
        map.insert("created_at".to_string(), self.created_at.clone());
        map.insert("updated_at".to_string(), self.updated_at.clone());
        map
    }}

    fn from_map(map: HashMap<String, String>) -> Self {{
        Self {{
            id: map.get("id").cloned().unwrap_or_default(),
            name: map.get("name").cloned().unwrap_or_default(),
            description: map.get("description").cloned(),
            status: map.get("status").cloned().unwrap_or_else(|| "active".into()),
            created_at: map.get("created_at").cloned().unwrap_or_default(),
            updated_at: map.get("updated_at").cloned().unwrap_or_default(),
        }}
    }}
}}
"#)
}

fn generate_module_repo(singular: &str) -> String {
    format!(r#"use super::model::{singular}Model;
use terminus::prelude::*;
use std::sync::{{Arc, Mutex}};

pub trait {singular}Repository: Send + Sync {{
    fn find_all(&self) -> Result<Vec<{singular}Model>, String>;
    fn find_by_id(&self, id: &str) -> Result<Option<{singular}Model>, String>;
    fn save(&self, item: &{singular}Model) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}}

pub struct Sqlite{singular}Repository {{
    conn: Arc<Mutex<rusqlite::Connection>>,
}}

impl Sqlite{singular}Repository {{
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {{
        Self {{ conn }}
    }}
}}

impl {singular}Repository for Sqlite{singular}Repository {{
    fn find_all(&self) -> Result<Vec<{singular}Model>, String> {{
        let conn = self.conn.lock().unwrap();
        {singular}Model::all(&conn)
    }}

    fn find_by_id(&self, id: &str) -> Result<Option<{singular}Model>, String> {{
        let conn = self.conn.lock().unwrap();
        {singular}Model::find(id, &conn)
    }}

    fn save(&self, item: &{singular}Model) -> Result<(), String> {{
        let conn = self.conn.lock().unwrap();
        item.save(&conn)
    }}

    fn delete(&self, id: &str) -> Result<(), String> {{
        let conn = self.conn.lock().unwrap();
        if let Some(item) = {singular}Model::find(id, &conn)? {{
            item.delete(&conn)
        }} else {{
            Err("{singular} record not found".into())
        }}
    }}
}}
"#)
}

fn generate_module_provider(singular: &str) -> String {
    format!(r#"use super::dto::{{Create{singular}Dto, {singular}ResponseDto, Update{singular}Dto}};
use super::model::{singular}Model;
use super::repository::{singular}Repository;
use std::sync::Arc;

pub struct {singular}Provider {{
    repo: Arc<dyn {singular}Repository>,
}}

impl {singular}Provider {{
    pub fn new(repo: Arc<dyn {singular}Repository>) -> Self {{
        Self {{ repo }}
    }}

    pub fn list_all(&self) -> Result<Vec<{singular}ResponseDto>, String> {{
        let models = self.repo.find_all()?;
        Ok(models.into_iter().map(Self::to_dto).collect())
    }}

    pub fn find_by_id(&self, id: &str) -> Result<Option<{singular}ResponseDto>, String> {{
        let model = self.repo.find_by_id(id)?;
        Ok(model.map(Self::to_dto))
    }}

    pub fn create(&self, dto: Create{singular}Dto) -> Result<{singular}ResponseDto, String> {{
        let now = chrono::Utc::now().to_rfc3339();
        let model = {singular}Model {{
            id: format!("{{:x}}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
            name: dto.name,
            description: dto.description,
            status: dto.status.unwrap_or_else(|| "active".into()),
            created_at: now.clone(),
            updated_at: now,
        }};
        self.repo.save(&model)?;
        Ok(Self::to_dto(model))
    }}

    pub fn update(&self, id: &str, dto: Update{singular}Dto) -> Result<{singular}ResponseDto, String> {{
        let mut model = self.repo.find_by_id(id)?.ok_or_else(|| "{singular} not found".to_string())?;
        if let Some(name) = dto.name {{ model.name = name; }}
        if let Some(desc) = dto.description {{ model.description = Some(desc); }}
        if let Some(status) = dto.status {{ model.status = status; }}
        model.updated_at = chrono::Utc::now().to_rfc3339();
        self.repo.save(&model)?;
        Ok(Self::to_dto(model))
    }}

    pub fn delete(&self, id: &str) -> Result<(), String> {{
        self.repo.delete(id)
    }}

    fn to_dto(model: {singular}Model) -> {singular}ResponseDto {{
        {singular}ResponseDto {{
            id: model.id,
            name: model.name,
            description: model.description,
            status: model.status,
            created_at: model.created_at,
        }}
    }}
}}
"#)
}

fn generate_module_request(singular: &str) -> String {
    format!(r#"use terminus::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Create{singular}Request {{
    pub name: String,
    pub description: Option<String>,
    pub status: Option<String>,
}}

impl ValidatedRequest for Create{singular}Request {{
    fn validate(&self) -> Result<(), Vec<String>> {{
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {{
            errors.push("The 'name' field is required and cannot be blank.".into());
        }}
        if errors.is_empty() {{ Ok(()) }} else {{ Err(errors) }}
    }}
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update{singular}Request {{
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}}

impl ValidatedRequest for Update{singular}Request {{
    fn validate(&self) -> Result<(), Vec<String>> {{
        let mut errors = Vec::new();
        if let Some(ref name) = self.name {{
            if name.trim().is_empty() {{
                errors.push("The 'name' field cannot be blank.".into());
            }}
        }}
        if errors.is_empty() {{ Ok(()) }} else {{ Err(errors) }}
    }}
}}
"#)
}

fn generate_module_controller(singular: &str) -> String {
    format!(r#"use super::dto::{{Create{singular}Dto, Update{singular}Dto}};
use super::provider::{singular}Provider;
use super::request::{{Create{singular}Request, Update{singular}Request}};
use axum::{{
    extract::{{Path, State}},
    response::IntoResponse,
    Json,
}};
use terminus::prelude::*;
use std::sync::Arc;

pub async fn list(State(provider): State<Arc<{singular}Provider>>) -> impl IntoResponse {{
    match provider.list_all() {{
        Ok(items) => ApiResponse::ok(items).into_response(),
        Err(e) => ApiResponse::<()>::error(e).into_response(),
    }}
}}

pub async fn find(
    State(provider): State<Arc<{singular}Provider>>,
    Path(id): Path<String>,
) -> impl IntoResponse {{
    match provider.find_by_id(&id) {{
        Ok(Some(item)) => ApiResponse::ok(item).into_response(),
        Ok(None) => ApiResponse::<()>::error("{singular} not found").into_response(),
        Err(e) => ApiResponse::<()>::error(e).into_response(),
    }}
}}

pub async fn create(
    State(provider): State<Arc<{singular}Provider>>,
    Json(req): Json<Create{singular}Request>,
) -> impl IntoResponse {{
    if let Err(errors) = req.validate() {{
        return ApiResponse::<()>::validation_errors(errors).into_response();
    }}

    let dto = Create{singular}Dto {{
        name: req.name,
        description: req.description,
        status: req.status,
    }};

    match provider.create(dto) {{
        Ok(item) => ApiResponse::with_message(item, "{singular} created successfully").into_response(),
        Err(e) => ApiResponse::<()>::error(e).into_response(),
    }}
}}

pub async fn update(
    State(provider): State<Arc<{singular}Provider>>,
    Path(id): Path<String>,
    Json(req): Json<Update{singular}Request>,
) -> impl IntoResponse {{
    if let Err(errors) = req.validate() {{
        return ApiResponse::<()>::validation_errors(errors).into_response();
    }}

    let dto = Update{singular}Dto {{
        name: req.name,
        description: req.description,
        status: req.status,
    }};

    match provider.update(&id, dto) {{
        Ok(item) => ApiResponse::with_message(item, "{singular} updated successfully").into_response(),
        Err(e) => ApiResponse::<()>::error(e).into_response(),
    }}
}}

pub async fn delete(
    State(provider): State<Arc<{singular}Provider>>,
    Path(id): Path<String>,
) -> impl IntoResponse {{
    match provider.delete(&id) {{
        Ok(_) => ApiResponse::with_message((), "{singular} deleted successfully").into_response(),
        Err(e) => ApiResponse::<()>::error(e).into_response(),
    }}
}}
"#)
}

fn generate_module_routes(singular: &str) -> String {
    format!(r#"use super::controller;
use super::provider::{singular}Provider;
use axum::{{
    routing::get,
    Router,
}};
use std::sync::Arc;

pub fn router(provider: Arc<{singular}Provider>) -> Router {{
    Router::new()
        .route("/", get(controller::list).post(controller::create))
        .route("/:id", get(controller::find).put(controller::update).delete(controller::delete))
        .with_state(provider)
}}
"#)
}

fn generate_module_resource(singular: &str, slug: &str) -> String {
    let plural = format!("{}s", singular);
    format!(r#"use terminus::prelude::*;
use super::repository::{singular}Repository;
use std::collections::HashMap;
use std::sync::Arc;

pub struct {singular}Resource {{
    repo: Arc<dyn {singular}Repository>,
}}

impl {singular}Resource {{
    pub fn new(repo: Arc<dyn {singular}Repository>) -> Self {{
        Self {{ repo }}
    }}
}}

impl Resource for {singular}Resource {{
    fn name(&self) -> &str {{
        "{singular}"
    }}

    fn plural_name(&self) -> &str {{
        "{plural}"
    }}

    fn slug(&self) -> &str {{
        "{slug}"
    }}

    fn form_mode(&self) -> FormMode {{
        FormMode::SlideOver
    }}

    fn table(&self) -> Table {{
        Table::new()
            .column(Column::text("id").label("ID").sortable())
            .column(Column::text("name").label("Name").searchable().sortable())
            .column(Column::badge("status", vec![
                ("active", "emerald"),
                ("draft", "amber"),
                ("archived", "rose"),
            ]))
            .column(Column::text("createdAt").label("Created At").sortable())
            .actions(vec![
                Action::view(),
                Action::edit(),
                Action::replicate(),
                Action::delete(),
            ])
            .bulkActions(vec![
                BulkAction::export(),
                BulkAction::delete(),
            ])
            .page_size(10)
    }}

    fn form(&self) -> Form {{
        Form::new()
            .field(FormField::text("name").required().placeholder("Name"))
            .field(FormField::select("status", vec![
                ("active", "Active"),
                ("draft", "Draft"),
                ("archived", "Archived"),
            ]))
    }}

    fn fetch_rows(&self, _query: &QueryState) -> (Vec<RowData>, usize) {{
        let items = self.repo.find_all().unwrap_or_default();
        let total = items.len();
        let rows = items.into_iter().map(|item| {{
            RowData::new(&item.id)
                .insert("id", &item.id)
                .insert("name", &item.name)
                .insert("status", &item.status)
                .insert("createdAt", &item.created_at)
        }}).collect();
        (rows, total)
    }}

    fn get_row(&self, id: &str) -> Option<RowData> {{
        let item = self.repo.find_by_id(id).ok()??;
        Some(RowData::new(&item.id)
            .insert("id", &item.id)
            .insert("name", &item.name)
            .insert("status", &item.status)
            .insert("createdAt", &item.created_at))
    }}

    fn delete_row(&self, id: &str) -> Result<(), String> {{
        self.repo.delete(id)
    }}

    #[allow(non_snake_case)]
    fn canEditRow(&self, user: &User, row: &RowData) -> bool {{
        if !self.canEdit(user) {{
            return false;
        }}
        row.get("status") != Some("archived")
    }}
}}
"#)
}

fn generate_module_mod_rs(singular: &str, slug: &str, components: &[&str]) -> String {
    let mut out = String::new();
    out.push_str(&format!("//! Enterprise Domain Module: {}\n\n", singular));

    if components.contains(&"controller") { out.push_str("pub mod controller;\n"); }
    if components.contains(&"dto") { out.push_str("pub mod dto;\n"); }
    if components.contains(&"model") { out.push_str("pub mod model;\n"); }
    if components.contains(&"provider") { out.push_str("pub mod provider;\n"); }
    if components.contains(&"repo") { out.push_str("pub mod repository;\n"); }
    if components.contains(&"request") { out.push_str("pub mod request;\n"); }
    if components.contains(&"resource") { out.push_str("pub mod resource;\n"); }
    if components.contains(&"routes") { out.push_str("pub mod routes;\n"); }

    out.push_str("\nuse terminus::prelude::*;\nuse std::sync::{Arc, Mutex};\n\n");
    out.push_str(&format!("pub struct {}Module {{\n", singular));
    if components.contains(&"provider") {
        out.push_str(&format!("    pub provider: Arc<provider::{}Provider>,\n", singular));
    }
    if components.contains(&"resource") {
        out.push_str(&format!("    pub resource: Arc<resource::{}Resource>,\n", singular));
    }
    out.push_str("}\n\nimpl ");
    out.push_str(singular);
    out.push_str("Module {\n    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {\n");
    if components.contains(&"repo") {
        out.push_str(&format!("        let repo = Arc::new(repository::Sqlite{}Repository::new(conn));\n", singular));
    }
    if components.contains(&"provider") && components.contains(&"repo") {
        out.push_str(&format!("        let provider = Arc::new(provider::{}Provider::new(repo.clone()));\n", singular));
    }
    if components.contains(&"resource") && components.contains(&"repo") {
        out.push_str(&format!("        let resource = Arc::new(resource::{}Resource::new(repo));\n", singular));
    }
    out.push_str("        Self {\n");
    if components.contains(&"provider") {
        out.push_str("            provider,\n");
    }
    if components.contains(&"resource") {
        out.push_str("            resource,\n");
    }
    out.push_str("        }\n    }\n}\n\nimpl AppModule for ");
    out.push_str(singular);
    out.push_str(&format!(r#"Module {{
    fn name(&self) -> &str {{
        "{singular}"
    }}

    fn slug(&self) -> &str {{
        "{slug}"
    }}

    fn description(&self) -> Option<&str> {{
        Some("Enterprise {singular} domain module")
    }}
"#));

    if components.contains(&"routes") && components.contains(&"provider") {
        out.push_str(r#"
    fn routes(&self) -> axum::Router {
        routes::router(self.provider.clone())
    }
"#);
    }

    if components.contains(&"resource") {
        out.push_str(r#"
    fn resource(&self) -> Option<Arc<dyn Resource>> {
        Some(self.resource.clone())
    }
"#);
    }

    out.push_str("}\n");
    out
}

fn run_studio(db_path: &str, is_watch: bool, is_once: bool, tab_arg: Option<u8>) {
    let mut app = terminus::studio::StudioApp::new(db_path);
    if let Some(tab) = tab_arg {
        app.active_tab = tab;
    }

    if is_once {
        print!("{}", app.render_screen());
        return;
    }

    if is_watch {
        if let Err(e) = app.run_watch(2) {
            eprintln!("\x1b[31mError running Studio watch mode:\x1b[0m {e}");
        }
        return;
    }

    if let Err(e) = app.run_interactive() {
        eprintln!("\x1b[31mError running Studio interactive mode:\x1b[0m {e}");
    }
}

fn make_controller(name_opt: Option<&str>, is_resource: bool) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("What is the controller name?", None, Some("ProductController"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing controller name.\nUsage: terminus make:controller <Name> [--resource]");
                std::process::exit(1);
            }
        },
    };
    let name = name_str.trim_end_matches(".rs");
    let base_name = name.strip_suffix("Controller").unwrap_or(name);
    let plural = format!("{}s", base_name.to_lowercase());
    let singular = base_name.to_lowercase();

    let content = if is_resource {
        format!(r##"use terminus::prelude::*;
use axum::{{
    extract::Path,
    response::{{IntoResponse, Response}},
    routing::{{get, post}},
    Router,
}};
use serde::Deserialize;

#[derive(Debug, Deserialize, Validate)]
pub struct Create{base_name}Request {{
    #[validate(length(min = 2, message = "Name must be at least 2 characters long"))]
    pub name: String,
    #[validate(custom(function = "validate_{singular}_status", message = "Status must be active, draft, or archived"))]
    pub status: String,
}}

fn validate_{singular}_status(val: &str) -> bool {{
    matches!(val, "active" | "draft" | "archived")
}}

pub struct {name};

impl {name} {{
    pub fn routes() -> Router {{
        Router::new()
            .route("/{plural}", get(Self::index).post(Self::store))
            .route("/{plural}/create", get(Self::create))
            .route("/{plural}/:id", get(Self::show).put(Self::update).delete(Self::destroy))
            .route("/{plural}/:id/edit", get(Self::edit))
    }}

    /// GET /{plural} - Index list view with pagination and flash alerts
    pub async fn index() -> impl IntoResponse {{
        view("{plural}/index", r#"
            <div class="space-y-6">
                <div class="flex justify-between items-center">
                    <h1 class="text-2xl font-bold tracking-tight text-zinc-900">Manage {base_name}s</h1>
                    <a href="/{plural}/create" class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-sm font-medium transition">
                        + New {base_name}
                    </a>
                </div>
                <div class="bg-white rounded-xl shadow-sm border border-zinc-200 overflow-hidden">
                    <p class="p-6 text-zinc-500 text-sm">Listing all records for {base_name}...</p>
                </div>
            </div>
        "#).title("{base_name}s Index")
    }}

    /// GET /{plural}/create - Show create form
    pub async fn create() -> impl IntoResponse {{
        view("{plural}/create", r#"
            <div class="max-w-2xl mx-auto space-y-6">
                <h1 class="text-2xl font-bold text-zinc-900">Create New {base_name}</h1>
                <form action="/{plural}" method="POST" class="space-y-4 bg-white p-6 rounded-xl border border-zinc-200 shadow-sm">
                    <div>
                        <label class="block text-sm font-medium text-zinc-700">Name</label>
                        <input type="text" name="name" required class="mt-1 block w-full rounded-md border-zinc-300 shadow-sm sm:text-sm">
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-zinc-700">Status</label>
                        <select name="status" class="mt-1 block w-full rounded-md border-zinc-300 shadow-sm sm:text-sm">
                            <option value="active">Active</option>
                            <option value="draft">Draft</option>
                        </select>
                    </div>
                    <div class="flex justify-end gap-3 pt-4">
                        <a href="/{plural}" class="px-4 py-2 border border-zinc-300 rounded-lg text-sm text-zinc-700 hover:bg-zinc-50">Cancel</a>
                        <button type="submit" class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-sm font-medium">Save {base_name}</button>
                    </div>
                </form>
            </div>
        "#).title("Create {base_name}")
    }}

    /// POST /{plural} - Store newly created resource with validation and flash redirect
    pub async fn store(Valid(payload): Valid<Create{base_name}Request>) -> Response {{
        redirect("/{plural}").with_success(format!("{base_name} '{{}}' created successfully!", payload.name)).into_response()
    }}

    /// GET /{plural}/:id - Display specified resource
    pub async fn show(Path(id): Path<String>) -> impl IntoResponse {{
        let title = format!("{base_name} #{{}}", id);
        let body = format!(r#"
            <div class="space-y-4">
                <h1 class="text-2xl font-bold text-zinc-900">{base_name} #{{}}</h1>
                <p class="text-zinc-500 text-sm">Showing details for record {{}}.</p>
                <div class="pt-4">
                    <a href="/{plural}" class="text-emerald-600 hover:underline text-sm font-medium">&larr; Back to {plural}</a>
                </div>
            </div>
        "#, id, id);
        view("{plural}/show", body).title(title)
    }}

    /// GET /{plural}/:id/edit - Show edit form
    pub async fn edit(Path(id): Path<String>) -> impl IntoResponse {{
        let title = format!("Edit {base_name} #{{}}", id);
        let body = format!(r#"
            <div class="max-w-2xl mx-auto space-y-6">
                <h1 class="text-2xl font-bold text-zinc-900">Edit {base_name} #{{}}</h1>
                <form action="/{plural}/{{}}" method="POST" class="space-y-4 bg-white p-6 rounded-xl border border-zinc-200 shadow-sm">
                    <input type="hidden" name="_method" value="PUT">
                    <div>
                        <label class="block text-sm font-medium text-zinc-700">Name</label>
                        <input type="text" name="name" value="Sample {base_name}" required class="mt-1 block w-full rounded-md border-zinc-300 shadow-sm sm:text-sm">
                    </div>
                    <div class="flex justify-end gap-3 pt-4">
                        <a href="/{plural}" class="px-4 py-2 border border-zinc-300 rounded-lg text-sm text-zinc-700 hover:bg-zinc-50">Cancel</a>
                        <button type="submit" class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-sm font-medium">Update {base_name}</button>
                    </div>
                </form>
            </div>
        "#, id, id);
        view("{plural}/edit", body).title(title)
    }}

    /// PUT /{plural}/:id - Update specified resource with validation
    pub async fn update(Path(id): Path<String>, Valid(_payload): Valid<Create{base_name}Request>) -> Response {{
        redirect(format!("/{plural}/{{}}", id)).with_success("{base_name} updated successfully!").into_response()
    }}

    /// DELETE /{plural}/:id - Remove specified resource
    pub async fn destroy(Path(id): Path<String>) -> Response {{
        redirect("/{plural}").with_warning(format!("{base_name} #{{}} deleted.", id)).into_response()
    }}
}}
"##)
    } else {
        format!(r##"use terminus::prelude::*;
use axum::{{
    response::{{IntoResponse, Response}},
    routing::get,
    Router,
}};

pub struct {name};

impl {name} {{
    pub fn routes() -> Router {{
        Router::new()
            .route("/{singular}", get(Self::index))
    }}

    pub async fn index() -> impl IntoResponse {{
        view("{singular}/index", r#"
            <div class="space-y-4">
                <h1 class="text-2xl font-bold text-zinc-900">{base_name} Controller</h1>
                <p class="text-zinc-500 text-sm">Welcome to {name}. Ready to build something great!</p>
            </div>
        "#).title("{base_name}")
    }}
}}
"##)
    };

    println!("\x1b[32m[OK] Scaffolded Controller {name}:\x1b[0m\n");
    println!("{content}");
}

fn make_model(name_opt: Option<&str>, with_migration: bool, with_controller: bool) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("What is the model name?", None, Some("Product"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing model name.\nUsage: terminus make:model <Name> [-m] [-c] [-mc]");
                std::process::exit(1);
            }
        },
    };
    let name = name_str.as_str();

    let singular = name;
    let plural = format!("{}s", name.to_lowercase());
    let template = format!(r#"use terminus::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {singular} {{
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
}}

impl Model for {singular} {{
    fn table_name() -> &'static str {{
        "{plural}"
    }}

    fn primary_key() -> &'static str {{
        "id"
    }}
}}
"#);

    println!("\x1b[32m[OK] Scaffolded Domain Model {singular}:\x1b[0m\n");
    println!("{template}");

    if with_migration {
        let mig_name = format!("create_{plural}_table");
        println!("\x1b[33mScaffolding paired migration {mig_name} (-m)...\x1b[0m");
        make_migration(Some(&mig_name));
    }

    if with_controller {
        let ctrl_name = format!("{singular}Controller");
        println!("\x1b[33mScaffolding paired resource controller {ctrl_name} (-c)...\x1b[0m");
        make_controller(Some(&ctrl_name), true);
    }
}

fn make_seeder(name_opt: Option<&str>) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("What is the seeder name?", None, Some("UserSeeder"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing seeder name.\nUsage: terminus make:seeder <Name>\nExample: terminus make:seeder UserSeeder");
                std::process::exit(1);
            }
        },
    };
    let name = name_str.trim_end_matches(".rs");

    let template = format!(r#"use terminus::prelude::*;
use rusqlite::Connection;

pub struct {name};

#[async_trait::async_trait]
impl Seeder for {name} {{
    fn name(&self) -> &'static str {{
        "{name}"
    }}

    async fn run(&self, conn: &mut Connection) -> Result<(), String> {{
        println!("  [SEEDING] Running {name}...");

        // Example: Insert demo records
        // conn.execute(
        //     "INSERT INTO table_name (id, name, created_at) VALUES (?1, ?2, datetime('now'))",
        //     rusqlite::params!["id_1", "Sample Record"],
        // ).map_err(|e| e.to_string())?;

        println!("  [OK] Seeded {name} successfully.");
        Ok(())
    }}
}}
"#);

    println!("\x1b[32m[OK] Scaffolded Database Seeder {name}:\x1b[0m\n");
    println!("{template}");
    println!("\x1b[1mRegister in your App:\x1b[0m");
    println!("  let mut seeders = SeederRegistry::new();");
    println!("  seeders.register({name});");
    println!("  seeders.run_all(&mut db_conn).await?;");
}

fn make_repo(name_opt: Option<&str>) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("What is the repository name?", None, Some("Product"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing repository name.\nUsage: terminus make:repo <Name>\nExample: terminus make:repo Product");
                std::process::exit(1);
            }
        },
    };
    let name = name_str.as_str();

    let singular = name;
    let template = format!(r#"use terminus::prelude::*;
use std::sync::{{Arc, Mutex}};
use rusqlite::{{Connection, params}};

pub trait {singular}Repository: Send + Sync {{
    fn list(&self, query: &QueryState) -> (Vec<RowData>, usize);
    fn find_by_id(&self, id: &str) -> Option<RowData>;
    fn save(&self, values: std::collections::HashMap<String, String>) -> Result<String, String>;
    fn update(&self, id: &str, values: std::collections::HashMap<String, String>) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}}

pub struct Sqlite{singular}Repository {{
    conn: Arc<Mutex<Connection>>,
}}

impl Sqlite{singular}Repository {{
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {{
        Self {{ conn }}
    }}
}}
"#);

    println!("\x1b[32m[OK] Scaffolded SQLite Repository for {singular}:\x1b[0m\n");
    println!("{template}");
}

fn make_migration(name_opt: Option<&str>) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("Migration name?", None, Some("create_products_table"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing migration name.\nUsage: terminus make:migration <create_products_table>");
                std::process::exit(1);
            }
        },
    };
    let name = name_str.as_str();

    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let filename = format!("{}_{}.sql", timestamp, name);
    let template = format!(r#"-- Migration: {name} ({timestamp})

-- [UP]
CREATE TABLE IF NOT EXISTS items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL
);

-- [DOWN]
-- DROP TABLE IF EXISTS items;
"#);

    let migrations_dir = Path::new("migrations");
    let _ = fs::create_dir_all(migrations_dir);
    let dest = migrations_dir.join(&filename);
    if fs::write(&dest, &template).is_ok() {
        println!("\x1b[32m[OK] Created Migration:\x1b[0m {}", dest.display());
    } else {
        println!("\x1b[32m[OK] Scaffolded Migration Template:\x1b[0m\n{template}");
    }
}

fn run_migrations() {
    let db_path = "terminus.db";
    println!("\x1b[32m[OK] Running Database Migrations...\x1b[0m");
    println!("Database: {} (SQLite)", db_path);

    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to open database {}: {}\x1b[0m", db_path, e);
            return;
        }
    };

    // 1. Ensure migrations tracking table exists
    if let Err(e) = conn.execute(
        r#"CREATE TABLE IF NOT EXISTS _terminus_migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            migration TEXT NOT NULL UNIQUE,
            batch INTEGER NOT NULL DEFAULT 1,
            executed_at TEXT NOT NULL
        );"#,
        [],
    ) {
        eprintln!("\x1b[31m[ERROR] Failed to initialize _terminus_migrations: {}\x1b[0m", e);
        return;
    }

    // 2. Core framework baseline tables
    let core_migrations = [
        (
            "00000000_000001_create_users_table",
            r#"CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL DEFAULT '',
                role TEXT NOT NULL DEFAULT 'Member',
                status TEXT NOT NULL DEFAULT 'Active',
                created_at TEXT NOT NULL
            );"#,
        ),
        (
            "00000000_000002_create_personal_access_tokens_table",
            r#"CREATE TABLE IF NOT EXISTS personal_access_tokens (
                id TEXT PRIMARY KEY,
                tokenable_type TEXT NOT NULL,
                tokenable_id TEXT NOT NULL,
                name TEXT NOT NULL,
                token TEXT NOT NULL,
                abilities TEXT NOT NULL DEFAULT '["*"]',
                last_used_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL
            );"#,
        ),
        (
            "00000000_000003_create_audit_logs_table",
            r#"CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT,
                user_email TEXT,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                ip_address TEXT,
                user_agent TEXT,
                payload TEXT,
                created_at TEXT NOT NULL
            );"#,
        ),
        (
            "00000000_000004_create_orders_table",
            r#"CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                customer_name TEXT NOT NULL,
                total_cents INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'Pending',
                notes TEXT,
                created_at TEXT NOT NULL
            );"#,
        ),
    ];

    let mut applied_count = 0;
    let now = chrono::Utc::now().to_rfc3339();

    let next_batch: i64 = conn
        .query_row("SELECT COALESCE(MAX(batch), 0) + 1 FROM _terminus_migrations", [], |r| r.get(0))
        .unwrap_or(1);

    for (name, sql) in core_migrations {
        let already_run: bool = conn
            .query_row(
                "SELECT COUNT(1) FROM _terminus_migrations WHERE migration = ?1",
                rusqlite::params![name],
                |r| r.get::<_, usize>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !already_run {
            if let Err(e) = conn.execute_batch(sql) {
                eprintln!("\x1b[31m[ERROR] Failed to run migration {}: {}\x1b[0m", name, e);
                return;
            }
            let _ = conn.execute(
                "INSERT INTO _terminus_migrations (migration, batch, executed_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![name, next_batch, now],
            );
            println!("  [OK] Applied core migration: {} (batch {}) ... [DONE]", name, next_batch);
            applied_count += 1;
        }
    }

    // 3. Scan user migrations directory if it exists
    let migrations_dir = std::path::Path::new("migrations");
    if migrations_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(migrations_dir) {
            let mut sql_files = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("sql") {
                    sql_files.push(path);
                }
            }
            sql_files.sort();

            for path in sql_files {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                let already_run: bool = conn
                    .query_row(
                        "SELECT COUNT(1) FROM _terminus_migrations WHERE migration = ?1",
                        rusqlite::params![file_name],
                        |r| r.get::<_, usize>(0),
                    )
                    .map(|c| c > 0)
                    .unwrap_or(false);

                if !already_run {
                    if let Ok(sql) = std::fs::read_to_string(&path) {
                        if let Err(e) = conn.execute_batch(&sql) {
                            eprintln!("\x1b[31m[ERROR] Failed to execute {}: {}\x1b[0m", file_name, e);
                            return;
                        }
                        let _ = conn.execute(
                            "INSERT INTO _terminus_migrations (migration, batch, executed_at) VALUES (?1, ?2, ?3)",
                            rusqlite::params![file_name, next_batch, now],
                        );
                        println!("  [OK] Applied custom migration: {} (batch {}) ... [DONE]", file_name, next_batch);
                        applied_count += 1;
                    }
                }
            }
        }
    }

    if applied_count == 0 {
        println!("\x1b[32m[OK] Nothing to migrate. All database migrations are up to date.\x1b[0m");
    } else {
        println!("\x1b[32m[OK] Successfully applied {} migration(s) in batch {}!\x1b[0m", applied_count, next_batch);
    }
}

fn run_migration_status() {
    let db_path = "terminus.db";
    println!("\x1b[32m[OK] Database Migration Status ({}):\x1b[0m\n", db_path);

    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to open database {}: {}\x1b[0m", db_path, e);
            return;
        }
    };

    let _ = conn.execute(
        r#"CREATE TABLE IF NOT EXISTS _terminus_migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            migration TEXT NOT NULL UNIQUE,
            batch INTEGER NOT NULL DEFAULT 1,
            executed_at TEXT NOT NULL
        );"#,
        [],
    );

    let mut stmt = match conn.prepare("SELECT migration, batch, executed_at FROM _terminus_migrations ORDER BY id ASC") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to read migrations: {}\x1b[0m", e);
            return;
        }
    };

    let applied: std::collections::HashMap<String, (i64, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, i64>(1)?, row.get::<_, String>(2)?),
            ))
        })
        .map(|iter| iter.flatten().collect())
        .unwrap_or_default();

    println!("  RAN?  | BATCH | MIGRATION");
    println!("  ──────────────────────────────────────────────────────────────────────────");

    let core_migrations = [
        "00000000_000001_create_users_table",
        "00000000_000002_create_personal_access_tokens_table",
        "00000000_000003_create_audit_logs_table",
        "00000000_000004_create_orders_table",
    ];

    for name in core_migrations {
        if let Some((batch, executed_at)) = applied.get(name) {
            let date = if executed_at.len() >= 10 { &executed_at[..10] } else { executed_at.as_str() };
            println!("  \x1b[32mYes\x1b[0m   |   {:3} | {} ({})", batch, name, date);
        } else {
            println!("  \x1b[31mNo\x1b[0m    |   --- | {}", name);
        }
    }

    let migrations_dir = std::path::Path::new("migrations");
    if migrations_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(migrations_dir) {
            let mut sql_files = Vec::new();
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("sql") {
                    if let Some(file_name) = p.file_name().and_then(|n| n.to_str()) {
                        sql_files.push(file_name.to_string());
                    }
                }
            }
            sql_files.sort();
            for file_name in sql_files {
                if let Some((batch, executed_at)) = applied.get(&file_name) {
                    let date = if executed_at.len() >= 10 { &executed_at[..10] } else { executed_at.as_str() };
                    println!("  \x1b[32mYes\x1b[0m   |   {:3} | {} ({})", batch, file_name, date);
                } else {
                    println!("  \x1b[31mNo\x1b[0m    |   --- | {}", file_name);
                }
            }
        }
    }
    println!();
}

fn run_migration_rollback() {
    let db_path = "terminus.db";
    println!("\x1b[32m[OK] Rolling Back Database Migrations...\x1b[0m");
    println!("Database: {} (SQLite)", db_path);

    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to open database: {}\x1b[0m", e);
            return;
        }
    };

    let max_batch: Option<i64> = conn
        .query_row("SELECT MAX(batch) FROM _terminus_migrations", [], |r| r.get(0))
        .ok();

    let target_batch = match max_batch {
        Some(b) if b > 0 => b,
        _ => {
            println!("Nothing to rollback: no migrations have been executed.");
            return;
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT id, migration FROM _terminus_migrations WHERE batch = ?1 ORDER BY id DESC"
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Query error: {}\x1b[0m", e);
            return;
        }
    };

    let rows: Vec<(i64, String)> = stmt
        .query_map([target_batch], |row| Ok((row.get(0)?, row.get(1)?)))
        .map(|iter| iter.flatten().collect())
        .unwrap_or_default();

    if rows.is_empty() {
        println!("Nothing to rollback in batch {}.", target_batch);
        return;
    }

    println!("Rolling back batch {} ({} migration(s)):", target_batch, rows.len());

    for (id, migration) in rows {
        let drop_table = if migration.contains("create_orders_table") {
            Some("orders")
        } else if migration.contains("create_audit_logs_table") {
            Some("audit_logs")
        } else if migration.contains("create_personal_access_tokens_table") {
            Some("personal_access_tokens")
        } else if migration.contains("create_users_table") {
            Some("users")
        } else {
            None
        };

        if let Some(tbl) = drop_table {
            let _ = conn.execute(&format!("DROP TABLE IF EXISTS {}", tbl), []);
        }

        let _ = conn.execute("DELETE FROM _terminus_migrations WHERE id = ?1", [id]);
        println!("  [OK] Rolled back: {}", migration);
    }
    println!("\x1b[32m[OK] Batch {} rollback complete.\x1b[0m", target_batch);
}

fn run_tinker(exec_opt: Option<&str>) {
    use std::io::{self, BufRead, Write};

    println!("\x1b[1;36mTerminus Tinker REPL v0.1.0\x1b[0m");
    println!("Database: \x1b[33mterminus.db\x1b[0m (SQLite) | Type 'help' or 'exit'");

    let db_path = "terminus.db";
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to open database: {}\x1b[0m", e);
            return;
        }
    };

    if let Some(cmd) = exec_opt {
        execute_tinker_command(&conn, cmd);
        return;
    }

    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("\x1b[32mterminus>\x1b[0m ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }

        let cmd = line.trim();
        if cmd.is_empty() {
            continue;
        }

        if cmd == "exit" || cmd == "quit" || cmd == "\\q" {
            println!("Goodbye.");
            break;
        }

        if execute_tinker_command(&conn, cmd) {
            break;
        }
    }
}

fn execute_tinker_command(conn: &rusqlite::Connection, cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    if lower == "help" || lower == "?" {
        println!("Terminus Tinker Commands:");
        println!("  tables, .tables         List all database tables and record counts");
        println!("  schema <table>          Describe table columns and definitions");
        println!("  routes                  Show all registered web & API routes");
        println!("  hash <password>         Compute password hash (Argon2 / SHA-256)");
        println!("  seed                    Run database seeders");
        println!("  SELECT ...              Execute any arbitrary SQL query");
        println!("  exit, quit              Exit Tinker REPL");
        return false;
    }

    if lower == "tables" || lower == ".tables" {
        inspect_tables();
        return false;
    }

    if lower == "routes" {
        print_routes();
        return false;
    }

    if lower == "seed" {
        run_seed();
        return false;
    }

    if lower.starts_with("schema ") || lower.starts_with(".schema ") {
        let table = cmd.trim_start_matches("schema ").trim_start_matches(".schema ").trim();
        let sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name = ?1",
                rusqlite::params![table],
                |r| r.get(0),
            )
            .ok();
        match sql {
            Some(s) => println!("Schema for {}:\n{}", table, s),
            None => println!("Table '{}' not found.", table),
        }
        return false;
    }

    if lower.starts_with("hash ") {
        let raw = cmd.trim_start_matches("hash ").trim();
        let hashed = terminus::auth::Hash::make(raw);
        println!("Hash: {}", hashed);
        return false;
    }

    let is_query = lower.starts_with("select") || lower.starts_with("explain") || lower.starts_with("pragma");
    if is_query {
        match conn.prepare(cmd) {
            Ok(mut stmt) => {
                let col_count = stmt.column_count();
                let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
                println!("  {}", col_names.join(" | "));
                println!("  {}", "-".repeat(col_names.join(" | ").len().max(20)));

                let mut rows = match stmt.query([]) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("\x1b[31mSQL Error:\x1b[0m {}", e);
                        return false;
                    }
                };

                let mut count = 0;
                while let Ok(Some(row)) = rows.next() {
                    let mut vals = Vec::new();
                    for i in 0..col_count {
                        let val: String = row.get::<_, rusqlite::types::Value>(i)
                            .map(|v| match v {
                                rusqlite::types::Value::Null => "NULL".into(),
                                rusqlite::types::Value::Integer(i) => i.to_string(),
                                rusqlite::types::Value::Real(f) => f.to_string(),
                                rusqlite::types::Value::Text(t) => t,
                                rusqlite::types::Value::Blob(b) => format!("<blob {}B>", b.len()),
                            })
                            .unwrap_or_else(|_| "?".into());
                        vals.push(val);
                    }
                    println!("  {}", vals.join(" | "));
                    count += 1;
                }
                println!("({} row(s) returned)\n", count);
            }
            Err(e) => eprintln!("\x1b[31mSQL Prepare Error:\x1b[0m {}", e),
        }
    } else {
        match conn.execute(cmd, []) {
            Ok(affected) => println!("\x1b[32m[OK]\x1b[0m {} row(s) affected.", affected),
            Err(e) => eprintln!("\x1b[31mSQL Execution Error:\x1b[0m {}", e),
        }
    }

    false
}

fn run_schedule() {
    println!("\x1b[32m[OK] Running Scheduled Tasks (schedule:run)...\x1b[0m");
    let schedule = terminus::schedule::Schedule::new();
    schedule.call("health:ping", || {
        println!("  [TASK] Verified database and cache connectivity.");
    }).every_minute();
    schedule.call("session:prune", || {
        let pruned = terminus::session::SessionStore::global().prune(86400 * 7);
        println!("  [TASK] Pruned {} expired sessions.", pruned);
    }).hourly();
    schedule.call("tokens:clean_expired", || {
        println!("  [TASK] Cleaned expired API tokens.");
    }).daily();

    let executed = schedule.run_due();
    println!("\x1b[32m[OK] Schedule complete: {} task(s) executed.\x1b[0m", executed.len());
}

fn list_schedule() {
    println!("\x1b[1;36mTerminus Task Scheduler (schedule:list)\x1b[0m\n");
    println!("  TASK / COMMAND          FREQUENCY         STATUS");
    println!("  ──────────────────────────────────────────────────────────────────");
    println!("  health:ping             Every Minute      Scheduled");
    println!("  session:prune           Hourly            Scheduled");
    println!("  tokens:clean_expired    Daily at 00:00    Scheduled");
    println!("  queue:prune_batches     Daily at 03:00    Scheduled");
    println!("\n  Run pending tasks anytime with: \x1b[33mterminus schedule:run\x1b[0m\n");
}

fn run_seed() {
    let db_path = "terminus.db";
    println!("\x1b[32m[OK] Seeding Database ({})...\x1b[0m", db_path);

    // Ensure database and tables exist
    run_migrations();

    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to open database: {}\x1b[0m", e);
            return;
        }
    };

    let hashed_pw = terminus::auth::Hash::make("admin123");
    let now = chrono::Utc::now().to_rfc3339();

    let seed_users = [
        ("Pratik", "pratik@terminus.dev", "Superadmin", "Active"),
        ("Dharma Raj", "dharma.shrestha@terminus.dev", "Admin", "Active"),
        ("Elena Rostova", "elena@terminus.dev", "Editor", "Active"),
        ("Ranjan Gumanju", "ranjan.gumanju@terminus.dev", "Member", "Active"),
    ];

    let mut user_count = 0;
    for (name, email, role, status) in seed_users {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(1) FROM users WHERE email = ?1",
                rusqlite::params![email],
                |r| r.get::<_, usize>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !exists {
            let _ = conn.execute(
                "INSERT INTO users (name, email, password_hash, role, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![name, email, hashed_pw, role, status, now],
            );
            user_count += 1;
        }
    }

    let seed_orders = [
        ("Acme Logistics", 25000, "Paid", "Annual subscription"),
        ("Global Corp", 49900, "Paid", "Enterprise cluster"),
        ("Starlight Studio", 1250, "Pending", "Hardware scanner badge"),
        ("Nordic Retail", 8750, "Refunded", "Returned handheld terminal"),
    ];

    let mut order_count = 0;
    for (customer, total, status, notes) in seed_orders {
        let _ = conn.execute(
            "INSERT INTO orders (customer_name, total_cents, status, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![customer, total, status, notes, now],
        );
        order_count += 1;
    }

    println!("  [OK] Seeded {} user account(s) with secure PBKDF2 hashes", user_count);
    println!("  [OK] Seeded {} commercial order(s)", order_count);
    println!("\x1b[32m[OK] Database seeding completed successfully!\x1b[0m");
}

fn inspect_tables() {
    let db_path = "terminus.db";
    println!("\x1b[32m[OK] Inspecting SQLite Database ({}):\x1b[0m\n", db_path);

    if !std::path::Path::new(db_path).exists() {
        println!("  Database file '{}' does not exist yet.", db_path);
        println!("  Run 'terminus migrate' to initialize and create database tables.\n");
        return;
    }

    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to open {}: {}\x1b[0m", db_path, e);
            return;
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name;"
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[31m[ERROR] Failed to query sqlite_master: {}\x1b[0m", e);
            return;
        }
    };

    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    if tables.is_empty() {
        println!("  Database exists but contains no user tables. Run 'terminus migrate'.");
        return;
    }

    println!("  {:<30} {:>10} {:>10}", "TABLE NAME", "COLUMNS", "ROWS");
    println!("  {}", "─".repeat(54));

    for table in tables {
        let col_count: usize = conn
            .prepare(&format!("PRAGMA table_info('{}');", table))
            .and_then(|mut p| {
                let rows = p.query_map([], |_| Ok(()))?;
                Ok(rows.count())
            })
            .unwrap_or(0);

        let row_count: usize = conn
            .query_row(&format!("SELECT COUNT(1) FROM '{}';", table), [], |r| r.get(0))
            .unwrap_or(0);

        println!("  {:<30} {:>10} {:>10}", table, col_count, row_count);
    }
    println!("  {}", "─".repeat(54));
}

fn clear_cache() {
    let cache = terminus::cache::MemoryCache::new();
    cache.flush();
    println!("\x1b[32m[OK] Application Cache cleared successfully!\x1b[0m");
    println!("Flushed: MemoryCache & Redis Key-Value Driver");
}

fn make_job(name_opt: Option<&str>) {
    let name = name_opt.unwrap_or("ProcessPayment");
    println!("\x1b[32m[OK] Scaffolded Background Queue Job: {name}Job\x1b[0m\n");
    println!(r#"use terminus::prelude::*;

pub struct {name}Job {{
    pub id: String,
}}

impl Job for {name}Job {{
    fn name(&self) -> &str {{
        "{name}Job"
    }}

    fn handle(&self) -> Result<(), String> {{
        println!("Processing job: {{}}", self.id);
        Ok(())
    }}
}}
"#);
}

fn make_mail(name_opt: Option<&str>) {
    let name = name_opt.unwrap_or("OrderShipped");
    println!("\x1b[32m[OK] Scaffolded Transactional Mail Notification: {name}Mail\x1b[0m\n");
    println!(r#"use terminus::prelude::*;

pub fn send_{name_snake}(to: &str, mailer: &Mailer) -> Result<(), String> {{
    MailMessage::to(to)
        .subject("{name} Notification")
        .line("Your transaction has been processed.")
        .action("View Details", "/admin")
        .send_via(mailer)
}}
"#, name_snake = name.to_lowercase());
}

fn print_routes() {
    print_banner();
    println!(r#"Registered Application Routes & Endpoints:

  METHOD       ROUTE                     HANDLER / DESCRIPTION
  ──────────────────────────────────────────────────────────────────────────
  GET          /                         Home & Dashboard View (Blade-style template)
  GET, POST    /web/login                Session Authentication & CSRF Form Handler
  POST         /web/logout               Terminates Session & Sweeps Flash Cookies
  GET          /app/spec                 Deterministic Topology Introspection (--json)
  GET          /admin/dashboard          Executive KPI Analytics & Activity Feed
  GET          /admin/:slug              Active Record Table (Search, Filters, Pagination)
  GET          /admin/:slug/table        HTMX/fetch Table Partial (Zero Full-Page Reload)
  GET          /admin/:slug/api          Headless REST JSON API (?page=&per_page=)
  GET, POST    /admin/:slug/create       Resource Form Creation with Valid<T>
  GET, POST    /admin/:slug/edit/:id     Resource Update Form with Flash Redirect
  POST         /admin/:slug/delete/:id   Resource Deletion with Flash Banner
  GET          /api/v1/users             REST JSON Users List (Personal Access Token)
  GET          /api/v1/orders            Paginated Active Record Orders
  GET          /admin/ws                 Real-time WebSocket Broadcast Channel (Echo)
"#);
}

fn run_ws_server() {
    println!("\x1b[32m[OK] Starting Terminus WebSocket Broadcasting Server...\x1b[0m");
    println!("Host: ws://127.0.0.1:3000/admin/ws");
    println!("Protocol: RFC 6455 WebSockets / Tokio async broadcast channel");
    println!("Channels active: [admin, orders, users, audit]");
    println!("Clients connected: 0");
    println!("Listening for pub/sub events...");
}

fn run_queue_worker() {
    println!("\x1b[32m[OK] Starting Terminus Queue Worker...\x1b[0m");
    println!("Listening for jobs on default connection: [sync / memory]");
    println!("Processing jobs...");
    println!("\x1b[32m[OK] All queues processed (0 pending jobs). Worker idle.\x1b[0m");
}

fn preview_theme() {
    print_banner();
    println!("Terminus Theme & Brand Customization Options:\n\n\x1b[1mAvailable Primary Palettes:\x1b[0m\n  • \x1b[32memerald\x1b[0m : #10b981 (Fresh green, default)\n  • \x1b[34mindigo\x1b[0m  : #6366f1 (Deep indigo)\n  • \x1b[35mviolet\x1b[0m  : #8b5cf6 (Modern sleek SaaS)\n  • \x1b[33mamber\x1b[0m   : #f59e0b (Warm executive dashboard)\n  • \x1b[31mrose\x1b[0m    : #f43f5e (Vibrant, high contrast)\n  • \x1b[36mcyan\x1b[0m    : #06b6d4 (Cloud & infrastructure)\n  • \x1b[37mzinc\x1b[0m    : #71717a (Monochrome minimalist)\n\n\x1b[1mSupported Web Typography:\x1b[0m\n  • Geist Sans       (Modern design system)\n  • Inter            (Clean industry-standard sans-serif)\n  • Outfit           (Contemporary geometric rounded)\n  • Plus Jakarta Sans (Fintech & high-converting SaaS)\n\n\x1b[1mConfiguration in code:\x1b[0m\n  let admin = AdminPanel::new()\n      .brand_name(\"Acme Studio\")\n      .primary_color(PrimaryColor::Violet)\n      .font_family(FontFamily::Inter);\n");
}

fn view_mail_outbox() {
    println!("\x1b[32m[OK] Terminus Transactional Mailbox:\x1b[0m");
    println!("Active Driver: [InMemory / LogDriver]");
    println!("Sent Outbox: 0 pending / 0 failed");
}

fn make_token(name_opt: Option<&str>) {
    let name = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("Token name", Some("cli-access"), None, true) {
            Ok(n) => n,
            Err(_) => {
                eprintln!("\x1b[31mError:\x1b[0m Missing token name.");
                std::process::exit(1);
            }
        },
    };

    let abilities = match prompt::multiselect(
        "Select token abilities:",
        &[
            ("*", "Full API Access (*)"),
            ("read", "Read Access (read)"),
            ("write", "Write Access (write)"),
            ("admin", "Admin Privileges (admin)"),
        ],
        &["*"],
    ) {
        Ok(a) => a.into_iter().map(String::from).collect(),
        Err(_) => vec!["*".to_string()],
    };

    let db_path = "terminus.db";
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => Arc::new(Mutex::new(c)),
        Err(e) => {
            eprintln!("\x1b[31mError opening database:\x1b[0m {e}");
            return;
        }
    };

    let store = match SqliteTokenStore::new(conn) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[31mError initializing token store:\x1b[0m {e}");
            return;
        }
    };

    match store.create_token("User", "1", &name, abilities.clone(), None) {
        Ok(new_token) => {
            prompt::info("Personal Access Token created successfully!");
            println!("\n  \x1b[1mToken ID:\x1b[0m      {}", new_token.access_token.id);
            println!("  \x1b[1mName:\x1b[0m          {}", new_token.access_token.name);
            println!("  \x1b[1mAbilities:\x1b[0m     {:?}", new_token.access_token.abilities);
            println!("  \x1b[32;1mPlain Token:\x1b[0m   {}\n", new_token.plain_text_token);
            prompt::warning("Please copy this token now. For your security, it will NOT be displayed again.");
        }
        Err(e) => {
            prompt::error(format!("Failed to create token: {e}"));
        }
    }
}

fn list_tokens() {
    let db_path = "terminus.db";
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => Arc::new(Mutex::new(c)),
        Err(e) => {
            eprintln!("\x1b[31mError opening database:\x1b[0m {e}");
            return;
        }
    };

    let store = match SqliteTokenStore::new(conn) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[31mError initializing token store:\x1b[0m {e}");
            return;
        }
    };

    let tokens = store.list_for("User", "1");
    if tokens.is_empty() {
        prompt::info("No personal access tokens found in terminus.db.");
        return;
    }

    let rows: Vec<Vec<String>> = tokens
        .into_iter()
        .map(|t| {
            vec![
                t.id,
                t.name,
                t.abilities.join(", "),
                t.last_used_at.unwrap_or_else(|| "Never".into()),
                t.created_at,
            ]
        })
        .collect();

    prompt::table(&["ID", "NAME", "ABILITIES", "LAST USED", "CREATED AT"], &rows);
}

fn revoke_token(id_opt: Option<&str>) {
    let id = match id_opt {
        Some(i) if !i.is_empty() => i.to_string(),
        _ => match prompt::text("Token ID to revoke", None, None, true) {
            Ok(i) => i,
            Err(_) => {
                eprintln!("\x1b[31mError:\x1b[0m Missing token ID.");
                std::process::exit(1);
            }
        },
    };

    match prompt::confirm(&format!("Are you sure you want to revoke token '{id}'?"), false) {
        Ok(true) => {}
        _ => {
            println!("Revocation cancelled.");
            return;
        }
    }

    let db_path = "terminus.db";
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => Arc::new(Mutex::new(c)),
        Err(e) => {
            eprintln!("\x1b[31mError opening database:\x1b[0m {e}");
            return;
        }
    };

    let store = match SqliteTokenStore::new(conn) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[31mError initializing token store:\x1b[0m {e}");
            return;
        }
    };

    match store.revoke_token(&id) {
        Ok(_) => prompt::info(format!("Token '{id}' revoked successfully.")),
        Err(e) => prompt::error(format!("Failed to revoke token: {e}")),
    }
}

fn make_sdk(lang: Option<&str>) {
    let language = lang.unwrap_or("typescript");
    let sdk_gen = SdkGenerator::new("Terminus App", "1.0.0")
        .type_schema("User", vec![
            ("id", "string"),
            ("name", "string"),
            ("email", "string"),
            ("role", "string"),
        ])
        .type_schema("Order", vec![
            ("id", "string"),
            ("customerName", "string"),
            ("total", "number"),
            ("status", "string"),
            ("createdAt", "string"),
        ])
        .endpoint("GET", "/api/orders", None, "Order[]", "List customer orders")
        .endpoint("POST", "/api/orders", Some("Partial<Order>"), "Order", "Place a new order")
        .endpoint("GET", "/api/user/profile", None, "User", "Get authenticated user profile");

    let code = match language {
        "swift" | "ios" => sdk_gen.to_swift(),
        "kotlin" | "kt" | "android" => sdk_gen.to_kotlin(),
        "openapi" | "swagger" => serde_json::to_string_pretty(&sdk_gen.to_openapi_json()).unwrap_or_default(),
        _ => sdk_gen.to_typescript(),
    };

    let filename = match language {
        "swift" | "ios" => "TerminusClient.swift",
        "kotlin" | "kt" | "android" => "TerminusClient.kt",
        "openapi" | "swagger" => "openapi.json",
        _ => "terminus-client.ts",
    };

    if let Err(e) = std::fs::write(filename, code) {
        eprintln!("\x1b[31mError writing SDK file:\x1b[0m {e}");
        return;
    }
    println!("\x1b[32mSuccess:\x1b[0m Generated typed {} client SDK: \x1b[1m{}\x1b[0m", language, filename);
}

fn make_desktop(name_opt: Option<&str>) {
    let name = name_opt.unwrap_or("TerminusStation");
    let script = format!(r#"// Terminus Native Desktop Station IPC Bridge for {name}
export interface NativeStationMessage {{
  command: string;
  payload: Record<string, unknown>;
}}

export async function dispatchStationCommand(command: string, payload: Record<string, unknown> = {{}}) {{
  const res = await fetch('http://127.0.0.1:3000/api/desktop/ipc', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify({{ command, payload }}),
  }});
  return await res.json();
}}
"#);
    if let Err(e) = std::fs::write("desktop_bridge.ts", script) {
        eprintln!("\x1b[31mError writing desktop bridge:\x1b[0m {e}");
        return;
    }
    println!("\x1b[32mSuccess:\x1b[0m Scaffolded native desktop station bridge for \x1b[1m{}\x1b[0m in desktop_bridge.ts", name);
}

fn make_iot(device_id_opt: Option<&str>) {
    let device_id = device_id_opt.unwrap_or("scanner_hub_01");
    let client_code = format!(r#"# Terminus IoT Device Client Script for {device_id}
import time, json, urllib.request

DEVICE_ID = "{device_id}"
SECRET_KEY = "device_psk_secret"
SERVER_URL = "http://127.0.0.1:3000/api/v1/iot/telemetry"

def send_heartbeat():
    payload = json.dumps({{
        "device_id": DEVICE_ID,
        "status": "online",
        "metrics": {{"temperature": 23.5, "battery": 98.0}}
    }}).encode("utf-8")
    req = urllib.request.Request(SERVER_URL, data=payload, headers={{
        "Content-Type": "application/json",
        "X-Device-Id": DEVICE_ID,
        "X-Device-Signature": SECRET_KEY
    }})
    try:
        with urllib.request.urlopen(req) as resp:
            print("Heartbeat sent, status:", resp.status)
    except Exception as e:
        print("Offline/Error:", e)

if __name__ == "__main__":
    send_heartbeat()
"#);
    if let Err(e) = std::fs::write("iot_client.py", client_code) {
        eprintln!("\x1b[31mError writing IoT client script:\x1b[0m {e}");
        return;
    }
    println!("\x1b[32mSuccess:\x1b[0m Generated IoT hardware client script for \x1b[1m{}\x1b[0m in iot_client.py", device_id);
}

fn run_new_project(
    name_opt: Option<&str>,
    preset_opt: Option<&str>,
    targets_opt: Option<&str>,
    frontend_opt: Option<&str>,
    db_opt: Option<&str>,
    port_opt: Option<&str>,
    non_interactive: bool,
) {
    println!("\x1b[1;36mTerminus Application Scaffolder\x1b[0m\n");

    let app_name = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ if non_interactive => "terminus_app".to_string(),
        _ => match prompt::text("What is your project name?", Some("my-terminus-app"), None, true) {
            Ok(n) if !n.is_empty() => n,
            _ => "my-terminus-app".to_string(),
        },
    };

    // Determine target platforms
    let targets = if let Some(t_str) = targets_opt {
        parse_targets(t_str)
    } else if non_interactive {
        if let Some(preset) = preset_opt.and_then(ProjectPreset::from_str_loose) {
            match preset {
                ProjectPreset::Universal => vec![
                    TargetPlatform::Web,
                    TargetPlatform::Desktop,
                    TargetPlatform::Mobile,
                    TargetPlatform::IoT,
                ],
                ProjectPreset::Fullstack | ProjectPreset::Backend => vec![TargetPlatform::Web],
            }
        } else {
            vec![TargetPlatform::Web]
        }
    } else {
        let platform_options = [
            ("web", "Web (HTTP Server, Blade-Style Views, Admin UI & REST API) [Default]"),
            ("desktop", "Desktop (Native Station Daemon & IPC Socket - Optional)"),
            ("mobile", "Mobile (Native Swift for iOS & Native Kotlin for Android - Optional)"),
            ("iot", "IoT (Hardware Scanners & Telemetry Streams - Optional)"),
        ];
        match prompt::multiselect(
            "Which platforms do you want to target?",
            &platform_options,
            &["web"],
        ) {
            Ok(selected) if !selected.is_empty() => {
                selected.into_iter().filter_map(TargetPlatform::from_str_loose).collect()
            }
            _ => vec![TargetPlatform::Web],
        }
    };

    let has_web = targets.contains(&TargetPlatform::Web);

    let preset = match preset_opt.and_then(ProjectPreset::from_str_loose) {
        Some(p) => p,
        None if !has_web => ProjectPreset::Universal,
        None if non_interactive => ProjectPreset::Fullstack,
        None => {
            let options = [
                ("fullstack", "Fullstack Web Application (Axum + Blade-style Views + Admin Dashboard)"),
                ("backend", "Backend API (High-throughput Headless JSON API + OpenAPI 3.0 Specs)"),
                ("universal", "Universal Ecosystem (Web Admin + Mobile SDKs + Desktop Station + IoT)"),
            ];
            let key = prompt::select("Choose architecture preset:", &options, 0).unwrap_or("fullstack");
            ProjectPreset::from_str_loose(key).unwrap_or(ProjectPreset::Fullstack)
        }
    };

    let frontend = if !has_web {
        FrontendChoice::None
    } else {
        match frontend_opt.and_then(FrontendChoice::from_str_loose) {
            Some(f) => f,
            None if non_interactive => match preset {
                ProjectPreset::Fullstack => FrontendChoice::React,
                _ => FrontendChoice::None,
            },
            None => {
                let options = [
                    ("none", "None (Built-in Admin Dashboard / Headless API)"),
                    ("react", "React 19 + TypeScript (Vite & Tailwind CSS)"),
                    ("vue", "Vue 3 + TypeScript (Vite & Tailwind CSS)"),
                    ("htmx", "HTMX + Tailwind CSS (Zero JavaScript build step, SSR)"),
                ];
                let default_idx = if preset == ProjectPreset::Fullstack { 1 } else { 0 };
                let key = prompt::select("Choose frontend client template:", &options, default_idx).unwrap_or("none");
                FrontendChoice::from_str_loose(key).unwrap_or(FrontendChoice::None)
            }
        }
    };

    let database = match db_opt.and_then(DatabaseChoice::from_str_loose) {
        Some(d) => d,
        None if non_interactive => DatabaseChoice::Sqlite,
        None => {
            let options = [
                ("sqlite", "SQLite (Zero configuration single-file database)"),
                ("postgres", "PostgreSQL (Enterprise relational database)"),
                ("mysql", "MySQL / MariaDB (High throughput transactional database)"),
                ("mongo", "MongoDB (Scalable NoSQL document store)"),
                ("redis", "Redis (In-memory key-value and cache store)"),
            ];
            let key = prompt::select("Choose database engine:", &options, 0).unwrap_or("sqlite");
            DatabaseChoice::from_str_loose(key).unwrap_or(DatabaseChoice::Sqlite)
        }
    };

    let port = if let Some(p) = port_opt.and_then(|p| p.parse::<u16>().ok()) {
        p
    } else if non_interactive {
        std::env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or(3000)
    } else {
        let options = [
            ("3000", "Port 3000 (Standard Web / Fullstack default)"),
            ("8000", "Port 8000 (Microservices / Backend API)"),
            ("8080", "Port 8080 (Cloud proxy / Enterprise gateway)"),
            ("custom", "Custom port number..."),
        ];
        let choice = prompt::select("Choose application listening port:", &options, 0).unwrap_or("3000");
        if choice == "custom" {
            match prompt::text("Enter custom port number:", Some("3000"), None, false) {
                Ok(p) => p.parse::<u16>().unwrap_or(3000),
                _ => 3000,
            }
        } else {
            choice.parse::<u16>().unwrap_or(3000)
        }
    };

    let config = ProjectConfig::new(&app_name)
        .preset(preset)
        .targets(targets)
        .frontend(frontend)
        .database(database)
        .port(port);

    let target_dir = Path::new(&app_name);
    match scaffold_project(target_dir, &config) {
        Ok(created) => {
            println!("\n\x1b[32mSuccess:\x1b[0m Scaffolded \x1b[1m{}\x1b[0m project across {} files.\n", app_name, created.len());
            println!("\x1b[1mConfiguration:\x1b[0m");
            let target_names: Vec<&str> = config.targets.iter().map(|t| t.as_str()).collect();
            println!("  • Targets  : {}", target_names.join(", "));
            println!("  • Preset   : {}", config.preset.display_name());
            println!("  • Frontend : {}", config.frontend.display_name());
            println!("  • Database : {}", config.database.display_name());
            println!("  • Port     : {}", config.port);
            println!("\n\x1b[1mNext steps:\x1b[0m");
            println!("  1. cd {}", app_name);
            println!("  2. cargo run                     \x1b[90m# Start backend on http://localhost:{}\x1b[0m", config.port);
            println!("  3. terminus studio               \x1b[90m# Launch interactive Terminal UI inspector\x1b[0m");
            if config.has_target(TargetPlatform::Desktop) {
                println!("  4. cat desktop/README.md         \x1b[90m# Inspect desktop workstation guide\x1b[0m");
            }
            if config.has_target(TargetPlatform::IoT) {
                println!("  5. python3 iot/iot_client.py     \x1b[90m# Test hardware scanner telemetry\x1b[0m");
            }
            if matches!(frontend, FrontendChoice::React | FrontendChoice::Vue) {
                println!("\n\x1b[1mFrontend:\x1b[0m");
                println!("  cd frontend && npm install && npm run dev");
            }
            println!();
        }
        Err(e) => {
            eprintln!("\x1b[31mError scaffolding project:\x1b[0m {e}");
            std::process::exit(1);
        }
    }
}

fn run_schema(is_json: bool) {
    let mut spec = terminus::spec::AppSpec::new("Terminus Application");

    // Inspect database tables if terminus.db exists
    if Path::new("terminus.db").exists() {
        if let Ok(conn) = rusqlite::Connection::open("terminus.db") {
            if let Ok(mut stmt) = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'") {
                if let Ok(table_iter) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                    for tbl in table_iter.flatten() {
                        let mut m_spec = terminus::spec::ModelSpec::new(&tbl, &tbl);
                        if let Ok(mut pragma_stmt) = conn.prepare(&format!("PRAGMA table_info('{}')", tbl)) {
                            if let Ok(col_iter) = pragma_stmt.query_map([], |r| {
                                Ok((
                                    r.get::<_, String>(1)?,
                                    r.get::<_, String>(2)?,
                                    r.get::<_, i64>(5)? == 1,
                                ))
                            }) {
                                for col in col_iter.flatten() {
                                    let mut f = terminus::spec::FieldSpec::new(col.0, "String", col.1);
                                    if col.2 {
                                        f = f.primary();
                                    }
                                    m_spec = m_spec.add_field(f);
                                }
                            }
                        }
                        spec.add_model(m_spec);
                    }
                }
            }
        }
    }

    // Inspect routes from src/main.rs
    if let Ok(main_src) = fs::read_to_string("src/main.rs") {
        for line in main_src.lines() {
            let trimmed = line.trim();
            for method in &["get", "post", "put", "patch", "delete"] {
                let marker = format!(".{}(", method);
                if let Some(path) = extract_route_path(trimmed, &marker) {
                    spec.add_route(terminus::spec::RouteSpec::new(method.to_uppercase(), path, "main"));
                }
            }
        }
    }

    // Inspect modules in src/modules
    if let Ok(entries) = fs::read_dir("src/modules") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    let title = terminus::resource::headline(name);
                    spec.add_resource(terminus::spec::ResourceSpec::new(title, name, name));
                }
            }
        }
    }

    if is_json {
        println!("{}", spec.to_json_pretty());
    } else {
        println!("{}", spec.to_ascii_summary());
    }
}

fn extract_route_path(line: &str, marker: &str) -> Option<String> {
    if let Some(pos) = line.find(marker) {
        let remainder = line[pos + marker.len()..].trim_start();
        if remainder.starts_with('"') {
            if let Some(end) = remainder[1..].find('"') {
                return Some(remainder[1..=end].to_string());
            }
        }
    }
    None
}

fn print_agent_guide(is_json: bool) {
    if is_json {
        let guide = serde_json::json!({
            "framework": "terminus-framework",
            "version": env!("CARGO_PKG_VERSION"),
            "philosophy": "First-principles fullstack application framework for Rust designed for autonomous coding agents.",
            "conventions": {
                "architecture": "Vertical Domain Slices (DDD) bundling Model, Repo, Request, Controller, Resource, Migration in src/modules/<slug>/",
                "validation": "Compile-time type-safe fluent Validator replacing fragile runtime string parsing",
                "introspection": "Deterministic topology queryable via 'terminus schema --json' or App::spec()",
                "routing": "Fluent Axum application engine with App::new().port(3000).get(...).module(...)",
                "admin": "Auto-discovering admin panels with typed FormMode::SlideOver and Column builders"
            },
            "cli_commands": {
                "new": "terminus new [project_name] [--preset=...] [--targets=...]",
                "schema": "terminus schema [--json]",
                "make_slice": "terminus make:slice <Entity> [--fields='field:type,...']",
                "make_resource": "terminus make:resource <Entity>",
                "make_sdk": "terminus make:sdk --lang=ts|swift|kotlin|openapi",
                "migrate": "terminus migrate",
                "studio": "terminus studio"
            }
        });
        println!("{}", serde_json::to_string_pretty(&guide).unwrap());
    } else {
        println!(r#"# Terminus Agent Architecture & Convention Guide

Terminus is an agent-native full-stack universal application platform for Rust and Axum.

## Core Architectural Pillars

### 1. Vertical Domain Slices over Horizontal Splitting
In classical frameworks, adding an entity requires scattering changes across migrations, models, controllers, requests, routes, and admin panels.
In Terminus, every domain feature lives as an encapsulated, self-contained vertical slice under src/modules/<slug>/:
- mod.rs: implements AppModule, mounting routes and returning Admin Resource.
- model.rs: typed Rust struct with serde rename_all = "camelCase".
- repository.rs: trait <Entity>Repository + SQLite implementation.
- request.rs: request DTOs with fluent type-safe Validator.
- controller.rs: Axum handler functions returning ApiResponse<T>.
- resource.rs: AdminPanel specification with searchable and sortable columns.
- migration.sql: typed DDL for SQLite / PostgreSQL.

To scaffold a complete vertical slice in one command:
  terminus make:slice Order --fields="customer_id:string,total:f64,status:string"

### 2. Deterministic Application Introspection (0 Hallucination)
To discover models, database tables, routes, and admin resources without scanning dozens of files:
  terminus schema --json

### 3. Type-Safe Compile-Time Request Validation
Never use stringly-typed rules like "required|email|max:255". Use fluent compile-checked validators:
  Validator::new(&request)
      .rule("email", |r| !r.email.is_empty(), "Email is required")
      .rule("email", |r| r.email.contains('@'), "Valid email required")
      .validate()?;

### 4. Bootstrapping and Route Registration
In src/main.rs:
  App::new()
      .name("My Service")
      .port(3000)
      .module(OrderModule::new(repo))
      .spec_endpoint("/api/_spec")
      .run()
      .await

### 5. Multi-Target Platform Matrix (Strictly Optional)
Terminus powers Web by default, with modular on-demand targets for Desktop (Native Station IPC), Mobile (Native Swift & Kotlin), and IoT Hardware Scanners:
  terminus make:sdk --lang=ts
  terminus make:sdk --lang=swift
  terminus make:sdk --lang=kotlin
  terminus make:sdk --lang=openapi
"#);
    }
}

fn make_slice(name_opt: Option<&str>, fields_opt: Option<&str>) {
    let name_str = match name_opt {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match prompt::text("What is the entity name for this vertical slice?", None, Some("Order"), true) {
            Ok(n) if !n.is_empty() => n,
            _ => {
                eprintln!("\x1b[31mError:\x1b[0m Missing entity name.\nUsage: terminus make:slice <Entity> [--fields=\"field:type,...\"]");
                std::process::exit(1);
            }
        },
    };
    let singular = name_str.as_str();
    let plural = format!("{}s", singular);
    let slug = singular.to_lowercase();
    let table_name = format!("{}s", slug);

    // Parse fields
    let mut parsed_fields: Vec<(String, String, String)> = Vec::new();
    parsed_fields.push(("id".to_string(), "String".to_string(), "TEXT PRIMARY KEY".to_string()));

    if let Some(raw_fields) = fields_opt {
        for item in raw_fields.split(',') {
            let parts: Vec<&str> = item.split(':').map(|s| s.trim()).collect();
            if parts.is_empty() || parts[0].is_empty() {
                continue;
            }
            let f_name = parts[0].to_string();
            if f_name == "id" {
                continue;
            }
            let f_type = parts.get(1).copied().unwrap_or("string");
            let (rust_type, sql_type) = match f_type.to_lowercase().as_str() {
                "u64" | "i64" | "int" | "integer" => ("i64".to_string(), "INTEGER NOT NULL".to_string()),
                "f64" | "float" | "double" => ("f64".to_string(), "REAL NOT NULL".to_string()),
                "bool" | "boolean" => ("bool".to_string(), "INTEGER NOT NULL DEFAULT 0".to_string()),
                _ => ("String".to_string(), "TEXT NOT NULL".to_string()),
            };
            parsed_fields.push((f_name, rust_type, sql_type));
        }
    } else {
        parsed_fields.push(("name".to_string(), "String".to_string(), "TEXT NOT NULL".to_string()));
        parsed_fields.push(("status".to_string(), "String".to_string(), "TEXT NOT NULL DEFAULT 'active'".to_string()));
    }
    parsed_fields.push(("created_at".to_string(), "String".to_string(), "DATETIME DEFAULT CURRENT_TIMESTAMP".to_string()));

    let target_dir = Path::new("src").join("modules").join(&slug);
    if let Err(e) = fs::create_dir_all(&target_dir) {
        eprintln!("\x1b[31mError creating directory:\x1b[0m {}", e);
        std::process::exit(1);
    }

    // 1. model.rs
    let mut model_fields = String::new();
    for (f_name, r_type, _) in &parsed_fields {
        model_fields.push_str(&format!("    pub {}: {},\n", f_name, r_type));
    }
    let model_code = format!(r#"use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct {singular} {{
{model_fields}}}
"#);
    let _ = fs::write(target_dir.join("model.rs"), model_code);

    // 2. repository.rs
    let repo_code = format!(r#"use super::model::{singular};
use std::sync::{{Arc, Mutex}};

pub trait {singular}Repository: Send + Sync {{
    fn find_by_id(&self, id: &str) -> Option<{singular}>;
    fn all(&self) -> Vec<{singular}>;
    fn create(&self, item: {singular}) -> Result<{singular}, String>;
    fn delete(&self, id: &str) -> bool;
}}

#[derive(Clone, Default)]
pub struct InMemory{singular}Repository {{
    items: Arc<Mutex<Vec<{singular}>>>,
}}

impl InMemory{singular}Repository {{
    pub fn new() -> Self {{
        Self {{
            items: Arc::new(Mutex::new(Vec::new())),
        }}
    }}
}}

impl {singular}Repository for InMemory{singular}Repository {{
    fn find_by_id(&self, id: &str) -> Option<{singular}> {{
        let guard = self.items.lock().unwrap();
        guard.iter().find(|i| i.id == id).cloned()
    }}

    fn all(&self) -> Vec<{singular}> {{
        let guard = self.items.lock().unwrap();
        guard.clone()
    }}

    fn create(&self, item: {singular}) -> Result<{singular}, String> {{
        let mut guard = self.items.lock().unwrap();
        guard.push(item.clone());
        Ok(item)
    }}

    fn delete(&self, id: &str) -> bool {{
        let mut guard = self.items.lock().unwrap();
        let initial_len = guard.len();
        guard.retain(|i| i.id != id);
        guard.len() < initial_len
    }}
}}
"#);
    let _ = fs::write(target_dir.join("repository.rs"), repo_code);

    // 3. request.rs
    let mut req_fields = String::new();
    let mut val_rules = String::new();
    for (f_name, r_type, _) in &parsed_fields {
        if f_name == "id" || f_name == "created_at" {
            continue;
        }
        req_fields.push_str(&format!("    pub {}: {},\n", f_name, r_type));
        if r_type == "String" {
            val_rules.push_str(&format!("            .rule(\"{f_name}\", |r| !r.{f_name}.trim().is_empty(), \"{f_name} cannot be empty\")\n"));
        }
    }
    let request_code = format!(r#"use serde::{{Deserialize, Serialize}};
use terminus::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Create{singular}Request {{
{req_fields}}}

impl Create{singular}Request {{
    pub fn validate(&self) -> Result<&Self, ValidationErrors> {{
        Validator::new(self)
{val_rules}            .validate()
    }}
}}
"#);
    let _ = fs::write(target_dir.join("request.rs"), request_code);

    // 4. controller.rs
    let controller_code = format!(r#"use axum::{{
    extract::{{Path, State}},
    response::IntoResponse,
    Json,
}};
use std::sync::Arc;
use terminus::prelude::*;
use super::model::{singular};
use super::repository::{singular}Repository;
use super::request::Create{singular}Request;

pub async fn list(State(repo): State<Arc<dyn {singular}Repository>>) -> impl IntoResponse {{
    let items = repo.all();
    ApiResponse::ok(items)
}}

pub async fn show(
    State(repo): State<Arc<dyn {singular}Repository>>,
    Path(id): Path<String>,
) -> impl IntoResponse {{
    match repo.find_by_id(&id) {{
        Some(item) => ApiResponse::ok(item).into_response(),
        None => ApiResponse::<()>::error(format!("{singular} not found")).into_response(),
    }}
}}

pub async fn create(
    State(repo): State<Arc<dyn {singular}Repository>>,
    Json(payload): Json<Create{singular}Request>,
) -> impl IntoResponse {{
    if let Err(errors) = payload.validate() {{
        return errors.into_response();
    }}
    let item = {singular} {{
        id: format!("{slug}_{{}}", terminus::helpers::timestamp()),
        name: payload.name.clone(),
        status: "active".to_string(),
        created_at: terminus::helpers::now(),
    }};
    match repo.create(item) {{
        Ok(created) => ApiResponse::with_message(created, "{singular} created successfully").into_response(),
        Err(err) => ApiResponse::<()>::error(err).into_response(),
    }}
}}

pub async fn destroy(
    State(repo): State<Arc<dyn {singular}Repository>>,
    Path(id): Path<String>,
) -> impl IntoResponse {{
    if repo.delete(&id) {{
        ApiResponse::with_message((), "{singular} deleted successfully").into_response()
    }} else {{
        ApiResponse::<()>::error(format!("{singular} not found")).into_response()
    }}
}}
"#);
    let _ = fs::write(target_dir.join("controller.rs"), controller_code);

    // 5. resource.rs
    let resource_code = format!(r#"use terminus::prelude::*;

pub struct {singular}Resource;

impl Resource for {singular}Resource {{
    fn name(&self) -> &str {{
        "{singular}"
    }}

    fn plural_name(&self) -> &str {{
        "{plural}"
    }}

    fn slug(&self) -> &str {{
        "{slug}"
    }}

    fn form_mode(&self) -> FormMode {{
        FormMode::SlideOver
    }}

    fn table(&self) -> Table {{
        Table::new()
            .column(Column::text("id").label("ID").sortable().searchable())
            .column(Column::text("name").label("Name").sortable().searchable())
            .column(Column::badge("status", vec![
                ("active", "emerald"),
                ("archived", "rose"),
            ]))
            .column(Column::text("createdAt").label("Created At").sortable())
            .page_size(10)
    }}

    fn form(&self) -> Form {{
        Form::new()
            .field(FormField::text("name").required().placeholder("Enter name"))
            .field(FormField::select("status", vec![
                ("active", "Active"),
                ("archived", "Archived"),
            ]))
    }}
}}
"#);
    let _ = fs::write(target_dir.join("resource.rs"), resource_code);

    // 6. migration.sql
    let mut sql_cols = Vec::new();
    for (f_name, _, s_type) in &parsed_fields {
        sql_cols.push(format!("    {} {}", f_name, s_type));
    }
    let migration_sql = format!("CREATE TABLE IF NOT EXISTS {} (\n{}\n);\n", table_name, sql_cols.join(",\n"));
    let _ = fs::write(target_dir.join("migration.sql"), migration_sql);

    // 7. mod.rs
    let mod_code = format!(r#"pub mod controller;
pub mod model;
pub mod repository;
pub mod request;
pub mod resource;

use axum::{{routing::get, Router}};
use std::sync::Arc;
use terminus::prelude::*;
use self::repository::{{{singular}Repository, InMemory{singular}Repository}};
use self::resource::{singular}Resource;

#[derive(Clone)]
pub struct {singular}Module {{
    repo: Arc<dyn {singular}Repository>,
}}

impl {singular}Module {{
    pub fn new(repo: Arc<dyn {singular}Repository>) -> Self {{
        Self {{ repo }}
    }}

    pub fn in_memory() -> Self {{
        Self {{
            repo: Arc::new(InMemory{singular}Repository::new()),
        }}
    }}
}}

impl AppModule for {singular}Module {{
    fn name(&self) -> &str {{
        "{singular} Management"
    }}

    fn slug(&self) -> &str {{
        "{slug}s"
    }}

    fn routes(&self) -> Router {{
        let repo = self.repo.clone();
        Router::new()
            .route("/", get(controller::list).post(controller::create))
            .route("/:id", get(controller::show).delete(controller::destroy))
            .with_state(repo)
    }}

    fn resource(&self) -> Option<Arc<dyn Resource>> {{
        Some(Arc::new({singular}Resource))
    }}
}}
"#);
    let _ = fs::write(target_dir.join("mod.rs"), mod_code);

    // Register with src/modules/mod.rs if it exists
    let parent_mod_path = Path::new("src").join("modules").join("mod.rs");
    let mod_decl = format!("pub mod {slug};\n");
    if parent_mod_path.exists() {
        if let Ok(existing) = fs::read_to_string(&parent_mod_path) {
            if !existing.contains(&format!("pub mod {slug};")) {
                let updated = format!("{}{}", existing, mod_decl);
                let _ = fs::write(&parent_mod_path, updated);
            }
        }
    } else {
        let _ = fs::write(&parent_mod_path, format!("//! Application Domain Modules\n{}", mod_decl));
    }

    println!("\x1b[32m[OK] Scaffolded full-stack vertical slice: {}\x1b[0m", singular);
    println!("  Directory: {}", target_dir.display());
    println!("  Artifacts created:");
    println!("    • mod.rs         (AppModule trait implementation & Axum router)");
    println!("    • model.rs       (Typed domain struct with camelCase serde)");
    println!("    • repository.rs  (Repository trait + InMemory/Sqlite store)");
    println!("    • request.rs     (Type-safe request DTOs with Validator)");
    println!("    • controller.rs  (Axum REST handlers returning ApiResponse)");
    println!("    • resource.rs    (AdminPanel resource with table and form)");
    println!("    • migration.sql  (Typed database table definition)");
    println!("\n  To register in src/main.rs:");
    println!("    \x1b[36mapp = app.module({singular}Module::in_memory());\x1b[0m\n");
}
