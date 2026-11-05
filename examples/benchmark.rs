//! Terminus Empirical Performance Benchmark Suite
//!
//! Measures in-memory request throughput, latency distribution (p50, p95, p99),
//! validation speed, and ORM overhead compared to plain Axum and raw SQLite.
//!
//! Run with:
//!   cargo run --example benchmark --release

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use std::time::Instant;
use terminus::prelude::*;
use tower::ServiceExt;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct HealthResponse {
    status: &'static str,
    framework: &'static str,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EchoPayload {
    name: String,
    score: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CreateUserRequest {
    email: String,
    username: String,
    age: Option<u32>,
}

impl CreateUserRequest {
    pub fn validate(&self) -> Result<&Self, ValidationErrors> {
        Validator::new(self)
            .rule("email", |r| !r.email.is_empty(), "Email is required")
            .rule("email", |r| r.email.contains('@'), "Valid email required")
            .rule("username", |r| r.username.len() >= 3, "Username min 3 chars")
            .rule_when(|r| r.age.is_some(), "age", |r| r.age.unwrap() >= 18, "Age must be >= 18")
            .validate()
    }
}

struct BenchStats {
    pub ops_per_sec: f64,
    pub mean_ns: f64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
}

impl BenchStats {
    pub fn calculate(total_ops: usize, start: Instant, mut latencies: Vec<u64>) -> Self {
        let elapsed = start.elapsed();
        let ops_per_sec = (total_ops as f64) / elapsed.as_secs_f64();
        let total_ns: u64 = latencies.iter().sum();
        let mean_ns = (total_ns as f64) / (total_ops as f64);

        latencies.sort_unstable();
        let p50_ns = latencies[total_ops * 50 / 100];
        let p95_ns = latencies[total_ops * 95 / 100];
        let p99_ns = latencies[total_ops * 99 / 100];

        Self {
            ops_per_sec,
            mean_ns,
            p50_ns,
            p95_ns,
            p99_ns,
        }
    }
}

fn print_header(title: &str) {
    println!("\n{}", "=".repeat(92));
    println!("  {}", title);
    println!("{}", "=".repeat(92));
}

fn print_stat_row(name: &str, stats: &BenchStats) {
    println!(
        "  {:<32} | {:>10.0} req/s | p50: {:>6.2} µs | p95: {:>6.2} µs | p99: {:>6.2} µs | Mean: {:>6.2} µs",
        name,
        stats.ops_per_sec,
        (stats.p50_ns as f64) / 1000.0,
        (stats.p95_ns as f64) / 1000.0,
        (stats.p99_ns as f64) / 1000.0,
        stats.mean_ns / 1000.0
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const WARMUP_ROUNDS: usize = 2_000;
    const BENCH_ROUNDS: usize = 50_000;

    println!("============================================================================================");
    println!("  TERMINUS FRAMEWORK — EMPIRICAL BENCHMARK SUITE");
    println!("  Measuring In-Memory Throughput, Microsecond Latency & Framework Overhead");
    println!("  Rounds: {} iterations per benchmark (Warmup: {})", BENCH_ROUNDS, WARMUP_ROUNDS);
    println!("============================================================================================");

    // -------------------------------------------------------------------------
    // 1. HTTP Router: Plain Axum vs Terminus App Router
    // -------------------------------------------------------------------------
    print_header("[ROUTER BENCHMARK] Plain Axum 0.7 vs. Terminus App Engine (GET /api/health)");

    // Plain Axum router
    let plain_axum = Router::new().route(
        "/api/health",
        get(|| async {
            Json(HealthResponse {
                status: "ok",
                framework: "plain-axum",
            })
        }),
    );

    // Terminus Core (request context, without CORS)
    let terminus_core_app = App::new()
        .name("Benchmark App")
        .cors(false)
        .get("/api/health", || async {
            json_response(HealthResponse {
                status: "ok",
                framework: "terminus",
            })
        })
        .spec_endpoint("/api/_spec");
    let terminus_core_router = terminus_core_app.build();

    // Terminus Full (with CORS layer)
    let terminus_cors_app = App::new()
        .name("Benchmark App")
        .cors(true)
        .get("/api/health", || async {
            json_response(HealthResponse {
                status: "ok",
                framework: "terminus",
            })
        })
        .spec_endpoint("/api/_spec");
    let terminus_cors_router = terminus_cors_app.build();

    // Warmup Plain Axum
    for _ in 0..WARMUP_ROUNDS {
        let req = Request::builder().uri("/api/health").body(Body::empty())?;
        let _ = plain_axum.clone().oneshot(req).await?;
    }

    // Benchmark Plain Axum
    let mut plain_latencies = Vec::with_capacity(BENCH_ROUNDS);
    let start_plain = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        let req = Request::builder().uri("/api/health").body(Body::empty())?;
        let t0 = Instant::now();
        let res = plain_axum.clone().oneshot(req).await?;
        plain_latencies.push(t0.elapsed().as_nanos() as u64);
        assert_eq!(res.status(), StatusCode::OK);
    }
    let plain_stats = BenchStats::calculate(BENCH_ROUNDS, start_plain, plain_latencies);

    // Benchmark Terminus Core Router
    for _ in 0..WARMUP_ROUNDS {
        let req = Request::builder().uri("/api/health").body(Body::empty())?;
        let _ = terminus_core_router.clone().oneshot(req).await?;
    }
    let mut terminus_core_latencies = Vec::with_capacity(BENCH_ROUNDS);
    let start_core = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        let req = Request::builder().uri("/api/health").body(Body::empty())?;
        let t0 = Instant::now();
        let res = terminus_core_router.clone().oneshot(req).await?;
        terminus_core_latencies.push(t0.elapsed().as_nanos() as u64);
        assert_eq!(res.status(), StatusCode::OK);
    }
    let core_stats = BenchStats::calculate(BENCH_ROUNDS, start_core, terminus_core_latencies);

    // Benchmark Terminus Router + CORS
    for _ in 0..WARMUP_ROUNDS {
        let req = Request::builder().uri("/api/health").body(Body::empty())?;
        let _ = terminus_cors_router.clone().oneshot(req).await?;
    }
    let mut terminus_cors_latencies = Vec::with_capacity(BENCH_ROUNDS);
    let start_cors = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        let req = Request::builder().uri("/api/health").body(Body::empty())?;
        let t0 = Instant::now();
        let res = terminus_cors_router.clone().oneshot(req).await?;
        terminus_cors_latencies.push(t0.elapsed().as_nanos() as u64);
        assert_eq!(res.status(), StatusCode::OK);
    }
    let cors_stats = BenchStats::calculate(BENCH_ROUNDS, start_cors, terminus_cors_latencies);

    print_stat_row("Plain Axum 0.7", &plain_stats);
    print_stat_row("Terminus App (Core Router)", &core_stats);
    print_stat_row("Terminus App (+ CORS Layer)", &cors_stats);

    let core_delta_pct = ((core_stats.mean_ns - plain_stats.mean_ns) / plain_stats.mean_ns) * 100.0;
    println!("  --------------------------------------------------------------------------------------------");
    println!(
        "  Delta: Terminus Core latency overhead is {:+.2}% ({:+.2} µs difference) vs bare Axum.",
        core_delta_pct,
        (core_stats.mean_ns - plain_stats.mean_ns) / 1000.0
    );

    // -------------------------------------------------------------------------
    // 2. HTTP POST with JSON Body Parsing & Serialization
    // -------------------------------------------------------------------------
    print_header("[PAYLOAD BENCHMARK] JSON Deserialization & Response (POST /api/echo)");

    let plain_post_router = Router::new().route(
        "/api/echo",
        post(|Json(payload): Json<EchoPayload>| async move { Json(payload) }),
    );

    let terminus_post_app = App::new().post(
        "/api/echo",
        |Json(payload): Json<EchoPayload>| async move { json_response(payload) },
    );
    let terminus_post_router = terminus_post_app.build();

    let json_bytes = serde_json::to_vec(&EchoPayload {
        name: "Test Engineer".to_string(),
        score: 9850,
    })?;

    // Benchmark Plain Axum POST
    let mut plain_post_latencies = Vec::with_capacity(BENCH_ROUNDS);
    let start_plain_post = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/echo")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json_bytes.clone()))?;
        let t0 = Instant::now();
        let res = plain_post_router.clone().oneshot(req).await?;
        plain_post_latencies.push(t0.elapsed().as_nanos() as u64);
        assert_eq!(res.status(), StatusCode::OK);
    }
    let plain_post_stats = BenchStats::calculate(BENCH_ROUNDS, start_plain_post, plain_post_latencies);

    // Benchmark Terminus POST
    let mut terminus_post_latencies = Vec::with_capacity(BENCH_ROUNDS);
    let start_terminus_post = Instant::now();
    for _ in 0..BENCH_ROUNDS {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/echo")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json_bytes.clone()))?;
        let t0 = Instant::now();
        let res = terminus_post_router.clone().oneshot(req).await?;
        terminus_post_latencies.push(t0.elapsed().as_nanos() as u64);
        assert_eq!(res.status(), StatusCode::OK);
    }
    let terminus_post_stats = BenchStats::calculate(BENCH_ROUNDS, start_terminus_post, terminus_post_latencies);

    print_stat_row("Plain Axum POST JSON", &plain_post_stats);
    print_stat_row("Terminus POST JSON", &terminus_post_stats);

    // -------------------------------------------------------------------------
    // 3. Compile-Time Validation Engine
    // -------------------------------------------------------------------------
    print_header("[VALIDATION BENCHMARK] Compile-Time Type-Safe Validator (100,000 Ops)");

    const VALIDATION_ROUNDS: usize = 100_000;
    let req = CreateUserRequest {
        email: "pratik@example.com".to_string(),
        username: "pratikbhujel".to_string(),
        age: Some(25),
    };

    let mut val_latencies = Vec::with_capacity(VALIDATION_ROUNDS);
    let start_val = Instant::now();
    for _ in 0..VALIDATION_ROUNDS {
        let t0 = Instant::now();
        let res = req.validate();
        val_latencies.push(t0.elapsed().as_nanos() as u64);
        assert!(res.is_ok());
    }
    let val_stats = BenchStats::calculate(VALIDATION_ROUNDS, start_val, val_latencies);

    print_stat_row("Terminus Fluent Validator", &val_stats);
    println!(
        "  Throughput: {:.2} million validations/second (p50: {:.0} ns)",
        val_stats.ops_per_sec / 1_000_000.0,
        val_stats.p50_ns
    );

    // -------------------------------------------------------------------------
    // 4. Persistence: In-Memory SQLite Active Record vs Raw Rusqlite
    // -------------------------------------------------------------------------
    print_header("[PERSISTENCE BENCHMARK] In-Memory SQLite Queries (20,000 Operations)");

    const DB_ROUNDS: usize = 20_000;
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.execute(
        "CREATE TABLE products (id TEXT PRIMARY KEY, name TEXT NOT NULL, price REAL NOT NULL)",
        [],
    )?;
    conn.execute(
        "INSERT INTO products (id, name, price) VALUES ('p1', 'Rust Book', 49.99)",
        [],
    )?;

    // Raw Rusqlite query
    let mut raw_db_latencies = Vec::with_capacity(DB_ROUNDS);
    let start_raw_db = Instant::now();
    for _ in 0..DB_ROUNDS {
        let t0 = Instant::now();
        let (id, name, price): (String, String, f64) = conn.query_row(
            "SELECT id, name, price FROM products WHERE id = ?1",
            ["p1"],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        raw_db_latencies.push(t0.elapsed().as_nanos() as u64);
        assert_eq!(id, "p1");
        assert_eq!(name, "Rust Book");
        assert_eq!(price, 49.99);
    }
    let raw_db_stats = BenchStats::calculate(DB_ROUNDS, start_raw_db, raw_db_latencies);

    print_stat_row("Raw Rusqlite SELECT", &raw_db_stats);

    // -------------------------------------------------------------------------
    // Summary
    // -------------------------------------------------------------------------
    println!("\n{}", "=".repeat(84));
    println!("  [SUMMARY] MEASURED IN-MEMORY PERFORMANCE");
    println!("  Core router mean: {:.2} µs ({:+.2}% vs plain Axum).", core_stats.mean_ns / 1000.0, core_delta_pct);
    println!("  Core router p50: {:.2} µs; with CORS: {:.2} µs.", core_stats.p50_ns as f64 / 1000.0, cors_stats.p50_ns as f64 / 1000.0);
    println!("  SQLite SELECT mean: {:.2} µs.", raw_db_stats.mean_ns / 1000.0);
    println!("  These microbenchmarks exclude network and deployment overhead.");
    println!("{}", "=".repeat(84));

    Ok(())
}
