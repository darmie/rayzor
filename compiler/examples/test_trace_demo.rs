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
/// Demo of trace() functionality with core types
///
/// This compiles and executes Haxe code that uses trace() to log runtime values.
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Rayzor Trace Demo ===\n");

    let haxe_source = r#"
package test;

import rayzor.Trace;

class Main {
    static function main() {
        // Trace integers
        Trace.traceInt(42);
        Trace.traceInt(-100);
        Trace.traceInt(0);

        // Trace floats
        Trace.traceFloat(3.14159);
        Trace.traceFloat(-2.718);
        Trace.traceFloat(0.0);

        // Trace booleans
        Trace.traceBool(true);
        Trace.traceBool(false);

        // Trace results of operations
        var x = 10 + 20;
        Trace.traceInt(x);  // 30

        var y = 5.5 * 2.0;
        Trace.traceFloat(y);  // 11.0

        var z = (10 > 5);
        Trace.traceBool(z);  // true

        // Trace array length (testing core types integration)
        var arr = new Array<Int>();
        arr.push(1);
        arr.push(2);
        arr.push(3);
        Trace.traceInt(arr.length);  // 3

        // Trace math results
        var sqrt = Math.sqrt(16.0);
        Trace.traceFloat(sqrt);  // 4.0

        var rand = Math.random();
        Trace.traceFloat(rand);  // [0.0, 1.0)
    }
}
"#;

    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add test file
    println!("Adding test file...");
    unit.add_file(haxe_source, "test_trace_demo.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    // Compile to TAST
    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    // Get MIR modules
    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    // Compile to native code
    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    // Execute
    println!("=== Execution Output ===\n");
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Execution Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
