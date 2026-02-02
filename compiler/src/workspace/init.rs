//! Project and workspace scaffolding.

use std::fs;
use std::path::Path;

/// Initialize a new Rayzor project.
///
/// Creates:
/// - `<dir>/rayzor.toml`
/// - `<dir>/src/Main.hx`
/// - `<dir>/.rayzor/cache/`
pub fn init_project(name: &str, dir: &Path) -> Result<(), String> {
    // Create directories
    fs::create_dir_all(dir.join("src")).map_err(|e| format!("Failed to create src/: {}", e))?;
    fs::create_dir_all(dir.join(".rayzor").join("cache"))
        .map_err(|e| format!("Failed to create .rayzor/cache/: {}", e))?;

    // Write rayzor.toml
    let manifest = format!(
        r#"[project]
name = "{name}"
version = "0.1.0"
entry = "src/Main.hx"

[build]
class-paths = ["src"]
opt-level = 2
preset = "application"
output = "build/{name}"

[cache]
enabled = true
"#,
    );

    fs::write(dir.join("rayzor.toml"), manifest)
        .map_err(|e| format!("Failed to write rayzor.toml: {}", e))?;

    // Write Main.hx
    let main_hx = r#"class Main {
    static function main() {
        trace("Hello from Rayzor!");
    }
}
"#;

    fs::write(dir.join("src").join("Main.hx"), main_hx)
        .map_err(|e| format!("Failed to write Main.hx: {}", e))?;

    // Write .gitignore for build artifacts
    let gitignore = "build/\n.rayzor/cache/\n";
    fs::write(dir.join(".gitignore"), gitignore)
        .map_err(|e| format!("Failed to write .gitignore: {}", e))?;

    Ok(())
}

/// Initialize a new Rayzor workspace.
///
/// Creates:
/// - `<dir>/rayzor.toml` with `[workspace]`
/// - `<dir>/.rayzor/cache/`
pub fn init_workspace(name: &str, dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir.join(".rayzor").join("cache"))
        .map_err(|e| format!("Failed to create .rayzor/cache/: {}", e))?;

    let manifest = format!(
        r#"[workspace]
members = []

[workspace.cache]
dir = ".rayzor/cache"
"#,
    );

    fs::write(dir.join("rayzor.toml"), manifest)
        .map_err(|e| format!("Failed to write rayzor.toml: {}", e))?;

    let gitignore = ".rayzor/cache/\n";
    fs::write(dir.join(".gitignore"), gitignore)
        .map_err(|e| format!("Failed to write .gitignore: {}", e))?;

    Ok(())
}
