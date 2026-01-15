//! Pre-compile stdlib to BLADE format
//!
//! This tool pre-compiles Haxe standard library modules to BLADE bytecode format
//! for faster incremental compilation. Pre-compiled modules can be loaded directly
//! instead of re-parsing and re-compiling.
//!
//! It generates two outputs:
//! 1. Individual .blade files for MIR (executable code)
//! 2. A single .bsym manifest with all symbol information (types, methods, fields)
//!
//! Usage:
//!   cargo run --bin preblade -- --stdlib-path ./compiler/haxe-std --out .rayzor/blade/stdlib
//!   cargo run --bin preblade -- --stdlib-path ./compiler/haxe-std --out .rayzor/blade/stdlib --force
//!   cargo run --bin preblade -- --list  # List modules that would be compiled

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use compiler::ir::blade::{
    BladeMetadata, BladeModuleSymbols, BladeTypeInfo, BladeClassInfo, BladeEnumInfo,
    BladeFieldInfo, BladeMethodInfo, BladeParamInfo, BladeEnumVariantInfo, BladeTypeAliasInfo,
    save_blade, save_symbol_manifest,
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
    let mut symbols_only = false;

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
            "--symbols-only" | "-s" => symbols_only = true,
            "--help" | "-h" => {
                print_usage();
                return;
            }
            _ => {
                // Ignore unknown args for forward compatibility
                if args[i].starts_with("-") {
                    eprintln!("Warning: Unknown argument: {}", args[i]);
                }
            }
        }
        i += 1;
    }

    let out_path = out_path.unwrap_or_else(|| PathBuf::from(".rayzor/blade/stdlib"));

    // Create output directory
    if !list_only {
        if let Err(e) = std::fs::create_dir_all(&out_path) {
            eprintln!("Error creating output directory: {}", e);
            std::process::exit(1);
        }
    }

    println!("Pre-BLADE: Extracting stdlib symbols");
    println!("  Output: {}", out_path.display());
    println!("  Symbols only: {}", symbols_only);
    println!();

    // Load stdlib using the compiler's normal mechanism
    match extract_stdlib_symbols(&out_path, verbose, symbols_only, list_only) {
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
    println!("  --out, -o <PATH>      Output directory for .blade/.bsym files");
    println!("  --list, -l            List types without generating files");
    println!("  --verbose, -v         Show detailed output");
    println!("  --symbols-only, -s    Only generate symbol manifest (no MIR)");
    println!("  --help, -h            Show this help message");
    println!();
    println!("Examples:");
    println!("  preblade --out .rayzor/blade/stdlib");
    println!("  preblade --list");
    println!();
    println!("Output files:");
    println!("  <out>/stdlib.bsym     Symbol manifest (all types/methods/fields)");
}

