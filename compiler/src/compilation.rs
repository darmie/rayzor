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
use crate::ir::{IrModule, IrInstruction, Monomorphizer, blade::{save_blade, load_blade, BladeMetadata}};
use parser::{HaxeFile, parse_haxe_file, parse_haxe_file_with_debug};
use std::rc::Rc;
use std::cell::RefCell;
use std::path::{PathBuf, Path};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, HashSet};
use log::{debug, info, warn, trace};

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

    /// Cache of types that failed to load on-demand (to avoid repeated attempts)
    pub failed_type_loads: HashSet<String>,

    /// Cache of files that have been successfully compiled (to avoid redundant recompilation)
    /// Maps filename to the TypedFile result
    compiled_files: HashMap<String, TypedFile>,

    /// Internal compilation pipeline (delegates to HaxeCompilationPipeline)
    pipeline: HaxeCompilationPipeline,

    /// MIR modules generated during compilation (collected from pipeline results)
    mir_modules: Vec<std::sync::Arc<crate::ir::IrModule>>,

    /// Stdlib typed files loaded on-demand (typedefs, etc. that need to be in HIR)
    loaded_stdlib_typed_files: Vec<TypedFile>,

    /// Mapping from HIR function symbols to MIR function IDs for stdlib functions
    /// This allows user code to call pure Haxe stdlib functions (like StringTools)
    stdlib_function_map: HashMap<crate::tast::SymbolId, crate::ir::IrFunctionId>,

    /// Name-based mapping from qualified function names to MIR function IDs
    /// This is used for cross-file lookups where SymbolIds differ between compilation units
    /// e.g., "StringTools.startsWith" -> IrFunctionId(N)
    stdlib_function_name_map: HashMap<String, crate::ir::IrFunctionId>,
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
                "StdTypes.hx".to_string(), // Contains Iterator typedef
                "String.hx".to_string(),
                "Array.hx".to_string(),
                // Concurrent types
                "rayzor/concurrent/Thread.hx".to_string(),
                "rayzor/concurrent/Channel.hx".to_string(),
                "rayzor/concurrent/Mutex.hx".to_string(),
                "rayzor/concurrent/Arc.hx".to_string(),
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
                info!("Found stdlib at HAXE_STD_PATH: {}", haxe_std_path);
                paths.push(path);
                return paths; // Use this path exclusively if set
            } else {
                warn!("HAXE_STD_PATH set but directory doesn't exist: {}", haxe_std_path);
            }
        }

        // 2. Check HAXE_HOME/std
        if let Ok(haxe_home) = std::env::var("HAXE_HOME") {
            let std_path = PathBuf::from(&haxe_home).join("std");
            if std_path.exists() {
                info!("Found stdlib at HAXE_HOME/std: {:?}", std_path);
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
            warn!("No standard library found. Set HAXE_STD_PATH environment variable.");
            warn!("         or install Haxe to a standard location.");
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
            failed_type_loads: HashSet::new(),
            compiled_files: HashMap::new(),
            pipeline,
            mir_modules: Vec::new(),
            loaded_stdlib_typed_files: Vec::new(),
            stdlib_function_map: HashMap::new(),
            stdlib_function_name_map: HashMap::new(),
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

        // Configure namespace resolver with stdlib paths for on-demand loading
        self.namespace_resolver.set_stdlib_paths(self.config.stdlib_paths.clone());

        // DON'T load stdlib files upfront - rely entirely on on-demand loading
        // Files will be loaded via load_import_file() when imports or qualified names are encountered
        debug!("Stdlib configured for pure on-demand loading (no files loaded at startup)");

        Ok(())
    }

    /// Set source paths for user code (for on-demand import loading)
    /// These paths are checked first when resolving imports
    pub fn set_source_paths(&mut self, paths: Vec<PathBuf>) {
        self.namespace_resolver.set_source_paths(paths);
    }

    // === BLADE Caching Methods ===

    /// Get the BLADE cache path for a source file
    fn blade_cache_path(&self, source_path: &str) -> Option<PathBuf> {
        let cache_dir = self.config.cache_dir.as_ref()?;

        // Convert source path to a cache-safe filename
        // e.g., "compiler/haxe-std/haxe/io/Bytes.hx" -> "haxe.io.Bytes.blade"
        let module_name = source_path
            .replace('/', ".")
            .replace('\\', ".")
            .replace(".hx", "")
            .split('.')
            .skip_while(|s| *s == "compiler" || *s == "haxe-std" || s.is_empty())
            .collect::<Vec<_>>()
            .join(".");

        if module_name.is_empty() {
            return None;
        }

        Some(cache_dir.join(format!("{}.blade", module_name)))
    }

    /// Compute hash of source content for cache validation
    fn hash_source(source: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    }

    /// Try to load a cached MIR module from BLADE cache
    /// Returns Some(IrModule) if cache is valid, None otherwise
    fn try_load_blade_cached(&self, source_path: &str, source: &str) -> Option<IrModule> {
        if !self.config.enable_cache {
            return None;
        }

        let blade_path = self.blade_cache_path(source_path)?;
        if !blade_path.exists() {
            trace!("[BLADE] Cache miss (no file): {}", source_path);
            return None;
        }

        match load_blade(&blade_path) {
            Ok((mir, metadata)) => {
                // Validate cache by checking source hash
                let current_hash = Self::hash_source(source);
                if metadata.source_hash == current_hash {
                    debug!("[BLADE] Cache hit: {} -> {}", source_path, blade_path.display());
                    Some(mir)
                } else {
                    trace!("[BLADE] Cache stale (hash mismatch): {}", source_path);
                    None
                }
            }
            Err(e) => {
                trace!("[BLADE] Cache read error for {}: {}", source_path, e);
                None
            }
        }
    }

    /// Save a MIR module to BLADE cache
    fn save_blade_cached(&self, source_path: &str, source: &str, mir: &IrModule, dependencies: Vec<String>) {
        if !self.config.enable_cache {
            return;
        }

        let blade_path = match self.blade_cache_path(source_path) {
            Some(p) => p,
            None => return,
        };

        // Ensure cache directory exists
        if let Some(parent) = blade_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    trace!("[BLADE] Failed to create cache dir: {}", e);
                    return;
                }
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let metadata = BladeMetadata {
            name: mir.name.clone(),
            source_path: source_path.to_string(),
            source_hash: Self::hash_source(source),
            source_timestamp: now, // We use hash for validation, not timestamp
            compile_timestamp: now,
            dependencies,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        match save_blade(&blade_path, mir, metadata) {
            Ok(()) => {
                debug!("[BLADE] Cached: {} -> {}", source_path, blade_path.display());
            }
            Err(e) => {
                trace!("[BLADE] Failed to cache {}: {}", source_path, e);
            }
        }
    }

    /// Load imports efficiently by pre-collecting all dependencies and compiling in topological order.
    /// This avoids the fail-retry pattern that causes exponential recompilation.
    pub fn load_imports_efficiently(&mut self, imports: &[String]) -> Result<(), String> {
        use std::collections::{HashMap, HashSet, VecDeque};

        // Step 1: Collect all files and their dependencies by parsing (not compiling)
        let mut all_files: HashMap<String, (PathBuf, String, Vec<String>)> = HashMap::new(); // path -> (filepath, source, deps)
        let mut to_process: VecDeque<String> = imports.iter().cloned().collect();
        let mut visited: HashSet<String> = HashSet::new();

        while let Some(qualified_path) = to_process.pop_front() {
            if visited.contains(&qualified_path) {
                continue;
            }
            visited.insert(qualified_path.clone());

            // Resolve to file path
            let file_path = if let Some(path) = self.namespace_resolver.resolve_qualified_path_to_file(&qualified_path) {
                path
            } else if !qualified_path.contains('.') {
                // Try common prefixes for unqualified names
                let prefixes = ["haxe.iterators", "haxe.ds", "haxe", "sys.thread", "sys", "haxe.exceptions", "haxe.io"];
                let mut found = None;
                for prefix in &prefixes {
                    let full = format!("{}.{}", prefix, qualified_path);
                    if let Some(path) = self.namespace_resolver.resolve_qualified_path_to_file(&full) {
                        found = Some(path);
                        break;
                    }
                }
                match found {
                    Some(p) => p,
                    None => continue, // Skip unresolvable imports
                }
            } else {
                continue; // Skip unresolvable
            };

            // Skip if already loaded
            if self.namespace_resolver.is_file_loaded(&file_path) {
                continue;
            }

            // Read and parse to extract imports
            let source = match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let filename = file_path.to_string_lossy().to_string();
            let deps = match parser::parse_haxe_file(&filename, &source, false) {
                Ok(ast) => {
                    ast.imports.iter()
                        .filter_map(|imp| {
                            if !imp.path.is_empty() {
                                Some(imp.path.join("."))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                }
                Err(_) => Vec::new(),
            };

            // Queue dependencies for processing
            for dep in &deps {
                if !visited.contains(dep) {
                    to_process.push_back(dep.clone());
                }
            }

            all_files.insert(qualified_path.clone(), (file_path, source, deps));
        }

        // Step 2: Topological sort using Kahn's algorithm
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for (name, (_, _, deps)) in &all_files {
            in_degree.entry(name.clone()).or_insert(0);
            for dep in deps {
                if all_files.contains_key(dep) {
                    graph.entry(dep.clone()).or_default().push(name.clone());
                    *in_degree.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut compile_order: Vec<String> = Vec::new();

        while let Some(name) = queue.pop_front() {
            compile_order.push(name.clone());
            if let Some(dependents) = graph.get(&name) {
                for dep in dependents {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        // Step 3: Compile in topological order (no retries needed!)
        for name in compile_order {
            if let Some((file_path, source, _)) = all_files.remove(&name) {
                // Skip if already compiled
                let filename = file_path.to_string_lossy().to_string();
                if self.compiled_files.contains_key(&filename) {
                    continue;
                }

                // Mark as loaded
                self.namespace_resolver.mark_file_loaded(file_path);

                // Compile - should succeed on first try since deps are already compiled
                match self.compile_file_with_shared_state(&filename, &source) {
                    Ok(typed_file) => {
                        self.loaded_stdlib_typed_files.push(typed_file);
                    }
                    Err(_) => {
                        // If it still fails, fall back to old retry mechanism
                        // This handles edge cases like Placeholder typedefs
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single file on-demand for import resolution (legacy - uses retry pattern)
    /// Prefer load_imports_efficiently for batch loading
    pub fn load_import_file(&mut self, qualified_path: &str) -> Result<(), String> {
        self.load_import_file_recursive(qualified_path, 0)
    }

    /// Internal recursive function for loading files with dependency resolution
    /// Max depth prevents infinite loops in circular dependencies
    fn load_import_file_recursive(&mut self, qualified_path: &str, depth: usize) -> Result<(), String> {
        const MAX_DEPTH: usize = 10;

        if depth > MAX_DEPTH {
            return Err(format!("Maximum dependency depth ({}) exceeded for: {}", MAX_DEPTH, qualified_path));
        }

        // Resolve the qualified path to a filesystem path
        // If not found directly, try common stdlib package prefixes for unqualified names
        let file_path = if let Some(path) = self.namespace_resolver.resolve_qualified_path_to_file(qualified_path) {
            path
        } else if !qualified_path.contains('.') {
            // Unqualified name - try common stdlib packages
            let prefixes = vec!["haxe.iterators", "haxe.ds", "haxe", "sys.thread", "sys", "haxe.exceptions", "haxe.io"];
            let mut found_path = None;
            for prefix in &prefixes {
                let qualified = format!("{}.{}", prefix, qualified_path);
                if let Some(path) = self.namespace_resolver.resolve_qualified_path_to_file(&qualified) {
                    found_path = Some(path);
                    break;
                }
            }
            found_path.ok_or_else(|| format!("Could not resolve import: {}", qualified_path))?
        } else {
            return Err(format!("Could not resolve import: {}", qualified_path));
        };

        // Skip if already loaded - this prevents redundant re-compilation
        if self.namespace_resolver.is_file_loaded(&file_path) {
            return Ok(());
        }

        let load_start = std::time::Instant::now();

        // Mark as loaded BEFORE compiling to prevent recursive loading
        self.namespace_resolver.mark_file_loaded(file_path.clone());

        // Read the file
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;

        let filename = file_path.to_string_lossy().to_string();

        // Try to compile - if it fails due to missing dependencies, extract and load them
        match self.compile_file_with_shared_state(&filename, &source) {
            Ok(typed_file) => {
                debug!("  ✓ Successfully compiled and registered: {}", qualified_path);
                // Store typedef files so they're included in HIR conversion
                if !typed_file.type_aliases.is_empty() {
                    trace!("    (contains {} type aliases)", typed_file.type_aliases.len());
                }

                // Check if any type aliases have Placeholder targets that need to be loaded
                // This handles cases like `typedef Bytes = rayzor.Bytes` where rayzor.Bytes hasn't been loaded yet
                let mut placeholder_targets = Vec::new();
                {
                    let type_table = self.type_table.borrow();
                    for alias in &typed_file.type_aliases {
                        if let Some(target_info) = type_table.get(alias.target_type) {
                            if let crate::tast::TypeKind::Placeholder { name } = &target_info.kind {
                                if let Some(placeholder_name) = self.string_interner.get(*name) {
                                    trace!("    Found typedef with Placeholder target: {}", placeholder_name);
                                    placeholder_targets.push(placeholder_name.to_string());
                                }
                            }
                        }
                    }
                }

                // If we found Placeholder targets, try to load them and retry
                if !placeholder_targets.is_empty() {
                    let mut any_loaded = false;
                    for target in &placeholder_targets {
                        if let Ok(_) = self.load_import_file_recursive(target, depth + 1) {
                            debug!("    ✓ Loaded typedef target: {}", target);
                            any_loaded = true;
                        }
                    }

                    if any_loaded {
                        // Retry compilation after loading typedef targets
                        debug!("  Retrying compilation of {} after loading typedef targets...", qualified_path);
                        match self.compile_file_with_shared_state(&filename, &source) {
                            Ok(recompiled_file) => {
                                self.loaded_stdlib_typed_files.push(recompiled_file);
                                return Ok(());
                            }
                            Err(_) => {
                                // Fall through and push the original typed_file
                            }
                        }
                    }
                }

                self.loaded_stdlib_typed_files.push(typed_file);
                Ok(())
            },
            Err(errors) => {
                // Extract UnresolvedType errors and try to load those dependencies
                let mut missing_types = Vec::new();
                for error in &errors {
                    if let Some(type_name) = Self::extract_unresolved_type_from_error(&error.message) {
                        // Skip generic type parameters and built-in typedefs
                        if !Self::is_generic_type_parameter(&type_name) && !self.failed_type_loads.contains(&type_name) {
                            missing_types.push(type_name);
                        }
                    }
                }

                // If we found missing types, try to load them recursively
                if !missing_types.is_empty() {
                    debug!("  Detected {} missing dependencies for {}: {:?}",
                        missing_types.len(), qualified_path, missing_types);

                    let mut load_success = false;
                    for missing_type in &missing_types {
                        // Check if this looks like a field reference (e.g., "haxe.SysTools.winMetaCharacters")
                        // If so, extract just the class part (e.g., "haxe.SysTools")
                        let type_to_load = if let Some(last_dot) = missing_type.rfind('.') {
                            let after_dot = &missing_type[last_dot + 1..];
                            // If the part after the last dot starts with lowercase, it's likely a field
                            if after_dot.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                                &missing_type[..last_dot]
                            } else {
                                missing_type.as_str()
                            }
                        } else {
                            missing_type.as_str()
                        };

                        // Try loading with the (possibly adjusted) name first
                        let loaded = if let Ok(_) = self.load_import_file_recursive(type_to_load, depth + 1) {
                            debug!("    ✓ Loaded dependency: {}", type_to_load);
                            true
                        } else if !type_to_load.contains('.') {
                            // If unqualified name failed, try with common stdlib packages
                            let prefixes = vec!["haxe.exceptions.", "haxe.io.", "haxe.ds."];
                            let mut prefix_loaded = false;
                            for prefix in prefixes {
                                let qualified = format!("{}{}", prefix, type_to_load);
                                if let Ok(_) = self.load_import_file_recursive(&qualified, depth + 1) {
                                    debug!("    ✓ Loaded dependency: {} (as {})", type_to_load, qualified);
                                    prefix_loaded = true;
                                    break;
                                }
                            }
                            prefix_loaded
                        } else {
                            false
                        };

                        if loaded {
                            load_success = true;
                        } else {
                            debug!("    ✗ Could not load dependency: {}", missing_type);
                            self.failed_type_loads.insert(missing_type.clone());
                        }
                    }

                    // If we successfully loaded at least one dependency, retry compilation
                    if load_success {
                        debug!("  Retrying compilation of {} after loading dependencies...", qualified_path);
                        match self.compile_file_with_shared_state(&filename, &source) {
                            Ok(typed_file) => {
                                // Store typedef files so they're included in HIR conversion
                                if !typed_file.type_aliases.is_empty() {
                                    trace!("    (contains {} type aliases after retry)", typed_file.type_aliases.len());
                                }

                                // Check if any type aliases have Placeholder targets that need to be loaded
                                // This handles cases like `typedef Bytes = rayzor.Bytes` where rayzor.Bytes hasn't been loaded yet
                                let mut placeholder_targets = Vec::new();
                                {
                                    let type_table = self.type_table.borrow();
                                    for alias in &typed_file.type_aliases {
                                        if let Some(target_info) = type_table.get(alias.target_type) {
                                            if let crate::tast::TypeKind::Placeholder { name } = &target_info.kind {
                                                if let Some(placeholder_name) = self.string_interner.get(*name) {
                                                    trace!("    Found typedef with Placeholder target (after deps): {}", placeholder_name);
                                                    placeholder_targets.push(placeholder_name.to_string());
                                                }
                                            }
                                        }
                                    }
                                }

                                // If we found Placeholder targets, try to load them and retry again
                                if !placeholder_targets.is_empty() {
                                    let mut any_loaded = false;
                                    for target in &placeholder_targets {
                                        if let Ok(_) = self.load_import_file_recursive(target, depth + 1) {
                                            debug!("    ✓ Loaded typedef target (after deps): {}", target);
                                            any_loaded = true;
                                        }
                                    }

                                    if any_loaded {
                                        // Retry compilation after loading typedef targets
                                        debug!("  Retrying compilation of {} after loading typedef targets...", qualified_path);
                                        match self.compile_file_with_shared_state(&filename, &source) {
                                            Ok(recompiled_file) => {
                                                self.loaded_stdlib_typed_files.push(recompiled_file);
                                                return Ok(());
                                            }
                                            Err(_) => {
                                                // Fall through and push the original typed_file
                                            }
                                        }
                                    }
                                }

                                self.loaded_stdlib_typed_files.push(typed_file);
                                return Ok(());
                            }
                            Err(errors) => {
                                // Check if any errors are UnresolvedType that we can try to load
                                let mut additional_missing = Vec::new();
                                for error in &errors {
                                    if let Some(type_name) = Self::extract_unresolved_type_from_error(&error.message) {
                                        if !Self::is_generic_type_parameter(&type_name) && !self.failed_type_loads.contains(&type_name) {
                                            additional_missing.push(type_name);
                                        }
                                    }
                                }

                                if !additional_missing.is_empty() {
                                    let mut loaded_any = false;
                                    for missing in &additional_missing {
                                        if let Ok(_) = self.load_import_file_recursive(missing, depth + 1) {
                                            debug!("    ✓ Loaded additional dependency: {}", missing);
                                            loaded_any = true;
                                        }
                                    }

                                    if loaded_any {
                                        // Try one more time
                                        debug!("  Retrying compilation of {} after loading additional dependencies...", qualified_path);
                                        match self.compile_file_with_shared_state(&filename, &source) {
                                            Ok(final_file) => {
                                                self.loaded_stdlib_typed_files.push(final_file);
                                                return Ok(());
                                            }
                                            Err(final_errors) => {
                                                let error_msgs: Vec<String> = final_errors.iter()
                                                    .map(|e| e.message.clone())
                                                    .collect();
                                                return Err(format!("Errors compiling {} (after loading additional dependencies): {}", filename, error_msgs.join(", ")));
                                            }
                                        }
                                    }
                                }

                                let error_msgs: Vec<String> = errors.iter()
                                    .map(|e| e.message.clone())
                                    .collect();
                                return Err(format!("Errors compiling {} (after loading dependencies): {}", filename, error_msgs.join(", ")));
                            }
                        }
                    }
                }

                // No missing types found or couldn't load them - return original error
                let error_msgs: Vec<String> = errors.iter()
                    .map(|e| e.message.clone())
                    .collect();
                Err(format!("Errors compiling {}: {}", filename, error_msgs.join(", ")))
            }
        }
    }

    /// Extract type name from UnresolvedType error messages
    /// Returns Some(type_name) if this is an UnresolvedType error, None otherwise
    fn extract_unresolved_type_from_error(error_msg: &str) -> Option<String> {
        // Match pattern: "UnresolvedType { type_name: \"SomeType\", ..."
        // Find the start of type_name: \" marker
        if let Some(type_name_start) = error_msg.find("type_name: \"") {
            // Move past 'type_name: "' to get to the actual name
            let after_marker = &error_msg[type_name_start + 12..]; // 12 = length of 'type_name: "'
            // Find the closing quote
            if let Some(end) = after_marker.find('"') {
                return Some(after_marker[..end].to_string());
            }
        }
        None
    }

    /// Check if a type name looks like a generic type parameter
    /// Returns true for single letters (T, K, V) or common parameter patterns
    fn is_generic_type_parameter(type_name: &str) -> bool {
        // Single uppercase letter
        if type_name.len() == 1 && type_name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
            return true;
        }
        // Common generic parameter patterns
        matches!(type_name, "Key" | "Value" | "Item" | "Element" | "Iterator" | "KeyValueIterator" | "Iterable" | "KeyValueIterable")
    }

    /// Pre-register type declarations from a file without full compilation
    /// This is the first pass that registers class/interface/enum names in the namespace
    /// so they can be referenced by other files during full compilation
    fn pre_register_file_types(&mut self, filename: &str, source: &str) -> Result<(), String> {
        use crate::tast::ast_lowering::AstLowering;
        use parser::parse_haxe_file_with_diagnostics;

        // Parse the file
        let parse_result = parse_haxe_file_with_diagnostics(filename, source)
            .map_err(|e| format!("Parse error in {}: {}", filename, e))?;

        let ast_file = parse_result.file;

        // Create a temporary AstLowering instance just for pre-registration
        let dummy_interner_rc = Rc::new(RefCell::new(StringInterner::new()));

        let mut lowering = AstLowering::new(
            &mut self.string_interner,
            dummy_interner_rc,
            &mut self.symbol_table,
            &self.type_table,
            &mut self.scope_tree,
            &mut self.namespace_resolver,
            &mut self.import_resolver,
        );

        // Pre-register only - call the pre_register_file method
        lowering.pre_register_file(&ast_file)
            .map_err(|e| format!("Pre-registration error in {}: {:?}", filename, e))?;

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
            debug!("⚠️  Warning: Circular dependencies detected!");
            for (i, cycle) in analysis.circular_dependencies.iter().enumerate() {
                debug!("\nCycle #{}:", i + 1);
                debug!("{}", cycle.format_error());
            }
            debug!("\nCompilation will proceed with best-effort ordering.\n");
        }

        Ok(analysis)
    }

    /// Compile a single file using shared state (string interner, symbol table, namespace resolver, etc.)
    /// This ensures symbols from different files can see each other
    ///
    /// If `skip_pre_registration` is true, assumes types have already been pre-registered
    /// and skips the first pass in lower_file.
    fn compile_file_with_shared_state_ex(
        &mut self,
        filename: &str,
        source: &str,
        skip_pre_registration: bool
    ) -> Result<TypedFile, Vec<CompilationError>> {
        use crate::tast::ast_lowering::AstLowering;
        use parser::parse_haxe_file_with_diagnostics;

        // Skip if already successfully compiled - return cached TypedFile
        if let Some(cached) = self.compiled_files.get(filename) {
            return Ok(cached.clone());
        }

        // Parse the file
        let parse_result = parse_haxe_file_with_diagnostics(filename, source)
            .map_err(|e| vec![CompilationError {
                message: format!("Parse error: {}", e),
                location: SourceLocation::unknown(),
                category: ErrorCategory::ParseError,
                suggestion: None,
                related_errors: Vec::new(),
            }])?;

        let ast_file = parse_result.file;
        let _source_map = parse_result.source_map;
        let file_id = diagnostics::FileId::new(0);

        // Lower to TAST using the SHARED state
        // NOTE: AstLowering needs an Rc<RefCell<StringInterner>> for TypedFile
        // We create a dummy one here - the actual interning happens via the &mut reference
        // TODO: Refactor CompilationUnit to store string_interner as Rc<RefCell<>> from the start
        let dummy_interner_rc = Rc::new(RefCell::new(StringInterner::new()));

        let mut lowering = AstLowering::new(
            &mut self.string_interner,
            dummy_interner_rc,
            &mut self.symbol_table,
            &self.type_table,
            &mut self.scope_tree,
            &mut self.namespace_resolver,
            &mut self.import_resolver,
        );

        // Skip pre-registration if requested (types already registered by CompilationUnit)
        lowering.set_skip_pre_registration(skip_pre_registration);

        lowering.initialize_span_converter_with_filename(
            file_id.as_usize() as u32,
            source.to_string(),
            filename.to_string(),
        );

        let typed_file = lowering.lower_file(&ast_file)
            .map_err(|e| vec![CompilationError {
                message: format!("Lowering error: {:?}", e),
                location: SourceLocation::unknown(),
                category: ErrorCategory::TypeError,
                suggestion: None,
                related_errors: Vec::new(),
            }])?;

        // Lower to HIR
        use crate::ir::tast_to_hir::lower_tast_to_hir;
        let hir_module = lower_tast_to_hir(
            &typed_file,
            &self.symbol_table,
            &self.type_table,
            &mut self.string_interner,
            None, // No semantic graphs for now
        ).map_err(|errors| {
            errors.into_iter().map(|e| CompilationError {
                message: format!("HIR lowering error: {:?}", e),
                location: SourceLocation::unknown(),
                category: ErrorCategory::InternalError,
                suggestion: None,
                related_errors: Vec::new(),
            }).collect::<Vec<_>>()
        })?;

        // Lower to MIR
        // Use lower_hir_to_mir_with_function_map to:
        // 1. Pass external function references from previously compiled stdlib files
        // 2. Collect function mappings for stdlib files so user code can call them
        use crate::ir::hir_to_mir::lower_hir_to_mir_with_function_map;

        // Check if this is a stdlib file BEFORE lowering so we can decide whether
        // to collect function mappings
        let is_stdlib_file = filename.contains("haxe-std") ||
                              filename.contains("/haxe-std/") ||
                              filename.contains("\\haxe-std\\");

        debug!("DEBUG: [MIR LOWERING] filename='{}', is_stdlib_file={}", filename, is_stdlib_file);

        // For user files, pass the stdlib function map so they can call stdlib functions
        // For stdlib files, pass an empty map (they can call each other once we accumulate the map)
        let external_functions = if is_stdlib_file {
            // Stdlib files can call previously compiled stdlib functions
            self.stdlib_function_map.clone()
        } else {
            // User files can call all compiled stdlib functions
            self.stdlib_function_map.clone()
        };

        // Name-based external function map for cross-file lookups where SymbolIds differ
        let external_functions_by_name = self.stdlib_function_name_map.clone();

        let mir_result = lower_hir_to_mir_with_function_map(
            &hir_module,
            &self.string_interner,
            &self.type_table,
            &self.symbol_table,
            external_functions,
            external_functions_by_name,
        ).map_err(|errors| {
            errors.into_iter().map(|e| CompilationError {
                message: format!("MIR lowering error: {:?}", e),
                location: SourceLocation::unknown(),
                category: ErrorCategory::InternalError,
                suggestion: None,
                related_errors: Vec::new(),
            }).collect::<Vec<_>>()
        })?;

        let mut mir_module = mir_result.module;

        // If this is a stdlib file, collect its function mappings
        if is_stdlib_file {
            debug!("DEBUG: Collecting {} function mappings from stdlib file: {}",
                      mir_result.function_map.len(), filename);
            for (symbol_id, func_id) in mir_result.function_map {
                self.stdlib_function_map.insert(symbol_id, func_id);
            }

            // Also collect name-based mappings for cross-file lookups
            // The function `name` field contains the qualified name (e.g., "StringTools.startsWith")
            let mut added_count = 0;
            let mut skipped_count = 0;
            for (func_id, func) in &mir_module.functions {
                // Only add non-empty CFG functions (skip forward refs/stubs)
                if !func.cfg.blocks.is_empty() {
                    self.stdlib_function_name_map.insert(func.name.clone(), *func_id);
                    debug!("DEBUG: [NAME MAP] Added '{}' -> {:?}", func.name, func_id);
                    added_count += 1;
                } else {
                    debug!("DEBUG: [NAME MAP SKIP] '{}' has empty CFG (forward ref/stub)", func.name);
                    skipped_count += 1;
                }
            }
            debug!("DEBUG: [NAME MAP] {} added, {} skipped for {}", added_count, skipped_count, filename);
        }

        // Only skip EXTERN stdlib files (those with Rust implementations in build_stdlib).
        // Pure Haxe stdlib files (like ArrayIterator) must compile when imported.
        let is_extern_stdlib_file =
            filename.contains("rayzor/concurrent/") ||
            filename.contains("rayzor\\concurrent\\") ||  // Windows compatibility
            // Also skip core types that have Rust implementations
            (filename.contains("haxe-std") && (
                filename.ends_with("/String.hx") ||
                filename.ends_with("\\String.hx") ||
                filename.ends_with("/Array.hx") ||
                filename.ends_with("\\Array.hx")
            ));

        if is_extern_stdlib_file {
            debug!("DEBUG: Skipping MIR module creation for EXTERN stdlib file: {}", filename);
            // Extern stdlib files have Rust implementations in build_stdlib().
            // The Haxe files are just type declarations.
            self.compiled_files.insert(filename.to_string(), typed_file.clone());
            return Ok(typed_file);
        }

        // Merge stdlib MIR (extern functions for Thread, Channel, Mutex, Arc, etc.)
        // This ensures extern runtime functions are available
        use crate::stdlib::build_stdlib;
        let mut stdlib_mir = build_stdlib();

        // DEBUG: Check a specific extern function signature before renumbering
        for (func_id, func) in &stdlib_mir.functions {
            if func.name == "rayzor_channel_init" {
                debug!("DEBUG: BEFORE renumbering - rayzor_channel_init (ID {}): params={}, extern={}",
                          func_id.0, func.signature.parameters.len(), func.cfg.blocks.is_empty());
            }
        }

        // CRITICAL FIX: Renumber stdlib function IDs to avoid collisions with user functions
        // Each MIR module starts function IDs from 0, so when merging stdlib and user modules,
        // IDs will collide. For example:
        //   - User module: IrFunctionId(2) = "indexOf"
        //   - Stdlib module: IrFunctionId(2) = "free"
        // Without renumbering, stdlib's "free" would be skipped, causing vec_u8_free to call "indexOf"!

        // DEBUG: Print user functions before merging
        debug!("DEBUG: User module has {} functions before merging:", mir_module.functions.len());
        let mut user_func_ids: Vec<_> = mir_module.functions.keys().collect();
        user_func_ids.sort_by_key(|id| id.0);
        for func_id in user_func_ids.iter().take(5) {
            let func = &mir_module.functions[func_id];
            debug!("  - User IrFunctionId({}) = '{}'", func_id.0, func.name);
        }

        // Find the maximum function ID in the user module
        let max_user_func_id = mir_module.functions.keys()
            .map(|id| id.0)
            .max()
            .unwrap_or(0);

        let max_user_extern_id = mir_module.extern_functions.keys()
            .map(|id| id.0)
            .max()
            .unwrap_or(0);

        let offset = std::cmp::max(max_user_func_id, max_user_extern_id) + 1;

        debug!("DEBUG: Renumbering stdlib functions with offset {} (max_user_func={}, max_user_extern={})",
                  offset, max_user_func_id, max_user_extern_id);

        // Build mapping of old stdlib IDs to new renumbered IDs
        use std::collections::HashMap;
        use crate::ir::IrFunctionId;
        let mut id_mapping: HashMap<IrFunctionId, IrFunctionId> = HashMap::new();

        // Note: extern_functions is not used - externs are in the functions map with empty CFGs
        // So we only need to renumber the functions map

        // FIRST PASS: Build complete ID mapping for all stdlib functions
        // We must do this BEFORE updating CallDirect instructions so that all IDs are available
        for (old_id, _) in &stdlib_mir.functions {
            let new_id = IrFunctionId(old_id.0 + offset);
            id_mapping.insert(*old_id, new_id);
        }

        // SECOND PASS: Renumber functions and update their internal references
        let mut renumbered_functions = HashMap::new();
        for (old_id, mut func) in stdlib_mir.functions {
            let new_id = *id_mapping.get(&old_id).unwrap();

            // Update the function's own ID
            func.id = new_id;

            // Update all CallDirect instructions that reference old stdlib function IDs
            use crate::ir::IrInstruction;
            for block in func.cfg.blocks.values_mut() {
                for inst in &mut block.instructions {
                    if let IrInstruction::CallDirect { func_id, .. } = inst {
                        if let Some(&new_func_id) = id_mapping.get(func_id) {
                            debug!("DEBUG: Updated CallDirect in {} from func_id {} -> {}",
                                      func.name, func_id.0, new_func_id.0);
                            *func_id = new_func_id;
                        }
                    }
                }
            }

            renumbered_functions.insert(new_id, func);
            debug!("DEBUG: Renumbered function '{}': {} -> {}",
                      renumbered_functions[&new_id].name, old_id.0, new_id.0);
        }

        // Merge renumbered stdlib functions - no collisions possible now!
        // (Note: extern functions are included in the functions map with empty CFGs)
        //
        // IMPORTANT: Replace user functions that have the same NAME as stdlib functions
        // The user module might have extern declarations (e.g. rayzor_channel_init) from
        // the lowering process, but these might have incorrect signatures due to type
        // inference issues. The stdlib version is the source of truth, so we REPLACE
        // the user's version with the stdlib's version.

        // Build map of function names to IDs in the user module (before merging)
        let mut user_func_name_to_id: HashMap<String, IrFunctionId> = HashMap::new();
        for (func_id, func) in &mir_module.functions {
            user_func_name_to_id.insert(func.name.clone(), *func_id);
        }

        // Build a map of old ID -> new ID for all replacements
        // This must be done BEFORE we start modifying the module
        let mut id_replacements: HashMap<IrFunctionId, IrFunctionId> = HashMap::new();

        for (func_id, func) in &renumbered_functions {
            if let Some(&existing_id) = user_func_name_to_id.get(&func.name) {
                debug!("DEBUG: Will replace user function '{}' (ID {}) with stdlib version (ID {})",
                          func.name, existing_id.0, func_id.0);
                id_replacements.insert(existing_id, *func_id);
            }
        }

        debug!("DEBUG: ID replacement map has {} entries:", id_replacements.len());
        for (old_id, new_id) in &id_replacements {
            if let Some(func) = mir_module.functions.get(old_id) {
                debug!("  {} (ID {}) -> ID {}", func.name, old_id.0, new_id.0);
            } else {
                debug!("  (unknown) ID {} -> ID {}", old_id.0, new_id.0);
            }
        }

        // Now merge the stdlib functions
        for (func_id, func) in renumbered_functions {
            // DEBUG: Check signature after renumbering
            if func.name == "rayzor_channel_init" {
                debug!("DEBUG: AFTER renumbering - rayzor_channel_init (ID {}): params={}, extern={}",
                          func_id.0, func.signature.parameters.len(), func.cfg.blocks.is_empty());
            }

            // If this function replaces an existing one, remove the old one
            if let Some(&existing_id) = user_func_name_to_id.get(&func.name) {
                mir_module.functions.remove(&existing_id);
            }

            mir_module.functions.insert(func_id, func);
        }

        // Update ALL instructions that reference replaced function IDs
        // This is done AFTER all merging to avoid ID conflicts
        if !id_replacements.is_empty() {
            for (_, caller_func) in mir_module.functions.iter_mut() {
                for block in caller_func.cfg.blocks.values_mut() {
                    for instr in &mut block.instructions {
                        match instr {
                            IrInstruction::CallDirect { func_id: ref mut called_func_id, .. } => {
                                if let Some(&new_id) = id_replacements.get(called_func_id) {
                                    debug!("DEBUG: Updated CallDirect in {} from func_id {} -> {}",
                                              caller_func.name, called_func_id.0, new_id.0);
                                    *called_func_id = new_id;
                                }
                            }
                            IrInstruction::FunctionRef { func_id: ref mut ref_func_id, .. } => {
                                if let Some(&new_id) = id_replacements.get(ref_func_id) {
                                    debug!("DEBUG: Updated FunctionRef in {} from func_id {} -> {}",
                                              caller_func.name, ref_func_id.0, new_id.0);
                                    *ref_func_id = new_id;
                                }
                            }
                            IrInstruction::MakeClosure { func_id: ref mut closure_func_id, .. } => {
                                if let Some(&new_id) = id_replacements.get(closure_func_id) {
                                    debug!("DEBUG: Updated MakeClosure in {} from func_id {} -> {}",
                                              caller_func.name, closure_func_id.0, new_id.0);
                                    *closure_func_id = new_id;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // DEBUG: Print all function IDs in the merged module
        debug!("DEBUG: Merged module has {} functions:", mir_module.functions.len());
        let mut func_ids: Vec<_> = mir_module.functions.keys().collect();
        func_ids.sort_by_key(|id| id.0);
        for func_id in func_ids.iter().take(10) {  // Print first 10
            let func = &mir_module.functions[func_id];
            debug!("  - IrFunctionId({}) = '{}' (extern: {})",
                      func_id.0, func.name, func.cfg.blocks.is_empty());
        }

        // Run monomorphization pass to specialize generic functions
        let mut monomorphizer = Monomorphizer::new();
        monomorphizer.monomorphize_module(&mut mir_module);
        let mono_stats = monomorphizer.stats();
        if mono_stats.generic_functions_found > 0 || mono_stats.instantiations_created > 0 {
            debug!("DEBUG: Monomorphization stats: {} generic functions, {} instantiations, {} call sites rewritten",
                      mono_stats.generic_functions_found,
                      mono_stats.instantiations_created,
                      mono_stats.call_sites_rewritten);
        }

        // Store the MIR module
        self.mir_modules.push(std::sync::Arc::new(mir_module));

        // Mark as successfully compiled to prevent redundant recompilation
        self.compiled_files.insert(filename.to_string(), typed_file.clone());

        Ok(typed_file)
    }

    /// Compile a single file using shared state (backward-compatible wrapper)
    fn compile_file_with_shared_state(&mut self, filename: &str, source: &str) -> Result<TypedFile, Vec<CompilationError>> {
        self.compile_file_with_shared_state_ex(filename, source, false)
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
    /// On-demand loading: If a type is unresolved, attempts to load and compile
    /// the file that should contain it based on qualified path resolution.
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

        // Step 2: Pre-load stdlib files for explicit imports AND using statements in user files
        // This ensures typedefs like sys.FileStat are available before compilation
        // Also handles root-level imports like "import StringTools;" and "using StringTools;"
        let (imports_to_load, usings_to_load): (Vec<String>, Vec<String>) = self.user_files.iter()
            .filter_map(|file| file.input.as_ref().map(|source| (file.filename.clone(), source.clone())))
            .fold((Vec::new(), Vec::new()), |(mut imports, mut usings), (filename, source)| {
                if let Ok(ast) = parser::parse_haxe_file(&filename, &source, false) {
                    // Collect imports
                    for import in &ast.imports {
                        if !import.path.is_empty() {
                            imports.push(import.path.join("."));
                        }
                    }
                    // Collect using statements (static extensions)
                    for using in &ast.using {
                        if !using.path.is_empty() {
                            usings.push(using.path.join("."));
                        }
                    }
                }
                (imports, usings)
            });

        // Pre-load imports using efficient topological loading (avoids retry loops)
        let mut all_imports = imports_to_load;
        all_imports.extend(usings_to_load);
        let _ = self.load_imports_efficiently(&all_imports);

        // Step 3: Compile import.hx files using SHARED state
        let import_sources: Vec<(String, String)> = self.import_hx_files.iter()
            .filter_map(|f| f.input.as_ref().map(|s| (f.filename.clone(), s.clone())))
            .collect();

        for (filename, source) in import_sources {
            match self.compile_file_with_shared_state(&filename, &source) {
                Ok(typed_file) => {
                    all_typed_files.push(typed_file);
                }
                Err(errors) => {
                    all_errors.extend(errors);
                }
            }
        }

        // Step 4: Compile user files in dependency order using SHARED state
        // This ensures user files can see symbols from stdlib and other user files
        let user_sources: Vec<(String, String)> = analysis.compilation_order.iter()
            .filter_map(|&idx| {
                let file = &self.user_files[idx];
                file.input.as_ref().map(|s| (file.filename.clone(), s.clone()))
            })
            .collect();

        for (filename, source) in user_sources {
            match self.compile_file_with_shared_state(&filename, &source) {
                Ok(typed_file) => {
                    all_typed_files.push(typed_file);
                }
                Err(errors) => {
                    // Check if any errors are unresolved types that we can try to load on-demand
                    let (loadable, other): (Vec<_>, Vec<_>) = errors.into_iter().partition(|e| {
                        e.message.contains("Unresolved type") || e.message.contains("UnresolvedType")
                    });

                    // Try to load unresolved types on-demand
                    let mut any_loaded = false;
                    for error in loadable {
                        if let Some(type_name) = self.extract_type_name_from_error(&error.message) {
                            // Skip if we already tried to load this type and it failed
                            if self.failed_type_loads.contains(&type_name) {
                                all_errors.push(error);
                                continue;
                            }
                            if let Err(load_err) = self.load_import_file(&type_name) {
                                debug!("On-demand load failed for {}: {}", type_name, load_err);
                                self.failed_type_loads.insert(type_name.clone());
                                all_errors.push(error);
                            } else {
                                // Successfully loaded! Mark that we should retry
                                any_loaded = true;
                            }
                        } else {
                            all_errors.push(error);
                        }
                    }

                    // If we successfully loaded any dependencies, retry compiling this file
                    if any_loaded {
                        debug!("  Retrying {} after loading dependencies...", filename);
                        match self.compile_file_with_shared_state(&filename, &source) {
                            Ok(typed_file) => {
                                all_typed_files.push(typed_file);
                            }
                            Err(retry_errors) => {
                                // Still failed after loading dependencies
                                // Check if retry revealed NEW unresolved types that need loading
                                let (retry_loadable, retry_other): (Vec<_>, Vec<_>) = retry_errors.into_iter().partition(|e| {
                                    e.message.contains("Unresolved type") || e.message.contains("UnresolvedType")
                                });

                                let mut retry_loaded = false;
                                for error in retry_loadable {
                                    if let Some(type_name) = self.extract_type_name_from_error(&error.message) {
                                        if !self.failed_type_loads.contains(&type_name) {
                                            if let Err(load_err) = self.load_import_file(&type_name) {
                                                debug!("On-demand load failed for {}: {}", type_name, load_err);
                                                self.failed_type_loads.insert(type_name.clone());
                                                all_errors.push(error);
                                            } else {
                                                retry_loaded = true;
                                            }
                                        } else {
                                            all_errors.push(error);
                                        }
                                    } else {
                                        all_errors.push(error);
                                    }
                                }

                                // If we loaded more dependencies on retry, try ONE more time
                                if retry_loaded {
                                    debug!("  Second retry of {} after loading more dependencies...", filename);
                                    match self.compile_file_with_shared_state(&filename, &source) {
                                        Ok(typed_file) => {
                                            all_typed_files.push(typed_file);
                                        }
                                        Err(final_errors) => {
                                            all_errors.extend(final_errors);
                                        }
                                    }
                                } else {
                                    all_errors.extend(retry_other);
                                }
                            }
                        }
                    } else {
                        // No dependencies loaded, keep original errors
                        all_errors.extend(other);
                    }
                }
            }
        }

        // Step 5: Report all errors if any were found
        if !all_errors.is_empty() {
            self.print_compilation_errors(&all_errors);
            return Err(all_errors);
        }

        // Step 6: Include loaded stdlib files (typedefs, etc.) in the result
        // These were loaded on-demand during import resolution and contain type aliases
        // that need to be processed by HIR
        for stdlib_file in std::mem::take(&mut self.loaded_stdlib_typed_files) {
            all_typed_files.push(stdlib_file);
        }

        Ok(all_typed_files)
    }

    /// Extract the type name from an unresolved type error message
    fn extract_type_name_from_error(&self, message: &str) -> Option<String> {
        // Try to extract type name from error message formats:
        // "UnresolvedType { type_name: \"haxe.iterators.ArrayIterator\", ... }"
        // "Unresolved type: haxe.iterators.ArrayIterator"
        let type_name = if let Some(start) = message.find("type_name: \"") {
            let start = start + "type_name: \"".len();
            if let Some(end) = message[start..].find('"') {
                Some(message[start..start + end].to_string())
            } else {
                None
            }
        } else if let Some(start) = message.find("Unresolved type: ") {
            let start = start + "Unresolved type: ".len();
            let end = message[start..].find(|c: char| !c.is_alphanumeric() && c != '.')
                .unwrap_or(message.len() - start);
            Some(message[start..start + end].to_string())
        } else {
            None
        };

        // Filter out generic type parameters and built-in typedefs:
        // - Single uppercase letters (T, K, V, E, R, etc.)
        // - Short names like "TKey", "TValue", etc.
        // - Built-in typedefs from StdTypes.hx (Iterator, KeyValueIterator, etc.)
        // These should NOT be treated as importable types
        if let Some(ref name) = type_name {
            // Skip single uppercase letter type parameters
            if name.len() == 1 && name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
                return None;
            }
            // Skip common generic type parameter patterns
            if name == "Key" || name == "Value" || name == "Item" || name == "Element" {
                return None;
            }
            // Skip built-in typedefs from StdTypes.hx (these are already loaded)
            if name == "Iterator" || name == "KeyValueIterator" || name == "Iterable" || name == "KeyValueIterable" {
                debug!("  Filtering out StdTypes typedef: {}", name);
                return None;
            }
        }

        type_name
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
                warn!("Failed to load cache for {:?}: {}", source_path, e);
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
                        debug!("Cache stale for {:?} (source: {}, cache: {})",
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
                debug!("Cache version mismatch for {:?} (cache: {}, current: {})",
                         source_path, metadata.compiler_version, current_version);
            }
            return None;
        }

        if self.config.enable_cache {
            debug!("Cache hit for {:?}", source_path);
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

        // Get source file timestamp and compute hash
        let source_timestamp = std::fs::metadata(source_path)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
            .unwrap_or(0);

        // Read source for hash computation
        let source_hash = std::fs::read_to_string(source_path)
            .map(|s| Self::hash_source(&s))
            .unwrap_or(0);

        let compile_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create metadata
        let metadata = BladeMetadata {
            name: module.name.clone(),
            source_path: source_path.to_string_lossy().to_string(),
            source_hash,
            source_timestamp,
            compile_timestamp,
            dependencies: Vec::new(), // TODO: Track dependencies for proper invalidation
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Save to BLADE file
        save_blade(&cache_path, module, metadata)
            .map_err(|e| format!("Failed to save cache: {}", e))?;

        if self.config.enable_cache {
            debug!("Cached MIR for {:?} -> {:?}", source_path, cache_path);
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
            debug!("Cache cleared: {:?}", cache_dir);
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

    /// Get the MIR modules that were generated during compilation
    /// Returns a vector of MIR modules corresponding to the compiled files
    pub fn get_mir_modules(&self) -> Vec<std::sync::Arc<crate::ir::IrModule>> {
        self.mir_modules.clone()
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
