# Terminus

**An agent-native, batteries-included application framework for Axum and Rust.**

Fullstack Web Admin • REST API • Multi-DB (PostgreSQL, MySQL, MongoDB, Redis, SQLite) • Native Mobile (iOS/Android) • Native Desktop & IoT

[![CI](https://github.com/prateekbhujel/terminus/actions/workflows/rust.yml/badge.svg)](https://github.com/prateekbhujel/terminus/actions/workflows/rust.yml)
[![Version: 0.1.0](https://img.shields.io/badge/crates.io-v0.1.0-emerald.svg)](https://crates.io/crates/terminus-framework)
[![Docs](https://img.shields.io/badge/docs.rs-terminus--framework-blue.svg)](https://docs.rs/terminus-framework)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## Why Terminus?

[Axum](https://github.com/tokio-rs/axum) is a world-class, high-performance web router. But it is intentionally minimal. For production applications, developers repeatedly re-implement the same foundational layers from scratch:

- An administrative dashboard and CRUD interfaces
- Multi-database persistence, connection management, and migrations
- Request validation with structured JSON error responses
- Authentication, dynamic RBAC, and personal access tokens
- **Deterministic machine introspection**, so autonomous AI coding agents can inspect models, routes, and database tables without hallucination

Terminus provides strong conventions and full-stack solutions to these needs, designed from first principles around what makes sense in Rust.

---

## 2-Minute Quickstart

### Method 1: The Global Installer (`terminus new`)

Install the standalone Terminus CLI globally:

**macOS / Linux / WSL / Alpine:**
```bash
curl -fsSL https://raw.githubusercontent.com/prateekbhujel/terminus/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/prateekbhujel/terminus/main/install.ps1 | iex
```

**Or via Cargo:**
```bash
cargo install terminus-framework
```

Once installed, scaffold a production-ready application from anywhere:

```bash
# Interactive project wizard
terminus new my_app

# Or scaffold non-interactively with specific presets
terminus new my_app --preset=fullstack
terminus new my_omni --targets=web,desktop,mobile

cd my_app
cargo run
```

Open `http://localhost:3000` in your browser. Browse to `http://localhost:3000/admin` to see the live admin dashboard backed by SQLite.

### Method 2: Run the Working Quickstart Example

Clone the repository and run the working reference quickstart:

```bash
git clone https://github.com/prateekbhujel/terminus.git
cd terminus
cargo run --example quickstart
```

### Method 3: Add to an Existing Cargo Project

Add Terminus to your `Cargo.toml`:

```toml
[dependencies]
terminus-framework = "0.1.0"
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

Create a minimal application in `src/main.rs`:

```rust
use terminus::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("terminus.db")?;
    let user_repo = Arc::new(SqliteUserRepository::new(db.connection_arc())?);

    let admin = AdminPanel::new()
        .brand_name("My App Admin")
        .resource(AuditLogResource::new(Arc::new(SqliteAuditRepository::new(db.connection_arc())?)));

    App::new()
        .name("My Application")
        .port(3000)
        .get("/", || async {
            html_response("<h1>Welcome to Terminus!</h1><p><a href='/admin'>Admin Panel &rarr;</a></p>")
        })
        .get("/api/health", || async {
            json_response(serde_json::json!({ "status": "ok", "timestamp": now() }))
        })
        .panel(admin)
        .spec_endpoint("/api/_spec")
        .run()
        .await
}
```

---

## Killer Feature: Deterministic Machine Introspection

In a world where developers pair program with autonomous AI coding agents, agents frequently hallucinate route names, database columns, and model relationships because they must guess across dozens of files.

Terminus solves this with **deterministic application introspection**:

```bash
terminus schema --json
```

Outputs a structured, zero-hallucination JSON specification of your entire application:

```json
{
  "framework": "terminus-framework",
  "version": "0.1.0",
  "app_name": "Warehouse Master",
  "routes": [
    { "method": "GET", "path": "/api/items", "handler_name": "list_items", "auth_required": true }
  ],
  "models": [
    {
      "name": "Item",
      "table": "items",
      "fields": [
        { "name": "id", "rust_type": "String", "sql_type": "TEXT", "primary_key": true },
        { "name": "sku", "rust_type": "String", "sql_type": "TEXT" },
        { "name": "quantity", "rust_type": "i64", "sql_type": "INTEGER" }
      ]
    }
  ],
  "resources": [
    { "name": "Item", "slug": "items", "searchable_fields": ["sku", "name"] }
  ]
}
```

You can also view a clean terminal summary:

```bash
terminus schema
```

---

## Full-Stack Vertical Domain Slices

In classical MVC frameworks, adding a single entity requires scattering code across migrations, models, controllers, requests, routes, and admin panels.

Terminus groups features into self-contained vertical domain slices under `src/modules/<slug>/`:

```bash
terminus make:slice Order --fields="customer_id:string,total:f64,status:string"
```

Generates:
- `mod.rs`: `AppModule` implementation mounting API routes and returning admin `Resource`.
- `model.rs`: Typed domain struct with `camelCase` serde serialization.
- `repository.rs`: Strongly-typed repository trait and SQLite store with CRUD methods.
- `request.rs`: Request DTOs with compile-time type-safe `Validator`.
- `controller.rs`: Axum REST handlers returning `ApiResponse<T>`.
- `resource.rs`: Admin panel resource with searchable and sortable columns.
- `migration.sql`: Typed database table definition.

To register the slice in `src/main.rs`:
```rust
app = app.module(OrderModule::new(repo));
```

---

## Compile-Time Type-Safe Validation

Classical web frameworks use stringly-typed rules like `"required|email|max:255"`, which fail silently at runtime if an agent or developer makes a typo.

Terminus uses fluent, compile-checked validation in Rust:

```rust
use terminus::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub name: String,
    pub age: Option<u32>,
}

impl CreateUserRequest {
    pub fn validate(&self) -> Result<&Self, ValidationErrors> {
        Validator::new(self)
            .rule("email", |r| !r.email.is_empty(), "Email is required")
            .rule("email", |r| r.email.contains('@'), "Valid email address required")
            .rule("name", |r| r.name.len() >= 2, "Name must be at least 2 characters")
            .rule_when(|r| r.age.is_some(), "age", |r| r.age.unwrap() >= 18, "Must be at least 18")
            .validate()
    }
}
```

When validation fails, `ValidationErrors` implements Axum's `IntoResponse`, automatically returning HTTP 422 with structured JSON:

```json
{
  "message": "Validation failed",
  "errors": {
    "email": ["Valid email address required"]
  }
}
```

For untyped JSON payloads, use `JsonValidator`:

```rust
let mut jv = JsonValidator::new(&payload);
let email = jv.require_email("email");
let price = jv.require_number("price");
jv.validate()?;
```

---

## First-Class Ergonomics: `Valid<T>`, `db::query!`, & Multi-Database Engines

Terminus bridges high-level developer ergonomics with Rust's memory safety and zero-cost abstractions:

### 1. The `Valid<T>` Extractor & `Validate` Trait

Extract, deserialize, and validate request payloads in a single parameter. If validation fails, Terminus immediately halts request execution and returns an HTTP 422 Unprocessable Entity response containing structured JSON errors:

```rust
use terminus::prelude::*;

#[derive(Deserialize, Serialize)]
pub struct Subscriber {
    pub name: String,
    pub email: String,
}

impl Validate for Subscriber {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if self.name.trim().is_empty() {
            errors.add("name", "Name is required");
        }
        if !self.email.contains('@') {
            errors.add("email", "Valid email address required");
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### 2. High-Level Controller Handlers & `db::query!`

Handlers can directly return `Result<Redirect>` or `Result<Json<T>>`. Parameterized queries execute with zero boilerplate:

```rust
pub async fn store(Valid(subscriber): Valid<Subscriber>) -> Result<Redirect> {
    db::query!(
        "INSERT INTO subscribers (name, email) VALUES (?, ?)",
        subscriber.name,
        subscriber.email
    )
    .insert()
    .await?;

    Ok(redirect("/subscribers"))
}
```

### 3. Multi-Database Engines (Postgres, MySQL, Mongo, Redis, SQLite)

Terminus connects to your database with a single connection URL string and automatically compiles parameter markers to match target engine dialects (`?` for SQLite/MySQL, `$1, $2` for PostgreSQL):

```rust
// Connect to PostgreSQL, MySQL, MongoDB, Redis, or SQLite
let db = Database::connect("postgres://postgres:secret@localhost:5432/ecommerce")?;
// Or SQLite file / in-memory
let db = Database::connect("sqlite://app.db")?;

// MongoDB NoSQL Document Collections
let events = db.collection("events");
events.insert_one(serde_json::json!({ "event": "order_placed", "amount": 149.99 }))?;

// Redis Key-Value & Session Store with TTL
let redis = db.redis();
redis.set("session:token_xyz", "user_101", Some(3600)); // 1 hour TTL
let user = redis.get("session:token_xyz");
```

### 4. Configurable Port Presets & Cloud Deployments

Select ports interactively during `terminus new`, specify via CLI flags (`--port=8080`), or use presets:

```rust
// Port presets
let app = App::port_3000(); // Standard web app (3000)
let app = App::port_8000(); // Microservices backend (8000)
let app = App::port_8080(); // Cloud proxy / Gateway (8080)
let app = App::new().port(9000); // Custom port

// Cloud-native: Automatically detects $PORT injected by Docker, Fly.io, Railway, or Render
app.run().await
```

### 5. Blade-Style Views, Sessions & One-Time Flash Messages

Render fullstack web views wrapped in responsive Tailwind layouts, with automatic CSRF tokens, flash alert banners, and seamless redirects:

```rust
use terminus::prelude::*;

// 1. Return a web view with Tailwind layout & flash banner
pub async fn index() -> impl IntoResponse {
    view("Dashboard", r#"
        <div class="space-y-6">
            <h1 class="text-2xl font-bold text-zinc-100">Welcome to Terminus Web</h1>
            <p class="text-zinc-400">Zero frontend build step needed. High-converting SSR.</p>
        </div>
    "#)
}

// 2. Redirect with chained flash notifications
pub async fn store(Valid(payload): Valid<CreateUserRequest>) -> Response {
    // Save to database...
    redirect("/users")
        .with_success("Account created successfully!")
        .into_response()
}
```

### 6. Active Record Pagination (`Paginated<T>`)

Paginate database query results with zero boilerplate:

```rust
// Paginate query (page 1, 15 per page)
let paginated: Paginated<RowData> = db::query!("SELECT * FROM orders ORDER BY created_at DESC")
    .paginate(1, 15)
    .await?;

// Output accessible Tailwind CSS pagination buttons & page range indicators
let html_links = paginated.links_html("/orders");
// Renders: "Showing 1 to 15 of 142 results" with Previous / Page / Next buttons
```

### 7. Interactive Tinker REPL (`terminus tinker`)

Explore your application, test SQL queries, inspect database schemas, and hash passwords interactively from your terminal:

```bash
$ terminus tinker
Terminus Tinker REPL v0.1.0
Database: terminus.db (SQLite) | Type 'help' or 'exit'

terminus> tables
  TABLE NAME                     COLUMNS       ROWS
  ──────────────────────────────────────────────────────
  users                                7          4
  orders                               6          4
  _terminus_migrations                 4          4

terminus> SELECT id, customer_name, total_cents FROM orders;
  id | customer_name | total_cents
  --------------------------------
  1  | Acme Logistics| 25000
  2  | Global Corp   | 49900
(2 row(s) returned)

terminus> hash my_secret_password
Hash: $argon2id$v=19$m=19456,t=2,p=1$...
```

### 8. In-Process Task Scheduling (`terminus schedule:run`)

Define background recurring tasks directly in Rust without external cron daemons:

```rust
let schedule = Schedule::new();

schedule.call("cleanup:sessions", || {
    SessionStore::global().prune(86400 * 7);
}).hourly();

schedule.call("orders:sync", || {
    // Background order processing...
}).every_five_minutes();
```

---

## Admin Panel Resources

Define rich, server-rendered admin interfaces using declarative Rust:

```rust
use terminus::prelude::*;

pub struct ProductResource;

impl Resource for ProductResource {
    fn name(&self) -> &str { "Product" }
    fn plural_name(&self) -> &str { "Products" }
    fn slug(&self) -> &str { "products" }

    fn table(&self) -> Table {
        Table::new()
            .column(Column::text("id").label("ID").sortable().searchable())
            .column(Column::text("name").label("Name").sortable().searchable())
            .column(Column::badge("status", vec![
                ("active", "emerald"),
                ("draft", "amber"),
                ("archived", "rose"),
            ]))
            .column(Column::text("createdAt").label("Created At").sortable())
            .page_size(10)
    }

    fn form(&self) -> Form {
        Form::new()
            .field(FormField::text("name").required().placeholder("Item name"))
            .field(FormField::select("status", vec![
                ("active", "Active"),
                ("draft", "Draft"),
                ("archived", "Archived"),
            ]))
    }

    fn fetch_rows(&self, query: &QueryState) -> (Vec<RowData>, usize) {
        // Query SQLite or in-memory store
        (vec![], 0)
    }
}
```

---

## Global Ergonomic Helpers

Access common application state from handlers and domain logic:

```rust
use terminus::prelude::*;

pub async fn handle_checkout() -> Result<ApiResponse<serde_json::Value>, AbortError> {
    // 1. Authorization gate
    abort_unless(can("orders.create"), 403, "Permission Denied")?;

    // 2. Access active user context
    if let Some(current_user) = user() {
        println!("Order by: {}", current_user.name);
    }

    // 3. Current request data
    let token = request().input("token").unwrap_or_default();

    Ok(ApiResponse::ok(serde_json::json!({
        "status": "success",
        "timestamp": now()
    })))
}
```

---

## Dynamic Role-Based Access Control (RBAC)

```rust
use terminus::prelude::*;

let mut user = User::new("1", "Pratik", "pratik@example.com", Role::Superadmin);

// 1. Direct permission grant
user.give_permission_to("orders.refund");

// 2. Explicit Deny Precedence (Overrides administrative default grants)
user.deny_permission("finance.export");

assert!(!user.can("finance.export")); // Denied: Explicit deny overrides Superadmin!
assert!(user.can("orders.view"));     // Allowed: Superadmin default grant
```

---

## Password Hashing & Cryptography

Terminus includes pure-Rust cryptographic primitives with zero external C dependencies:

- **PBKDF2-HMAC-SHA256 (`Hash::make` & `Hash::check`)** conforming to RFC 2898 / NIST SP 800-132:
  ```rust
  use terminus::prelude::*;

  let hash = Hash::make("secret_password");
  assert!(Hash::check("secret_password", &hash));
  assert!(!Hash::check("wrong_password", &hash));
  ```
- **Cryptographic Webhook Verification (`WebhookVerifier`)**:
  Supports Stripe-style timestamped signatures (`t=...,v1=...`), GitHub `sha256=...` signatures, and constant-time comparison to prevent timing attacks.

---

## Terminal UI Studio (`terminus studio`)

Terminus includes an interactive cross-platform terminal dashboard to inspect database tables, active tokens, and service health directly in your terminal:

```bash
terminus studio
```

```text
================================================================================
  TERMINUS STUDIO — Terminal Telemetry, Database & Cache Inspector
  ONLINE  |  DB: CONNECTED (SQLite)  |  HOST: 127.0.0.1:3000
================================================================================
   [1] Overview  [2] Database  [3] Cache & Redis  [4] Queue  [5] API Tokens
--------------------------------------------------------------------------------
[TELEMETRY] SYSTEM TELEMETRY & SERVICE HEALTH:

  ┌──────────────────────┬──────────────────────┬──────────────────────┐
  │ DATABASE (SQLite)    │ CACHE & KEY-VALUE    │ QUEUE & JOBS         │
  │ Active Connections   │ In-Memory + Redis    │ 0 Pending Jobs       │
  │ Path: terminus.db    │ Sub-millisecond TTL  │ Worker: Idle / Ready │
  └──────────────────────┴──────────────────────┴──────────────────────┘

Shortcuts: [1-5] Switch Tab  [R] Refresh  [T <table_name>] Select Table  [Q] Exit
```

---

## CLI Reference

Terminus commands can be run directly as `terminus <command>` (or as a Cargo plugin `cargo terminus <command>`):

```bash
terminus <command> [options]
```

| Command | Description |
|---|---|
| `new [name] [--preset=...]` | Scaffold a new project (interactive wizard or flags) |
| `schema [--json]` | Inspect application topology (models, routes, admin resources) |
| `route:list \| routes` | List registered web routes, admin resources, and API endpoints |
| `tinker [--execute=...]` | Interactive database REPL console with query and command evaluation |
| `make:controller <Name> [-r]` | Scaffold a web controller (or RESTful resource controller) |
| `make:model <Name> [-mc]` | Scaffold a model with paired migration (-m) and controller (-c) |
| `make:seeder <Name>` | Scaffold a database seeder class |
| `make:slice <Name> [--fields=...]` | Scaffold a self-contained vertical domain slice |
| `agent:guide [--json]` | Display AI agent conventions and architecture directives |
| `studio` | Launch terminal UI telemetry dashboard and database inspector |
| `make:resource <Name>` | Scaffold an administrative resource |
| `make:module <Name>` | Scaffold a domain module |
| `make:repo <Name>` | Scaffold a SQLite repository implementation |
| `make:migration <Name>` | Generate a timestamped SQL migration file |
| `migrate` | Run pending database migrations |
| `migrate:status` | Show execution status of each migration batch |
| `migrate:rollback` | Rollback the latest batch of executed database migrations |
| `db:tables` | Inspect database schema and statistics |
| `db:seed` | Seed demo accounts and data into database |
| `schedule:run` | Execute due scheduled tasks and recurring in-process cron jobs |
| `schedule:list` | List scheduled tasks and recurring execution frequencies |
| `make:sdk [--lang=ts\|kotlin\|swift\|openapi]` | Export typed client SDKs (Optional) |
| `make:token [name]` | Issue a personal access token |
| `token:list` | List active access tokens |
| `token:revoke [id]` | Revoke an access token |
| `cache:clear` | Clear memory cache store |

---

## Modular Extensions (Strictly Optional Multi-Platform Matrix)

While Terminus defaults strictly to a pure Fullstack Web application, it provides scaffoldable native adapters connecting directly to your Rust domain engine:

- **Desktop Station Daemon (`--targets=desktop`)**: Native Rust workstation daemon (`desktop/station.rs`) with sub-millisecond cold boot and direct IPC socket communication.
- **Native Android SDK (`make:sdk --lang=kotlin`)**: Native Kotlin data classes, Coroutines (`suspend fun`), and `Dispatchers.IO` network client.
- **Native iOS SDK (`make:sdk --lang=swift`)**: Native Swift 5.9+ data structs and `async`/`await` URLSession client.
- **IoT Hardware Gateway (`--targets=iot`)**: HMAC-SHA256 authenticated barcode scanner & sensor telemetry scripts.

These are modular client bindings generated only on demand, keeping your web backend lean and uncluttered.

---

## Headless Testing & Assertions

Writing HTTP integration tests for web APIs often requires spinning up TCP listeners, managing OS ports, or writing verbose `tower::ServiceExt::oneshot` boilerplate.

Terminus provides an in-memory, headless `TestClient` with fluent assertions:

```rust
use terminus::prelude::*;

#[tokio::test]
async fn test_items_api() {
    let app = App::new()
        .get("/api/health", || async {
            json_response(serde_json::json!({ "status": "ok" }))
        })
        .post("/api/items", |axum::Json(payload): axum::Json<serde_json::Value>| async move {
            let mut jv = JsonValidator::new(&payload);
            jv.require_str("name");
            match jv.validate() {
                Ok(_) => (
                    axum::http::StatusCode::CREATED,
                    [("Content-Type", "application/json")],
                    payload.to_string(),
                )
                    .into_response(),
                Err(e) => e.into_response(),
            }
        });

    let client = TestClient::new(app);

    // 1. GET assertion
    let res = client.get("/api/health").await;
    res.assert_ok().assert_contains("ok");

    // 2. Validation failure assertion (HTTP 422)
    let invalid = client.post("/api/items", serde_json::json!({})).await;
    invalid.assert_unprocessable();

    // 3. Successful POST assertion (HTTP 201)
    let valid = client.post("/api/items", serde_json::json!({ "name": "Laptop" })).await;
    valid.assert_created();
}
```

---

## Empirical Performance: Terminus vs. Plain Axum

Run the benchmark suite locally:

```bash
cargo run --example benchmark --release
```

Measured in-memory across 50,000 iterations per scenario on Apple Silicon (zero socket jitter):

| Scenario | Throughput | p50 Latency | p95 Latency | p99 Latency | Framework Overhead |
|---|---|---|---|---|---|
| **Plain Axum 0.7 (GET)** | 951,123 req/s | 0.75 µs | 0.88 µs | 0.96 µs | Baseline |
| **Terminus App (Core Router)** | **1,025,256 req/s** | **0.75 µs** | **0.88 µs** | **0.92 µs** | **0.00 µs (0%)** |
| **Terminus App (+ CORS Layer)** | 497,362 req/s | 1.79 µs | 1.92 µs | 2.00 µs | +1.04 µs (CORS headers) |
| **Plain Axum (POST JSON)** | 631,719 req/s | 1.29 µs | 1.42 µs | 1.50 µs | Baseline |
| **Terminus App (POST JSON)** | 382,570 req/s | 2.29 µs | 2.46 µs | 2.58 µs | +1.00 µs (CORS headers) |
| **Terminus Fluent Validator** | **13,159,554 ops/s** | **42 ns** | **42 ns** | **42 ns** | Zero string parsing |
| **SQLite In-Memory Query** | 457,049 ops/s | 2.08 µs | 2.21 µs | 2.88 µs | Direct C-binding |

Key takeaway: Terminus is built directly on Axum. When using Terminus Core, your HTTP routing latency is identical to plain Axum (0.75 µs p50). You get batteries-included features (Admin panel, machine introspection, request validation, RBAC) with zero performance penalty.

---

## Verification & Tests

Terminus maintains a comprehensive test suite across feature tests, audit logging, and RBAC:

```bash
cargo test
```

All 68 unit and integration tests run in under 1 second with zero external service dependencies.

---

## License

MIT License © 2026 Pratik.
