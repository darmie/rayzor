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
//! Quick test of actual mandelbrot benchmark file

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use std::fs;

fn main() {
    println!("=== Testing actual mandelbrot.hx benchmark ===\n");

    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    let bench_path = concat!(env!("CARGO_MANIFEST_DIR"), "/benchmarks/src/mandelbrot.hx");
    let source = fs::read_to_string(bench_path).expect("read benchmark");

    println!("Source loaded: {} bytes", source.len());

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "mandelbrot.hx").expect("parse");
    unit.lower_to_tast().expect("tast");

    let mir_modules = unit.get_mir_modules();
    println!("MIR modules: {}", mir_modules.len());

    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");

    for module in &mir_modules {
        println!("Compiling module: {}", module.name);
        backend.compile_module(module).expect("compile");
    }
    println!("Compilation done");

    println!("\nExecuting (this will take a while for 875x500 grid)...");
    let start = std::time::Instant::now();
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            let elapsed = start.elapsed();
            println!("\nSUCCESS! Execution time: {:?}", elapsed);
            return;
        }
    }
    println!("\nFAILED: No main executed");
}
