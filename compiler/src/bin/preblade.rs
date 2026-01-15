//! Pre-compile stdlib to BLADE format
//!
//! This tool pre-compiles Haxe standard library modules to BLADE bytecode format
//! for faster incremental compilation. Pre-compiled modules can be loaded directly
//! instead of re-parsing and re-compiling.
//!
//! Usage:
//!   cargo run --bin preblade -- --stdlib-path ./compiler/haxe-std --out .rayzor/blade/stdlib
//!   cargo run --bin preblade -- --stdlib-path ./compiler/haxe-std --out .rayzor/blade/stdlib --force
//!   cargo run --bin preblade -- --list  # List modules that would be compiled

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Information about a discovered module
struct ModuleInfo {
    name: String,
    source_path: PathBuf,
    relative_path: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let mut stdlib_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut force = false;
    let mut list_only = false;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--stdlib-path" => {
                i += 1;
                if i < args.len() {
                    stdlib_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--out" | "-o" => {
                i += 1;
                if i < args.len() {
                    out_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--force" | "-f" => force = true,
            "--list" | "-l" => list_only = true,
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_usage();
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // Default stdlib path
    let stdlib_path = stdlib_path.unwrap_or_else(|| {
        // Try common locations
        let candidates = [
            PathBuf::from("compiler/haxe-std"),
            PathBuf::from("./haxe-std"),
            PathBuf::from("../compiler/haxe-std"),
        ];
        for path in &candidates {
            if path.exists() {
                return path.clone();
            }
        }
        PathBuf::from("compiler/haxe-std")
    });

    if !stdlib_path.exists() {
        eprintln!("Error: stdlib path does not exist: {}", stdlib_path.display());
        eprintln!("Use --stdlib-path to specify the location of haxe-std");
        std::process::exit(1);
    }

    // Discover modules dynamically
    let modules = discover_modules_recursive(&stdlib_path);

    if list_only {
        println!("Modules to pre-compile from {}:", stdlib_path.display());
        for module in &modules {
            println!("  {} -> {}", module.relative_path, module.name);
        }
        println!("\nTotal: {} modules", modules.len());
        return;
    }

    let out_path = out_path.unwrap_or_else(|| PathBuf::from(".rayzor/blade/stdlib"));

    // Create output directory
    if let Err(e) = std::fs::create_dir_all(&out_path) {
        eprintln!("Error creating output directory: {}", e);
        std::process::exit(1);
    }

    println!("Pre-BLADE: Compiling stdlib to {}", out_path.display());
    println!("  Source: {}", stdlib_path.display());
    println!("  Modules: {} discovered", modules.len());
    println!("  Force: {}", force);
    println!();

    // Create shared compilation context
    let mut ctx = SharedCompilationContext::new();

    let mut compiled = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for module in &modules {
        let blade_path = out_path.join(format!("{}.blade", module.name));

        // Check if we can skip
        if !force && is_blade_current(&blade_path, &module.source_path) {
            if verbose {
                println!("  [skip] {}", module.name);
            }
            skipped += 1;
            continue;
        }

        print!("  [blade] {}...", module.name);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        match ctx.compile_module(&module.source_path, &blade_path, &module.name) {
            Ok(()) => {
                println!(" OK");
                compiled += 1;
            }
            Err(e) => {
                println!(" FAILED");
                if verbose {
                    eprintln!("    Error: {}", e);
                }
                failed += 1;
            }
        }
    }

    println!();
    println!("Pre-BLADE complete:");
    println!("  Compiled: {}", compiled);
    println!("  Skipped:  {}", skipped);
    println!("  Failed:   {}", failed);
    println!("  Total:    {}", modules.len());

    if failed > 0 {
        std::process::exit(1);
    }
}

fn print_usage() {
    println!("preblade - Pre-compile Haxe stdlib to BLADE format");
    println!();
    println!("Usage:");
    println!("  preblade [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --stdlib-path <PATH>  Path to haxe-std directory");
    println!("  --out, -o <PATH>      Output directory for .blade files");
    println!("  --force, -f           Force recompilation of all modules");
    println!("  --list, -l            List modules without compiling");
    println!("  --verbose, -v         Show detailed output");
    println!("  --help, -h            Show this help message");
    println!();
    println!("Examples:");
    println!("  preblade --stdlib-path ./compiler/haxe-std --out .rayzor/blade/stdlib");
    println!("  preblade --list");
    println!("  preblade --force");
}

/// Discover all .hx modules recursively in the stdlib directory
fn discover_modules_recursive(stdlib_path: &Path) -> Vec<ModuleInfo> {
    let mut modules = Vec::new();
    discover_modules_in_dir(stdlib_path, stdlib_path, &mut modules);

    // Sort by name for consistent ordering
    modules.sort_by(|a, b| a.name.cmp(&b.name));

    modules
}

fn discover_modules_in_dir(base_path: &Path, current_path: &Path, modules: &mut Vec<ModuleInfo>) {
    let entries = match std::fs::read_dir(current_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Skip hidden directories and platform-specific directories
            let dir_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if dir_name.starts_with('.') || dir_name.starts_with('_') {
                continue;
            }

            // Skip platform-specific directories (we only want cross-platform code)
            let skip_dirs = ["cpp", "cs", "flash", "hl", "java", "js", "lua", "neko", "php", "python", "eval"];
            if skip_dirs.contains(&dir_name) {
                continue;
            }

            discover_modules_in_dir(base_path, &path, modules);
        } else if path.extension().map(|e| e == "hx").unwrap_or(false) {
            // Skip import.hx files (they're not modules)
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if file_name == "import.hx" {
                continue;
            }

            // Get relative path from base
            let relative = path.strip_prefix(base_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_name.to_string());

            // Convert path to module name: "haxe/io/Bytes.hx" -> "haxe.io.Bytes"
            let name = relative
                .replace('/', ".")
                .replace('\\', ".")
                .replace(".hx", "");

            modules.push(ModuleInfo {
                name,
                source_path: path,
                relative_path: relative,
            });
        }
    }
}

fn is_blade_current(blade_path: &Path, source_path: &Path) -> bool {
    // Check if blade file exists and is newer than source
    let blade_mtime = std::fs::metadata(blade_path)
        .and_then(|m| m.modified())
        .ok();
    let source_mtime = std::fs::metadata(source_path)
        .and_then(|m| m.modified())
        .ok();

    match (blade_mtime, source_mtime) {
        (Some(b), Some(s)) => b >= s,
        _ => false,
    }
}

/// Shared compilation context for compiling multiple modules
struct SharedCompilationContext {
    unit: compiler::compilation::CompilationUnit,
    initialized: bool,
}

impl SharedCompilationContext {
    fn new() -> Self {
        let config = compiler::compilation::CompilationConfig::default();
        let unit = compiler::compilation::CompilationUnit::new(config);
        Self { unit, initialized: false }
    }

