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

#[test]
fn cli_schema_respects_database_url_for_postgres_and_mysql_when_configured() {
    if let Ok(pg_url) = std::env::var("TERMINUS_TEST_POSTGRES_URL") {
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

    if let Ok(mysql_url) = std::env::var("TERMINUS_TEST_MYSQL_URL") {
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
