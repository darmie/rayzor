//! Multi-file Compilation Infrastructure
//!
//! This module provides the proper architecture for compiling multiple source files
//! together, including standard library loading, package management, and symbol resolution.

use crate::tast::{
    AstLowering, StringInterner, SymbolTable, TypeTable, ScopeTree, ScopeId,
    namespace::{NamespaceResolver, ImportResolver},
    stdlib_loader::{StdLibConfig, StdLibLoader},
    TypedFile, SourceLocation,
};
use crate::pipeline::{
    CompilationError, ErrorCategory, HaxeCompilationPipeline,
    PipelineConfig, CompilationResult,
};
use crate::dependency_graph::{DependencyGraph, DependencyAnalysis, CircularDependency};
use crate::ir::{IrModule, blade::{save_blade, load_blade, BladeMetadata}};
use parser::{HaxeFile, parse_haxe_file, parse_haxe_file_with_debug};
use std::rc::Rc;
use std::cell::RefCell;
use std::path::{PathBuf, Path};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

/// Represents a complete compilation unit with multiple source files
pub struct CompilationUnit {
    /// Stdlib files (loaded first with haxe.* package)
    pub stdlib_files: Vec<HaxeFile>,

    /// Global import.hx files (loaded after stdlib, before user files)
    pub import_hx_files: Vec<HaxeFile>,

    /// User source files
    pub user_files: Vec<HaxeFile>,

    /// Shared string interner
    pub string_interner: StringInterner,

    /// Symbol table (shared across all files)
    pub symbol_table: SymbolTable,

    /// Type table (shared across all files)
    pub type_table: Rc<RefCell<TypeTable>>,

    /// Scope tree (shared across all files)
    pub scope_tree: ScopeTree,

    /// Namespace resolver
    pub namespace_resolver: NamespaceResolver,

    /// Import resolver
    pub import_resolver: ImportResolver,

    /// Configuration
    pub config: CompilationConfig,

    /// Internal compilation pipeline (delegates to HaxeCompilationPipeline)
    pipeline: HaxeCompilationPipeline,
}

/// Configuration for compilation
#[derive(Clone)]
pub struct CompilationConfig {
    /// Paths to search for standard library files
    pub stdlib_paths: Vec<PathBuf>,

    /// Default stdlib imports to load
    pub default_stdlib_imports: Vec<String>,

    /// Whether to load stdlib
    pub load_stdlib: bool,

    /// Root package for stdlib (e.g., "haxe")
    pub stdlib_root_package: Option<String>,

    /// Global import.hx files to process (loaded before user files, after stdlib)
    pub global_import_hx_files: Vec<PathBuf>,

    /// Enable incremental compilation with BLADE cache
    pub enable_cache: bool,

    /// Directory for BLADE cache files
    pub cache_dir: Option<PathBuf>,

    /// Pipeline configuration for analysis and optimization
    pub pipeline_config: PipelineConfig,
}

impl Default for CompilationConfig {
    fn default() -> Self {
        Self {
            stdlib_paths: Self::discover_stdlib_paths(),
            default_stdlib_imports: vec![
                "StdTypes.hx".to_string(),
                "Iterator.hx".to_string(), // Must come before Array since Array uses Iterator
                "String.hx".to_string(),
                "Array.hx".to_string(),
            ],
            load_stdlib: true,
            stdlib_root_package: Some("haxe".to_string()), // Prefix stdlib with "haxe.*" namespace
            global_import_hx_files: Vec::new(), // No global import.hx by default
            enable_cache: false, // Cache disabled by default
            cache_dir: None, // Auto-discover cache directory when needed
            pipeline_config: PipelineConfig::default(),
        }
    }
}

