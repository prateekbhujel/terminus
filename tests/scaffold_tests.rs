use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use terminus::scaffold::{
    generate_cargo_toml, scaffold_project, FrontendChoice, ProjectConfig, ProjectPreset,
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
    assert!(manifest.contains(&format!(
        "terminus-framework = \"{}\"",
        env!("CARGO_PKG_VERSION")
    )));
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
    fs::write(
        project.0.join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [{}]\nresolver = \"2\"\n\n[patch.crates-io]\nterminus-framework = {{ path = {} }}\n",
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
