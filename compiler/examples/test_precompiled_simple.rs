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
//! Simple test for precompiled bundle execution

use compiler::codegen::tiered_backend::{TierPreset, TieredBackend};
use compiler::ir::load_bundle;
use std::path::Path;

fn main() {
    let symbols: Vec<(&str, *const u8)> = rayzor_runtime::plugin_impl::get_plugin()
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    let bundle_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/precompiled/mandelbrot_simple.rzb");

    println!("Loading bundle from: {:?}", bundle_path);

    let bundle = load_bundle(&bundle_path).expect("Failed to load bundle");

    println!("Bundle loaded successfully");
    println!(
        "  Entry module: {}",
        bundle
            .entry_module()
            .map(|m| &m.name)
            .unwrap_or(&"None".to_string())
    );
    println!("  Entry function: {}", bundle.entry_function());
    println!("  Entry function ID: {:?}", bundle.entry_function_id());
    println!("  Module count: {}", bundle.module_count());

    let entry_func_id = bundle.entry_function_id().expect("No entry function ID");

    // Use Script preset with Cranelift JIT (not interpreter)
    let mut config = TierPreset::Script.to_config();
    config.start_interpreted = false;
    config.verbosity = 2; // Verbose output

    println!("\nCreating TieredBackend...");
    let mut backend =
        TieredBackend::with_symbols(config, &symbols).expect("Failed to create backend");

    println!("Loading modules...");
    for (i, module) in bundle.modules().iter().enumerate() {
        println!("  Loading module {}: {}", i, module.name);
        backend
            .compile_module(module.clone())
            .expect("Failed to load module");
    }

    println!("\nExecuting entry function {:?}...", entry_func_id);
    match backend.execute_function(entry_func_id, vec![]) {
        Ok(result) => println!("Execution completed: {:?}", result),
        Err(e) => println!("Execution failed: {:?}", e),
    }

    println!("\nTest completed successfully!");
}