impl CompilationConfig {
    /// Discover standard library paths from environment and standard locations
    ///
    /// Search order:
    /// 1. HAXE_STD_PATH environment variable
    /// 2. HAXE_HOME environment variable (looking for std/ subdirectory)
    /// 3. Current project's haxe-std directory
    /// 4. Parent directory's haxe-std
    /// 5. Standard installation locations (platform-specific)
    pub fn discover_stdlib_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Check HAXE_STD_PATH environment variable
        if let Ok(haxe_std_path) = std::env::var("HAXE_STD_PATH") {
            let path = PathBuf::from(&haxe_std_path);
            if path.exists() {
                println!("Found stdlib at HAXE_STD_PATH: {}", haxe_std_path);
                paths.push(path);
                return paths; // Use this path exclusively if set
            } else {
                eprintln!("Warning: HAXE_STD_PATH set but directory doesn't exist: {}", haxe_std_path);
            }
        }

        // 2. Check HAXE_HOME/std
        if let Ok(haxe_home) = std::env::var("HAXE_HOME") {
            let std_path = PathBuf::from(&haxe_home).join("std");
            if std_path.exists() {
                println!("Found stdlib at HAXE_HOME/std: {:?}", std_path);
                paths.push(std_path);
            }
        }

        // 3. Check current project's haxe-std directory
        let project_stdlib = PathBuf::from("compiler/haxe-std");
        if project_stdlib.exists() {
            paths.push(project_stdlib);
        }

        // 4. Check parent directory's haxe-std
        let parent_stdlib = PathBuf::from("../haxe-std");
        if parent_stdlib.exists() {
            paths.push(parent_stdlib);
        }

        let current_dir_stdlib = PathBuf::from("./haxe-std");
        if current_dir_stdlib.exists() {
            paths.push(current_dir_stdlib);
        }

