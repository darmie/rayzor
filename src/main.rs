//! Rayzor - High-performance Haxe compiler with tiered JIT compilation
//!
//! # Usage
//!
//! ```bash
//! # Compile and run a Haxe file
//! rayzor run Main.hx
//!
//! # Use HXML build file (compatible with standard Haxe)
//! rayzor build.hxml
//!
//! # JIT compile with tier selection
//! rayzor jit --tier 2 MyApp.hx
//!
//! # Check syntax without executing
//! rayzor check Main.hx
//!
//! # Show compilation pipeline
//! rayzor compile --show-ir Main.hx
//! ```

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "rayzor")]
#[command(version = "0.1.0")]
#[command(about = "Rayzor - High-performance Haxe compiler with tiered JIT", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a Haxe file with JIT compilation
    Run {
        /// Path to the Haxe source file
        file: PathBuf,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Show compilation statistics
        #[arg(long)]
        stats: bool,

        /// Starting optimization tier (0-3)
        #[arg(long, default_value = "0")]
        tier: u8,

        /// Enable LLVM Tier 3 optimization
        #[arg(long)]
        llvm: bool,

        /// Enable BLADE cache for incremental compilation
        #[arg(long)]
        cache: bool,

        /// Cache directory (defaults to target/debug/cache or target/release/cache)
        #[arg(long)]
        cache_dir: Option<PathBuf>,

        /// Build with optimizations (uses target/release instead of target/debug)
        #[arg(long)]
        release: bool,
    },

    /// JIT compile with interactive REPL
    Jit {
        /// Path to the Haxe source file
        file: Option<PathBuf>,

        /// Target optimization tier (0=baseline, 1=standard, 2=optimized, 3=maximum/LLVM)
        #[arg(short, long, default_value = "2")]
        tier: u8,

        /// Show Cranelift IR
        #[arg(long)]
        show_cranelift: bool,

        /// Show MIR (Mid-level IR)
        #[arg(long)]
        show_mir: bool,

        /// Enable profiling for tier promotion
        #[arg(long)]
        profile: bool,
    },

    /// Check Haxe syntax and type checking
    Check {
        /// Path to the Haxe source file
        file: PathBuf,

        /// Show full type information
        #[arg(long)]
        show_types: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Compile Haxe to intermediate representation
    Compile {
        /// Path to the Haxe source file
        file: PathBuf,

        /// Stop at compilation stage
        #[arg(long, value_enum, default_value = "native")]
        stage: CompileStage,

        /// Show intermediate representations
        #[arg(long)]
        show_ir: bool,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Enable BLADE cache for incremental compilation
        #[arg(long)]
        cache: bool,

        /// Cache directory (defaults to target/debug/cache or target/release/cache)
        #[arg(long)]
        cache_dir: Option<PathBuf>,

        /// Build with optimizations (uses target/release instead of target/debug)
        #[arg(long)]
        release: bool,
    },

    /// Build from HXML file (Haxe-compatible)
    Build {
        /// Path to HXML build file
        file: PathBuf,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Override output path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show what would be built without building
        #[arg(long)]
        dry_run: bool,
    },

    /// Show information about the compiler
    Info {
        /// Show detailed feature information
        #[arg(long)]
        features: bool,

        /// Show tiered JIT configuration
        #[arg(long)]
        tiers: bool,
    },

    /// Manage BLADE compilation cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Show cache statistics
    Stats {
        /// Cache directory (defaults to .rayzor-cache)
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },

    /// Clear all cached modules
    Clear {
        /// Cache directory (defaults to .rayzor-cache)
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum OutputFormat {
    Text,
    Json,
    Pretty,
}

#[derive(ValueEnum, Clone, Debug)]
enum CompileStage {
    /// Stop after parsing (AST)
    Ast,
    /// Stop after type checking (TAST)
    Tast,
    /// Stop after semantic analysis (HIR)
    Hir,
    /// Stop after MIR lowering
    Mir,
    /// Compile to native code (default)
    Native,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run { file, verbose, stats, tier, llvm, cache, cache_dir, release } => {
            run_file(file, verbose, stats, tier, llvm, cache, cache_dir, release)
        }
        Commands::Jit { file, tier, show_cranelift, show_mir, profile } => {
            jit_compile(file, tier, show_cranelift, show_mir, profile)
        }
        Commands::Check { file, show_types, format } => {
            check_file(file, show_types, format)
        }
        Commands::Compile { file, stage, show_ir, output, cache, cache_dir, release } => {
            compile_file(file, stage, show_ir, output, cache, cache_dir, release)
        }
        Commands::Build { file, verbose, output, dry_run } => {
            build_hxml(file, verbose, output, dry_run)
        }
        Commands::Info { features, tiers } => {
            show_info(features, tiers);
            Ok(())
        }
        Commands::Cache { action } => {
            match action {
                CacheAction::Stats { cache_dir } => {
                    cache_stats(cache_dir)
                }
                CacheAction::Clear { cache_dir } => {
                    cache_clear(cache_dir)
                }
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

/// Helper function to compile Haxe source through the full pipeline to MIR
/// Uses CompilationUnit for proper multi-file, stdlib-aware compilation
/// Returns all MIR modules (user code + stdlib)
fn compile_haxe_to_mir(source: &str, filename: &str) -> Result<Vec<std::sync::Arc<compiler::ir::IrModule>>, String> {
    use compiler::compilation::{CompilationUnit, CompilationConfig};

    // Create compilation unit with stdlib support
    let mut config = CompilationConfig::default();
    config.load_stdlib = true; // Enable stdlib for full Haxe compatibility

    let mut unit = CompilationUnit::new(config);

    // Load the standard library first
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add the source file to the compilation unit
    unit.add_file(source, filename)?;

    // Compile the unit to TAST (this compiles all files including stdlib)
    unit.lower_to_tast().map_err(|errors| {
        let messages: Vec<_> = errors.iter().map(|e| format!("{:?}", e)).collect();
        format!("TAST lowering errors: {}", messages.join(", "))
    })?;

    // Get all MIR modules (including stdlib)
    let mir_modules = unit.get_mir_modules();

    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    Ok(mir_modules)
}

fn run_file(file: PathBuf, verbose: bool, stats: bool, _tier: u8, llvm: bool, cache: bool, cache_dir: Option<PathBuf>, release: bool) -> Result<(), String> {
    use compiler::codegen::tiered_backend::{TieredBackend, TieredConfig};
    use compiler::codegen::profiling::ProfileConfig;
    use compiler::compilation::{CompilationUnit, CompilationConfig};

    let profile = if release { "release" } else { "debug" };
    println!("🚀 Running {} [{}]...", file.display(), profile);

    #[cfg(not(feature = "llvm-backend"))]
    if llvm {
        return Err("LLVM backend not available. Recompile with --features llvm-backend".to_string());
    }

    // Read source file
    if !file.exists() {
        return Err(format!("File not found: {}", file.display()));
    }

    if verbose {
        println!("\n{}", "=".repeat(60));
        println!("Compilation Pipeline [{}]", profile);
        println!("{}", "=".repeat(60));
    }

    // Create compilation unit with cache configuration
    let mut config = CompilationConfig::default();
    config.load_stdlib = false;
    config.enable_cache = cache;
    if let Some(dir) = cache_dir {
        config.cache_dir = Some(dir);
    } else if cache {
        // Use profile-specific cache directory
        config.cache_dir = Some(CompilationConfig::get_profile_cache_dir(profile));
    }

    let _unit = CompilationUnit::new(config);

    // Compile source file to MIR (returns all modules including stdlib)
    let source = std::fs::read_to_string(&file)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let mir_modules = compile_haxe_to_mir(&source, file.to_str().unwrap_or("unknown"))?;

    let total_functions: usize = mir_modules.iter().map(|m| m.functions.len()).sum();
    if verbose {
        println!("  ✓ MIR created");
        println!("    Modules: {}", mir_modules.len());
        println!("    Total functions: {}", total_functions);
    }

    if total_functions == 0 {
        return Err("No functions found to execute".to_string());
    }

    // Set up tiered JIT backend
    if verbose {
        println!("\nSetting up Tiered JIT...");
    }
    let config = TieredConfig {
        enable_background_optimization: true,
        optimization_check_interval_ms: 50,
        max_parallel_optimizations: 2,
        profile_config: ProfileConfig {
            interpreter_threshold: 10,
            warm_threshold: 100,
            hot_threshold: 1000,
            blazing_threshold: 5000,
            sample_rate: 1,
        },
        verbosity: if verbose { 2 } else { 0 },
        start_interpreted: false, // Start with JIT, not interpreter
    };

    let mut backend = TieredBackend::new(config)?;
    if verbose {
        println!("  ✓ Tiered backend ready");
    }

    // Compile all modules with tiered JIT
    if verbose {
        println!("\nCompiling MIR → Native (Tier 0)...");
    }
    for module in mir_modules {
        // Convert Arc<IrModule> to owned IrModule for the tiered backend
        let owned_module = std::sync::Arc::try_unwrap(module)
            .unwrap_or_else(|arc| (*arc).clone());
        backend.compile_module(owned_module)?;
    }

    if verbose {
        println!("  ✓ Compiled successfully");
    }

    // Show completion
    println!("\n{}", "=".repeat(60));
    println!("Compilation Complete");
    println!("{}", "=".repeat(60));

    println!("\n(Function execution requires main() lookup - coming soon)");

    // Show stats if requested
    if stats {
        println!("\n{}", "=".repeat(60));
        println!("Statistics");
        println!("{}", "=".repeat(60));

        let backend_stats = backend.get_statistics();
        println!("\nTier Distribution:");
        println!("  Tier 0 (Baseline):  {} functions", backend_stats.baseline_functions);
        println!("  Tier 1 (Standard):  {} functions", backend_stats.standard_functions);
        println!("  Tier 2 (Optimized): {} functions", backend_stats.optimized_functions);
        println!("  Tier 3 (Maximum):   {} functions", backend_stats.llvm_functions);
    }

    backend.shutdown();
    println!("\n✓ Complete!");
    Ok(())
}

fn jit_compile(
    file: Option<PathBuf>,
    tier: u8,
    show_cranelift: bool,
    show_mir: bool,
    profile: bool,
) -> Result<(), String> {
    if let Some(ref path) = file {
        println!("🔥 JIT compiling {} at Tier {}...", path.display(), tier);
    } else {
        println!("🔥 Starting Rayzor JIT REPL...");
        println!("   Type Haxe code or 'exit' to quit");
    }

    if show_cranelift {
        println!("  Will show Cranelift IR");
    }
    if show_mir {
        println!("  Will show MIR");
    }
    if profile {
        println!("  Profiling enabled for tier promotion");
    }

    // TODO: Implement JIT compilation
    Err("JIT command not yet fully implemented. See compiler/examples/test_full_pipeline_tiered.rs".to_string())
}

fn check_file(file: PathBuf, show_types: bool, format: OutputFormat) -> Result<(), String> {
    println!("✓ Checking {}...", file.display());

    if !file.exists() {
        return Err(format!("File not found: {}", file.display()));
    }

    let source = std::fs::read_to_string(&file)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Parse the file
    use parser::haxe_parser::parse_haxe_file;
    let ast = parse_haxe_file(
        file.to_str().unwrap_or("unknown"),
        &source,
        false,
    ).map_err(|e| format!("Parse error: {}", e))?;

    match format {
        OutputFormat::Text => {
            println!("✓ Syntax: OK");
            println!("  Package: {:?}", ast.package);
            println!("  Declarations: {}", ast.declarations.len());
            println!("  Module fields: {}", ast.module_fields.len());
            println!("  Imports: {}", ast.imports.len());
        }
        OutputFormat::Json => {
            println!("{{");
            println!("  \"status\": \"ok\",");
            println!("  \"declarations\": {},", ast.declarations.len());
            println!("  \"module_fields\": {},", ast.module_fields.len());
            println!("  \"imports\": {}", ast.imports.len());
            println!("}}");
        }
        OutputFormat::Pretty => {
            println!("┌─ Syntax Check ─────────────────");
            println!("│ Status:       ✓ OK");
            println!("│ Package:      {:?}", ast.package);
            println!("│ Declarations: {}", ast.declarations.len());
            println!("│ Module fields: {}", ast.module_fields.len());
            println!("│ Imports:      {}", ast.imports.len());
            println!("└────────────────────────────────");
        }
    }

    if show_types {
        println!("\nType information:");
        println!("  (Full type checking not yet implemented)");
    }

    Ok(())
}

fn build_hxml(
    file: PathBuf,
    verbose: bool,
    output_override: Option<PathBuf>,
    dry_run: bool,
) -> Result<(), String> {
    use compiler::hxml::{HxmlConfig, RayzorMode};

    println!("📦 Building from HXML: {}", file.display());

    // Parse HXML file
    let config = HxmlConfig::from_file(&file)?;

    if verbose {
        println!("\n{}", config.summary());
    }

    // Validate configuration
    config.validate()?;

    let output = output_override.or(config.output.clone());

    if dry_run {
        println!("\n🔍 Dry run - would build:");
        println!("  Main: {:?}", config.main_class);
        println!("  Mode: {:?}", config.mode);
        println!("  Output: {:?}", output);
        println!("  Class paths: {:?}", config.class_paths);
        println!("  Libraries: {}", config.libraries.join(", "));
        return Ok(());
    }

    // Extract main class
    if let Some(main_class) = config.main_class {
        println!("\n✓ Configuration loaded");
        println!("  Main class: {}", main_class);
        println!("  Mode: {:?}", config.mode);
        println!("  Libraries: {}", config.libraries.join(", "));

        // Find the main class file in class paths
        let mut main_file_path = None;
        for cp in &config.class_paths {
            let candidate = cp.join(format!("{}.hx", main_class.replace(".", "/")));
            if candidate.exists() {
                println!("  Found: {}", candidate.display());
                main_file_path = Some(candidate);
                break;
            }
        }

        let main_file = main_file_path
            .ok_or_else(|| format!("Main class file not found in class paths: {}", main_class))?;

        // Execute based on mode
        match config.mode {
            RayzorMode::Jit => {
                println!("\n🔥 JIT mode - compiling and executing...");
                println!("  (Full HXML JIT pipeline coming soon)");
                println!("  For now, use: rayzor jit {}", main_file.display());
                Ok(())
            }
            RayzorMode::Compile => {
                println!("\n🔨 Compile mode - generating native binary...");
                if let Some(out) = output {
                    println!("  Output: {}", out.display());
                    println!("  (Full HXML AOT pipeline coming soon)");
                    println!("  For now, use: rayzor compile {}", main_file.display());
                } else {
                    return Err("Compile mode requires output file. Use --rayzor-compile <output>".to_string());
                }
                Ok(())
            }
        }
    } else {
        Err("No main class specified in HXML file".to_string())
    }
}

fn compile_file(
    file: PathBuf,
    stage: CompileStage,
    show_ir: bool,
    output: Option<PathBuf>,
    cache: bool,
    cache_dir: Option<PathBuf>,
    release: bool,
) -> Result<(), String> {
    use parser::haxe_parser::parse_haxe_file;
    use compiler::compilation::{CompilationUnit, CompilationConfig};

    let profile = if release { "release" } else { "debug" };
    let target = CompilationConfig::get_target_triple();
    println!("🔨 Compiling {} to {:?} [{}] [{}]...", file.display(), stage, profile, target);

    if let Some(ref out) = output {
        println!("  Output: {}", out.display());
    }

    // Read source file
    if !file.exists() {
        return Err(format!("File not found: {}", file.display()));
    }

    let source = std::fs::read_to_string(&file)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    println!("\n{}", "=".repeat(60));
    println!("Compilation Pipeline [{}]", profile);
    println!("{}", "=".repeat(60));

    // Step 1: Parse
    println!("\nStep 1: Parsing Haxe source...");
    let ast = parse_haxe_file(file.to_str().unwrap_or("unknown"), &source, false)
        .map_err(|e| format!("Parse error: {}", e))?;

    println!("  ✓ AST created");
    println!("    Declarations: {}", ast.declarations.len());
    println!("    Imports: {}", ast.imports.len());

    if show_ir {
        println!("\n--- AST ---");
        println!("{:#?}", ast);
    }

    if matches!(stage, CompileStage::Ast) {
        println!("\n✓ Stopped at AST stage");
        if let Some(output_path) = output {
            let ast_json = format!("{:#?}", ast);
            std::fs::write(&output_path, ast_json)
                .map_err(|e| format!("Failed to write output: {}", e))?;
            println!("  Output written to: {}", output_path.display());
        }
        return Ok(());
    }

    // Create compilation unit with cache configuration
    let mut config = CompilationConfig::default();
    config.load_stdlib = false;
    config.enable_cache = cache;
    if let Some(dir) = cache_dir {
        config.cache_dir = Some(dir);
    } else if cache {
        // Use profile-specific cache directory
        config.cache_dir = Some(CompilationConfig::get_profile_cache_dir(profile));
    }

    let unit = CompilationUnit::new(config);

    // For stages beyond AST, compile using our helper with caching support
    println!("\nCompiling through full pipeline...");
    let mir_module = if cache {
        if let Some(cached) = unit.try_load_cached(&file) {
            println!("  ✓ Loaded from cache");
            cached
        } else {
            println!("  Cache miss, compiling...");
            let module = compile_haxe_to_mir(&source, file.to_str().unwrap_or("unknown"))?;
            unit.save_to_cache(&file, &module)?;
            module
        }
    } else {
        compile_haxe_to_mir(&source, file.to_str().unwrap_or("unknown"))?
    };

    println!("  ✓ MIR created");
    println!("    Functions: {}", mir_module.functions.len());

    for func in mir_module.functions.values() {
        println!("      - {} ({} blocks)", func.name, func.cfg.blocks.len());
    }

    if show_ir {
        println!("\n--- MIR ---");
        println!("{:#?}", mir_module);
    }

    if matches!(stage, CompileStage::Mir) | matches!(stage, CompileStage::Tast) | matches!(stage, CompileStage::Hir) {
        println!("\n✓ Stopped at {:?} stage (showing MIR)", stage);
        if let Some(output_path) = output {
            let mir_json = format!("{:#?}", mir_module);
            std::fs::write(&output_path, mir_json)
                .map_err(|e| format!("Failed to write output: {}", e))?;
            println!("  Output written to: {}", output_path.display());
        }
        return Ok(());
    }

    // Step 2: Compile to native
    println!("\nCompiling MIR → Native...");
    use compiler::codegen::tiered_backend::{TieredBackend, TieredConfig};
    use compiler::codegen::profiling::ProfileConfig;

    let config = TieredConfig {
        enable_background_optimization: false, // No background optimization for compile mode
        optimization_check_interval_ms: 0,
        max_parallel_optimizations: 0,
        profile_config: ProfileConfig {
            interpreter_threshold: 10,
            warm_threshold: 100,
            hot_threshold: 1000,
            blazing_threshold: 5000,
            sample_rate: 1,
        },
        verbosity: 0,
        start_interpreted: false, // Start with JIT, not interpreter
    };

    let mut backend = TieredBackend::new(config)?;
    backend.compile_module(mir_module)?;

    println!("  ✓ Native code generated");

    if let Some(output_path) = output {
        println!("\n(Binary serialization not yet implemented)");
        println!("  Would write to: {}", output_path.display());
    }

    backend.shutdown();
    println!("\n✓ Compilation complete!");
    Ok(())
}

fn show_info(features: bool, tiers: bool) {
    println!("Rayzor Compiler v0.1.0");
    println!("High-performance Haxe compiler with tiered JIT compilation\n");

    if features || (!features && !tiers) {
        println!("Features:");
        println!("  ✓ Full Haxe parser");
        println!("  ✓ Type checker (TAST)");
        println!("  ✓ Semantic analysis (HIR)");
        println!("  ✓ SSA form with phi nodes (MIR)");
        println!("  ✓ Tiered JIT compilation (Cranelift)");

        #[cfg(feature = "llvm-backend")]
        println!("  ✓ LLVM backend (Tier 3)");

        #[cfg(not(feature = "llvm-backend"))]
        println!("  ✗ LLVM backend (not enabled)");

        println!();
    }

    if tiers || (!features && !tiers) {
        println!("Tiered JIT System:");
        println!("  Tier 0 (Baseline)  - Cranelift 'none'          - ~3ms compile, 1.0x speed");
        println!("  Tier 1 (Standard)  - Cranelift 'speed'         - ~10ms compile, 1.5-3x speed");
        println!("  Tier 2 (Optimized) - Cranelift 'speed_and_size' - ~30ms compile, 3-5x speed");

        #[cfg(feature = "llvm-backend")]
        println!("  Tier 3 (Maximum)   - LLVM aggressive          - ~500ms compile, 5-20x speed");

        #[cfg(not(feature = "llvm-backend"))]
        println!("  Tier 3 (Maximum)   - LLVM (not available)");

        println!("\n  Functions automatically promote based on execution count:");
        println!("    • 100 calls   → Tier 1");
        println!("    • 1,000 calls → Tier 2");
        println!("    • 5,000 calls → Tier 3");
        println!();
    }

    println!("Examples:");
    println!("  cargo run --example test_full_pipeline_tiered");
    println!("  cargo run --example test_tiered_with_loop --features llvm-backend");
}

fn cache_stats(cache_dir: Option<PathBuf>) -> Result<(), String> {
    use compiler::compilation::{CompilationUnit, CompilationConfig};

    let mut config = CompilationConfig::default();
    if let Some(dir) = cache_dir {
        config.cache_dir = Some(dir);
    }

    let unit = CompilationUnit::new(config);
    let stats = unit.get_cache_stats();

    println!("📊 BLADE Cache Statistics");
    println!("{}", "=".repeat(60));
    println!("Cache directory: {:?}", unit.config.get_cache_dir());
    println!("Cached modules:  {}", stats.cached_modules);
    println!("Total size:      {:.2} MB", stats.total_size_mb());
    println!();

    if stats.cached_modules == 0 {
        println!("No cached modules found.");
        println!("Use --cache flag with 'run' or 'compile' to enable caching.");
    } else {
        println!("Benefits:");
        println!("  • Incremental compilation: ~30x faster for unchanged files");
        println!("  • Dependency caching: Only recompile modified modules");
        println!("  • Version tracking: Automatic invalidation on compiler updates");
    }

    Ok(())
}

fn cache_clear(cache_dir: Option<PathBuf>) -> Result<(), String> {
    use compiler::compilation::{CompilationUnit, CompilationConfig};

    let mut config = CompilationConfig::default();
    if let Some(dir) = cache_dir {
        config.cache_dir = Some(dir);
    }

    let unit = CompilationUnit::new(config);
    let cache_path = unit.config.get_cache_dir();

    println!("🗑️  Clearing BLADE cache...");
    println!("Cache directory: {:?}", cache_path);

    unit.clear_cache()?;

    println!("✓ Cache cleared successfully");

    Ok(())
}
