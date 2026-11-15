use axum::{routing::get, Router};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use terminus::prelude::*;

struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "terminus_schema_contract_{}_{unique}_{seq}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn run_cli(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_terminus"));
        cmd.args(args)
            .current_dir(&self.0)
            .env_remove("DATABASE_URL")
            .env_remove("TERMINUS_APP_SPEC")
            .env_remove("TERMINUS_EXPORT_SPEC");
        for (k, v) in envs {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct OrdersModule;

impl AppModule for OrdersModule {
    fn name(&self) -> &str {
        "Orders"
    }

    fn slug(&self) -> &str {
        "orders"
    }

    fn routes(&self) -> Router {
        Router::new()
            .route("/", get(|| async { "orders index" }))
            .route("/:id/receipt", get(|| async { "receipt" }))
    }

    fn route_specs(&self) -> Vec<RouteSpec> {
        vec![
            RouteSpec::new("GET", "/", "orders::index").public(),
            RouteSpec::new("GET", "/:id/receipt", "orders::receipt").requires_auth(),
        ]
    }
}

struct OpaqueLegacyModule;

impl AppModule for OpaqueLegacyModule {
    fn name(&self) -> &str {
        "Legacy"
    }

    fn slug(&self) -> &str {
        "legacy"
    }

    fn routes(&self) -> Router {
        Router::new().route("/ping", get(|| async { "pong" }))
    }
}

async fn list_items() -> &'static str {
    "list"
}

async fn create_item() -> &'static str {
    "created"
}

async fn replace_item() -> &'static str {
    "replaced"
}

async fn patch_item() -> &'static str {
    "patched"
}

async fn delete_item() -> &'static str {
    "deleted"
}

fn find_route<'a>(routes: &'a [Value], method: &str, path: &str) -> Option<&'a Value> {
    routes
        .iter()
        .find(|r| r["method"] == method && r["path"] == path)
}

#[tokio::test]
async fn exported_spec_captures_nested_groups_methods_modules_auth_and_opaque_mounts() {
    let app = App::new()
        .name("Contract App")
        .port(4100)
        .model(
            ModelSpec::new("Widget", "widgets")
                .add_field(FieldSpec::new("id", "i64", "INTEGER").primary())
                .add_field(FieldSpec::new("sku", "String", "TEXT")),
        )
        .group("/api", |api| {
            api.group("/v1", |v1| {
                v1.get("/items", list_items)
                    .post("/items", create_item)
                    .put("/items/:id", replace_item)
                    .patch("/items/:id", patch_item)
                    .delete("/items/:id", delete_item)
            })
        })
        .module(OrdersModule)
        .module(OpaqueLegacyModule)
        .api(
            "/raw",
            Router::new().route("/hidden", get(|| async { "hidden" })),
        )
        .spec_endpoint("/_spec");

    let exported = app.export_spec();
    let exported_json = serde_json::to_value(&exported).unwrap();

    assert_eq!(exported_json["contract_version"], SCHEMA_CONTRACT_VERSION);
    assert_eq!(exported_json["origin"]["kind"], "application_export");
    assert_eq!(exported_json["origin"]["authoritative_app_contract"], true);

    let routes = exported_json["routes"].as_array().unwrap();
    for (method, path) in [
        ("GET", "/api/v1/items"),
        ("POST", "/api/v1/items"),
        ("PUT", "/api/v1/items/:id"),
        ("PATCH", "/api/v1/items/:id"),
        ("DELETE", "/api/v1/items/:id"),
    ] {
        let route = find_route(routes, method, path)
            .unwrap_or_else(|| panic!("Missing nested route {method} {path} in {routes:?}"));
        assert_eq!(route["auth"], "unknown");
    }

    let orders_index = find_route(routes, "GET", "/orders").unwrap();
    assert_eq!(orders_index["auth"], "public");
    assert_eq!(orders_index["handler_name"], "orders::index");

    let orders_receipt = find_route(routes, "GET", "/orders/:id/receipt").unwrap();
    assert_eq!(orders_receipt["auth"], "required");
    assert_eq!(orders_receipt["handler_name"], "orders::receipt");

    let spec_route = find_route(routes, "GET", "/_spec").unwrap();
    assert_eq!(spec_route["auth"], "public");

    let unavailable = exported_json["unavailable"].as_array().unwrap();
    assert!(unavailable
        .iter()
        .any(|u| u["category"] == "module_routes" && u["target"] == "/legacy"));
    assert!(unavailable
        .iter()
        .any(|u| u["category"] == "router_mount" && u["target"] == "/raw"));

    let dir = TempWorkspace::new();
    let manifest_path = dir.path().join(".terminus/app-spec.json");
    app.write_spec(&manifest_path).unwrap();

    let client = TestClient::new(app);
    for (method, path) in [
        ("GET", "/api/v1/items"),
        ("POST", "/api/v1/items"),
        ("PUT", "/api/v1/items/7"),
        ("PATCH", "/api/v1/items/7"),
        ("DELETE", "/api/v1/items/7"),
        ("GET", "/orders"),
        ("GET", "/orders/9/receipt"),
        ("GET", "/raw/hidden"),
    ] {
        let res = match method {
            "GET" => client.get(path).await,
            "POST" => client.post(path, serde_json::json!({})).await,
            "PUT" => client.put(path, serde_json::json!({})).await,
            "PATCH" => client.patch(path, serde_json::json!({})).await,
            "DELETE" => client.delete(path).await,
            _ => unreachable!(),
        };
        res.assert_ok();
    }

    let live_spec = client.get("/_spec").await;
    live_spec.assert_ok();
    let live_json = live_spec.json();
    assert!(find_route(
        live_json["routes"].as_array().unwrap(),
        "DELETE",
        "/api/v1/items/:id"
    )
    .is_some());

    let cli_out = dir.run_cli(&["schema", "--json"], &[]);
    assert!(
        cli_out.status.success(),
        "cli stderr: {}",
        String::from_utf8_lossy(&cli_out.stderr)
    );
    let cli_json: Value = serde_json::from_slice(&cli_out.stdout).unwrap();
    assert_eq!(cli_json["contract_version"], SCHEMA_CONTRACT_VERSION);
    assert_eq!(cli_json["origin"]["kind"], "cli_inspection");
    assert_eq!(cli_json["origin"]["authoritative_app_contract"], true);
    assert!(find_route(
        cli_json["routes"].as_array().unwrap(),
        "POST",
        "/api/v1/items"
    )
    .is_some());
}