        // 5. Platform-specific standard installation locations
        #[cfg(target_os = "linux")]
        {
            let linux_paths = vec![
                PathBuf::from("/usr/share/haxe/std"),
                PathBuf::from("/usr/local/share/haxe/std"),
                PathBuf::from("/opt/haxe/std"),
            ];
            for path in linux_paths {
                if path.exists() {
                    paths.push(path);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let macos_paths = vec![
                PathBuf::from("/usr/local/lib/haxe/std"),
                PathBuf::from("/opt/homebrew/lib/haxe/std"),
                PathBuf::from("/Library/Haxe/std"),
            ];
            for path in macos_paths {
                if path.exists() {
                    paths.push(path);
                }
            }

            // Check user's home directory
            if let Ok(home) = std::env::var("HOME") {
                let user_haxe = PathBuf::from(home).join(".haxe/std");
                if user_haxe.exists() {
                    paths.push(user_haxe);
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let windows_paths = vec![
                PathBuf::from("C:\\HaxeToolkit\\haxe\\std"),
                PathBuf::from("C:\\Program Files\\Haxe\\std"),
                PathBuf::from("C:\\Program Files (x86)\\Haxe\\std"),
            ];
            for path in windows_paths {
                if path.exists() {
                    paths.push(path);
                }
            }

            // Check user's AppData
            if let Ok(appdata) = std::env::var("APPDATA") {
                let user_haxe = PathBuf::from(appdata).join("Haxe\\std");
                if user_haxe.exists() {
                    paths.push(user_haxe);
                }
            }
        }

        if paths.is_empty() {
            eprintln!("Warning: No standard library found. Set HAXE_STD_PATH environment variable.");
            eprintln!("         or install Haxe to a standard location.");
            // Still provide fallback paths for development
            paths.push(PathBuf::from("compiler/haxe-std"));
            paths.push(PathBuf::from("../haxe-std"));
            paths.push(PathBuf::from("./haxe-std"));
        }

        paths
    }

    /// Get the current target triple (e.g., "x86_64-macos", "aarch64-linux")
    pub fn get_target_triple() -> String {
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;
        format!("{}-{}", arch, os)
    }

    /// Get or create the cache directory
    pub fn get_cache_dir(&self) -> PathBuf {
        if let Some(ref cache_dir) = self.cache_dir {
            return cache_dir.clone();
        }

        // Default: target/<arch>-<os>/debug/cache
        let triple = Self::get_target_triple();
        let default_cache = PathBuf::from("target")
            .join(&triple)
            .join("debug")
            .join("cache");

        // Try to create it if it doesn't exist
        if !default_cache.exists() {
            let _ = std::fs::create_dir_all(&default_cache);
        }

        default_cache
    }

    /// Get the target directory for the given profile
    pub fn get_target_dir(profile: &str) -> PathBuf {
        let triple = Self::get_target_triple();
        PathBuf::from("target").join(triple).join(profile)
    }

    /// Get the build directory for intermediate artifacts
    pub fn get_build_dir(profile: &str) -> PathBuf {
        Self::get_target_dir(profile).join("build")
    }

    /// Get the cache directory for a specific profile
    pub fn get_profile_cache_dir(profile: &str) -> PathBuf {
        Self::get_target_dir(profile).join("cache")
    }

    /// Get the output directory for executables
    pub fn get_output_dir(profile: &str) -> PathBuf {
        Self::get_target_dir(profile)
    }

    /// Get the cache file path for a given source file
    pub fn get_cache_path(&self, source_path: &Path) -> PathBuf {
        let cache_dir = self.get_cache_dir();

        // Create a cache filename based on the source path
        // Convert path to a safe filename by replacing separators with underscores
        let source_str = source_path.to_string_lossy();
        let cache_name = source_str
            .replace(['/', '\\', ':'], "_")
            .replace(".hx", ".blade");

        cache_dir.join(cache_name)
    }
}

impl CompilationUnit {
    /// Create a new compilation unit with the given configuration
    pub fn new(config: CompilationConfig) -> Self {
        let string_interner = StringInterner::new();
        let namespace_resolver = NamespaceResolver::new(&string_interner);
        let import_resolver = ImportResolver::new(&namespace_resolver);

        // Create pipeline with config
        let pipeline = HaxeCompilationPipeline::with_config(config.pipeline_config.clone());

        Self {
            stdlib_files: Vec::new(),
            import_hx_files: Vec::new(),
            user_files: Vec::new(),
            string_interner,
            symbol_table: SymbolTable::new(),
            type_table: Rc::new(RefCell::new(TypeTable::new())),
            scope_tree: ScopeTree::new(ScopeId::from_raw(0)),
            namespace_resolver,
            import_resolver,
            config,
            pipeline,
        }
    }

    /// Load standard library files
    /// This should be called FIRST, before any user files are added
    pub fn load_stdlib(&mut self) -> Result<(), String> {
        if !self.config.load_stdlib {
            return Ok(());
        }

        // Configure stdlib loader
        let mut loader_config = StdLibConfig::default();
        loader_config.std_paths = self.config.stdlib_paths.clone();
        loader_config.default_imports = self.config.default_stdlib_imports.clone();

        let mut loader = StdLibLoader::new(loader_config);

        // Load stdlib AST files directly into stdlib_files
        self.stdlib_files = loader.load_default_imports();

        Ok(())
    }

    /// Load global import.hx files
    /// These are processed AFTER stdlib but BEFORE user files
    /// They provide global imports available to all user code
    pub fn load_global_imports(&mut self) -> Result<(), String> {
        use std::fs;

        for import_path in &self.config.global_import_hx_files.clone() {
            let source = fs::read_to_string(import_path)
                .map_err(|e| format!("Failed to read import.hx at {:?}: {}", import_path, e))?;

            let haxe_file = parse_haxe_file(
                import_path.to_str().unwrap_or("import.hx"),
                &source,
                true
            ).map_err(|e| format!("Parse error in {:?}: {}", import_path, e))?;

            self.import_hx_files.push(haxe_file);
        }

        Ok(())
    }

    /// Add a user source file to the compilation unit
    pub fn add_file(&mut self, source: &str, file_path: &str) -> Result<(), String> {
        // Parse the file (file_name, input, recovery mode=true, debug=true to preserve source)
        let haxe_file = parse_haxe_file_with_debug(file_path, source, true, true)
            .map_err(|e| format!("Parse error in {}: {}", file_path, e))?;

        self.user_files.push(haxe_file);
        Ok(())
    }

    /// Add a file from filesystem path
    /// This resolves the file's path and loads it, making it easier to work with
    /// real projects on disk
    pub fn add_file_from_path(&mut self, path: &PathBuf) -> Result<(), String> {
        use std::fs;

        let source = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file {:?}: {}", path, e))?;

        let file_path_str = path.to_str()
            .ok_or_else(|| format!("Invalid UTF-8 in path: {:?}", path))?;

        self.add_file(&source, file_path_str)
    }

    /// Add all .hx files from a directory (recursively)
    /// This is useful for loading entire source trees
    ///
    /// # Arguments
    /// * `dir_path` - The directory to scan for .hx files
    /// * `recursive` - Whether to scan subdirectories
    pub fn add_directory(&mut self, dir_path: &PathBuf, recursive: bool) -> Result<usize, String> {
        use std::fs;

        let mut added_count = 0;

        let entries = fs::read_dir(dir_path)
            .map_err(|e| format!("Failed to read directory {:?}: {}", dir_path, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "hx" {
                        self.add_file_from_path(&path)?;
                        added_count += 1;
                    }
                }
            } else if path.is_dir() && recursive {
                added_count += self.add_directory(&path, recursive)?;
            }
        }

        Ok(added_count)
    }

    /// Resolve an import path to a filesystem path
    /// For example: "com.example.model.User" -> "src/com/example/model/User.hx"
    ///
    /// # Arguments
    /// * `import_path` - The import path (e.g., "com.example.model.User")
    /// * `source_paths` - Directories to search for source files (e.g., ["src", "lib"])
    pub fn resolve_import_path(&self, import_path: &str, source_paths: &[PathBuf]) -> Option<PathBuf> {
        // Convert import path to filesystem path
        // "com.example.model.User" -> "com/example/model/User.hx"
        let file_path = import_path.replace('.', "/") + ".hx";

        // Search in each source path
        for source_path in source_paths {
            let full_path = source_path.join(&file_path);
            if full_path.exists() {
                return Some(full_path);
            }
        }

        None
    }

    /// Add a file by import path (e.g., "com.example.model.User")
    /// This automatically searches source paths to find the file
    ///
    /// # Arguments
    /// * `import_path` - The import path
    /// * `source_paths` - Directories to search for source files
    pub fn add_file_by_import(&mut self, import_path: &str, source_paths: &[PathBuf]) -> Result<(), String> {
        let path = self.resolve_import_path(import_path, source_paths)
            .ok_or_else(|| format!("Could not resolve import: {}", import_path))?;

        self.add_file_from_path(&path)
    }

    /// Analyze dependencies and get compilation order
    ///
    /// This builds a dependency graph from all user files and determines
    /// the correct compilation order. It also detects circular dependencies.
    ///
    /// Returns (compilation_order, circular_dependencies)
    pub fn analyze_dependencies(&self) -> Result<DependencyAnalysis, Vec<CompilationError>> {
        if self.user_files.is_empty() {
            return Ok(DependencyAnalysis {
                compilation_order: Vec::new(),
                circular_dependencies: Vec::new(),
            });
        }

        // Build dependency graph
        let graph = DependencyGraph::from_files(&self.user_files);

        // Analyze
        let analysis = graph.analyze();

        // Report circular dependencies as warnings (not errors)
        if !analysis.circular_dependencies.is_empty() {
            eprintln!("⚠️  Warning: Circular dependencies detected!");
            for (i, cycle) in analysis.circular_dependencies.iter().enumerate() {
                eprintln!("\nCycle #{}:", i + 1);
                eprintln!("{}", cycle.format_error());
            }
            eprintln!("\nCompilation will proceed with best-effort ordering.\n");
        }

        Ok(analysis)
    }

    /// Lower all files (stdlib + user) to TAST with full pipeline analysis
    ///
    /// This method delegates to HaxeCompilationPipeline for each file to leverage
    /// the complete analysis infrastructure including:
    /// - Type checking with diagnostics
    /// - Flow-sensitive analysis
    /// - Ownership and lifetime analysis
    /// - Memory safety validation
    ///
    /// Order of compilation:
    /// 1. Stdlib files (with haxe.* package)
    /// 2. Import.hx files (for global imports)
    /// 3. User files (in dependency order - dependencies first)
    ///
    /// IMPORTANT: On error, this automatically prints formatted diagnostics to stderr
    pub fn lower_to_tast(&mut self) -> Result<Vec<TypedFile>, Vec<CompilationError>> {
        // Step 1: Analyze dependencies for user files
        let analysis = match self.analyze_dependencies() {
            Ok(a) => a,
            Err(errors) => {
                self.print_compilation_errors(&errors);
                return Err(errors);
            }
        };

        let mut all_typed_files = Vec::new();
        let mut all_errors = Vec::new();

        // Step 2: Compile stdlib files through pipeline
        for stdlib_file in &self.stdlib_files {
            if let Some(ref source) = stdlib_file.input {
                let result = self.pipeline.compile_file(&stdlib_file.filename, source);

                // Collect errors
                all_errors.extend(result.errors);

                // Add typed files
                all_typed_files.extend(result.typed_files);
            }
        }

        // Step 3: Compile import.hx files through pipeline
        for import_file in &self.import_hx_files {
            if let Some(ref source) = import_file.input {
                let result = self.pipeline.compile_file(&import_file.filename, source);

                all_errors.extend(result.errors);
                all_typed_files.extend(result.typed_files);
            }
        }

        // Step 4: Compile user files in dependency order through pipeline
        // The pipeline will run:
        // - Parse → TAST lowering
        // - Type checking with diagnostics
        // - Semantic graph construction (CFG, DFG, CallGraph, OwnershipGraph)
        // - Basic flow analysis
        // - Enhanced flow analysis (if enabled)
        // - Memory safety analysis: ownership + lifetime (if enabled)
        for &file_index in &analysis.compilation_order {
            let user_file = &self.user_files[file_index];
            if let Some(ref source) = user_file.input {
                let result = self.pipeline.compile_file(&user_file.filename, source);

                all_errors.extend(result.errors);
                all_typed_files.extend(result.typed_files);
            }
        }

        // Step 5: Report all errors if any were found
        if !all_errors.is_empty() {
            self.print_compilation_errors(&all_errors);
            return Err(all_errors);
        }

        Ok(all_typed_files)
    }

    /// Try to load a cached MIR module from a BLADE file
    ///
    /// Returns Some(IrModule) if cache is valid, None if cache doesn't exist or is stale
    pub fn try_load_cached(&self, source_path: &Path) -> Option<IrModule> {
        if !self.config.enable_cache {
            return None;
        }

        let cache_path = self.config.get_cache_path(source_path);
        if !cache_path.exists() {
            return None;
        }

        // Load BLADE file
        let (mir_module, metadata) = match load_blade(&cache_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Warning: Failed to load cache for {:?}: {}", source_path, e);
                return None;
            }
        };

        // Check if source file has been modified since cache was created
        if let Ok(source_meta) = std::fs::metadata(source_path) {
            if let Ok(modified) = source_meta.modified() {
                let source_timestamp = modified
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Cache is stale if source was modified after cache was created
                if source_timestamp > metadata.compile_timestamp {
                    if self.config.enable_cache {
                        println!("Cache stale for {:?} (source: {}, cache: {})",
                                 source_path, source_timestamp, metadata.compile_timestamp);
                    }
                    return None;
                }
            }
        }

        // Check compiler version matches
        let current_version = env!("CARGO_PKG_VERSION");
        if metadata.compiler_version != current_version {
            if self.config.enable_cache {
                println!("Cache version mismatch for {:?} (cache: {}, current: {})",
                         source_path, metadata.compiler_version, current_version);
            }
            return None;
        }

        if self.config.enable_cache {
            println!("Cache hit for {:?}", source_path);
        }

        Some(mir_module)
    }

    /// Save a compiled MIR module to the BLADE cache
    pub fn save_to_cache(&self, source_path: &Path, module: &IrModule) -> Result<(), String> {
        if !self.config.enable_cache {
            return Ok(());
        }

        let cache_path = self.config.get_cache_path(source_path);

        // Ensure cache directory exists
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        }

        // Get source file timestamp
        let source_timestamp = std::fs::metadata(source_path)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
            .unwrap_or(0);

        let compile_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create metadata
        let metadata = BladeMetadata {
            name: module.name.clone(),
            source_path: source_path.to_string_lossy().to_string(),
            source_timestamp,
            compile_timestamp,
            dependencies: Vec::new(), // TODO: Track dependencies for proper invalidation
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Save to BLADE file
        save_blade(&cache_path, module, metadata)
            .map_err(|e| format!("Failed to save cache: {}", e))?;

        if self.config.enable_cache {
            println!("Cached MIR for {:?} -> {:?}", source_path, cache_path);
        }

        Ok(())
    }

    /// Clear all cached BLADE files
    pub fn clear_cache(&self) -> Result<(), String> {
        let cache_dir = self.config.get_cache_dir();
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to clear cache: {}", e))?;
            std::fs::create_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to recreate cache directory: {}", e))?;
            println!("Cache cleared: {:?}", cache_dir);
        }
        Ok(())
    }

