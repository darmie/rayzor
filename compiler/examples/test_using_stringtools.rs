#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
// Test StringTools with 'using' syntax
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    let source = include_str!("test_using_stringtools.hx");

    // Create compilation unit with stdlib
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    if let Err(e) = unit.load_stdlib() {
        eprintln!("Failed to load stdlib: {}", e);
        std::process::exit(1);
    }

    // Add the test file
    if let Err(e) = unit.add_file(source, "test_using_stringtools.hx") {
        eprintln!("Failed to add file: {}", e);
        std::process::exit(1);
    }

    // Compile to TAST
    let _typed_files = match unit.lower_to_tast() {
        Ok(files) => {
            println!("TAST lowering succeeded ({} files)", files.len());
            files
        }
        Err(errors) => {
            eprintln!("TAST lowering failed with {} errors:", errors.len());
            for e in &errors {
                eprintln!("  - {:?}", e);
            }
            std::process::exit(1);
        }
    };

    // Get MIR modules
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        eprintln!("No MIR modules generated");
        std::process::exit(1);
    }

    println!("MIR lowering succeeded ({} modules)", mir_modules.len());

    // Get runtime symbols from the plugin system
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    // Create Cranelift backend with runtime symbols
    let mut backend =
        CraneliftBackend::with_symbols(&symbols_ref).expect("Failed to create backend");

    for (i, module) in mir_modules.iter().enumerate() {
        println!("Compiling module {}...", i);
        if let Err(e) = backend.compile_module(module) {
            eprintln!("Failed to compile module {}: {}", i, e);
            std::process::exit(1);
        }
    }

    // Find and execute main
    for module in mir_modules.iter() {
        if let Ok(()) = backend.call_main(module) {
            println!("\nTest completed successfully!");
            return;
        }
    }

    eprintln!("No main function found");
    std::process::exit(1);
}