    fn ensure_initialized(&mut self) -> Result<(), String> {
        if !self.initialized {
            self.unit.load_stdlib()
                .map_err(|e| format!("Failed to load stdlib: {:?}", e))?;
            self.initialized = true;
        }
        Ok(())
    }

    fn compile_module(&mut self, source_path: &Path, blade_path: &Path, module_name: &str) -> Result<(), String> {
        use compiler::ir::blade::{save_blade, BladeMetadata};

        self.ensure_initialized()?;

        // Read source
        let source = std::fs::read_to_string(source_path)
            .map_err(|e| format!("Failed to read source: {}", e))?;

        // Add the file we want to compile
        let filename = source_path.to_string_lossy().to_string();
        self.unit.add_file(&source, &filename)
            .map_err(|e| format!("Failed to add file: {}", e))?;

        // Compile to TAST and MIR
        self.unit.lower_to_tast()
            .map_err(|errors| {
                let error_msgs: Vec<String> = errors.iter()
                    .take(3)  // Only show first 3 errors
                    .map(|e| e.message.clone())
                    .collect();
                format!("{}", error_msgs.join("; "))
            })?;

        // Get MIR modules
        let mir_modules = self.unit.get_mir_modules();

        // Find the module we compiled (should be the last one, or match by name)
        let mir = mir_modules.iter()
            .find(|m| m.name.contains(module_name) || m.source_file.contains(&filename))
            .or_else(|| mir_modules.last())
            .ok_or_else(|| "No MIR module generated".to_string())?;

        // Compute source hash
        let source_hash = hash_source(&source);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let metadata = BladeMetadata {
            name: module_name.to_string(),
            source_path: source_path.to_string_lossy().to_string(),
            source_hash,
            source_timestamp: now,
            compile_timestamp: now,
            dependencies: Vec::new(), // TODO: Extract from imports
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Save to BLADE
        save_blade(blade_path, &mir, metadata)
            .map_err(|e| format!("Failed to save BLADE: {}", e))?;

        Ok(())
    }
}

fn hash_source(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}
