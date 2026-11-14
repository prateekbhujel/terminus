use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use terminus::scaffold::{
    generate_cargo_toml, generate_env_example, scaffold_project, DatabaseChoice, FrontendChoice,
    ProjectConfig, ProjectPreset,
};

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "terminus_scaffold_{}_{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scaffold_uses_current_framework_version() {
    let manifest = generate_cargo_toml(&ProjectConfig::new("version_check"));
    assert!(manifest.contains(&format!("terminus-rs = \"{}\"", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn scaffolded_presets_compile() {
    let project = TempProject::new();
    let configurations = [
        (
            "fullstack_none",
            ProjectPreset::Fullstack,
            FrontendChoice::None,
        ),
        (
            "fullstack_htmx",
            ProjectPreset::Fullstack,
            FrontendChoice::Htmx,
        ),
        ("backend_none", ProjectPreset::Backend, FrontendChoice::None),
        ("backend_htmx", ProjectPreset::Backend, FrontendChoice::Htmx),
    ];
    let mut members = Vec::new();
    for (name, preset, frontend) in configurations {
        let config = ProjectConfig::new(name).preset(preset).frontend(frontend);
        scaffold_project(&project.0.join(name), &config).unwrap();
        members.push(format!("\"{name}\""));
    }

    // Compile the actual templates against this checkout, without depending on
    // a published release or allowing a registry feature to hide template errors.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let _ = fs::copy(
        manifest_dir.join("Cargo.lock"),
        project.0.join("Cargo.lock"),
    );
    fs::write(
        project.0.join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [{}]\nresolver = \"2\"\n\n[patch.crates-io]\nterminus-rs = {{ path = {} }}\n",
            members.join(", "),
            serde_json::to_string(&manifest_dir).unwrap()
        ),
    ).unwrap();

    // Keep a separate target directory so this nested Cargo invocation cannot
    // contend for the parent test runner's build lock, while reusing its cache.
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--workspace"])
        .current_dir(&project.0)
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("target/scaffold-checks"),
        )
        .output()
        .expect("Cargo must be available to compile scaffolded applications");
    assert!(
        output.status.success(),
        "Scaffolded applications failed to compile:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn scaffold_database_choice_parses_all_backends_and_writes_env() {
    assert_eq!(
        DatabaseChoice::from_str_loose("sqlite"),
        Some(DatabaseChoice::Sqlite)
    );
    assert_eq!(
        DatabaseChoice::from_str_loose("postgres"),
        Some(DatabaseChoice::Postgres)
    );
    assert_eq!(
        DatabaseChoice::from_str_loose("postgresql"),
        Some(DatabaseChoice::Postgres)
    );
    assert_eq!(
        DatabaseChoice::from_str_loose("pg"),
        Some(DatabaseChoice::Postgres)
    );
    assert_eq!(
        DatabaseChoice::from_str_loose("mysql"),
        Some(DatabaseChoice::Mysql)
    );
    assert_eq!(
        DatabaseChoice::from_str_loose("mariadb"),
        Some(DatabaseChoice::Mysql)
    );
    assert_eq!(DatabaseChoice::from_str_loose("unknown_db"), None);

    let pg_env = generate_env_example(
        &ProjectConfig::new("orders_service").database(DatabaseChoice::Postgres),
    );
    assert!(
        pg_env.contains("DATABASE_URL=postgres://terminus:change-me@localhost:5432/orders_service")
    );

    let mysql_env =
        generate_env_example(&ProjectConfig::new("orders_service").database(DatabaseChoice::Mysql));
    assert!(
        mysql_env.contains("DATABASE_URL=mysql://terminus:change-me@localhost:3306/orders_service")
    );
}

#[test]
fn documented_quickstart_compiles() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(manifest_dir.join("README.md")).expect("read README.md");

    let method3 = readme
        .split("### Method 3: Add to an Existing Cargo Project")
        .nth(1)
        .expect("Method 3 section must exist in README.md");

    let deps_block = method3
        .split("```toml\n")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .expect("README Method 3 must contain a ```toml dependencies block");
    assert!(
        deps_block.contains(&format!("terminus-rs = \"{}\"", env!("CARGO_PKG_VERSION"))),
        "README.md Cargo.toml snippet must match CARGO_PKG_VERSION ({})",
        env!("CARGO_PKG_VERSION")
    );

    let rust_block = method3
        .split("```rust\n")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .expect("README Method 3 must contain a ```rust src/main.rs block");

    let project = TempProject::new();
    let _ = fs::copy(
        manifest_dir.join("Cargo.lock"),
        project.0.join("Cargo.lock"),
    );
    let src_dir = project.0.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), rust_block).unwrap();
    fs::write(
        project.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"readme_quickstart_check\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n{deps_block}\n[patch.crates-io]\nterminus-rs = {{ path = {} }}\n",
            serde_json::to_string(&manifest_dir).unwrap()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline"])
        .current_dir(&project.0)
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("target/scaffold-checks"),
        )
        .output()
        .expect("Cargo must be available to compile README quickstart");
    assert!(
        output.status.success(),
        "Documented README quickstart failed to compile:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
