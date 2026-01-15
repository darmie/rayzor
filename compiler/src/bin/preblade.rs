//! Pre-compile stdlib to BLADE format
//!
//! This tool pre-compiles Haxe standard library modules to BLADE bytecode format
//! for faster incremental compilation. Pre-compiled modules can be loaded directly
//! instead of re-parsing and re-compiling.
//!
//! It generates a .bsym manifest with all symbol information (types, methods, fields)
//!
//! Usage:
//!   cargo run --bin preblade -- --out .rayzor/blade/stdlib
//!   cargo run --bin preblade -- --list  # List modules that would be compiled

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use compiler::ir::blade::{
    BladeModuleSymbols, BladeTypeInfo, BladeClassInfo, BladeEnumInfo,
    BladeFieldInfo, BladeMethodInfo, BladeParamInfo, BladeEnumVariantInfo, BladeTypeAliasInfo,
    save_symbol_manifest,
};
use compiler::tast::type_checker::format_type_for_error;
use compiler::tast::symbols::Visibility;
use compiler::tast::node::TypedFile;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let mut out_path: Option<PathBuf> = None;
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
            "--list" | "-l" => list_only = true,
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_usage();
                return;
            }
            _ => {
                if args[i].starts_with("-") {
                    eprintln!("Warning: Unknown argument: {}", args[i]);
                }
            }
        }
        i += 1;
    }

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
    println!("preblade - Pre-compile Haxe stdlib to BLADE format");
    println!();
    println!("Usage:");
    println!("  preblade [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --out, -o <PATH>      Output directory for .bsym files");
    println!("  --list, -l            List types without generating files");
    println!("  --verbose, -v         Show detailed output");
    println!("  --help, -h            Show this help message");
    println!();
    println!("Output files:");
    println!("  <out>/stdlib.bsym     Symbol manifest (all types/methods/fields)");
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
            let dir_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip hidden and platform-specific directories
            if dir_name.starts_with('.') || dir_name.starts_with('_') {
                continue;
            }
            let skip_dirs = ["cpp", "cs", "flash", "hl", "java", "js", "lua", "neko", "php", "python", "eval"];
            if skip_dirs.contains(&dir_name) {
                continue;
            }

            discover_modules_recursive(base_path, &path, modules);
        } else if path.extension().map(|e| e == "hx").unwrap_or(false) {
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip import.hx files
            if file_name == "import.hx" {
                continue;
            }

            // Convert path to qualified module name
            if let Ok(relative) = path.strip_prefix(base_path) {
                let module_name = relative.to_string_lossy()
                    .replace('/', ".")
                    .replace('\\', ".")
                    .replace(".hx", "");
                modules.push(module_name);
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
    use compiler::compilation::{CompilationConfig, CompilationUnit};

    let mut config = CompilationConfig::default();
    config.enable_cache = false;
    let mut unit = CompilationUnit::new(config);

    println!("Loading stdlib configuration...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {:?}", e))?;

    // Find stdlib path from the unit's configuration
    let stdlib_path = PathBuf::from("compiler/haxe-std");
    if !stdlib_path.exists() {
        return Err("Could not find stdlib path".to_string());
    }

    // Discover all modules
    let all_modules = discover_stdlib_modules(&stdlib_path);
    println!("  Discovered {} modules in stdlib", all_modules.len());

    if list_only {
        println!();
        for module in &all_modules {
            println!("  {}", module);
        }
        return Ok((all_modules.len(), 0, 0));
    }

    // Load all modules using the efficient batch loader
    println!("  Loading all modules...");
    if let Err(e) = unit.load_imports_efficiently(&all_modules) {
        println!("  Warning: Some modules failed to load: {}", e);
    }

    // Now trigger full compilation to get typed files
    println!("  Compiling to TAST...");
    let typed_files = match unit.lower_to_tast() {
        Ok(files) => files,
        Err(errors) => {
            println!("  Warning: TAST lowering had {} errors", errors.len());
            if verbose {
                for (i, e) in errors.iter().take(5).enumerate() {
                    println!("    {}: {}", i + 1, e.message);
                }
            }
            Vec::new()
        }
    };
    println!("  Compiled {} typed files", typed_files.len());

    let mut total_classes = 0;
    let mut total_enums = 0;
    let mut total_aliases = 0;
    let mut all_module_symbols: Vec<BladeModuleSymbols> = Vec::new();

    for typed_file in &typed_files {
        let type_info = extract_type_info_from_file(typed_file, &unit.type_table);

        let module_name = typed_file.metadata.package_name.clone()
            .unwrap_or_else(|| typed_file.metadata.file_path.clone());

        let class_count = type_info.classes.len();
        let enum_count = type_info.enums.len();
        let alias_count = type_info.type_aliases.len();

        if verbose && (class_count > 0 || enum_count > 0 || alias_count > 0) {
            println!("  {}: {} classes, {} enums, {} aliases",
                module_name, class_count, enum_count, alias_count);
        }

        total_classes += class_count;
        total_enums += enum_count;
        total_aliases += alias_count;

        if !type_info.classes.is_empty() || !type_info.enums.is_empty() || !type_info.type_aliases.is_empty() {
            let source_hash = hash_string(&typed_file.metadata.file_path);
            all_module_symbols.push(BladeModuleSymbols {
                name: module_name,
                source_path: typed_file.metadata.file_path.clone(),
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
    println!("  {} modules with symbol information", all_module_symbols.len());

    if let Err(e) = save_symbol_manifest(&manifest_path, all_module_symbols) {
        eprintln!("Failed to save symbol manifest: {}", e);
    } else {
        println!("  Symbol manifest saved successfully");
    }

    Ok((total_classes, total_enums, total_aliases))
}

/// Extract type information from a TypedFile
fn extract_type_info_from_file(
    typed_file: &TypedFile,
    type_table: &RefCell<compiler::tast::TypeTable>,
) -> BladeTypeInfo {
    let mut type_info = BladeTypeInfo::default();
    let string_interner = typed_file.string_interner.borrow();
    let package = typed_file.metadata.package_name.as_deref().unwrap_or("");

    for class in &typed_file.classes {
        let class_name = string_interner.get(class.name).unwrap_or("").to_string();
        let package_parts: Vec<String> = package.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();

        let extends = class.super_class.map(|tid| {
            format_type_for_error(tid, type_table, &string_interner)
        });

        let implements: Vec<String> = class.interfaces.iter()
            .map(|&tid| format_type_for_error(tid, type_table, &string_interner))
            .collect();

        let type_params: Vec<String> = class.type_parameters.iter()
            .filter_map(|tp| string_interner.get(tp.name).map(|s| s.to_string()))
            .collect();

        let fields: Vec<BladeFieldInfo> = class.fields.iter()
            .filter(|f| !f.is_static)
            .map(|f| BladeFieldInfo {
                name: string_interner.get(f.name).unwrap_or("").to_string(),
                field_type: format_type_for_error(f.field_type, type_table, &string_interner),
                is_public: matches!(f.visibility, Visibility::Public),
                is_static: false,
                is_final: matches!(f.mutability, compiler::tast::symbols::Mutability::Immutable),
                has_default: f.initializer.is_some(),
            })
            .collect();

        let static_fields: Vec<BladeFieldInfo> = class.fields.iter()
            .filter(|f| f.is_static)
            .map(|f| BladeFieldInfo {
                name: string_interner.get(f.name).unwrap_or("").to_string(),
                field_type: format_type_for_error(f.field_type, type_table, &string_interner),
                is_public: matches!(f.visibility, Visibility::Public),
                is_static: true,
                is_final: matches!(f.mutability, compiler::tast::symbols::Mutability::Immutable),
                has_default: f.initializer.is_some(),
            })
            .collect();

        let methods: Vec<BladeMethodInfo> = class.methods.iter()
            .filter(|m| !m.is_static)
            .map(|m| extract_method_info(m, type_table, &string_interner))
            .collect();

        let static_methods: Vec<BladeMethodInfo> = class.methods.iter()
            .filter(|m| m.is_static)
            .map(|m| extract_method_info(m, type_table, &string_interner))
            .collect();

        let constructor = class.constructors.first().map(|c| {
            extract_method_info(c, type_table, &string_interner)
        });

        type_info.classes.push(BladeClassInfo {
            name: class_name,
            package: package_parts,
            extends,
            implements,
            type_params,
            is_extern: false,
            is_abstract: false,
            is_final: false,
            fields,
            methods,
            static_fields,
            static_methods,
            constructor,
        });
    }

    for enum_def in &typed_file.enums {
        let enum_name = string_interner.get(enum_def.name).unwrap_or("").to_string();
        let package_parts: Vec<String> = package.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();

        let type_params: Vec<String> = enum_def.type_parameters.iter()
            .filter_map(|tp| string_interner.get(tp.name).map(|s| s.to_string()))
            .collect();

        let variants: Vec<BladeEnumVariantInfo> = enum_def.variants.iter()
            .enumerate()
            .map(|(idx, v)| {
                let params: Vec<BladeParamInfo> = v.parameters.iter()
                    .map(|p| BladeParamInfo {
                        name: string_interner.get(p.name).unwrap_or("").to_string(),
                        param_type: format_type_for_error(p.param_type, type_table, &string_interner),
                        has_default: p.default_value.is_some(),
                        is_optional: false,
                    })
                    .collect();

                BladeEnumVariantInfo {
                    name: string_interner.get(v.name).unwrap_or("").to_string(),
                    params,
                    index: idx,
                }
            })
            .collect();

        type_info.enums.push(BladeEnumInfo {
            name: enum_name,
            package: package_parts,
            type_params,
            variants,
            is_extern: false,
        });
    }

    for alias in &typed_file.type_aliases {
        let alias_name = string_interner.get(alias.name).unwrap_or("").to_string();
        let package_parts: Vec<String> = package.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();

        let type_params: Vec<String> = alias.type_parameters.iter()
            .filter_map(|tp| string_interner.get(tp.name).map(|s| s.to_string()))
            .collect();

        type_info.type_aliases.push(BladeTypeAliasInfo {
            name: alias_name,
            package: package_parts,
            type_params,
            target_type: format_type_for_error(alias.target_type, type_table, &string_interner),
        });
    }

    type_info
}

fn extract_method_info(
    method: &compiler::tast::node::TypedFunction,
    type_table: &RefCell<compiler::tast::TypeTable>,
    string_interner: &compiler::tast::StringInterner,
) -> BladeMethodInfo {
    let params: Vec<BladeParamInfo> = method.parameters.iter()
        .map(|p| BladeParamInfo {
            name: string_interner.get(p.name).unwrap_or("").to_string(),
            param_type: format_type_for_error(p.param_type, type_table, string_interner),
            has_default: p.default_value.is_some(),
            is_optional: false,
        })
        .collect();

    let type_params: Vec<String> = method.type_parameters.iter()
        .filter_map(|tp| string_interner.get(tp.name).map(|s| s.to_string()))
        .collect();

    BladeMethodInfo {
        name: string_interner.get(method.name).unwrap_or("").to_string(),
        params,
        return_type: format_type_for_error(method.return_type, type_table, string_interner),
        is_public: matches!(method.visibility, Visibility::Public),
        is_static: method.is_static,
        is_inline: false,
        type_params,
    }
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
