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
//! Test precompiled bundle execution with parallel threads (like benchmark)

use compiler::codegen::tiered_backend::{TierPreset, TieredBackend};
use compiler::ir::load_bundle;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

fn run_precompiled(symbols: &[(&str, *const u8)]) -> Result<(), String> {
    let bundle_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/precompiled/mandelbrot_simple.rzb");

    let bundle =
        load_bundle(&bundle_path).map_err(|e| format!("Failed to load bundle: {:?}", e))?;
    let entry_func_id = bundle.entry_function_id().ok_or("No entry function ID")?;

    let mut config = TierPreset::Script.to_config();
    config.start_interpreted = false;
    config.verbosity = 0;

    let mut backend = TieredBackend::with_symbols(config, symbols)
        .map_err(|e| format!("Failed to create backend: {}", e))?;

    for module in bundle.modules() {
        backend
            .compile_module(module.clone())
            .map_err(|e| format!("Failed to load module: {}", e))?;
    }

    backend
        .execute_function(entry_func_id, vec![])
        .map_err(|e| format!("Execution failed: {:?}", e))?;

    Ok(())
}

fn get_symbols() -> Vec<(&'static str, *const u8)> {
    rayzor_runtime::plugin_impl::get_plugin()
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect()
}

fn main() {
    println!("Testing parallel execution of precompiled bundles...\n");

    let (tx, rx) = mpsc::channel();

    // Spawn 3 threads running precompiled benchmark in parallel
    let mut handles = vec![];
    for i in 0..3 {
        let tx = tx.clone();
        let handle = thread::spawn(move || {
            // Each thread gets its own symbols reference
            let symbols = get_symbols();

            // Run 5 times in each thread
            for j in 0..5 {
                match run_precompiled(&symbols) {
                    Ok(_) => println!("Thread {} run {} completed", i, j),
                    Err(e) => {
                        let _ = tx.send(format!("Thread {} run {} failed: {}", i, j, e));
                        return;
                    }
                }
            }
            let _ = tx.send(format!("Thread {} completed all runs", i));
        });
        handles.push(handle);
    }
    drop(tx);

    // Collect results
    while let Ok(msg) = rx.recv() {
        println!("{}", msg);
    }

    // Wait for all threads
    for handle in handles {
        let _ = handle.join();
    }

    println!("\nParallel test completed!");
}
