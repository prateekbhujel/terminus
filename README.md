# Terminus

**The agent-native, batteries-included application framework for Axum and Rust.**

Web Admin • Embedded SQLite Persistence • HMAC Sessions • Compile-Time Validation • Deterministic Machine Introspection

[![CI](https://github.com/prateekbhujel/terminus/actions/workflows/rust.yml/badge.svg)](https://github.com/prateekbhujel/terminus/actions/workflows/rust.yml)
[![Version: 0.1.0](https://img.shields.io/badge/crates.io-v0.1.0-emerald.svg)](https://crates.io/crates/terminus-framework)
[![Docs](https://img.shields.io/badge/docs.rs-terminus--framework-blue.svg)](https://docs.rs/terminus-framework)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## Why Terminus?

[Axum](https://github.com/tokio-rs/axum) is a world-class, high-performance web router. But it is intentionally minimal. For production web services, developers repeatedly re-implement the same foundational layers from scratch:

- An administrative dashboard and CRUD interfaces
- SQLite database connection management and batch migrations
- Request validation with structured JSON error responses
- Signed session cookies, CSRF protection, and flash message bags
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

Scaffold a production-ready application from anywhere:

```bash
# Interactive project wizard
terminus new my_app

# Or scaffold non-interactively
terminus new my_app --preset=fullstack --port=3000

cd my_app
cargo run
```

Open `http://localhost:3000` in your browser. Browse to `http://localhost:3000/admin` to inspect the admin dashboard backed by embedded SQLite.

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
serde = { version = "1.0", features = ["derive"] }
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
        .database(db)
        .panel(admin)
        .run()
        .await
}
```

---

## Killer Feature: Deterministic Machine Introspection

When developers pair program with autonomous AI coding agents, agents frequently hallucinate route names, database columns, and model relationships because they must guess across dozens of files.

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

## Compile-Time Type-Safe Validation (`Valid<T>`)

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

impl Validate for CreateUserRequest {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if self.name.trim().is_empty() {
            errors.add("name", "Name is required");
        }
        if !self.email.contains('@') {
            errors.add("email", "Valid email address required");
        }
        if let Some(age) = self.age {
            if age < 18 {
                errors.add("age", "Must be at least 18 years old");
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### The `Valid<T>` Extractor

Extract, deserialize, and validate request payloads in a single handler argument. If validation fails, Terminus halts request execution and returns an HTTP 422 response with structured JSON:

```rust
pub async fn store(Valid(payload): Valid<CreateUserRequest>) -> Result<Redirect> {
    // Execution only reaches here if payload passed all validation checks!
    Ok(redirect("/users").with_success("User created successfully!"))
}
```

HTTP 422 JSON error response:

```json
{
  "message": "Validation failed",
  "errors": {
    "email": ["Valid email address required"]
  }
}
```

---

## Embedded SQLite Persistence & Batch Migrations

Terminus includes a zero-configuration SQLite persistence layer backed by `rusqlite`:

```rust
use terminus::prelude::*;

let db = Database::open("app.db")?;

// 1. Raw DDL execution
db.execute("CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, title TEXT)")?;

// 2. Ergonomic parameterized query execution
let affected = db.with_conn(|conn| {
    conn.execute(
        "INSERT INTO items (title) VALUES (?1)",
        rusqlite::params!["Hardware Scanner Node"],
    )
})?;
```

### Batch Migrations & Rollback

Manage database evolution with deterministic SQL migrations:

```bash
terminus make:migration create_orders_table
```

Creates timestamped migrations under `migrations/`:

```sql
-- migrations/20261018090000_create_orders_table.sql
CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY,
    customer_name TEXT NOT NULL,
    total_cents INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Manage migrations via CLI:

```bash
terminus migrate           # Run all pending migrations in a batch
terminus migrate:status    # Inspect execution batch and timestamp
terminus migrate:rollback  # Roll back the most recent migration batch
```

---

## Cryptographic HMAC-SHA256 Signed Sessions

Terminus features pure-Rust RFC 2104 HMAC-SHA256 session management with zero external C dependencies:

```rust
use terminus::prelude::*;

let secret = b"production_secret_key_at_least_32_bytes";
let session = Session::new();
session.set("user_id", "usr_99182");

// 1. Generate tamper-proof signed cookie header (id.signature)
let cookie_header = session.signed_cookie_header(secret);

// 2. Constant-time signature verification
let verified_id = Session::verify_signed_cookie(cookie_val, secret);
assert_eq!(verified_id, Some(session.id().to_string()));
```

- **Constant-Time Verification**: Prevents timing attacks when verifying session signatures.
- **CSRF Protection**: Automatic per-session tokens (`session.csrf_token()`) and verification (`session.verify_csrf_token(&token)`).
- **One-Time Flash Messages**: Alerts persist across HTTP redirects and clear automatically on subsequent requests.

---

## Server-Rendered Admin Panel

Define rich admin interfaces using declarative Rust:

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
        (vec![], 0)
    }
}
```

---

## Headless Integration Testing (`TestClient`)

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
        .post("/api/items", |Valid(item): Valid<CreateItemRequest>| async move {
            json_response(serde_json::json!({ "created": true }))
        });

    let client = TestClient::new(app);

    // 1. GET assertion
    let res = client.get("/api/health").await;
    res.assert_ok().assert_contains("ok");

    // 2. Validation failure assertion (HTTP 422)
    let invalid = client.post("/api/items", serde_json::json!({})).await;
    invalid.assert_unprocessable();
}
```

---

## OpenAPI 3.0.3 & TypeScript Client Contracts

Rather than maintaining brittle bespoke SDK generators, Terminus generates standard OpenAPI 3.0.3 specifications and typed TypeScript contracts:

```bash
# Export standard OpenAPI 3.0.3 specification
terminus make:sdk --lang=openapi

# Export typed TypeScript client
terminus make:sdk --lang=ts
```

The resulting `openapi.json` can be plugged directly into any industry-standard code generator (`openapi-generator-cli`, `@hey-api/openapi-ts`, or native language generators).

---

## Empirical Benchmarks: Terminus vs. Plain Axum

Run the benchmark suite locally:

```bash
cargo run --example benchmark --release
```

Measured in-memory across 50,000 iterations per scenario on Apple Silicon:

| Scenario | Throughput | p50 Latency | p95 Latency | p99 Latency | Framework Overhead |
|---|---|---|---|---|---|
| **Plain Axum 0.7 (GET)** | 205,227 req/s | 4.25 µs | 5.58 µs | 6.79 µs | Baseline |
| **Terminus App (Core Router)** | **206,817 req/s** | **4.38 µs** | **4.54 µs** | **5.62 µs** | **-0.03 µs (<2%)** |
| **Terminus App (+ CORS Layer)** | 104,214 req/s | 8.92 µs | 9.54 µs | 12.50 µs | +4.54 µs (CORS headers) |
| **Terminus Fluent Validator** | **7,037,504 ops/s** | **83 ns** | **120 ns** | **210 ns** | Compile-time checking |
| **Raw Rusqlite SELECT** | 179,421 ops/s | 4.96 µs | 5.25 µs | 20.38 µs | Embedded persistence |

Key takeaway: Terminus is built directly on Axum. When using Terminus Core, HTTP routing latency is virtually identical to plain Axum. You get batteries-included features (Admin panel, machine introspection, request validation, RBAC) with zero performance penalty.

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
   [1] Overview  [2] Database  [3] Cache  [4] Queue  [5] API Tokens
--------------------------------------------------------------------------------
[TELEMETRY] SYSTEM TELEMETRY & SERVICE HEALTH:

  ┌──────────────────────┬──────────────────────┬──────────────────────┐
  │ DATABASE (SQLite)    │ CACHE & STORE        │ QUEUE & JOBS         │
  │ Active Connections   │ In-Memory Store      │ 0 Pending Jobs       │
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
| `tinker [--execute=...]` | Interactive database REPL console with query evaluation |
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
| `make:sdk [--lang=ts\|openapi]` | Export typed TypeScript client or OpenAPI 3.0.3 specification |
| `make:token [name]` | Issue a personal access token |
| `token:list` | List active access tokens |
| `token:revoke [id]` | Revoke an access token |
| `cache:clear` | Clear memory cache store |

---

## Verification & Tests

Terminus maintains a comprehensive test suite across feature tests, audit logging, and RBAC:

```bash
cargo test
```

All 70 unit and integration tests run in under 1 second with zero external service dependencies.

---

## License

MIT License © 2026 Pratik.