    /// Print compilation errors with formatted diagnostics to stderr
    /// Uses the diagnostics crate's ErrorFormatter for consistent formatting
    fn print_compilation_errors(&self, errors: &[CompilationError]) {
        use diagnostics::{SourceMap, ErrorFormatter};

        // Build source map with all parsed files
        let mut source_map = SourceMap::new();

        // Add stdlib files
        for stdlib_file in &self.stdlib_files {
            if let Some(ref source) = stdlib_file.input {
                source_map.add_file(stdlib_file.filename.clone(), source.clone());
            }
        }

        // Add import.hx files
        for import_file in &self.import_hx_files {
            if let Some(ref source) = import_file.input {
                source_map.add_file(import_file.filename.clone(), source.clone());
            }
        }

        // Add user files
        for user_file in &self.user_files {
            if let Some(ref source) = user_file.input {
                source_map.add_file(user_file.filename.clone(), source.clone());
            }
        }

        let formatter = ErrorFormatter::with_colors();

        for error in errors {
            let diagnostic = error.to_diagnostic(&source_map);
            let formatted = formatter.format_diagnostic(&diagnostic, &source_map);
            eprint!("{}", formatted);
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        let cache_dir = self.config.get_cache_dir();
        let mut stats = CacheStats::default();

        if !cache_dir.exists() {
            return stats;
        }

        // Count .blade files and calculate total size
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if entry.path().extension().and_then(|s| s.to_str()) == Some("blade") {
                        stats.cached_modules += 1;
                        stats.total_size_bytes += metadata.len();
                    }
                }
            }
        }

        stats
    }
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub cached_modules: usize,
    pub total_size_bytes: u64,
}

