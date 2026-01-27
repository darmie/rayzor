#![allow(
    unused_imports,
    dead_code,
    clippy::redundant_closure,
    clippy::collapsible_str_replace,
    clippy::single_component_path_imports
)]

//! Pre-compile stdlib to BLADE format
//!
//! This tool pre-compiles Haxe standard library modules to BLADE bytecode format
//! for faster incremental compilation. Pre-compiled modules can be loaded directly
//! instead of re-parsing and re-compiling.
//!
//! It generates:
//! - A .bsym manifest with all symbol information (types, methods, fields)
//! - Optional: A .rzb bundle containing all MIR modules in one file
//!
//! Usage:
//!   cargo run --bin preblade -- --out .rayzor/blade/stdlib
//!   cargo run --bin preblade -- --list  # List modules that would be compiled
//!   cargo run --bin preblade -- --bundle app.rzb Main.hx  # Create a bundle

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::blade::{
    save_bundle, save_symbol_manifest, BladeAbstractInfo, BladeClassInfo, BladeEnumInfo,
    BladeEnumVariantInfo, BladeFieldInfo, BladeMethodInfo, BladeModuleSymbols, BladeParamInfo,
    BladeTypeAliasInfo, BladeTypeInfo, RayzorBundle,
};
use parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let mut out_path: Option<PathBuf> = None;
    let mut bundle_path: Option<PathBuf> = None;
    let mut source_files: Vec<String> = Vec::new();
    let mut list_only = false;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--out" | "-o" => {
                i += 1;
                if i < args.len() {
                    out_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--bundle" | "-b" => {
                i += 1;
                if i < args.len() {
                    bundle_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--list" | "-l" => list_only = true,
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_usage();
                return;
            }
            arg if !arg.starts_with("-") => {
                // Source file
                source_files.push(arg.to_string());
            }
            _ => {
                eprintln!("Warning: Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    // Bundle mode
    if let Some(bundle_out) = bundle_path {
        if source_files.is_empty() {
            eprintln!("Error: No source files specified for bundle");
            eprintln!("Usage: preblade --bundle app.rzb Main.hx [other.hx ...]");
            std::process::exit(1);
        }

        match create_bundle(&bundle_out, &source_files, verbose) {
            Ok(module_count) => {
                println!();
                println!("Bundle created: {}", bundle_out.display());
                println!("  Modules: {}", module_count);
            }
            Err(e) => {
                eprintln!("Bundle creation failed: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Standard symbol extraction mode
    let out_path = out_path.unwrap_or_else(|| PathBuf::from(".rayzor/blade/stdlib"));

    if !list_only {
        if let Err(e) = std::fs::create_dir_all(&out_path) {
            eprintln!("Error creating output directory: {}", e);
            std::process::exit(1);
        }
    }

    println!("Pre-BLADE: Extracting stdlib symbols");
    println!("  Output: {}", out_path.display());
    println!();

    match extract_stdlib_symbols(&out_path, verbose, list_only) {
        Ok((classes, enums, aliases)) => {
            println!();
            println!("Pre-BLADE complete:");
            println!("  Classes: {}", classes);
            println!("  Enums:   {}", enums);
            println!("  Aliases: {}", aliases);
        }
        Err(e) => {
            eprintln!("Pre-BLADE failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!("preblade - Pre-compile Haxe to BLADE format");
    println!();
    println!("Usage:");
    println!("  preblade [OPTIONS] [SOURCE_FILES...]");
    println!();
    println!("Modes:");
    println!("  Symbol extraction (default):");
    println!("    preblade --out .rayzor/blade/stdlib");
    println!();
    println!("  Bundle creation:");
    println!("    preblade --bundle app.rzb Main.hx [other.hx ...]");
    println!();
    println!("Options:");
    println!("  --out, -o <PATH>      Output directory for .bsym files");
    println!("  --bundle, -b <FILE>   Create a .rzb bundle from source files");
    println!("  --list, -l            List types without generating files");
    println!("  --verbose, -v         Show detailed output");
    println!("  --help, -h            Show this help message");
    println!();
    println!("Output files:");
    println!("  <out>/stdlib.bsym     Symbol manifest (all types/methods/fields)");
    println!("  <bundle>.rzb          Rayzor Bundle (all MIR in one file)");
}

/// Create a .rzb bundle from source files
fn create_bundle(output: &Path, source_files: &[String], verbose: bool) -> Result<usize, String> {
    use std::time::Instant;

    println!("Creating Rayzor Bundle: {}", output.display());
    println!();

    let t0 = Instant::now();

    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    if verbose {
        println!("  Loading stdlib...");
    }
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add source files
    for source_file in source_files {
        if verbose {
            println!("  Adding: {}", source_file);
        }
        let source = std::fs::read_to_string(source_file)
            .map_err(|e| format!("Failed to read {}: {}", source_file, e))?;
        unit.add_file(&source, source_file)
            .map_err(|e| format!("Failed to add {}: {}", source_file, e))?;
    }

    // Lower to TAST
    if verbose {
        println!("  Lowering to TAST...");
    }
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST lowering failed: {:?}", errors))?;

    // Get MIR modules
    if verbose {
        println!("  Generating MIR...");
    }
    let mir_modules = unit.get_mir_modules();

    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    // Convert Arc<IrModule> to IrModule for the bundle
    let modules: Vec<_> = mir_modules.iter().map(|m| (**m).clone()).collect();

    let module_count = modules.len();

    // Find entry module and function
    let entry_module = modules
        .iter()
        .rev() // User modules are at the end
        .find(|m| {
            m.functions
                .values()
                .any(|f| f.name == "main" || f.name == "Main_main" || f.name.ends_with("_main"))
        })
        .map(|m| m.name.clone())
        .ok_or("No entry point found (no main function)")?;

    let entry_function = modules
        .iter()
        .find(|m| m.name == entry_module)
        .and_then(|m| {
            m.functions
                .values()
                .find(|f| f.name == "main" || f.name == "Main_main" || f.name.ends_with("_main"))
                .map(|f| f.name.clone())
        })
        .ok_or("Entry function not found")?;

    if verbose {
        println!("  Entry: {}::{}", entry_module, entry_function);
    }

    // Create and save bundle
    let bundle = RayzorBundle::new(modules, &entry_module, &entry_function, None);

    if verbose {
        println!("  Saving bundle...");
    }
    save_bundle(output, &bundle).map_err(|e| format!("Failed to save bundle: {}", e))?;

    let elapsed = t0.elapsed();
    println!("  Compiled {} modules in {:?}", module_count, elapsed);

    // Show bundle size
    if let Ok(meta) = std::fs::metadata(output) {
        let size = meta.len();
        if size > 1024 * 1024 {
            println!("  Bundle size: {:.2} MB", size as f64 / (1024.0 * 1024.0));
        } else if size > 1024 {
            println!("  Bundle size: {:.2} KB", size as f64 / 1024.0);
        } else {
            println!("  Bundle size: {} bytes", size);
        }
    }

    Ok(module_count)
}

/// Discover all .hx modules in stdlib directory
fn discover_stdlib_modules(stdlib_path: &Path) -> Vec<String> {
    let mut modules = Vec::new();
    discover_modules_recursive(stdlib_path, stdlib_path, &mut modules);
    modules.sort();
    modules
}

fn discover_modules_recursive(base_path: &Path, current_path: &Path, modules: &mut Vec<String>) {
    let entries = match std::fs::read_dir(current_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip hidden and platform-specific directories
            if dir_name.starts_with('.') || dir_name.starts_with('_') {
                continue;
            }
            let skip_dirs = [
                "cpp", "cs", "flash", "hl", "java", "js", "lua", "neko", "php", "python", "eval",
            ];
            if skip_dirs.contains(&dir_name) {
                continue;
            }

            discover_modules_recursive(base_path, &path, modules);
        } else if path.extension().map(|e| e == "hx").unwrap_or(false) {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip import.hx files
            if file_name == "import.hx" {
                continue;
            }

            // Convert path to qualified module name
            if let Ok(relative) = path.strip_prefix(base_path) {
                let module_name = relative
                    .to_string_lossy()
                    .replace('/', ".")
                    .replace('\\', ".")
                    .replace(".hx", "");
                modules.push(module_name);
            }
        }
    }
}

/// Discover all .hx files with their full paths
fn discover_stdlib_files(stdlib_path: &Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    discover_files_recursive(stdlib_path, stdlib_path, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn discover_files_recursive(
    base_path: &Path,
    current_path: &Path,
    files: &mut Vec<(String, PathBuf)>,
) {
    let entries = match std::fs::read_dir(current_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip hidden and platform-specific directories
            if dir_name.starts_with('.') || dir_name.starts_with('_') {
                continue;
            }
            let skip_dirs = [
                "cpp", "cs", "flash", "hl", "java", "js", "lua", "neko", "php", "python", "eval",
            ];
            if skip_dirs.contains(&dir_name) {
                continue;
            }

            discover_files_recursive(base_path, &path, files);
        } else if path.extension().map(|e| e == "hx").unwrap_or(false) {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip import.hx files
            if file_name == "import.hx" {
                continue;
            }

            // Convert path to qualified module name
            if let Ok(relative) = path.strip_prefix(base_path) {
                let module_name = relative
                    .to_string_lossy()
                    .replace('/', ".")
                    .replace('\\', ".")
                    .replace(".hx", "");
                files.push((module_name, path.clone()));
            }
        }
    }
}

/// Extract symbols from stdlib
fn extract_stdlib_symbols(
    out_path: &Path,
    verbose: bool,
    list_only: bool,
) -> Result<(usize, usize, usize), String> {
    // Find stdlib path
    let stdlib_path = PathBuf::from("compiler/haxe-std");
    if !stdlib_path.exists() {
        return Err("Could not find stdlib path".to_string());
    }

    // Discover all modules with their file paths
    let all_files = discover_stdlib_files(&stdlib_path);
    println!("  Discovered {} modules in stdlib", all_files.len());

    if list_only {
        println!();
        for (module_name, _) in &all_files {
            println!("  {}", module_name);
        }
        return Ok((all_files.len(), 0, 0));
    }

    let mut total_classes = 0;
    let mut total_enums = 0;
    let mut total_aliases = 0;
    let mut all_module_symbols: Vec<BladeModuleSymbols> = Vec::new();

    println!(
        "  Parsing and extracting types from {} files...",
        all_files.len()
    );

    for (module_name, file_path) in &all_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(e) => {
                if verbose {
                    println!("    Warning: Could not read {}: {}", file_path.display(), e);
                }
                continue;
            }
        };

        let filename = file_path.to_string_lossy().to_string();

        // Parse the file directly - don't compile, just extract type declarations
        let haxe_file = match parser::parse_haxe_file(&filename, &source, true) {
            Ok(f) => f,
            Err(e) => {
                if verbose {
                    println!("    Warning: Parse error in {}: {}", module_name, e);
                }
                continue;
            }
        };

        // Extract type information directly from the AST
        let type_info = extract_type_info_from_ast(&haxe_file);

        let class_count = type_info.classes.len();
        let enum_count = type_info.enums.len();
        let alias_count = type_info.type_aliases.len();
        let abstract_count = type_info.abstracts.len();

        if verbose && (class_count > 0 || enum_count > 0 || alias_count > 0 || abstract_count > 0) {
            println!(
                "  {}: {} classes, {} enums, {} aliases, {} abstracts",
                module_name, class_count, enum_count, alias_count, abstract_count
            );
        }

        total_classes += class_count;
        total_enums += enum_count;
        total_aliases += alias_count;

        if !type_info.classes.is_empty()
            || !type_info.enums.is_empty()
            || !type_info.type_aliases.is_empty()
            || !type_info.abstracts.is_empty()
        {
            let source_hash = hash_string(&filename);

            all_module_symbols.push(BladeModuleSymbols {
                name: module_name.clone(),
                source_path: filename,
                source_hash,
                types: type_info,
                dependencies: Vec::new(),
            });
        }
    }

    // Save symbol manifest
    let manifest_path = out_path.join("stdlib.bsym");
    println!();
    println!("Saving symbol manifest to {}...", manifest_path.display());
    println!(
        "  {} modules with symbol information",
        all_module_symbols.len()
    );

    if let Err(e) = save_symbol_manifest(&manifest_path, all_module_symbols) {
        eprintln!("Failed to save symbol manifest: {}", e);
    } else {
        println!("  Symbol manifest saved successfully");
    }

    Ok((total_classes, total_enums, total_aliases))
}

/// Extract type information directly from parsed AST (without full compilation)
fn extract_type_info_from_ast(haxe_file: &parser::HaxeFile) -> BladeTypeInfo {
    let mut type_info = BladeTypeInfo::default();

    let package: Vec<String> = haxe_file
        .package
        .as_ref()
        .map(|p| p.path.clone())
        .unwrap_or_default();

    for decl in &haxe_file.declarations {
        match decl {
            parser::TypeDeclaration::Class(class) => {
                let is_extern = class.modifiers.contains(&parser::Modifier::Extern);

                let extends = class.extends.as_ref().map(|t| type_to_string(t));
                let implements: Vec<String> =
                    class.implements.iter().map(|t| type_to_string(t)).collect();
                let type_params: Vec<String> =
                    class.type_params.iter().map(|tp| tp.name.clone()).collect();

                // Extract fields
                let mut fields: Vec<BladeFieldInfo> = Vec::new();
                let mut static_fields: Vec<BladeFieldInfo> = Vec::new();
                let mut methods: Vec<BladeMethodInfo> = Vec::new();
                let mut static_methods: Vec<BladeMethodInfo> = Vec::new();
                let mut constructor: Option<BladeMethodInfo> = None;

                for field in &class.fields {
                    let is_static = field.modifiers.contains(&parser::Modifier::Static);
                    let is_public = matches!(field.access, Some(parser::Access::Public));

                    match &field.kind {
                        parser::ClassFieldKind::Var {
                            name,
                            type_hint,
                            expr,
                        } => {
                            let field_info = BladeFieldInfo {
                                name: name.clone(),
                                field_type: type_hint
                                    .as_ref()
                                    .map(|t| type_to_string(t))
                                    .unwrap_or_else(|| "Dynamic".to_string()),
                                is_public,
                                is_static,
                                is_final: false,
                                has_default: expr.is_some(),
                            };
                            if is_static {
                                static_fields.push(field_info);
                            } else {
                                fields.push(field_info);
                            }
                        }
                        parser::ClassFieldKind::Final {
                            name,
                            type_hint,
                            expr,
                        } => {
                            let field_info = BladeFieldInfo {
                                name: name.clone(),
                                field_type: type_hint
                                    .as_ref()
                                    .map(|t| type_to_string(t))
                                    .unwrap_or_else(|| "Dynamic".to_string()),
                                is_public,
                                is_static,
                                is_final: true,
                                has_default: expr.is_some(),
                            };
                            if is_static {
                                static_fields.push(field_info);
                            } else {
                                fields.push(field_info);
                            }
                        }
                        parser::ClassFieldKind::Property {
                            name, type_hint, ..
                        } => {
                            let field_info = BladeFieldInfo {
                                name: name.clone(),
                                field_type: type_hint
                                    .as_ref()
                                    .map(|t| type_to_string(t))
                                    .unwrap_or_else(|| "Dynamic".to_string()),
                                is_public,
                                is_static,
                                is_final: false,
                                has_default: false,
                            };
                            if is_static {
                                static_fields.push(field_info);
                            } else {
                                fields.push(field_info);
                            }
                        }
                        parser::ClassFieldKind::Function(func) => {
                            let method_info = extract_method_from_ast(func, is_public, is_static);
                            if func.name == "new" {
                                constructor = Some(method_info);
                            } else if is_static {
                                static_methods.push(method_info);
                            } else {
                                methods.push(method_info);
                            }
                        }
                    }
                }

                type_info.classes.push(BladeClassInfo {
                    name: class.name.clone(),
                    package: package.clone(),
                    extends,
                    implements,
                    type_params,
                    is_extern,
                    is_abstract: false,
                    is_final: class.modifiers.contains(&parser::Modifier::Final),
                    fields,
                    methods,
                    static_fields,
                    static_methods,
                    constructor,
                });
            }
            parser::TypeDeclaration::Enum(enum_decl) => {
                let type_params: Vec<String> = enum_decl
                    .type_params
                    .iter()
                    .map(|tp| tp.name.clone())
                    .collect();

                let variants: Vec<BladeEnumVariantInfo> = enum_decl
                    .constructors
                    .iter()
                    .enumerate()
                    .map(|(idx, v)| {
                        let params: Vec<BladeParamInfo> = v
                            .params
                            .iter()
                            .map(|p| BladeParamInfo {
                                name: p.name.clone(),
                                param_type: p
                                    .type_hint
                                    .as_ref()
                                    .map(|t| type_to_string(t))
                                    .unwrap_or_else(|| "Dynamic".to_string()),
                                has_default: p.default_value.is_some(),
                                is_optional: p.optional,
                            })
                            .collect();
                        BladeEnumVariantInfo {
                            name: v.name.clone(),
                            params,
                            index: idx,
                        }
                    })
                    .collect();

                type_info.enums.push(BladeEnumInfo {
                    name: enum_decl.name.clone(),
                    package: package.clone(),
                    type_params,
                    variants,
                    is_extern: false, // Enums don't have modifiers field in this AST
                });
            }
            parser::TypeDeclaration::Typedef(typedef) => {
                let type_params: Vec<String> = typedef
                    .type_params
                    .iter()
                    .map(|tp| tp.name.clone())
                    .collect();

                type_info.type_aliases.push(BladeTypeAliasInfo {
                    name: typedef.name.clone(),
                    package: package.clone(),
                    type_params,
                    target_type: type_to_string(&typedef.type_def),
                });
            }
            parser::TypeDeclaration::Abstract(abstract_decl) => {
                let type_params: Vec<String> = abstract_decl
                    .type_params
                    .iter()
                    .map(|tp| tp.name.clone())
                    .collect();

                let underlying_type = abstract_decl
                    .underlying
                    .as_ref()
                    .map(|t| type_to_string(t))
                    .unwrap_or_else(|| "Dynamic".to_string());

                let from_types: Vec<String> = abstract_decl
                    .from
                    .iter()
                    .map(|t| type_to_string(t))
                    .collect();
                let to_types: Vec<String> =
                    abstract_decl.to.iter().map(|t| type_to_string(t)).collect();

                // Extract methods
                let mut methods: Vec<BladeMethodInfo> = Vec::new();
                let mut static_methods: Vec<BladeMethodInfo> = Vec::new();

                for field in &abstract_decl.fields {
                    if let parser::ClassFieldKind::Function(func) = &field.kind {
                        let is_static = field.modifiers.contains(&parser::Modifier::Static);
                        let is_public = matches!(field.access, Some(parser::Access::Public));
                        let method_info = extract_method_from_ast(func, is_public, is_static);
                        if is_static {
                            static_methods.push(method_info);
                        } else {
                            methods.push(method_info);
                        }
                    }
                }

                type_info.abstracts.push(BladeAbstractInfo {
                    name: abstract_decl.name.clone(),
                    package: package.clone(),
                    type_params,
                    underlying_type,
                    forward_fields: vec![],
                    from_types,
                    to_types,
                    methods,
                    static_methods,
                });
            }
            parser::TypeDeclaration::Interface(iface) => {
                // Treat interfaces similar to classes for type resolution
                let extends: Option<String> = iface.extends.first().map(|t| type_to_string(t));
                let implements: Vec<String> = iface
                    .extends
                    .iter()
                    .skip(1)
                    .map(|t| type_to_string(t))
                    .collect();
                let type_params: Vec<String> =
                    iface.type_params.iter().map(|tp| tp.name.clone()).collect();

                let mut methods: Vec<BladeMethodInfo> = Vec::new();
                for field in &iface.fields {
                    if let parser::ClassFieldKind::Function(func) = &field.kind {
                        let is_public = true; // Interface methods are always public
                        let is_static = field.modifiers.contains(&parser::Modifier::Static);
                        let method_info = extract_method_from_ast(func, is_public, is_static);
                        methods.push(method_info);
                    }
                }

                type_info.classes.push(BladeClassInfo {
                    name: iface.name.clone(),
                    package: package.clone(),
                    extends,
                    implements,
                    type_params,
                    is_extern: false,
                    is_abstract: true, // Mark interfaces as abstract
                    is_final: false,
                    fields: vec![],
                    methods,
                    static_fields: vec![],
                    static_methods: vec![],
                    constructor: None,
                });
            }
            parser::TypeDeclaration::Conditional(_) => {
                // Skip conditional compilation blocks
            }
        }
    }

    type_info
}

/// Extract method info from parsed function AST
fn extract_method_from_ast(
    func: &parser::Function,
    is_public: bool,
    is_static: bool,
) -> BladeMethodInfo {
    let params: Vec<BladeParamInfo> = func
        .params
        .iter()
        .map(|p| BladeParamInfo {
            name: p.name.clone(),
            param_type: p
                .type_hint
                .as_ref()
                .map(|t| type_to_string(t))
                .unwrap_or_else(|| "Dynamic".to_string()),
            has_default: p.default_value.is_some(),
            is_optional: p.optional,
        })
        .collect();

    let type_params: Vec<String> = func.type_params.iter().map(|tp| tp.name.clone()).collect();

    BladeMethodInfo {
        name: func.name.clone(),
        params,
        return_type: func
            .return_type
            .as_ref()
            .map(|t| type_to_string(t))
            .unwrap_or_else(|| "Void".to_string()),
        is_public,
        is_static,
        is_inline: false,
        type_params,
    }
}

/// Convert parsed Type to string representation
fn type_to_string(ty: &parser::Type) -> String {
    match ty {
        parser::Type::Path { path, params, .. } => {
            // Build the base type name from package + name
            let mut base = if path.package.is_empty() {
                path.name.clone()
            } else {
                format!("{}.{}", path.package.join("."), path.name)
            };

            // Include sub-type if present (e.g., "Class.SubType")
            if let Some(sub) = &path.sub {
                base = format!("{}.{}", base, sub);
            }

            if params.is_empty() {
                base
            } else {
                let param_strs: Vec<String> = params.iter().map(|t| type_to_string(t)).collect();
                format!("{}<{}>", base, param_strs.join(", "))
            }
        }
        parser::Type::Function { params, ret, .. } => {
            let param_strs: Vec<String> = params.iter().map(|t| type_to_string(t)).collect();
            format!("({}) -> {}", param_strs.join(", "), type_to_string(ret))
        }
        parser::Type::Anonymous { fields, .. } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_to_string(&f.type_hint)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        parser::Type::Optional { inner, .. } => format!("Null<{}>", type_to_string(inner)),
        parser::Type::Parenthesis { inner, .. } => type_to_string(inner),
        parser::Type::Intersection { left, right, .. } => {
            format!("{} & {}", type_to_string(left), type_to_string(right))
        }
        parser::Type::Wildcard { .. } => "?".to_string(),
    }
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