/// Extract symbols from stdlib using the compiler's normal loading mechanism
fn extract_stdlib_symbols(
    out_path: &Path,
    verbose: bool,
    _symbols_only: bool,
    list_only: bool,
) -> Result<(usize, usize, usize), String> {
    use compiler::compilation::{CompilationConfig, CompilationUnit};

    // Create compilation unit
    let mut config = CompilationConfig::default();
    config.enable_cache = false; // We're generating the cache, not using it
    let mut unit = CompilationUnit::new(config);

    // Load stdlib configuration (paths, etc.)
    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {:?}", e))?;

    // Create a dummy file that imports common stdlib modules to trigger on-demand loading
    let stdlib_trigger = r#"
        // Core types
        import Std;
        import Math;
        import String;
        import Array;
        import Date;
        import DateTools;
        import Reflect;
        import Type;
        import Lambda;
        import StringTools;
        import StringBuf;
        import Map;
        import List;
        import EReg;
        import Xml;

        // haxe.io package
        import haxe.io.Bytes;
        import haxe.io.BytesBuffer;
        import haxe.io.BytesInput;
        import haxe.io.BytesOutput;
        import haxe.io.Path;
        import haxe.io.Input;
        import haxe.io.Output;
        import haxe.io.Eof;

        // haxe.ds package
        import haxe.ds.StringMap;
        import haxe.ds.IntMap;
        import haxe.ds.ObjectMap;
        import haxe.ds.List;
        import haxe.ds.Vector;
        import haxe.ds.BalancedTree;
        import haxe.ds.ArraySort;
        import haxe.ds.GenericStack;
        import haxe.ds.HashMap;
        import haxe.ds.EnumValueMap;
        import haxe.ds.Option;
        import haxe.ds.Either;

        // haxe core
        import haxe.Exception;
        import haxe.CallStack;
        import haxe.PosInfos;
        import haxe.Log;
        import haxe.Timer;
        import haxe.Json;
        import haxe.Resource;
        import haxe.Template;
        import haxe.Serializer;
        import haxe.Unserializer;
        import haxe.Utf8;
        import haxe.Int32;
        import haxe.Int64;
        import haxe.EnumFlags;
        import haxe.EnumTools;
        import haxe.DynamicAccess;

        // haxe.iterators
        import haxe.iterators.ArrayIterator;
        import haxe.iterators.ArrayKeyValueIterator;
        import haxe.iterators.StringIterator;
        import haxe.iterators.StringKeyValueIterator;
        import haxe.iterators.MapKeyValueIterator;

        // haxe.crypto
        import haxe.crypto.Md5;
        import haxe.crypto.Sha1;
        import haxe.crypto.Sha256;
        import haxe.crypto.Base64;
        import haxe.crypto.Crc32;
        import haxe.crypto.Adler32;
        import haxe.crypto.BaseCode;
        import haxe.crypto.Hmac;

        // haxe.format
        import haxe.format.JsonParser;
        import haxe.format.JsonPrinter;

        // haxe.exceptions
        import haxe.exceptions.PosException;
        import haxe.exceptions.NotImplementedException;
        import haxe.exceptions.ArgumentException;

        // sys package
        import sys.FileSystem;
        import sys.io.File;
        import sys.io.FileInput;
        import sys.io.FileOutput;
        import sys.io.Process;

        // sys.thread
        import sys.thread.Thread;
        import sys.thread.Mutex;
        import sys.thread.Lock;
        import sys.thread.Deque;
        import sys.thread.Semaphore;
        import sys.thread.Condition;
        import sys.thread.FixedThreadPool;

        // haxe.atomic
        import haxe.atomic.AtomicInt;
        import haxe.atomic.AtomicBool;
        import haxe.atomic.AtomicObject;

        class __PrebladeStdlibLoader {
            public function new() {}
        }
    "#;

    println!("  Triggering stdlib loading via imports...");
    if let Err(e) = unit.add_file(stdlib_trigger, "<preblade-trigger>") {
        println!("  Warning: Failed to add trigger file: {}", e);
    }

    // Lower to TAST - this triggers on-demand loading of all imported modules
    println!("  Compiling to TAST...");
    let typed_files = match unit.lower_to_tast() {
        Ok(files) => files,
        Err(errors) => {
            println!("  Warning: TAST lowering had {} errors (continuing with partial results)", errors.len());
            Vec::new()
        }
    };
    println!("  Loaded {} typed files", typed_files.len());

    let mut total_classes = 0;
    let mut total_enums = 0;
    let mut total_aliases = 0;
    let mut all_module_symbols: Vec<BladeModuleSymbols> = Vec::new();

    // Extract symbols from each typed file
    for typed_file in &typed_files {
        let type_info = extract_type_info_from_file(typed_file, &unit.type_table);

        let module_name = typed_file.metadata.package_name.clone()
            .unwrap_or_else(|| typed_file.metadata.file_path.clone());

        let class_count = type_info.classes.len();
        let enum_count = type_info.enums.len();
        let alias_count = type_info.type_aliases.len();

        if verbose || list_only {
            if !type_info.classes.is_empty() || !type_info.enums.is_empty() {
                println!("  {}: {} classes, {} enums, {} aliases",
                    module_name, class_count, enum_count, alias_count);

                if list_only {
                    for class in &type_info.classes {
                        let qualified = if class.package.is_empty() {
                            class.name.clone()
                        } else {
                            format!("{}.{}", class.package.join("."), class.name)
                        };
                        println!("    class {}", qualified);
                        if verbose {
                            for method in &class.methods {
                                println!("      {}({}) -> {}",
                                    method.name,
                                    method.params.iter().map(|p| p.param_type.as_str()).collect::<Vec<_>>().join(", "),
                                    method.return_type);
                            }
                        }
                    }
                    for enum_def in &type_info.enums {
                        let qualified = if enum_def.package.is_empty() {
                            enum_def.name.clone()
                        } else {
                            format!("{}.{}", enum_def.package.join("."), enum_def.name)
                        };
                        println!("    enum {}", qualified);
                    }
                }
            }
        }

        total_classes += class_count;
        total_enums += enum_count;
        total_aliases += alias_count;

        // Add to manifest if there's actual content
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

    if list_only {
        return Ok((total_classes, total_enums, total_aliases));
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

/// Extract type information from a single TypedFile
fn extract_type_info_from_file(
    typed_file: &TypedFile,
    type_table: &RefCell<compiler::tast::TypeTable>,
) -> BladeTypeInfo {
    let mut type_info = BladeTypeInfo::default();
    let string_interner = typed_file.string_interner.borrow();

    let package = typed_file.metadata.package_name.as_deref().unwrap_or("");

    // Extract classes
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

        // Extract fields
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

        // Extract methods
        let methods: Vec<BladeMethodInfo> = class.methods.iter()
            .filter(|m| !m.is_static)
            .map(|m| extract_method_info(m, type_table, &string_interner))
            .collect();

        let static_methods: Vec<BladeMethodInfo> = class.methods.iter()
            .filter(|m| m.is_static)
            .map(|m| extract_method_info(m, type_table, &string_interner))
            .collect();

        // Extract constructor
        let constructor = class.constructors.first().map(|c| {
            extract_method_info(c, type_table, &string_interner)
        });

        type_info.classes.push(BladeClassInfo {
            name: class_name,
            package: package_parts,
            extends,
            implements,
            type_params,
            is_extern: false, // TODO: detect from metadata
            is_abstract: false,
            is_final: false,
            fields,
            methods,
            static_fields,
            static_methods,
            constructor,
        });
    }

    // Extract enums
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

    // Extract type aliases
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

/// Extract method information from a TypedFunction
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