impl CacheStats {
    pub fn total_size_mb(&self) -> f64 {
        self.total_size_bytes as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compilation_unit_with_stdlib() {
        let mut unit = CompilationUnit::new(CompilationConfig::default());

        // Load stdlib
        unit.load_stdlib().expect("Failed to load stdlib");

        // Verify stdlib files were loaded
        assert!(unit.stdlib_files.len() > 0, "No stdlib files loaded");
        assert_eq!(unit.user_files.len(), 0, "Should have no user files");
    }

    #[test]
    fn test_compilation_unit_add_user_file() {
        let mut unit = CompilationUnit::new(CompilationConfig::default());

        let source = r#"
            package test;
            class MyClass {
                public function new() {}
            }
        "#;

        unit.add_file(source, "MyClass.hx").expect("Failed to add file");

        assert_eq!(unit.user_files.len(), 1);
        assert_eq!(unit.stdlib_files.len(), 0);
    }

    #[test]
    fn test_compilation_unit_full_pipeline() {
        let mut unit = CompilationUnit::new(CompilationConfig::default());

        // Load stdlib first
        unit.load_stdlib().expect("Failed to load stdlib");

        // Add user file
        let source = r#"
            package test;
            class MyClass {
                public function new() {}

                public function useArray():Void {
                    var arr = [1, 2, 3];
                    arr.push(4);
                }
            }
        "#;

        unit.add_file(source, "MyClass.hx").expect("Failed to add file");

        // Lower to TAST - this should succeed now with proper stdlib propagation
        let typed_files = unit.lower_to_tast().expect("Failed to lower to TAST");

        assert!(typed_files.len() > 0, "Should have typed files");
    }
}
