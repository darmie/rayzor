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
//! Test precompiled bundle execution with multiple runs (like benchmark)

use compiler::codegen::tiered_backend::{TierPreset, TieredBackend};
use compiler::ir::load_bundle;
use std::path::Path;

fn run_once(symbols: &[(&str, *const u8)], run_num: usize) {
    let bundle_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/precompiled/mandelbrot_simple.rzb");

    let bundle = load_bundle(&bundle_path).expect("Failed to load bundle");
    let entry_func_id = bundle.entry_function_id().expect("No entry function ID");

    // Use Script preset with Cranelift JIT (not interpreter)
    let mut config = TierPreset::Script.to_config();
    config.start_interpreted = false;
    config.verbosity = 0;

    let mut backend =
        TieredBackend::with_symbols(config, symbols).expect("Failed to create backend");

    // Load ALL modules from bundle
    for module in bundle.modules() {
        backend
            .compile_module(module.clone())
            .expect("Failed to load module");
    }

    // Execute
    backend
        .execute_function(entry_func_id, vec![])
        .expect("Execution failed");

    println!("Run {} completed", run_num);
}

fn main() {
    let symbols: Vec<(&str, *const u8)> = rayzor_runtime::plugin_impl::get_plugin()
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    println!("Running precompiled benchmark 25 times (15 warmup + 10 bench)...\n");

    for i in 0..25 {
        run_once(&symbols, i);
    }

    println!("\nAll runs completed successfully!");
}