#[test]
fn cli_schema_does_not_regex_parse_source_files_or_invent_rust_types_for_database_tables() {
    let dir = TempWorkspace::new();
    fs::create_dir_all(dir.path().join("src/modules/ghost_module")).unwrap();
    fs::write(
        dir.path().join("src/main.rs"),
        r#"fn main() { let _ = app.get("/hallucinated", h).post("/fake", h); }"#,
    )
    .unwrap();

    let db_path = dir.path().join("custom.sqlite");
    let db = Database::open(&db_path).unwrap();
    db.execute_params(
        "CREATE TABLE inventory (id INTEGER PRIMARY KEY, qty INTEGER NOT NULL, note TEXT)",
        &[],
    )
    .unwrap();
    drop(db);

    let db_url = format!("sqlite://{}", db_path.display());
    let out = dir.run_cli(&["schema", "--json"], &[("DATABASE_URL", &db_url)]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["contract_version"], SCHEMA_CONTRACT_VERSION);
    assert_eq!(json["origin"]["kind"], "cli_inspection");
    assert_eq!(json["origin"]["authoritative_app_contract"], false);
    assert_eq!(json["origin"]["database_backend"], "sqlite");
    assert_eq!(json["origin"]["database_source"], "DATABASE_URL");

    assert!(json["routes"].as_array().unwrap().is_empty());
    assert!(json["resources"].as_array().unwrap().is_empty());
    assert!(json["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u["category"] == "app_contract"));

    let models = json["models"].as_array().unwrap();
    let inventory = models
        .iter()
        .find(|m| m["table"] == "inventory")
        .expect("inventory table observed");
    assert_eq!(inventory["source"], "database_schema");
    for field in inventory["fields"].as_array().unwrap() {
        assert!(
            field["rust_type"].is_null(),
            "DB inspection must never fabricate Rust field types: {field:?}"
        );
    }
    let qty = inventory["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "qty")
        .unwrap();
    assert_eq!(qty["sql_type"], "INTEGER");
    assert_eq!(qty["nullable"], false);
    let note = inventory["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "note")
        .unwrap();
    assert_eq!(note["nullable"], true);
}

#[test]
fn cli_schema_missing_explicit_spec_fails_nonzero() {
    let dir = TempWorkspace::new();
    let out = dir.run_cli(&["schema", "--json", "--spec=missing.json"], &[]);
    assert!(!out.status.success());
}

#[test]
fn runnable_quickstart_exports_spec_manifest_for_terminus_schema() {
    let dir = TempWorkspace::new();
    let manifest_rel = ".terminus/app-spec.json";
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let export_out = Command::new(env!("CARGO"))
        .args([
            "run",
            "--offline",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--example",
            "quickstart",
        ])
        .current_dir(dir.path())
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("target/scaffold-checks"),
        )
        .env("TERMINUS_EXPORT_SPEC", manifest_rel)
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(
        export_out.status.success(),
        "quickstart export failed: {}",
        String::from_utf8_lossy(&export_out.stderr)
    );

    let cli_out = dir.run_cli(&["schema", "--json"], &[]);
    assert!(
        cli_out.status.success(),
        "terminus schema failed: {}",
        String::from_utf8_lossy(&cli_out.stderr)
    );
    let json: Value = serde_json::from_slice(&cli_out.stdout).unwrap();
    assert_eq!(json["contract_version"], SCHEMA_CONTRACT_VERSION);
    assert_eq!(json["app_name"], "Terminus Quickstart");
    assert_eq!(json["origin"]["authoritative_app_contract"], true);

    let routes = json["routes"].as_array().unwrap();
    let login = find_route(routes, "GET", "/admin/login").unwrap();
    assert_eq!(login["auth"], "public");
    let dashboard = find_route(routes, "GET", "/admin/dashboard").unwrap();
    assert_eq!(dashboard["auth"], "required");

    let resources = json["resources"].as_array().unwrap();
    let user_res = resources
        .iter()
        .find(|r| r["slug"] == "users")
        .expect("UserResource from AdminPanel should appear in exported resources");
    assert_eq!(
        user_res["searchable_fields"],
        serde_json::json!(["name", "email"])
    );
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "redis"))]
fn external_test_url(var: &str, service: &str) -> Option<String> {
    if let Ok(url) = std::env::var(var) {
        let trimmed = url.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    let required = std::env::var("TERMINUS_REQUIRE_EXTERNAL_TESTS")
        .ok()
        .map(|raw| {
            let val = raw.trim().to_ascii_lowercase();
            match val.as_str() {
                "1" | "true" | "sql" => matches!(service, "postgres" | "mysql" | "redis"),
                "all" => true,
                _ => val.split(',').any(|item| item.trim() == service),
            }
        })
        .unwrap_or(false);
    if required {
        panic!(
            "{var} must be set when TERMINUS_REQUIRE_EXTERNAL_TESTS requires '{service}' (external-service tests must not silently skip)"
        );
    }
    eprintln!("skipping: {var} not set");
    None
}

#[test]
fn cli_schema_respects_database_url_for_postgres_and_mysql_when_configured() {
    #[cfg(feature = "postgres")]
    if let Some(pg_url) = external_test_url("TERMINUS_TEST_POSTGRES_URL", "postgres") {
        let dir = TempWorkspace::new();
        let db = Database::connect(&pg_url).unwrap();
        let table = format!("schema_probe_{}", std::process::id());
        let drop_sql = format!("DROP TABLE IF EXISTS {table}");
        let _ = db.execute_params(&drop_sql, &[]);
        db.execute_params(
            &format!(
                "CREATE TABLE {table} (id INTEGER PRIMARY KEY, flag BOOLEAN NOT NULL, label TEXT)"
            ),
            &[],
        )
        .unwrap();

        let out = dir.run_cli(&["schema", "--json"], &[("DATABASE_URL", &pg_url)]);
        let _ = db.execute_params(&drop_sql, &[]);
        assert!(out.status.success());

        let json: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(json["origin"]["database_backend"], "postgres");
        assert_eq!(json["origin"]["database_source"], "DATABASE_URL");
        let tbl = json["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["table"] == table)
            .expect("Postgres table observed");
        for field in tbl["fields"].as_array().unwrap() {
            assert!(field["rust_type"].is_null());
        }
    }

    #[cfg(feature = "mysql")]
    if let Some(mysql_url) = external_test_url("TERMINUS_TEST_MYSQL_URL", "mysql") {
        let dir = TempWorkspace::new();
        let db = Database::connect(&mysql_url).unwrap();
        let table = format!("schema_probe_{}", std::process::id());
        let drop_sql = format!("DROP TABLE IF EXISTS {table}");
        let _ = db.execute_params(&drop_sql, &[]);
        db.execute_params(
            &format!("CREATE TABLE {table} (id INT PRIMARY KEY, active TINYINT(1) NOT NULL, title VARCHAR(120))"),
            &[],
        )
        .unwrap();

        let out = dir.run_cli(&["schema", "--json"], &[("DATABASE_URL", &mysql_url)]);
        let _ = db.execute_params(&drop_sql, &[]);
        assert!(out.status.success());

        let json: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(json["origin"]["database_backend"], "mysql");
        assert_eq!(json["origin"]["database_source"], "DATABASE_URL");
        let tbl = json["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["table"] == table)
            .expect("MySQL table observed");
        for field in tbl["fields"].as_array().unwrap() {
            assert!(field["rust_type"].is_null());
        }
    }
}

#[test]
fn cli_cache_clear_flushes_persistent_sqlite_and_redis_stores() {
    let dir = TempWorkspace::new();
    let custom_db_path = dir.path().join("custom_app.db");

    // 1. Seed persistent SQLite cache table
    {
        let cache = terminus::cache::SqliteCache::new(custom_db_path.to_str().unwrap()).unwrap();
        cache.put("user_session_42", "{\"role\":\"admin\"}", Some(3600));
        assert_eq!(
            cache.get("user_session_42"),
            Some("{\"role\":\"admin\"}".to_string())
        );
    }

    // 2. Invoke `terminus cache:clear` with explicit DATABASE_URL pointing to custom_app.db
    let db_url = format!("sqlite://{}", custom_db_path.display());
    let out = dir.run_cli(&["cache:clear"], &[("DATABASE_URL", &db_url)]);
    assert!(out.status.success(), "cache:clear must exit successfully");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("[OK] Application Cache cleared successfully!"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("SQLite Cache (_terminus_cache in"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("custom_app.db"),
        "stdout should report the configured SQLite database file: {stdout}"
    );
    assert!(
        !stdout.contains("MemoryCache"),
        "stdout must not report an ephemeral in-process MemoryCache"
    );
    assert!(
        !stdout.contains("Redis"),
        "stdout must not report Redis when REDIS_URL is unset"
    );

    // 3. Verify SQLite _terminus_cache is now completely empty
    {
        let conn = rusqlite::Connection::open(&custom_db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _terminus_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "All persistent cache entries should be deleted");
    }

    // 4. Redis test when configured via external environment variable
    #[cfg(feature = "redis")]
    if let Some(redis_url) = external_test_url("TERMINUS_TEST_REDIS_URL", "redis") {
        if let Ok(redis_cache) = terminus::cache::RedisCache::new(&redis_url) {
            redis_cache.put("test_redis_probe_key", "active", Some(60));
            assert_eq!(
                redis_cache.get("test_redis_probe_key"),
                Some("active".to_string())
            );

            let out_redis = dir.run_cli(
                &["cache:clear"],
                &[("REDIS_URL", &redis_url), ("DATABASE_URL", &db_url)],
            );
            assert!(out_redis.status.success());
            let redis_stdout = String::from_utf8_lossy(&out_redis.stdout);
            assert!(
                redis_stdout.contains("RedisCache (redis://"),
                "stdout was: {redis_stdout}"
            );
            assert_eq!(redis_cache.get("test_redis_probe_key"), None);
        }
    }
}

#[test]
fn cli_route_list_and_make_sdk_consume_exported_app_spec() {
    let dir = TempWorkspace::new();
    let spec_path = dir.path().join("exported-spec.json");

    // 1. Without manifest, route:list and make:sdk must fail nonzero
    let out_routes_missing = dir.run_cli(&["route:list"], &[]);
    assert!(
        !out_routes_missing.status.success(),
        "route:list must fail when no manifest exists"
    );
    let err_routes = String::from_utf8_lossy(&out_routes_missing.stderr);
    assert!(
        err_routes.contains("No application-exported AppSpec manifest found"),
        "stderr was: {err_routes}"
    );

    let out_sdk_missing = dir.run_cli(&["make:sdk"], &[]);
    assert!(
        !out_sdk_missing.status.success(),
        "make:sdk must fail when no manifest exists"
    );
    let err_sdk = String::from_utf8_lossy(&out_sdk_missing.stderr);
    assert!(
        err_sdk.contains("No application-exported AppSpec manifest found"),
        "stderr was: {err_sdk}"
    );

    // 2. make:sdk rejects unknown languages with exit code 1
    let out_sdk_bad_lang = dir.run_cli(&["make:sdk", "--lang=cobol"], &[]);
    assert!(
        !out_sdk_bad_lang.status.success(),
        "make:sdk must fail on unsupported language"
    );
    let err_lang = String::from_utf8_lossy(&out_sdk_bad_lang.stderr);
    assert!(
        err_lang.contains("Unsupported language 'cobol'"),
        "stderr was: {err_lang}"
    );

    // 3. Create a valid AppSpec manifest
    let mut spec = terminus::spec::AppSpec::new("Inventory App");
    spec.add_route(
        terminus::spec::RouteSpec::new("GET", "/api/v2/items", "list_items")
            .with_auth(true)
            .with_description("List warehouse items"),
    );
    spec.add_route(
        terminus::spec::RouteSpec::new("POST", "/api/v2/items", "create_item")
            .with_auth(true)
            .with_description("Create warehouse item"),
    );
    let mut model = terminus::spec::ModelSpec::new("Item", "items");
    model = model.add_field(terminus::spec::FieldSpec::new("id", "i64", "BIGINT").primary());
    model = model.add_field(terminus::spec::FieldSpec::new(
        "sku",
        "String",
        "VARCHAR(64)",
    ));
    spec.add_model(model);
    spec.add_resource(terminus::spec::ResourceSpec::new("Item", "items", "items"));
    spec.write_to_path(&spec_path).unwrap();

    let spec_flag = format!("--spec={}", spec_path.display());

    // 4. route:list with --spec
    let out_routes = dir.run_cli(&["route:list", &spec_flag], &[]);
    assert!(
        out_routes.status.success(),
        "route:list with --spec must succeed"
    );
    let stdout_routes = String::from_utf8_lossy(&out_routes.stdout);
    assert!(
        stdout_routes.contains("Inventory App"),
        "stdout was: {stdout_routes}"
    );
    assert!(
        stdout_routes.contains("/api/v2/items"),
        "stdout was: {stdout_routes}"
    );
    assert!(
        stdout_routes.contains("List warehouse items"),
        "stdout was: {stdout_routes}"
    );
    assert!(
        stdout_routes.contains("Resource: Item (/admin/items)"),
        "stdout was: {stdout_routes}"
    );

    // 5. make:sdk --lang=typescript with --spec
    let out_sdk_ts = dir.run_cli(&["make:sdk", "--lang=typescript", &spec_flag], &[]);
    assert!(
        out_sdk_ts.status.success(),
        "make:sdk typescript must succeed"
    );
    let ts_file = dir.path().join("terminus-client.ts");
    assert!(ts_file.exists(), "terminus-client.ts should be generated");
    let ts_code = fs::read_to_string(&ts_file).unwrap();
    assert!(
        ts_code.contains("export interface Item"),
        "ts code was: {ts_code}"
    );
    assert!(ts_code.contains("sku: string;"), "ts code was: {ts_code}");
    assert!(ts_code.contains("/api/v2/items"), "ts code was: {ts_code}");

    // 6. make:sdk --lang=openapi with --spec
    let out_sdk_openapi = dir.run_cli(&["make:sdk", "--lang=openapi", &spec_flag], &[]);
    assert!(
        out_sdk_openapi.status.success(),
        "make:sdk openapi must succeed"
    );
    let openapi_file = dir.path().join("openapi.json");
    assert!(openapi_file.exists(), "openapi.json should be generated");
    let openapi_code = fs::read_to_string(&openapi_file).unwrap();
    let openapi_json: Value = serde_json::from_str(&openapi_code).unwrap();
    assert_eq!(openapi_json["openapi"], "3.0.3");
    assert_eq!(openapi_json["info"]["title"], "Inventory App");
    assert!(openapi_json["paths"]["/api/v2/items"].is_object());
}
